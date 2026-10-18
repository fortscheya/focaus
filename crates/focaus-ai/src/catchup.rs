//! C09 focaus context selection, validation and host submission.
//!
//! `catchup` remains the internal command/task name for compatibility; the
//! user-facing result semantics are `focaus_context_v2`.  The builder keeps
//! the body cutoff and the live progress pointer as separate values so a later
//! progress update cannot rewrite an already generated body.

use crate::{binding_matches, invalid, invalid_argument, stale, task_error, models, PROMPT_FOCAUS_V2};
use focaus_contracts::{
    BodyBlock, CatchupCoverage, CatchupGenerateInput, CatchupResult,
    CatchupResultCompletionStatus, CatchupResultPrerequisitesReasonValue, CatchupWindow,
    ContextResult, CurrentFocus, CurrentFocusStatus, ErrorCode, HostServices,
    InputSnapshot, ModelId, PathEdge, PathEdgeCertainty, PathNode, PathNodeEvidenceRole,
    PathNodeRole, Prerequisite, ProgressBinding, ProgressState, ResultSubmitInput, SafeError,
    SourceRef, TaskKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const FOCAUS_RESULT_SEMANTICS: &str = "focaus_context_v2";
pub const FOCAUS_DEADLINE_MS: u64 = 15_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocausDraft {
    pub current_focus: CurrentFocus,
    pub path_nodes: Vec<PathNode>,
    pub path_edges: Vec<PathEdge>,
    pub prerequisites: Vec<Prerequisite>,
    pub prerequisites_reason: Option<CatchupResultPrerequisitesReasonValue>,
    pub body: Vec<BodyBlock>,
    pub presentation_tier: u64,
    pub coverage: CatchupCoverage,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocausPublication {
    Complete,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineBudget {
    pub triggered_at_mono_ms: u64,
    pub now_mono_ms: u64,
}

impl DeadlineBudget {
    pub fn elapsed_ms(&self) -> u64 {
        self.now_mono_ms.saturating_sub(self.triggered_at_mono_ms)
    }

    pub fn remaining_ms(&self) -> u64 {
        FOCAUS_DEADLINE_MS.saturating_sub(self.elapsed_ms())
    }

    pub fn permits_complete(&self) -> bool {
        self.elapsed_ms() <= FOCAUS_DEADLINE_MS
    }
}

/// Validate a context snapshot returned by the host.  Retrieval may use older
/// definitions, but it must stay under the original evidence cutoff and be
/// explicitly linked to the original snapshot.
pub fn validate_retrieved_context(
    original: &InputSnapshot,
    context: &ContextResult,
) -> Result<(), SafeError> {
    if context.snapshot.parent_snapshot_id.as_deref() != Some(original.snapshot_id.as_str())
        && context.snapshot.snapshot_id != original.snapshot_id
    {
        return Err(stale("context_snapshot_not_derived_from_request"));
    }
    if context.snapshot.contract_version != original.contract_version {
        return Err(stale("context_contract_version_changed"));
    }
    for cutoff in &context.snapshot.evidence_cutoff {
        let Some(original_cutoff) = original
            .evidence_cutoff
            .iter()
            .find(|candidate| candidate.recording_id == cutoff.recording_id)
        else {
            return Err(stale("context_introduced_new_recording_evidence"));
        };
        match (original_cutoff.end_ms, cutoff.end_ms) {
            (Some(outer), Some(inner)) if inner <= outer => {}
            (None, None) => {}
            _ => return Err(stale("context_evidence_is_after_request_cutoff")),
        }
    }
    Ok(())
}

pub fn validate_draft(
    snapshot: &InputSnapshot,
    context: &ContextResult,
    draft: &FocausDraft,
) -> Result<(), SafeError> {
    if snapshot.purpose != TaskKind::Catchup {
        return Err(invalid_argument("focaus_snapshot_purpose_mismatch"));
    }
    if snapshot.model_id != Some(ModelId::Qwen38Flash)
        || snapshot.region.as_deref() != Some(models::REGION)
        || snapshot.prompt_version.as_deref() != Some(PROMPT_FOCAUS_V2)
    {
        return Err(invalid_argument("focaus_snapshot_binding_mismatch"));
    }
    validate_retrieved_context(snapshot, context)?;
    if draft.presentation_tier == 0 || draft.presentation_tier > 5 {
        return Err(invalid_argument("focaus_language_tier_out_of_range"));
    }
    if draft.body.is_empty() || draft.body.iter().any(|block| block.text.trim().is_empty()) {
        return Err(invalid("focaus_body_missing"));
    }
    if draft.path_nodes.is_empty() {
        return Err(invalid("focaus_path_missing"));
    }
    let node_ids: BTreeSet<&str> = draft.path_nodes.iter().map(|node| node.node_id.as_str()).collect();
    if node_ids.len() != draft.path_nodes.len() {
        return Err(invalid("focaus_duplicate_path_node"));
    }
    if !draft.path_nodes.iter().any(|node| node.role == PathNodeRole::Current) {
        return Err(invalid("focaus_current_node_missing"));
    }
    for node in &draft.path_nodes {
        if node.source_refs.is_empty() {
            return Err(invalid("focaus_path_node_source_missing"));
        }
        if node.evidence_role == PathNodeEvidenceRole::TeacherPath
            && node.source_refs.iter().all(|source| !is_audio_source(source))
        {
            return Err(invalid("teacher_path_requires_audio_evidence"));
        }
    }
    for edge in &draft.path_edges {
        if !node_ids.contains(edge.from_node_id.as_str()) || !node_ids.contains(edge.to_node_id.as_str()) {
            return Err(invalid("focaus_edge_references_unknown_node"));
        }
        if edge.certainty == PathEdgeCertainty::Supported && edge.source_refs.is_empty() {
            return Err(invalid("supported_focaus_edge_source_missing"));
        }
    }
    let body_ids: BTreeSet<&str> = draft.body.iter().map(|block| block.block_id.as_str()).collect();
    if draft.path_nodes.iter().any(|node| node.body_block_ids.iter().any(|id| !body_ids.contains(id.as_str()))) {
        return Err(invalid("focaus_body_block_reference_missing"));
    }
    if draft.prerequisites.is_empty() {
        if draft.prerequisites_reason.is_none() {
            return Err(invalid("focaus_prerequisite_reason_missing"));
        }
    } else if draft.prerequisites_reason.is_some() {
        return Err(invalid("focaus_prerequisite_reason_conflicts_with_items"));
    }
    if draft.current_focus.status == CurrentFocusStatus::Supported && draft.current_focus.source_refs.is_empty() {
        return Err(invalid("supported_current_focus_source_missing"));
    }
    if draft.coverage.range_status == focaus_contracts::CatchupCoverageRangeStatus::Complete
        && (!draft.coverage.audio_gaps.is_empty() || !draft.coverage.asr_gaps.is_empty())
    {
        return Err(invalid("complete_focaus_cannot_hide_audio_or_asr_gaps"));
    }
    Ok(())
}

fn is_audio_source(source: &SourceRef) -> bool {
    serde_json::to_value(&source.selector)
        .ok()
        .and_then(|value| value.get("kind").and_then(Value::as_str).map(|kind| kind == "audio"))
        .unwrap_or(false)
}

fn progress_binding(lesson_id: &str, progress: Option<&ProgressState>) -> ProgressBinding {
    match progress {
        Some(progress) => ProgressBinding {
            lesson_id: lesson_id.to_owned(),
            recording_id: Some(progress.recording_id.clone()),
            assignment_epoch: Some(progress.assignment_epoch),
            no_pointer_reason: None,
        },
        None => ProgressBinding {
            lesson_id: lesson_id.to_owned(),
            recording_id: None,
            assignment_epoch: None,
            no_pointer_reason: Some("no_progress_at_trigger".to_string()),
        },
    }
}

pub fn build_result(
    result_id: impl Into<String>,
    body_revision_id: impl Into<String>,
    request_generation: u64,
    snapshot: &InputSnapshot,
    requested_window: CatchupWindow,
    effective_window: CatchupWindow,
    progress_at_trigger: Option<ProgressState>,
    draft: FocausDraft,
    triggered_at: impl Into<String>,
    generated_at: impl Into<String>,
    budget: DeadlineBudget,
) -> Result<(CatchupResult, FocausPublication), SafeError> {
    let context = ContextResult {
        snapshot: snapshot.clone(),
        definitions: vec![],
        symbol_bindings: vec![],
        source_refs: vec![],
        limitations: vec![],
    };
    // This local snapshot is enough for structural validation. Callers with
    // retrieved definitions use build_result_with_context, which additionally
    // validates the derived context cutoff.
    validate_draft(snapshot, &context, &draft)?;
    let publication = if budget.permits_complete() { FocausPublication::Complete } else { FocausPublication::Partial };
    let completion_status = match publication {
        FocausPublication::Complete => CatchupResultCompletionStatus::Complete,
        FocausPublication::Partial => CatchupResultCompletionStatus::Partial,
        FocausPublication::Failed => CatchupResultCompletionStatus::Failed,
    };
    let lesson_id = snapshot.lesson_id.clone().unwrap_or_else(|| snapshot.target_id.clone());
    let generated_at = generated_at.into();
    let mut limitations = draft.limitations;
    if publication == FocausPublication::Partial && !limitations.iter().any(|item| item == "deadline_exceeded") {
        limitations.push("deadline_exceeded".to_string());
    }
    let result = CatchupResult {
        result_semantics: FOCAUS_RESULT_SEMANTICS.to_string(),
        result_id: result_id.into(),
        request_generation,
        snapshot_id: snapshot.snapshot_id.clone(),
        body_revision_id: body_revision_id.into(),
        requested_window,
        effective_window,
        estimate_basis: "host_supplied_window".to_string(),
        current_focus: draft.current_focus,
        path_nodes: draft.path_nodes,
        path_edges: draft.path_edges,
        prerequisites: draft.prerequisites,
        prerequisites_reason: draft.prerequisites_reason,
        body: draft.body,
        presentation_tier: draft.presentation_tier,
        body_evidence_cutoff: snapshot.evidence_cutoff.clone(),
        body_generated_at: generated_at.clone(),
        progress_at_trigger: progress_at_trigger.clone(),
        progress_unavailable_reason: if progress_at_trigger.is_some() { None } else { Some("no_progress_at_trigger".to_string()) },
        progress_binding: progress_binding(&lesson_id, progress_at_trigger.as_ref()),
        coverage: draft.coverage,
        limitations,
        completion_status,
        triggered_at: triggered_at.into(),
        completed_at: Some(generated_at),
        displayed_complete_elapsed_ms: None,
    };
    Ok((result, publication))
}

pub fn build_result_with_context(
    result_id: impl Into<String>,
    body_revision_id: impl Into<String>,
    request_generation: u64,
    input: &CatchupGenerateInput,
    draft: FocausDraft,
    triggered_at: impl Into<String>,
    generated_at: impl Into<String>,
    budget: DeadlineBudget,
) -> Result<(CatchupResult, FocausPublication), SafeError> {
    validate_draft(&input.snapshot, &input.context, &draft)?;
    let (result, publication) = build_result(
        result_id,
        body_revision_id,
        request_generation,
        &input.snapshot,
        input.window.clone(),
        input.window.clone(),
        input.progress_at_trigger.clone(),
        draft,
        triggered_at,
        generated_at,
        budget,
    )?;
    Ok((result, publication))
}

pub fn validate_publication(result: &CatchupResult, elapsed_ms: u64) -> Result<(), SafeError> {
    if result.result_semantics != FOCAUS_RESULT_SEMANTICS {
        return Err(invalid("unknown_focaus_result_semantics"));
    }
    if result.completion_status == CatchupResultCompletionStatus::Complete && elapsed_ms > FOCAUS_DEADLINE_MS {
        return Err(task_error(ErrorCode::DeadlineExceeded, "focaus_complete_after_deadline", false));
    }
    if result.completion_status == CatchupResultCompletionStatus::Complete
        && result.body_evidence_cutoff.is_empty()
        && result.current_focus.status == CurrentFocusStatus::Supported
    {
        return Err(invalid("supported_focaus_without_evidence_cutoff"));
    }
    Ok(())
}

pub fn decode_focaus_result_candidate(value: &Value) -> Result<CatchupResult, SafeError> {
    let result: CatchupResult = serde_json::from_value(value.clone())
        .map_err(|_| invalid("focaus_result_candidate_invalid"))?;
    if result.result_semantics != FOCAUS_RESULT_SEMANTICS {
        return Err(invalid("unknown_focaus_result_semantics"));
    }
    if result.result_id.is_empty() || result.snapshot_id.is_empty() || result.body_revision_id.is_empty() {
        return Err(stale("focaus_result_candidate_identity_missing"));
    }
    Ok(result)
}

pub fn validate_focaus_result_submission(
    submission: &ResultSubmitInput,
) -> Result<CatchupResult, SafeError> {
    if submission.task_id.is_empty() || submission.snapshot_id.is_empty() || submission.attempt == 0 {
        return Err(invalid_argument("focaus_result_submission_identity_required"));
    }
    let result = decode_focaus_result_candidate(&submission.candidate)?;
    let expected_sources: Vec<SourceRef> = result
        .path_nodes
        .iter()
        .flat_map(|node| node.source_refs.iter().cloned())
        .collect();
    if result.snapshot_id != submission.snapshot_id
        || submission.source_refs != expected_sources
        || result.body_revision_id.is_empty()
    {
        return Err(stale("focaus_result_submission_identity_mismatch"));
    }
    Ok(result)
}

/// The host owns snapshot creation and result submission.  This helper is
/// intentionally transport agnostic: a future model client can produce the
/// `FocausDraft`, while all source and CAS checks remain in the host path.
pub async fn submit_result<H: HostServices>(
    host: &H,
    task_id: impl Into<String>,
    attempt: u64,
    result: &CatchupResult,
) -> Result<focaus_contracts::CommandResult, SafeError> {
    let candidate = serde_json::to_value(result).map_err(|_| invalid("focaus_result_serialization_failed"))?;
    let submission = ResultSubmitInput {
        task_id: task_id.into(),
        attempt,
        snapshot_id: result.snapshot_id.clone(),
        candidate,
        source_refs: result
            .path_nodes
            .iter()
            .flat_map(|node| node.source_refs.iter().cloned())
            .collect(),
        resources: vec![],
    };
    validate_focaus_result_submission(&submission)?;
    host.result_submit(submission).await
}

/// Build a bounded qwen3.8-flash request.  It contains only the already
/// frozen snapshot and retrieved context; it has no path or credential field.
pub fn build_generation_request(input: &CatchupGenerateInput) -> Result<Value, SafeError> {
    let binding = crate::binding_for_model(ModelId::Qwen38Flash);
    build_generation_request_with_binding(input, &binding)
}

pub fn build_generation_request_with_binding(
    input: &CatchupGenerateInput,
    binding: &focaus_contracts::CredentialBinding,
) -> Result<Value, SafeError> {
    binding_matches(binding, ModelId::Qwen38Flash)?;
    if input.snapshot.contract_version != focaus_contracts::CONTRACT_VERSION {
        return Err(task_error(
            ErrorCode::UnsupportedVersion,
            "focaus_contract_version_unsupported",
            false,
        ));
    }
    if input.snapshot.purpose != TaskKind::Catchup
        || input.snapshot.prompt_version.as_deref() != Some(PROMPT_FOCAUS_V2)
        || input.snapshot.model_id != Some(ModelId::Qwen38Flash)
        || input.snapshot.region.as_deref() != Some(models::REGION)
    {
        return Err(invalid_argument("focaus_generation_model_mismatch"));
    }
    validate_retrieved_context(&input.snapshot, &input.context)?;
    let model_input = json!({
        "snapshot_id": input.snapshot.snapshot_id,
        "evidence_cutoff": input.snapshot.evidence_cutoff,
        "window": input.window,
        "progress_at_trigger": input.progress_at_trigger,
        "retrieved_context": {
            "definitions": input.context.definitions,
            "symbol_bindings": input.context.symbol_bindings,
            "source_refs": input.context.source_refs,
            "limitations": input.context.limitations,
        },
        "output_contract": FOCAUS_RESULT_SEMANTICS,
        "constraints": [
            "one_understanding_path",
            "separate_teacher_path_and_system_explanation",
            "separate_body_cutoff_and_live_progress",
            "complete_only_within_15000ms"
        ]
    });
    Ok(crate::transport::build_chat_completion_body(
        models::MULTIMODAL,
        PROMPT_FOCAUS_V2,
        "Generate one concise focaus understanding path. Keep teacher_path separate from system explanation and return only the requested JSON object.",
        model_input,
    ))
}

pub async fn retrieve_context<H: HostServices>(
    host: &H,
    input: focaus_contracts::ContextRetrieveInput,
) -> Result<ContextResult, SafeError> {
    let context = host.context_retrieve(input.clone()).await?;
    validate_retrieved_context(&input.snapshot, &context)?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{CurrentFocusActivity, EvidenceCutoff, Selector, TextAnchor, TextSelector};

    fn source(kind: &str) -> SourceRef {
        let selector = match kind {
            "audio" => Selector::V0(focaus_contracts::AudioSelector { kind: "audio".into(), recording_id: "00000000-0000-4000-8000-000000000006".into(), audio_span_id: "00000000-0000-4000-8000-000000000012".into(), track_id: "t".into(), start_frame: 0, end_frame: 10, start_ms: 0, end_ms: 100, mapping_revision_id: "00000000-0000-4000-8000-000000000013".into() }),
            _ => Selector::V1(TextSelector { kind: "text".into(), block_id: "b".into(), anchor: TextAnchor { left_element_id: None, right_element_id: None, element_ids: vec!["e".into()], scalar_range: focaus_contracts::ScalarRange { start: 0, end: 1 } } }),
        };
        SourceRef { entity_id: "00000000-0000-4000-8000-000000000001".into(), revision_id: "00000000-0000-4000-8000-000000000002".into(), content_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(), selector }
    }

    fn input() -> CatchupGenerateInput {
        let snapshot = InputSnapshot {
            snapshot_id: "00000000-0000-4000-8000-000000000003".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose: TaskKind::Catchup,
            target_id: "00000000-0000-4000-8000-000000000004".into(), lesson_id: Some("00000000-0000-4000-8000-000000000005".into()), recording_id: Some("00000000-0000-4000-8000-000000000006".into()), assignment_epoch: Some(1), sources: vec![source("audio")], attachment_set_revision: "00000000-0000-4000-8000-000000000007".into(), fact_set_revision: None, target_base_revision: None, edit_revision: None, language_tier: Some(3), evidence_cutoff: vec![EvidenceCutoff { recording_id: "00000000-0000-4000-8000-000000000006".into(), end_ms: Some(1000), status: focaus_contracts::EvidenceCutoffStatus::Known }], available_at: "2026-01-01T00:00:00Z".into(), executor_id: "ai".into(), executor_version: "1".into(), model_id: Some(ModelId::Qwen38Flash), region: Some("cn-beijing".into()), prompt_version: Some(PROMPT_FOCAUS_V2.into()), parameters_hash: "0000000000000000000000000000000000000000000000000000000000000001".into(), calibration_seq: None, decision_seq: Some(7),
        };
        let context_snapshot = InputSnapshot { snapshot_id: "00000000-0000-4000-8000-000000000008".into(), parent_snapshot_id: Some("00000000-0000-4000-8000-000000000003".into()), ..snapshot.clone() };
        CatchupGenerateInput { snapshot, progress_at_trigger: None, window: CatchupWindow { recording_id: Some("00000000-0000-4000-8000-000000000006".into()), start_ms: Some(0), end_ms: Some(1000), start_source: None }, context: ContextResult { snapshot: context_snapshot, definitions: vec![], symbol_bindings: vec![], source_refs: vec![], limitations: vec![] } }
    }

    fn draft() -> FocausDraft {
        FocausDraft {
            current_focus: CurrentFocus { topic: "limits".into(), problem: "what happens at the boundary".into(), activity: CurrentFocusActivity::Derivation, step: Some("substitute".into()), source_refs: vec![source("audio")], status: CurrentFocusStatus::Supported },
            path_nodes: vec![PathNode { node_id: "n1".into(), role: PathNodeRole::Current, fact_ids: vec![], definition_ids: vec![], binding_ids: vec![], body_block_ids: vec!["00000000-0000-4000-8000-000000000009".into()], evidence_role: PathNodeEvidenceRole::TeacherPath, source_refs: vec![source("audio")] }],
            path_edges: vec![], prerequisites: vec![], prerequisites_reason: Some(CatchupResultPrerequisitesReasonValue::NoneNeeded), body: vec![BodyBlock { block_id: "00000000-0000-4000-8000-000000000009".into(), text: "The teacher now substitutes the boundary condition.".into() }], presentation_tier: 3, coverage: CatchupCoverage { audio_gaps: vec![], asr_gaps: vec![], material_supplement_used: false, range_status: focaus_contracts::CatchupCoverageRangeStatus::Complete }, limitations: vec![],
        }
    }

    #[test]
    fn focaus_keeps_body_cutoff_and_progress_pointer_separate() {
        let input = input();
        let (result, publication) = build_result_with_context("00000000-0000-4000-8000-000000000010", "00000000-0000-4000-8000-000000000011", 1, &input, draft(), "2026-01-01T00:00:00Z", "2026-01-01T00:00:01Z", DeadlineBudget { triggered_at_mono_ms: 10, now_mono_ms: 100 }).unwrap();
        assert_eq!(publication, FocausPublication::Complete);
        assert_eq!(result.body_evidence_cutoff[0].end_ms, Some(1000));
        assert!(result.progress_binding.no_pointer_reason.is_some());
        validate_publication(&result, 90).unwrap();
        let wire = serde_json::to_value(&result).unwrap();
        focaus_contracts::validate("CatchupResult", &wire).unwrap();
        let source_refs = result.path_nodes.iter().flat_map(|node| node.source_refs.iter().cloned()).collect::<Vec<_>>();
        let submission = ResultSubmitInput { task_id: "catchup-task".into(), attempt: 1, snapshot_id: result.snapshot_id.clone(), candidate: wire.clone(), source_refs, resources: vec![] };
        assert_eq!(validate_focaus_result_submission(&submission).unwrap(), result);
        let mut wrong_semantics = wire.clone();
        wrong_semantics["result_semantics"] = serde_json::json!("secondary_segment_v1");
        assert_eq!(decode_focaus_result_candidate(&wrong_semantics).unwrap_err().code, ErrorCode::InvalidResult);
        let mut wrong_outer = submission.clone();
        wrong_outer.snapshot_id = "other-snapshot".into();
        assert_eq!(validate_focaus_result_submission(&wrong_outer).unwrap_err().code, ErrorCode::StaleInput);
        let mut zero_attempt = submission;
        zero_attempt.attempt = 0;
        assert_eq!(validate_focaus_result_submission(&zero_attempt).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn supported_teacher_path_requires_audio_and_deadline_is_hard() {
        let input = input();
        let mut bad = draft();
        bad.path_nodes[0].source_refs = vec![source("text")];
        assert!(validate_draft(&input.snapshot, &input.context, &bad).is_err());
        let (mut result, _) = build_result_with_context("00000000-0000-4000-8000-000000000010", "00000000-0000-4000-8000-000000000011", 1, &input, draft(), "2026-01-01T00:00:00Z", "2026-01-01T00:00:20Z", DeadlineBudget { triggered_at_mono_ms: 0, now_mono_ms: 15_001 }).unwrap();
        result.completion_status = CatchupResultCompletionStatus::Complete;
        assert_eq!(validate_publication(&result, 15_001).unwrap_err().code, ErrorCode::DeadlineExceeded);
    }

    #[test]
    fn generation_request_has_exact_model_and_no_credentials() {
        let body = build_generation_request(&input()).unwrap();
        assert_eq!(body["model"], models::MULTIMODAL);
        assert!(body["messages"][1]["content"][0]["text"].as_str().unwrap().contains(FOCAUS_RESULT_SEMANTICS));
        assert!(body.to_string().find("credential").is_none());
        let wrong_binding = crate::binding_for_model(ModelId::DecisionModelPreview);
        assert_eq!(build_generation_request_with_binding(&input(), &wrong_binding).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut wrong_prompt = input();
        wrong_prompt.snapshot.prompt_version = Some("old_prompt".into());
        assert_eq!(build_generation_request(&wrong_prompt).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut old_contract = input();
        old_contract.snapshot.contract_version = "1.0.0".into();
        assert_eq!(build_generation_request(&old_contract).unwrap_err().code, ErrorCode::UnsupportedVersion);
    }

    #[test]
    fn focaus_refuses_progress_or_asr_model_bindings() {
        let mut wrong = input();
        wrong.snapshot.model_id = Some(ModelId::DecisionModelPreview);
        assert_eq!(build_generation_request(&wrong).unwrap_err().code, ErrorCode::InvalidArgument);
        wrong.snapshot.model_id = Some(ModelId::QwenAudio31AsrFlashStreaming);
        assert_eq!(build_generation_request(&wrong).unwrap_err().code, ErrorCode::InvalidArgument);
    }
}

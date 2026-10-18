//! Secondary transcript, correction and language-variant proposal helpers.
//!
//! The secondary layer is a source-preserving cleanup.  It may remove verbal
//! filler and repair punctuation, but it cannot paraphrase the teacher into a
//! third-person summary.  Critical semantic corrections remain suggestions
//! for user review.

use crate::{invalid, invalid_argument, models, stale, task_error, PROMPT_SECONDARY_V1};
use focaus_contracts::{
    Command, CommandActor, CommandName, CommandResult, CommandResultStatus,
    CorrectionAcceptInput, CorrectionDraftInput, CorrectionRejectInput,
    CorrectionSuggestion, CorrectionSuggestionSemanticRisk,
    CorrectionSuggestionState, Definition, EntityId, HostServices, InputSnapshot,
    LanguageVariant, LanguageVariantSyncStatus, ModelId, ResultSubmitInput, Selector,
    SafeError, SecondarySegment, SourceRef, SymbolBinding, TaskKind, UserPatch,
    CONTRACT_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const FILLERS: &[&str] = &["嗯", "呃", "啊", "这个", "那个", "you know", "um", "uh"];
const THIRD_PERSON_MARKERS: &[&str] = &["老师说", "老师认为", "他说", "她说", "他说道", "the teacher says", "he says", "she says"];
const NEGATION_MARKERS: &[&str] = &["不", "无", "非", "未", "没有", "不能", "不会", "not", "never", "without"];
const CONDITION_MARKERS: &[&str] = &["如果", "若", "只有", "当", "则", "否则", "if", "unless", "only if"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTextUnit {
    pub utterance_id: EntityId,
    pub recording_id: EntityId,
    pub text: String,
    pub source_ref: SourceRef,
    pub span_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecondaryDraft {
    pub recording_id: EntityId,
    pub raw_refs: Vec<SourceRef>,
    pub ordered_spans: Vec<EntityId>,
    pub text: String,
    pub suggestions: Vec<CorrectionSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimmedText {
    pub text: String,
    pub removed_fillers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionAction {
    Accept,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionReceipt {
    Committed { revision_id: EntityId },
    Accepted { task_id: EntityId },
}

fn source_units_for_secondary<'a>(
    recording_id: &str,
    raw_refs: &[SourceRef],
    ordered_spans: &[EntityId],
    raw_units: &'a [RawTextUnit],
) -> Result<Vec<&'a RawTextUnit>, SafeError> {
    if recording_id.is_empty() || raw_refs.is_empty() || raw_refs.len() != ordered_spans.len() {
        return Err(invalid("secondary_source_refs_mismatch"));
    }
    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(ordered_spans.len());
    for (source_ref, span_id) in raw_refs.iter().zip(ordered_spans) {
        if source_ref.entity_id.is_empty() || source_ref.revision_id.is_empty() {
            return Err(invalid("secondary_raw_source_ref_incomplete"));
        }
        if span_id.is_empty() || !seen.insert(span_id.clone()) {
            return Err(invalid("secondary_source_span_not_unique"));
        }
        let Some(unit) = raw_units
            .iter()
            .find(|unit| unit.recording_id.as_str() == recording_id && unit.span_id.as_str() == span_id.as_str())
        else {
            return Err(crate::task_error(
                focaus_contracts::ErrorCode::SourceUnavailable,
                "secondary_raw_source_unavailable",
                false,
            ));
        };
        if &unit.source_ref != source_ref {
            return Err(stale("secondary_raw_source_revision_mismatch"));
        }
        selected.push(unit);
    }
    Ok(selected)
}

fn validate_raw_units_for_request(raw_units: &[RawTextUnit]) -> Result<(), SafeError> {
    if raw_units.is_empty() {
        return Err(invalid_argument("secondary_raw_text_required"));
    }
    let recording_id = &raw_units[0].recording_id;
    let mut spans = BTreeSet::new();
    for unit in raw_units {
        if unit.recording_id.as_str() != recording_id.as_str()
            || unit.span_id.is_empty()
            || !spans.insert(unit.span_id.clone())
        {
            return Err(invalid("secondary_raw_units_not_orderable"));
        }
        if unit.source_ref.entity_id.is_empty() || unit.source_ref.revision_id.is_empty() {
            return Err(invalid("secondary_raw_source_ref_incomplete"));
        }
    }
    Ok(())
}

pub fn validate_secondary_snapshot(snapshot: &InputSnapshot) -> Result<(), SafeError> {
    if snapshot.contract_version != focaus_contracts::CONTRACT_VERSION {
        return Err(task_error(
            focaus_contracts::ErrorCode::UnsupportedVersion,
            "secondary_contract_version_unsupported",
            false,
        ));
    }
    if snapshot.purpose != TaskKind::Secondary
        || snapshot.model_id != Some(ModelId::Qwen38Flash)
        || snapshot.region.as_deref() != Some(models::REGION)
        || snapshot.prompt_version.as_deref() != Some(PROMPT_SECONDARY_V1)
        || snapshot.language_tier.is_some_and(|tier| !(1..=5).contains(&tier))
    {
        return Err(invalid_argument("secondary_snapshot_binding_mismatch"));
    }
    Ok(())
}

/// Deterministic cleanup used for a local fallback and as a semantic guard on
/// model output.  It keeps the original order and does not invent wording.
pub fn trim_direct_speech(text: &str) -> TrimmedText {
    let mut result = text.to_string();
    let mut removed = Vec::new();
    for filler in FILLERS {
        if result.contains(filler) {
            removed.push((*filler).to_string());
            result = result.replace(filler, "");
        }
    }
    // Repeated punctuation is a presentation cleanup, not a semantic edit.
    while result.contains("。。") {
        result = result.replace("。。", "。");
    }
    while result.contains("，，") {
        result = result.replace("，，", "，");
    }
    TrimmedText { text: result.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string(), removed_fillers: removed }
}

pub fn validate_secondary_draft(
    draft: &SecondaryDraft,
    raw_units: &[RawTextUnit],
) -> Result<(), SafeError> {
    if draft.recording_id.is_empty() || draft.raw_refs.is_empty() || draft.text.trim().is_empty() {
        return Err(invalid("secondary_draft_missing_identity_or_source"));
    }
    let selected = source_units_for_secondary(
        &draft.recording_id,
        &draft.raw_refs,
        &draft.ordered_spans,
        raw_units,
    )?;
    let raw_text = selected.iter().map(|unit| unit.text.as_str()).collect::<String>();
    let source_normalized = strip_non_content(&raw_text);
    let draft_normalized = strip_non_content(&draft.text);
    if !is_subsequence(&draft_normalized, &source_normalized) {
        return Err(invalid("secondary_output_is_paraphrase_or_reordered"));
    }
    for marker in THIRD_PERSON_MARKERS {
        if draft.text.to_lowercase().contains(&marker.to_lowercase())
            && !raw_text.to_lowercase().contains(&marker.to_lowercase())
        {
            return Err(invalid("secondary_third_person_rewrite"));
        }
    }
    for suggestion in &draft.suggestions {
        if suggestion.original_text.trim().is_empty() || suggestion.suggested_text.trim().is_empty() {
            return Err(invalid("secondary_correction_text_missing"));
        }
        let expected = classify_semantic_risk(&suggestion.original_text, &suggestion.suggested_text);
        if suggestion.semantic_risk != expected {
            return Err(invalid("secondary_correction_risk_mismatch"));
        }
        if suggestion.state != CorrectionSuggestionState::Pending {
            return Err(invalid("secondary_draft_contains_applied_correction"));
        }
    }
    Ok(())
}

fn strip_non_content(text: &str) -> Vec<char> {
    text.chars().filter(|ch| !ch.is_whitespace() && !"，。！？；：,.!?;:".contains(*ch)).collect()
}

fn is_subsequence(candidate: &[char], source: &[char]) -> bool {
    if candidate.is_empty() {
        return false;
    }
    let mut index = 0;
    for ch in candidate {
        let Some(found) = source[index..].iter().position(|source_ch| source_ch == ch) else { return false };
        index += found + 1;
    }
    true
}

pub fn classify_semantic_risk(original: &str, suggested: &str) -> CorrectionSuggestionSemanticRisk {
    if number_tokens(original) != number_tokens(suggested)
        || polarity_tokens(original) != polarity_tokens(suggested)
        || condition_tokens(original) != condition_tokens(suggested)
        || formula_shape(original) != formula_shape(suggested)
    {
        CorrectionSuggestionSemanticRisk::Critical
    } else {
        CorrectionSuggestionSemanticRisk::Ordinary
    }
}

fn number_tokens(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '%' {
            current.push(ch);
        } else if !current.is_empty() {
            output.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() { output.push(current); }
    output
}

fn polarity_tokens(text: &str) -> Vec<&'static str> {
    NEGATION_MARKERS.iter().copied().filter(|marker| text.to_lowercase().contains(&marker.to_lowercase())).collect()
}

fn condition_tokens(text: &str) -> Vec<&'static str> {
    CONDITION_MARKERS.iter().copied().filter(|marker| text.to_lowercase().contains(&marker.to_lowercase())).collect()
}

fn formula_shape(text: &str) -> String {
    text.chars().filter(|ch| "=+-*/^()<>".contains(*ch)).collect()
}

pub fn build_secondary_segment(
    secondary_id: impl Into<String>,
    generation_revision: impl Into<String>,
    revision_id: impl Into<String>,
    units: &[RawTextUnit],
) -> Result<SecondarySegment, SafeError> {
    if units.is_empty() {
        return Err(invalid_argument("secondary_units_required"));
    }
    let recording_id = units[0].recording_id.clone();
    if units.iter().any(|unit| unit.recording_id != recording_id) {
        return Err(invalid("secondary_mixes_recordings"));
    }
    let mut text_parts = Vec::new();
    let mut refs = Vec::new();
    let mut spans = Vec::new();
    for unit in units {
        let trimmed = trim_direct_speech(&unit.text);
        if !trimmed.text.is_empty() {
            text_parts.push(trimmed.text);
        }
        refs.push(unit.source_ref.clone());
        spans.push(unit.span_id.clone());
    }
    let draft = SecondaryDraft { recording_id: recording_id.clone(), raw_refs: refs.clone(), ordered_spans: spans.clone(), text: text_parts.join(" "), suggestions: vec![] };
    validate_secondary_draft(&draft, units)?;
    Ok(SecondarySegment { secondary_id: secondary_id.into(), recording_id, raw_refs: refs, ordered_spans: spans, text: draft.text, suggestions: vec![], generation_revision: generation_revision.into(), revision_id: revision_id.into() })
}

pub fn make_correction(
    suggestion_id: impl Into<String>,
    secondary_id: impl Into<String>,
    source_revision: impl Into<String>,
    anchor: focaus_contracts::TextAnchor,
    original_text: impl Into<String>,
    suggested_text: impl Into<String>,
    reason: impl Into<String>,
) -> CorrectionSuggestion {
    let original_text = original_text.into();
    let suggested_text = suggested_text.into();
    CorrectionSuggestion { suggestion_id: suggestion_id.into(), secondary_id: secondary_id.into(), source_revision: source_revision.into(), anchor, original_text: original_text.clone(), suggested_text: suggested_text.clone(), reason: reason.into(), semantic_risk: classify_semantic_risk(&original_text, &suggested_text), state: CorrectionSuggestionState::Pending }
}

/// Validate one model-produced correction against the exact secondary
/// revision and the raw spans from which that secondary was built.  Raw ASR
/// is deliberately never an output target of this function.
pub fn validate_correction_candidate(
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
) -> Result<(), SafeError> {
    source_units_for_secondary(
        &segment.recording_id,
        &segment.raw_refs,
        &segment.ordered_spans,
        raw_units,
    )?;
    if suggestion.secondary_id != segment.secondary_id {
        return Err(stale("correction_secondary_target_stale"));
    }
    if suggestion.source_revision != segment.revision_id {
        return Err(stale("correction_source_revision_stale"));
    }
    if suggestion.state != CorrectionSuggestionState::Pending {
        return Err(stale("correction_suggestion_not_pending"));
    }
    if suggestion.original_text.trim().is_empty() || suggestion.suggested_text.trim().is_empty() {
        return Err(invalid("secondary_correction_text_missing"));
    }
    if suggestion.semantic_risk
        != classify_semantic_risk(&suggestion.original_text, &suggestion.suggested_text)
    {
        return Err(invalid("secondary_correction_risk_mismatch"));
    }
    let segment_text = strip_non_content(&segment.text);
    if !is_subsequence(&strip_non_content(&suggestion.original_text), &segment_text) {
        return Err(stale("correction_anchor_source_stale"));
    }
    let range = &suggestion.anchor.scalar_range;
    let text_len = segment.text.chars().count() as u64;
    if range.start > range.end || range.end > text_len {
        return Err(stale("correction_anchor_range_stale"));
    }
    Ok(())
}

fn validate_correction_action(
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    action: CorrectionAction,
    final_text: &str,
    critical_confirmed: bool,
) -> Result<(), SafeError> {
    validate_correction_candidate(suggestion, segment, raw_units)?;
    let Some(current) = segment
        .suggestions
        .iter()
        .find(|candidate| candidate.suggestion_id == suggestion.suggestion_id)
    else {
        return Err(stale("correction_candidate_not_current"));
    };
    if current != suggestion {
        return Err(stale("correction_candidate_revision_mismatch"));
    }
    if final_text.trim().is_empty() {
        return Err(invalid_argument("correction_final_text_required"));
    }
    if matches!(action, CorrectionAction::Reject) {
        if final_text != suggestion.original_text {
            return Err(invalid_argument("rejected_correction_must_keep_original"));
        }
        return Ok(());
    }
    let final_risk = classify_semantic_risk(&suggestion.original_text, final_text);
    if (suggestion.semantic_risk == CorrectionSuggestionSemanticRisk::Critical
        || final_risk == CorrectionSuggestionSemanticRisk::Critical)
        && !critical_confirmed
    {
        return Err(invalid_argument("critical_correction_requires_confirmation"));
    }
    Ok(())
}

fn validate_correction_edit_scope(
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    user_patches: &[UserPatch],
    composition_active: bool,
) -> Result<(), SafeError> {
    if composition_active {
        return Err(task_error(
            focaus_contracts::ErrorCode::EditProtected,
            "correction_composition_active",
            false,
        ));
    }
    let selector = Selector::V1(focaus_contracts::TextSelector {
        kind: "text".into(),
        block_id: segment.secondary_id.clone(),
        anchor: suggestion.anchor.clone(),
    });
    if proposal_intersects_user_patch(&segment.secondary_id, &selector, user_patches) {
        return Err(task_error(
            focaus_contracts::ErrorCode::EditProtected,
            "correction_intersects_user_patch",
            false,
        ));
    }
    Ok(())
}

/// Build the exact user command consumed by NativeHost's ContentStore path.
/// The command carries the current Secondary revision as both its base and
/// expected revision; a stale UI cannot migrate the anchor to a newer body.
pub fn build_correction_command(
    command_id: impl Into<String>,
    idempotency_key: impl Into<String>,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    action: CorrectionAction,
    final_text: impl Into<String>,
    critical_confirmed: bool,
) -> Result<Command, SafeError> {
    let command_id = command_id.into();
    let idempotency_key = idempotency_key.into();
    let final_text = final_text.into();
    if command_id.trim().is_empty() || idempotency_key.trim().is_empty() {
        return Err(invalid_argument("correction_command_identity_required"));
    }
    validate_correction_action(
        suggestion,
        segment,
        raw_units,
        action,
        &final_text,
        critical_confirmed,
    )?;
    build_correction_command_after_validation(
        command_id,
        idempotency_key,
        window_id,
        suggestion,
        segment,
        action,
        final_text,
    )
}

fn build_correction_command_after_validation(
    command_id: String,
    idempotency_key: String,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    action: CorrectionAction,
    final_text: String,
) -> Result<Command, SafeError> {
    let mut expected_revisions = std::collections::BTreeMap::new();
    expected_revisions.insert(segment.secondary_id.clone(), segment.revision_id.clone());
    let (name, payload) = match action {
        CorrectionAction::Accept => (
            CommandName::CorrectionAccept,
            serde_json::to_value(CorrectionAcceptInput {
                suggestion_id: suggestion.suggestion_id.clone(),
                base_revision: segment.revision_id.clone(),
                final_text: final_text.clone(),
            })
            .map_err(|_| invalid("correction_command_serialization_failed"))?,
        ),
        CorrectionAction::Reject => (
            CommandName::CorrectionReject,
            serde_json::to_value(CorrectionRejectInput {
                suggestion_id: suggestion.suggestion_id.clone(),
                base_revision: segment.revision_id.clone(),
                final_text,
            })
            .map_err(|_| invalid("correction_command_serialization_failed"))?,
        ),
    };
    Ok(Command {
        contract_version: CONTRACT_VERSION.to_string(),
        command_id,
        name,
        actor: CommandActor::User,
        window_id,
        payload,
        idempotency_key: Some(idempotency_key),
        expected_revisions: Some(expected_revisions),
    })
}

/// Guarded accept/reject path for the formal host call.  A user patch or an
/// active IME composition blocks the command before it reaches ContentStore.
pub fn build_correction_command_with_guards(
    command_id: impl Into<String>,
    idempotency_key: impl Into<String>,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    user_patches: &[UserPatch],
    composition_active: bool,
    action: CorrectionAction,
    final_text: impl Into<String>,
    critical_confirmed: bool,
) -> Result<Command, SafeError> {
    validate_correction_edit_scope(suggestion, segment, user_patches, composition_active)?;
    build_correction_command(
        command_id,
        idempotency_key,
        window_id,
        suggestion,
        segment,
        raw_units,
        action,
        final_text,
        critical_confirmed,
    )
}

pub fn build_correction_draft_command(
    command_id: impl Into<String>,
    idempotency_key: impl Into<String>,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    draft: impl Into<String>,
    composition_active: bool,
) -> Result<Command, SafeError> {
    let command_id = command_id.into();
    let idempotency_key = idempotency_key.into();
    let draft = draft.into();
    if command_id.trim().is_empty() || idempotency_key.trim().is_empty() {
        return Err(invalid_argument("correction_command_identity_required"));
    }
    validate_correction_candidate(suggestion, segment, raw_units)?;
    if !segment
        .suggestions
        .iter()
        .any(|candidate| candidate == suggestion)
    {
        return Err(stale("correction_candidate_not_current"));
    }
    if draft.trim().is_empty() {
        return Err(invalid_argument("correction_draft_required"));
    }
    let mut expected_revisions = std::collections::BTreeMap::new();
    expected_revisions.insert(segment.secondary_id.clone(), segment.revision_id.clone());
    let payload = serde_json::to_value(CorrectionDraftInput {
        suggestion_id: suggestion.suggestion_id.clone(),
        draft,
        composition_active,
    })
    .map_err(|_| invalid("correction_command_serialization_failed"))?;
    Ok(Command {
        contract_version: CONTRACT_VERSION.to_string(),
        command_id,
        name: CommandName::CorrectionDraft,
        actor: CommandActor::User,
        window_id,
        payload,
        idempotency_key: Some(idempotency_key),
        expected_revisions: Some(expected_revisions),
    })
}

pub fn validate_correction_receipt(
    receipt: &CommandResult,
    command: &Command,
    segment: &SecondarySegment,
) -> Result<CorrectionReceipt, SafeError> {
    if receipt.contract_version != CONTRACT_VERSION {
        return Err(task_error(
            focaus_contracts::ErrorCode::UnsupportedVersion,
            "correction_receipt_contract_version_unsupported",
            false,
        ));
    }
    if receipt.command_id != command.command_id {
        return Err(stale("correction_receipt_command_mismatch"));
    }
    match &receipt.status {
        CommandResultStatus::Error => Err(receipt
            .error
            .clone()
            .unwrap_or_else(|| invalid("correction_command_failed_without_error"))),
        CommandResultStatus::Accepted => {
            let task_id = receipt
                .task_id
                .clone()
                .filter(|task_id| !task_id.is_empty())
                .ok_or_else(|| invalid("correction_accepted_task_missing"))?;
            if receipt.current_state.is_none() {
                return Err(invalid("correction_accepted_state_missing"));
            }
            Ok(CorrectionReceipt::Accepted { task_id })
        }
        CommandResultStatus::Ok => {
            let revision_id = receipt
                .committed_revisions
                .as_ref()
                .and_then(|revisions| revisions.get(&segment.secondary_id))
                .cloned()
                .filter(|revision| !revision.is_empty())
                .ok_or_else(|| invalid("correction_committed_revision_missing"))?;
            if revision_id == segment.revision_id {
                return Err(stale("correction_receipt_revision_unchanged"));
            }
            Ok(CorrectionReceipt::Committed { revision_id })
        }
    }
}

/// Invoke the same host command route used by the UI for an explicit accept or
/// reject.  The host remains authoritative for the transaction and revision
/// commit; this helper only performs the preflight source gate.
pub async fn apply_correction<H: HostServices>(
    host: &H,
    command_id: impl Into<String>,
    idempotency_key: impl Into<String>,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    action: CorrectionAction,
    final_text: impl Into<String>,
    critical_confirmed: bool,
) -> Result<CommandResult, SafeError> {
    let command = build_correction_command(
        command_id,
        idempotency_key,
        window_id,
        suggestion,
        segment,
        raw_units,
        action,
        final_text,
        critical_confirmed,
    )?;
    host.invoke(command).await
}

pub async fn apply_correction_with_guards<H: HostServices>(
    host: &H,
    command_id: impl Into<String>,
    idempotency_key: impl Into<String>,
    window_id: Option<EntityId>,
    suggestion: &CorrectionSuggestion,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
    user_patches: &[UserPatch],
    composition_active: bool,
    action: CorrectionAction,
    final_text: impl Into<String>,
    critical_confirmed: bool,
) -> Result<(Command, CommandResult), SafeError> {
    let command = build_correction_command_with_guards(
        command_id,
        idempotency_key,
        window_id,
        suggestion,
        segment,
        raw_units,
        user_patches,
        composition_active,
        action,
        final_text,
        critical_confirmed,
    )?;
    let receipt = host.invoke(command.clone()).await?;
    validate_correction_receipt(&receipt, &command, segment)?;
    Ok((command, receipt))
}

/// Submit a Secondary result through the formal task/result boundary.  The
/// task snapshot must carry the current Secondary base revision; raw source
/// refs are sent separately so NativeHost can perform its own CAS checks.
pub async fn submit_secondary_result<H: HostServices>(
    host: &H,
    task_id: impl Into<String>,
    attempt: u64,
    snapshot: &InputSnapshot,
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
) -> Result<CommandResult, SafeError> {
    validate_secondary_snapshot(snapshot)?;
    let Some(expected_revision) = snapshot.target_base_revision.as_deref() else {
        return Err(invalid_argument("secondary_target_revision_required"));
    };
    if expected_revision != segment.revision_id {
        return Err(stale("secondary_result_revision_stale"));
    }
    if snapshot.recording_id.as_deref() != Some(segment.recording_id.as_str()) {
        return Err(stale("secondary_result_recording_stale"));
    }
    validate_secondary_segment(segment, raw_units)?;
    let candidate = serde_json::to_value(segment)
        .map_err(|_| invalid("secondary_result_serialization_failed"))?;
    host.result_submit(ResultSubmitInput {
        task_id: task_id.into(),
        attempt,
        snapshot_id: snapshot.snapshot_id.clone(),
        candidate,
        source_refs: segment.raw_refs.clone(),
        resources: vec![],
    })
    .await
}

pub fn validate_secondary_segment(
    segment: &SecondarySegment,
    raw_units: &[RawTextUnit],
) -> Result<(), SafeError> {
    if segment.secondary_id.is_empty()
        || segment.recording_id.is_empty()
        || segment.generation_revision.is_empty()
        || segment.revision_id.is_empty()
        || segment.text.trim().is_empty()
    {
        return Err(invalid("secondary_segment_identity_or_text_missing"));
    }
    source_units_for_secondary(
        &segment.recording_id,
        &segment.raw_refs,
        &segment.ordered_spans,
        raw_units,
    )?;
    let draft = SecondaryDraft {
        recording_id: segment.recording_id.clone(),
        raw_refs: segment.raw_refs.clone(),
        ordered_spans: segment.ordered_spans.clone(),
        text: segment.text.clone(),
        suggestions: segment.suggestions.clone(),
    };
    validate_secondary_draft(&draft, raw_units)?;
    for suggestion in &segment.suggestions {
        validate_correction_candidate(suggestion, segment, raw_units)?;
    }
    Ok(())
}

pub fn validate_five_tier_variants(
    note_id: &str,
    fact_set_revision: &str,
    fact_ids: &[EntityId],
    variants: &[LanguageVariant],
) -> Result<(), SafeError> {
    if variants.len() != 5 {
        return Err(invalid("language_variants_must_have_five_tiers"));
    }
    let expected: BTreeSet<String> = fact_ids.iter().cloned().collect();
    if expected.len() != fact_ids.len() {
        return Err(invalid("language_variant_fact_set_contains_duplicates"));
    }
    let mut tiers = BTreeSet::new();
    for variant in variants {
        if variant.note_id != note_id
            || variant.fact_set_revision != fact_set_revision
            || variant.fact_ids.len() != expected.len()
            || variant.fact_ids.iter().cloned().collect::<BTreeSet<_>>() != expected
            || !(1..=5).contains(&variant.tier)
        {
            return Err(invalid("language_variant_fact_set_mismatch"));
        }
        if !tiers.insert(variant.tier) {
            return Err(invalid("duplicate_language_tier"));
        }
    }
    Ok(())
}

/// The host supplies exact selectors and user patches.  AI treats an exact or
/// overlapping protected selector as review-required; it never silently
/// rewrites an active composition or a remembered delete intent.
pub fn proposal_intersects_user_patch(
    target_id: &str,
    selector: &Selector,
    patches: &[UserPatch],
) -> bool {
    patches.iter().any(|patch| {
        patch.target_id == target_id && selectors_intersect(selector, &patch.selector)
    })
}

fn selectors_intersect(left: &Selector, right: &Selector) -> bool {
    let left_json = serde_json::to_value(left).ok();
    let right_json = serde_json::to_value(right).ok();
    if left_json == right_json {
        return true;
    }
    match (left, right) {
        (Selector::V0(a), Selector::V0(b)) => {
            a.recording_id == b.recording_id
                && a.audio_span_id == b.audio_span_id
                && a.track_id == b.track_id
                && a.start_ms < b.end_ms
                && b.start_ms < a.end_ms
        }
        (Selector::V1(a), Selector::V1(b)) => {
            a.block_id == b.block_id
                && a.anchor.scalar_range.start < b.anchor.scalar_range.end
                && b.anchor.scalar_range.start < a.anchor.scalar_range.end
        }
        (Selector::V2(a), Selector::V2(b)) => {
            a.material_id == b.material_id
                && a.source_version_id == b.source_version_id
                && a.parse_version_id == b.parse_version_id
                && a.page_id == b.page_id
                && a.region_id == b.region_id
        }
        (Selector::V3(a), Selector::V3(b)) => a.object_id == b.object_id,
        (Selector::V4(_), Selector::V4(_)) => true,
        _ => false,
    }
}

pub fn validate_symbol_binding_scope(
    binding: &SymbolBinding,
    active_scope_id: &str,
) -> Result<(), SafeError> {
    if binding.binding_id.is_empty()
        || binding.symbol.trim().is_empty()
        || binding.scope_id != active_scope_id
        || binding.definition_id.is_empty()
        || binding.valid_from_anchor.entity_id.is_empty()
    {
        return Err(invalid("symbol_binding_scope_mismatch"));
    }
    if binding.value.is_none() && binding.meaning.trim().is_empty() && binding.expression.trim().is_empty() {
        return Err(invalid("symbol_without_value_or_meaning"));
    }
    Ok(())
}

/// Resolve a definition only within the host-filtered lesson/scope.  The
/// caller may label a returned definition outside the requested window; this
/// function never imports a cross-course definition as if it were current.
pub fn choose_scoped_definition<'a>(
    definitions: &'a [Definition],
    lesson_id: &str,
    scope_id: &str,
    concept_id: &str,
) -> Option<&'a Definition> {
    definitions
        .iter()
        .filter(|definition| definition.lesson_id == lesson_id && definition.concept_id == concept_id)
        .find(|definition| definition.scope_id == scope_id)
        .or_else(|| definitions.iter().find(|definition| definition.lesson_id == lesson_id && definition.concept_id == concept_id))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementProposal {
    pub kind: ArrangementKind,
    pub text: String,
    pub source_refs: Vec<SourceRef>,
    pub relative_date_status: RelativeDateStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrangementKind {
    TeacherAssignment,
    TeacherDeadline,
    TeacherExam,
    TeacherEmphasis,
    AiInference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeDateStatus {
    Resolved,
    Unknown,
    NotApplicable,
}

pub fn validate_arrangement(proposal: &ArrangementProposal) -> Result<(), SafeError> {
    if proposal.text.trim().is_empty() {
        return Err(invalid("arrangement_text_missing"));
    }
    if !matches!(proposal.kind, ArrangementKind::AiInference) && proposal.source_refs.is_empty() {
        return Err(invalid("teacher_arrangement_source_missing"));
    }
    if matches!(proposal.kind, ArrangementKind::TeacherDeadline) && proposal.relative_date_status == RelativeDateStatus::Resolved && proposal.text.contains("?") {
        return Err(invalid("resolved_deadline_contains_uncertainty"));
    }
    Ok(())
}

pub fn make_language_variant(
    variant_id: impl Into<String>,
    note_id: impl Into<String>,
    tier: u64,
    fact_set_revision: impl Into<String>,
    fact_ids: Vec<EntityId>,
    presentation_revision: impl Into<String>,
) -> Result<LanguageVariant, SafeError> {
    if !(1..=5).contains(&tier) {
        return Err(invalid_argument("language_tier_out_of_range"));
    }
    Ok(LanguageVariant { variant_id: variant_id.into(), note_id: note_id.into(), tier, fact_set_revision: fact_set_revision.into(), fact_ids, presentation_revision: presentation_revision.into(), sync_status: LanguageVariantSyncStatus::Generating })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaPart {
    pub class: QaClass,
    pub text: String,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QaClass {
    Classroom,
    Material,
    AiBackground,
}

pub fn validate_qa_parts(parts: &[QaPart]) -> Result<(), SafeError> {
    if parts.is_empty() {
        return Err(invalid("qa_answer_empty"));
    }
    for part in parts {
        if part.text.trim().is_empty() {
            return Err(invalid("qa_part_empty"));
        }
        if matches!(part.class, QaClass::Classroom | QaClass::Material) && part.source_refs.is_empty() {
            return Err(invalid("sourced_qa_part_missing_source"));
        }
    }
    Ok(())
}

pub fn build_secondary_request(snapshot: &InputSnapshot, raw_text: &[RawTextUnit]) -> Result<Value, SafeError> {
    let binding = crate::binding_for_model(ModelId::Qwen38Flash);
    build_secondary_request_with_binding(snapshot, &binding, raw_text)
}

pub fn build_secondary_request_with_binding(
    snapshot: &InputSnapshot,
    binding: &focaus_contracts::CredentialBinding,
    raw_text: &[RawTextUnit],
) -> Result<Value, SafeError> {
    validate_secondary_snapshot(snapshot)?;
    crate::binding_matches(binding, ModelId::Qwen38Flash)?;
    validate_raw_units_for_request(raw_text)?;
    Ok(crate::transport::build_chat_completion_body(
        models::MULTIMODAL,
        PROMPT_SECONDARY_V1,
        "Clean direct speech without paraphrasing or third-person narration. Return only the requested JSON object.",
        json!({
            "snapshot_id": snapshot.snapshot_id,
            "current_secondary_revision": snapshot.target_base_revision,
            "source_refs": raw_text.iter().map(|unit| unit.source_ref.clone()).collect::<Vec<_>>(),
            "raw_units": raw_text,
            "output_contract": "direct_speech_cleanup_with_review_suggestions",
            "constraints": ["no_third_person_rewrite", "preserve_order", "critical_changes_are_suggestions"]
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{EvidenceCutoff, EvidenceCutoffStatus, ModelId, Selector, TaskKind, WholeSelector};

    fn source() -> SourceRef {
        SourceRef { entity_id: "u".into(), revision_id: "r".into(), content_hash: "h".into(), selector: Selector::V4(WholeSelector { kind: "whole".into() }) }
    }

    fn unit(text: &str) -> RawTextUnit {
        RawTextUnit { utterance_id: "u".into(), recording_id: "rec".into(), text: text.into(), source_ref: source(), span_id: "span".into() }
    }

    fn correction_fixture() -> (SecondarySegment, RawTextUnit, CorrectionSuggestion) {
        let raw = unit("the value is 2");
        let mut segment = build_secondary_segment("sec", "generation-1", "secondary-rev-1", &[raw.clone()]).unwrap();
        let suggestion = make_correction(
            "suggestion-1",
            "sec",
            "secondary-rev-1",
            focaus_contracts::TextAnchor {
                left_element_id: None,
                right_element_id: None,
                element_ids: vec![],
                scalar_range: focaus_contracts::ScalarRange { start: 0, end: 14 },
            },
            "the value is 2",
            "the value is 3",
            "check the numeric value",
        );
        segment.suggestions = vec![suggestion.clone()];
        (segment, raw, suggestion)
    }

    fn secondary_snapshot(model: ModelId) -> InputSnapshot {
        InputSnapshot { snapshot_id: "snap".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose: TaskKind::Secondary, target_id: "rec".into(), lesson_id: None, recording_id: Some("rec".into()), assignment_epoch: Some(1), sources: vec![source()], attachment_set_revision: "attach".into(), fact_set_revision: None, target_base_revision: None, edit_revision: None, language_tier: Some(3), evidence_cutoff: vec![EvidenceCutoff { recording_id: "rec".into(), end_ms: Some(10), status: EvidenceCutoffStatus::Known }], available_at: "2026-01-01T00:00:00Z".into(), executor_id: "ai".into(), executor_version: "1".into(), model_id: Some(model), region: Some("cn-beijing".into()), prompt_version: Some(PROMPT_SECONDARY_V1.into()), parameters_hash: "p".into(), calibration_seq: None, decision_seq: None }
    }

    #[test]
    fn trimming_preserves_direct_speech_and_removes_fillers() {
        let result = build_secondary_segment("sec", "g", "r", &[unit("嗯 这个 theorem。。 holds")]).unwrap();
        assert_eq!(result.text, "theorem。 holds");
        assert!(result.text.contains("theorem"));
    }

    #[test]
    fn secondary_rejects_non_multimodal_model_before_generation() {
        assert!(validate_secondary_snapshot(&secondary_snapshot(ModelId::DecisionModelPreview)).is_err());
        assert!(validate_secondary_snapshot(&secondary_snapshot(ModelId::QwenAudio31AsrFlashFiletrans)).is_err());
        assert!(build_secondary_request(&secondary_snapshot(ModelId::Qwen38Flash), &[unit("it works")]).is_ok());
        let wrong_binding = crate::binding_for_model(ModelId::DecisionModelPreview);
        assert_eq!(
            build_secondary_request_with_binding(&secondary_snapshot(ModelId::Qwen38Flash), &wrong_binding, &[unit("it works")]).unwrap_err().code,
            focaus_contracts::ErrorCode::InvalidArgument,
        );
        let mut old_contract = secondary_snapshot(ModelId::Qwen38Flash);
        old_contract.contract_version = "1.0.0".into();
        assert_eq!(validate_secondary_snapshot(&old_contract).unwrap_err().code, focaus_contracts::ErrorCode::UnsupportedVersion);
        let mut unknown_tier = secondary_snapshot(ModelId::Qwen38Flash);
        unknown_tier.language_tier = Some(6);
        assert_eq!(validate_secondary_snapshot(&unknown_tier).unwrap_err().code, focaus_contracts::ErrorCode::InvalidArgument);
    }

    #[test]
    fn secondary_draft_cannot_use_unselected_or_stale_raw_sources() {
        let first = unit("first");
        let mut second = unit("second");
        second.span_id = "span-2".into();
        let draft = SecondaryDraft {
            recording_id: "rec".into(),
            raw_refs: vec![first.source_ref.clone()],
            ordered_spans: vec![first.span_id.clone()],
            text: "second".into(),
            suggestions: vec![],
        };
        assert_eq!(validate_secondary_draft(&draft, &[first.clone(), second]).unwrap_err().code, focaus_contracts::ErrorCode::InvalidResult);
        let mut changed = first.clone();
        changed.source_ref.revision_id = "raw-rev-2".into();
        assert_eq!(validate_secondary_draft(&draft, &[changed]).unwrap_err().code, focaus_contracts::ErrorCode::StaleInput);
    }

    #[test]
    fn correction_candidate_is_bound_to_current_secondary_and_raw_revisions() {
        let (segment, raw, suggestion) = correction_fixture();
        validate_correction_candidate(&suggestion, &segment, &[raw.clone()]).unwrap();

        let mut stale_candidate = suggestion.clone();
        stale_candidate.source_revision = "secondary-rev-0".into();
        assert_eq!(validate_correction_candidate(&stale_candidate, &segment, &[raw.clone()]).unwrap_err().code, focaus_contracts::ErrorCode::StaleInput);

        let mut changed_raw = raw;
        changed_raw.source_ref.revision_id = "raw-rev-2".into();
        assert_eq!(validate_correction_candidate(&suggestion, &segment, &[changed_raw]).unwrap_err().code, focaus_contracts::ErrorCode::StaleInput);
    }

    #[test]
    fn correction_commands_gate_critical_accept_and_preserve_raw_layer() {
        let (segment, raw, suggestion) = correction_fixture();
        let raw_before = raw.clone();
        assert_eq!(
            build_correction_command("cmd-1", "idem-1", None, &suggestion, &segment, &[raw.clone()], CorrectionAction::Accept, "the value is 3", false).unwrap_err().code,
            focaus_contracts::ErrorCode::InvalidArgument,
        );
        let accepted = build_correction_command("cmd-2", "idem-2", None, &suggestion, &segment, &[raw.clone()], CorrectionAction::Accept, "the value is 3", true).unwrap();
        assert_eq!(accepted.name, CommandName::CorrectionAccept);
        assert_eq!(accepted.payload["base_revision"], "secondary-rev-1");
        assert_eq!(accepted.expected_revisions.as_ref().unwrap()["sec"], "secondary-rev-1");
        assert_eq!(raw, raw_before);

        let rejected = build_correction_command("cmd-3", "idem-3", None, &suggestion, &segment, &[raw.clone()], CorrectionAction::Reject, "the value is 2", false).unwrap();
        assert_eq!(rejected.name, CommandName::CorrectionReject);
        assert_eq!(rejected.payload["final_text"], "the value is 2");
        assert_eq!(build_correction_command("cmd-4", "idem-4", None, &suggestion, &segment, &[raw], CorrectionAction::Reject, "the value is 3", false).unwrap_err().code, focaus_contracts::ErrorCode::InvalidArgument);

        let draft = build_correction_draft_command("cmd-5", "idem-5", None, &suggestion, &segment, &[unit("the value is 2")], "the value is 3", true).unwrap();
        assert_eq!(draft.name, CommandName::CorrectionDraft);
        assert!(draft.payload["composition_active"].as_bool().unwrap());

        let receipt = CommandResult {
            contract_version: focaus_contracts::CONTRACT_VERSION.into(),
            command_id: accepted.command_id.clone(),
            status: CommandResultStatus::Ok,
            result: Some(json!({"state":"accepted"})),
            committed_revisions: Some(std::collections::BTreeMap::from([(String::from("sec"), String::from("secondary-rev-2"))])),
            event_seq: Some(1),
            task_id: None,
            current_state: None,
            error: None,
        };
        assert_eq!(validate_correction_receipt(&receipt, &accepted, &segment).unwrap(), CorrectionReceipt::Committed { revision_id: "secondary-rev-2".into() });
        let accepted_receipt = CommandResult {
            contract_version: focaus_contracts::CONTRACT_VERSION.into(), command_id: accepted.command_id.clone(), status: CommandResultStatus::Accepted,
            result: None, committed_revisions: None, event_seq: None, task_id: Some("task-1".into()), current_state: Some(focaus_contracts::TaskState::Running), error: None,
        };
        assert_eq!(validate_correction_receipt(&accepted_receipt, &accepted, &segment).unwrap(), CorrectionReceipt::Accepted { task_id: "task-1".into() });

        let patch = UserPatch {
            patch_id: "patch".into(), target_id: "sec".into(), base_revision: "secondary-rev-1".into(),
            operation: focaus_contracts::UserPatchOperation::TextReplace,
            selector: Selector::V1(focaus_contracts::TextSelector { kind: "text".into(), block_id: "sec".into(), anchor: suggestion.anchor.clone() }),
            before_hash: "hash".into(), after_value: json!("user"), author: "user".into(), intent: "edit".into(), undo_group_id: "undo".into(), created_at: "now".into(),
        };
        assert_eq!(build_correction_command_with_guards("cmd-6", "idem-6", None, &suggestion, &segment, &[unit("the value is 2")], &[patch], false, CorrectionAction::Accept, "the value is 3", true).unwrap_err().code, focaus_contracts::ErrorCode::EditProtected);
        assert_eq!(build_correction_command_with_guards("cmd-7", "idem-7", None, &suggestion, &segment, &[unit("the value is 2")], &[], true, CorrectionAction::Accept, "the value is 3", true).unwrap_err().code, focaus_contracts::ErrorCode::EditProtected);
    }

    #[test]
    fn paraphrase_and_unbacked_third_person_are_rejected() {
        let bad = SecondaryDraft { recording_id: "rec".into(), raw_refs: vec![source()], ordered_spans: vec!["span".into()], text: "the teacher says it works".into(), suggestions: vec![] };
        assert!(validate_secondary_draft(&bad, &[unit("it works")]).is_err());
        let reordered = SecondaryDraft { recording_id: "rec".into(), raw_refs: vec![source()], ordered_spans: vec!["span".into()], text: "works it".into(), suggestions: vec![] };
        assert!(validate_secondary_draft(&reordered, &[unit("it works")]).is_err());
    }

    #[test]
    fn critical_changes_are_redline_suggestions() {
        let ordinary = classify_semantic_risk("the value is 2", "the value is 2");
        assert_eq!(ordinary, CorrectionSuggestionSemanticRisk::Ordinary);
        assert_eq!(classify_semantic_risk("x is 2", "x is 3"), CorrectionSuggestionSemanticRisk::Critical);
        assert_eq!(classify_semantic_risk("if x", "x"), CorrectionSuggestionSemanticRisk::Critical);
    }

    #[test]
    fn five_tiers_share_exact_fact_set() {
        let variants = (1..=5).map(|tier| make_language_variant(format!("v{tier}"), "note", tier, "facts", vec!["f1".into(), "f2".into()], format!("p{tier}")).unwrap()).collect::<Vec<_>>();
        validate_five_tier_variants("note", "facts", &["f1".into(), "f2".into()], &variants).unwrap();
        let mut bad = variants;
        bad[4].fact_ids.push("extra".into());
        assert!(validate_five_tier_variants("note", "facts", &["f1".into(), "f2".into()], &bad).is_err());
        let duplicate_variants = (1..=5).map(|tier| make_language_variant(format!("d{tier}"), "note", tier, "facts", vec!["f1".into(), "f1".into()], format!("p{tier}")).unwrap()).collect::<Vec<_>>();
        assert!(validate_five_tier_variants("note", "facts", &["f1".into(), "f1".into()], &duplicate_variants).is_err());
    }

    #[test]
    fn qa_keeps_classroom_and_background_separate() {
        validate_qa_parts(&[QaPart { class: QaClass::Classroom, text: "teacher said this".into(), source_refs: vec![source()] }, QaPart { class: QaClass::AiBackground, text: "additional context".into(), source_refs: vec![] }]).unwrap();
        assert!(validate_qa_parts(&[QaPart { class: QaClass::Classroom, text: "unsourced".into(), source_refs: vec![] }]).is_err());
    }

    #[test]
    fn user_patches_and_scoped_symbols_are_protected() {
        let selector = Selector::V1(focaus_contracts::TextSelector { kind: "text".into(), block_id: "00000000-0000-4000-8000-000000000020".into(), anchor: focaus_contracts::TextAnchor { left_element_id: None, right_element_id: None, element_ids: vec![], scalar_range: focaus_contracts::ScalarRange { start: 0, end: 5 } } });
        let patch = UserPatch { patch_id: "00000000-0000-4000-8000-000000000021".into(), target_id: "00000000-0000-4000-8000-000000000022".into(), base_revision: "00000000-0000-4000-8000-000000000023".into(), operation: focaus_contracts::UserPatchOperation::TextReplace, selector: selector.clone(), before_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(), after_value: json!("user"), author: "user".into(), intent: "edit".into(), undo_group_id: "00000000-0000-4000-8000-000000000024".into(), created_at: "2026-01-01T00:00:00Z".into() };
        let target_id = patch.target_id.clone();
        assert!(proposal_intersects_user_patch(&target_id, &selector, &[patch]));
    }
}

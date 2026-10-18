//! Global classroom progress adapter for `decision-model-preview`.
//!
//! The decision model chooses among host-provided page/region candidates. It
//! never writes focaus prose and it cannot infer a page from elapsed wall time.

use crate::{binding_matches, invalid, invalid_argument, stale, task_error, models, PROMPT_PROGRESS_V1};
use focaus_contracts::{
    CredentialBinding, ErrorCode, InputSnapshot, ModelId, ProgressState,
    ProgressStateFreshness, ProgressStatePositionStatus, ProgressStateRecordingState, SafeError,
    TaskKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const PROGRESS_MODEL_ID: &str = models::PROGRESS;
pub const FRESHNESS_THRESHOLD_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCandidate {
    pub page_id: String,
    pub region_id: Option<String>,
    pub material_id: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressEvidence {
    pub lesson_id: String,
    pub recording_id: String,
    pub assignment_epoch: u64,
    pub current_recording_ms: u64,
    pub evidence_cutoff_ms: Option<u64>,
    pub transcript_text: String,
    pub pages: Vec<PageCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressDecision {
    pub snapshot_id: String,
    pub lesson_id: String,
    pub recording_id: String,
    pub assignment_epoch: u64,
    pub decision_seq: u64,
    pub calibration_seq: u64,
    pub evidence_cutoff_ms: Option<u64>,
    pub position_status: ProgressStatePositionStatus,
    pub page_id: Option<String>,
    pub region_id: Option<String>,
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressAccept {
    Accept,
    IgnoreOlderDecision,
    IgnoreOlderCalibration,
    IgnoreOlderEvidence,
    IgnoreWrongAssignment,
}

pub fn validate_progress_binding(
    snapshot: &InputSnapshot,
    binding: &CredentialBinding,
) -> Result<(), SafeError> {
    if snapshot.purpose != TaskKind::Progress
        || snapshot.model_id != Some(ModelId::DecisionModelPreview)
        || snapshot.region.as_deref() != Some(models::REGION)
        || snapshot.prompt_version.as_deref() != Some(PROMPT_PROGRESS_V1)
    {
        return Err(invalid_argument("progress_snapshot_binding_mismatch"));
    }
    binding_matches(binding, ModelId::DecisionModelPreview)
}

pub fn build_system_one_request(
    snapshot: &InputSnapshot,
    binding: &CredentialBinding,
    evidence: &ProgressEvidence,
) -> Result<Value, SafeError> {
    validate_progress_binding(snapshot, binding)?;
    validate_evidence(evidence)?;
    if snapshot.recording_id.as_deref() != Some(evidence.recording_id.as_str())
        || snapshot.assignment_epoch != Some(evidence.assignment_epoch)
    {
        return Err(stale("progress_evidence_assignment_mismatch"));
    }
    let choices: Vec<Value> = evidence
        .pages
        .iter()
        .map(|page| {
            json!({
                "key": page.page_id,
                "description": format!("{}: {}", page.title, page.text),
                "region_id": page.region_id,
            })
        })
        .chain(std::iter::once(json!({
            "key": "__unlocated__",
            "description": "证据不足或不在本堂课件范围",
        })))
        .collect();
    if choices.len() > 255 {
        return Err(invalid_argument("progress_choice_limit_exceeded"));
    }
    let criteria = choices.iter().filter_map(|choice| {
        let key = choice.get("key")?.as_str()?.to_string();
        let description = choice.get("description")?.as_str()?.to_string();
        Some((key, Value::String(description)))
    }).collect::<serde_json::Map<_, _>>();
    Ok(json!({
        "model": PROGRESS_MODEL_ID,
        "protocol": "system_one",
        "snapshot_id": snapshot.snapshot_id,
        "state": {
            "lesson_id": evidence.lesson_id,
            "recording_id": evidence.recording_id,
            "assignment_epoch": evidence.assignment_epoch,
            "transcript_cutoff_ms": evidence.evidence_cutoff_ms,
            "transcript": evidence.transcript_text,
            "pages": evidence.pages,
        },
        "questions": {
            "current_position": {
                "type": "choice",
                "instructions": "选择当前讲解对应的本堂课件位置；证据不足时选择 __unlocated__。",
                "criteria": criteria,
            }
        }
    }))
}

pub fn parse_system_one_choice(value: &Value, question_id: &str) -> Result<String, SafeError> {
    let answer = value.get("answers").and_then(|answers| answers.get(question_id)).ok_or_else(|| invalid("progress_answer_missing"))?;
    if answer.get("type").and_then(Value::as_str) != Some("choice") {
        return Err(invalid("progress_answer_type_mismatch"));
    }
    answer.get("choice").and_then(Value::as_str).filter(|choice| !choice.is_empty()).map(str::to_owned).ok_or_else(|| invalid("progress_choice_missing"))
}

fn validate_evidence(evidence: &ProgressEvidence) -> Result<(), SafeError> {
    if evidence.lesson_id.is_empty() || evidence.recording_id.is_empty() || evidence.transcript_text.trim().is_empty() {
        return Err(invalid("progress_evidence_missing"));
    }
    let mut ids = BTreeSet::new();
    for page in &evidence.pages {
        if page.page_id.is_empty() || page.material_id.is_empty() || !ids.insert(page.page_id.as_str()) {
            return Err(invalid("progress_page_candidate_invalid"));
        }
    }
    if evidence.evidence_cutoff_ms.is_some_and(|cutoff| cutoff > evidence.current_recording_ms) {
        return Err(stale("progress_evidence_from_future"));
    }
    Ok(())
}

pub fn validate_decision(
    decision: &ProgressDecision,
    evidence: &ProgressEvidence,
    current: Option<&ProgressState>,
) -> Result<ProgressAccept, SafeError> {
    validate_evidence(evidence)?;
    if decision.snapshot_id.is_empty()
        || decision.recording_id != evidence.recording_id
        || decision.lesson_id != evidence.lesson_id
        || decision.assignment_epoch != evidence.assignment_epoch
    {
        return Err(stale("progress_decision_assignment_mismatch"));
    }
    if decision.evidence_cutoff_ms.is_some_and(|cutoff| cutoff > evidence.current_recording_ms) {
        return Err(stale("progress_decision_uses_future_evidence"));
    }
    if let Some(current) = current {
        if decision.assignment_epoch != current.assignment_epoch
            || decision.recording_id != current.recording_id
            || decision.lesson_id != current.lesson_id
        {
            return Ok(ProgressAccept::IgnoreWrongAssignment);
        }
        if decision.calibration_seq < current.calibration_seq {
            return Ok(ProgressAccept::IgnoreOlderCalibration);
        }
        if decision.decision_seq < current.decision_seq {
            return Ok(ProgressAccept::IgnoreOlderDecision);
        }
        if decision.evidence_cutoff_ms.is_some_and(|new_cutoff| current.evidence_cutoff_ms.is_some_and(|old| new_cutoff < old)) {
            return Ok(ProgressAccept::IgnoreOlderEvidence);
        }
    }
    if let Some(page_id) = &decision.page_id {
        if decision.position_status == ProgressStatePositionStatus::Located
            && !evidence.pages.iter().any(|page| &page.page_id == page_id)
        {
            return Err(invalid("progress_decision_page_outside_scope"));
        }
    }
    if decision.position_status == ProgressStatePositionStatus::Located && decision.page_id.is_none() {
        return Err(invalid("located_progress_requires_page"));
    }
    Ok(ProgressAccept::Accept)
}

pub fn to_progress_state(
    decision: &ProgressDecision,
    evidence: &ProgressEvidence,
    snapshot_id: impl Into<String>,
    recording_state: ProgressStateRecordingState,
) -> Result<ProgressState, SafeError> {
    validate_decision(decision, evidence, None)?;
    let freshness = match decision.evidence_cutoff_ms {
        Some(cutoff) if evidence.current_recording_ms.saturating_sub(cutoff) <= FRESHNESS_THRESHOLD_MS => ProgressStateFreshness::Current,
        _ => ProgressStateFreshness::PendingUpdate,
    };
    Ok(ProgressState {
        lesson_id: decision.lesson_id.clone(),
        recording_id: decision.recording_id.clone(),
        assignment_epoch: decision.assignment_epoch,
        snapshot_id: snapshot_id.into(),
        position_status: decision.position_status.clone(),
        page_id: decision.page_id.clone(),
        region_id: decision.region_id.clone(),
        evidence_cutoff_ms: decision.evidence_cutoff_ms,
        calibration_seq: decision.calibration_seq,
        decision_seq: decision.decision_seq,
        recording_state,
        freshness,
    })
}

/// Single-flight/latest-wins scheduler state.  The host may call `schedule`
/// every ten seconds, but only the latest not-yet-started input is retained.
#[derive(Debug, Default, Clone)]
pub struct LatestProgress {
    latest_seq: u64,
    running_seq: Option<u64>,
}

impl LatestProgress {
    pub fn schedule(&mut self, sequence: u64) -> Result<bool, SafeError> {
        if sequence <= self.latest_seq {
            return Err(task_error(ErrorCode::IdempotencyMismatch, "progress_sequence_not_monotonic", false));
        }
        self.latest_seq = sequence;
        Ok(self.running_seq.is_none())
    }

    pub fn start_latest(&mut self) -> Option<u64> {
        if self.running_seq.is_none() && self.latest_seq != 0 {
            self.running_seq = Some(self.latest_seq);
        }
        self.running_seq
    }

    pub fn finish(&mut self, sequence: u64) -> Result<bool, SafeError> {
        if self.running_seq != Some(sequence) {
            return Err(stale("progress_finish_is_not_running_sequence"));
        }
        self.running_seq = None;
        Ok(self.latest_seq > sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_for_model;
    use focaus_contracts::{EvidenceCutoff, EvidenceCutoffStatus};

    fn snapshot() -> InputSnapshot {
        InputSnapshot {
            snapshot_id: "snap".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose: TaskKind::Progress, target_id: "lesson".into(), lesson_id: Some("lesson".into()), recording_id: Some("rec".into()), assignment_epoch: Some(2), sources: vec![], attachment_set_revision: "a".into(), fact_set_revision: None, target_base_revision: None, edit_revision: None, language_tier: None, evidence_cutoff: vec![EvidenceCutoff { recording_id: "rec".into(), end_ms: Some(1000), status: EvidenceCutoffStatus::Known }], available_at: "now".into(), executor_id: "progress".into(), executor_version: "1".into(), model_id: Some(ModelId::DecisionModelPreview), region: Some("cn-beijing".into()), prompt_version: Some(PROMPT_PROGRESS_V1.into()), parameters_hash: "p".into(), calibration_seq: Some(3), decision_seq: Some(4),
        }
    }

    fn evidence(cutoff: Option<u64>) -> ProgressEvidence {
        ProgressEvidence { lesson_id: "lesson".into(), recording_id: "rec".into(), assignment_epoch: 2, current_recording_ms: 31_001, evidence_cutoff_ms: cutoff, transcript_text: "the theorem continues".into(), pages: vec![PageCandidate { page_id: "p1".into(), region_id: None, material_id: "m".into(), title: "Theorem".into(), text: "boundary".into() }] }
    }

    #[test]
    fn progress_request_is_latest_evidence_only_and_exactly_bound() {
        let request = build_system_one_request(&snapshot(), &binding_for_model(ModelId::DecisionModelPreview), &evidence(Some(1000))).unwrap();
        assert_eq!(request["model"], PROGRESS_MODEL_ID);
        assert_eq!(request["state"]["transcript_cutoff_ms"], 1000);
        assert_eq!(request["questions"]["current_position"]["type"], "choice");
        assert!(request.to_string().find("credential").is_none());
    }

    #[test]
    fn wrong_model_or_credential_slot_is_rejected_before_progress_request() {
        let mut wrong_model = snapshot();
        wrong_model.model_id = Some(ModelId::Qwen38Flash);
        assert_eq!(build_system_one_request(&wrong_model, &binding_for_model(ModelId::DecisionModelPreview), &evidence(Some(1000))).unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(build_system_one_request(&snapshot(), &binding_for_model(ModelId::Qwen38Flash), &evidence(Some(1000))).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn thirty_second_age_sets_pending_update() {
        let d = ProgressDecision { snapshot_id: "snap".into(), lesson_id: "lesson".into(), recording_id: "rec".into(), assignment_epoch: 2, decision_seq: 4, calibration_seq: 3, evidence_cutoff_ms: Some(1000), position_status: ProgressStatePositionStatus::Located, page_id: Some("p1".into()), region_id: None, confidence: Some("high".into()) };
        let state = to_progress_state(&d, &evidence(Some(1000)), "snap", ProgressStateRecordingState::Recording).unwrap();
        assert_eq!(state.freshness, ProgressStateFreshness::PendingUpdate);
        let state = to_progress_state(&ProgressDecision { evidence_cutoff_ms: Some(2_000), ..d }, &ProgressEvidence { current_recording_ms: 31_000, ..evidence(Some(1000)) }, "snap", ProgressStateRecordingState::Recording).unwrap();
        assert_eq!(state.freshness, ProgressStateFreshness::Current);
    }

    #[test]
    fn unknown_page_and_old_sequences_are_rejected_or_ignored() {
        let mut d = ProgressDecision { snapshot_id: "snap".into(), lesson_id: "lesson".into(), recording_id: "rec".into(), assignment_epoch: 2, decision_seq: 4, calibration_seq: 3, evidence_cutoff_ms: Some(1000), position_status: ProgressStatePositionStatus::Located, page_id: Some("p2".into()), region_id: None, confidence: None };
        assert_eq!(validate_decision(&d, &evidence(Some(1000)), None).unwrap_err().code, ErrorCode::InvalidResult);
        d.page_id = Some("p1".into());
        let old = ProgressState { lesson_id: "lesson".into(), recording_id: "rec".into(), assignment_epoch: 2, snapshot_id: "old".into(), position_status: ProgressStatePositionStatus::Located, page_id: Some("p1".into()), region_id: None, evidence_cutoff_ms: Some(1000), calibration_seq: 3, decision_seq: 5, recording_state: ProgressStateRecordingState::Recording, freshness: ProgressStateFreshness::Current };
        assert_eq!(validate_decision(&d, &evidence(Some(1000)), Some(&old)).unwrap(), ProgressAccept::IgnoreOlderDecision);
    }

    #[test]
    fn scheduler_keeps_single_flight_and_latest_pending() {
        let mut scheduler = LatestProgress::default();
        assert!(scheduler.schedule(1).unwrap());
        assert_eq!(scheduler.start_latest(), Some(1));
        assert!(!scheduler.schedule(2).unwrap());
        assert!(scheduler.finish(1).unwrap());
        assert_eq!(scheduler.start_latest(), Some(2));
    }

    #[test]
    fn system_one_fixture_parses_only_the_choice_answer() {
        let value = serde_json::json!({"model":"decision-model-preview","answers":{"current_position":{"type":"choice","choice":"p1","confidence":0.9,"probabilities":{"p1":0.9,"__unlocated__":0.1}}}});
        assert_eq!(parse_system_one_choice(&value, "current_position").unwrap(), "p1");
        assert!(parse_system_one_choice(&serde_json::json!({"answers":{}}), "current_position").is_err());
    }
}

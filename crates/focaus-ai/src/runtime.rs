//! Host-facing runtime orchestration without persistence.
//!
//! N owns audio capture, durable spans, task persistence and the provider
//! socket. A owns the in-memory protocol state below and returns candidates to
//! N through `result_submit`; it never writes a database or decides that a
//! candidate is authoritative.

use crate::asr::{
    build_stream_finish_task, decode_stream_event, plan_retry, submit_candidate,
    DecodedStreamEvent, PublishDecision, ResultGate, RetryPlan,
    SentenceDisposition, StreamingSession,
};
use crate::catchup::{
    build_result_with_context, validate_publication, DeadlineBudget,
    FocausDraft, FocausPublication,
};
use crate::progress::{
    parse_system_one_choice, to_progress_state, validate_decision, ProgressDecision, ProgressEvidence,
};
use crate::{cancelled, invalid, invalid_argument, stale, task_error};
use focaus_contracts::{
    AsrRawCandidate, AsrStreamOpen, AudioChunk, CatchupGenerateInput, CatchupResult, ErrorCode,
    InputSnapshot, ProgressState, ProgressStatePositionStatus, ProgressStateRecordingState, SafeError,
};
use serde_json::Value;

pub const DEFAULT_PROGRESS_INTERVAL_MS: u64 = 10_000;
pub const FOCAUS_DEADLINE_MS: u64 = crate::catchup::FOCAUS_DEADLINE_MS;

/// Minimal host wiring contract. N/C can use this list as the integration
/// checklist; none of these calls grant A database or credential access.
pub const HOST_WIRING_REQUIREMENTS: &[&str] = &[
    "N creates AsrService from the host-built InputSnapshot and TimeMapping.",
    "N passes each durable mono audio chunk to AsrService::feed_chunk without blocking the writer.",
    "N decodes provider WebSocket events and calls AsrService::ingest_provider_event; A returns candidates only.",
    "N submits returned candidates through asr::submit_candidate and owns CAS, epoch, tombstone and DB writes.",
    "N calls AsrService::finish_task_request after the final audio chunk and marks provider completion separately.",
    "N provides the credential capability and EndpointPolicy; A never reads a secret or selects a fallback key.",
    "N schedules progress at the cadence boundary and supplies only latest transcript plus page text.",
    "N starts focaus with a frozen snapshot/context and uses FocausController to reject cancelled, stale or late bodies.",
];

pub fn host_wiring_requirements() -> &'static [&'static str] {
    HOST_WIRING_REQUIREMENTS
}

#[derive(Debug, Clone)]
pub struct AsrServiceConfig {
    pub open: AsrStreamOpen,
    pub provider_task_id: String,
    pub connection_epoch: u64,
    pub attempt: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFeedResult {
    pub chunk_seq: u64,
    pub missing_before: Option<(u64, u64)>,
    pub audio_span_id: String,
    pub timeline_start_ms: u64,
    pub timeline_end_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AsrEventOutcome {
    TaskStarted,
    Ignored,
    Candidate(AsrRawCandidate),
    Updated(AsrRawCandidate),
    Duplicate,
    Rejected(PublishDecision),
    TaskFinished,
}

/// In-memory streaming adapter state. It is intentionally disposable and
/// restartable; N persists the source spans and task checkpoints.
pub struct AsrService {
    session: StreamingSession,
    gate: ResultGate,
    binding: focaus_contracts::CredentialBinding,
    next_chunk_seq: u64,
    finish_sent: bool,
    cancelled: bool,
}

impl AsrService {
    pub fn new(config: AsrServiceConfig) -> Result<Self, SafeError> {
        let session = StreamingSession::open(
            &config.open,
            config.provider_task_id,
            config.connection_epoch,
            config.attempt,
        )?;
        let gate = ResultGate::new(
            config.open.task_id.clone(),
            config.attempt,
            config.open.snapshot.snapshot_id.clone(),
            config.open.snapshot.assignment_epoch,
            config.open.snapshot.assignment_epoch,
        );
        Ok(Self {
            session,
            gate,
            binding: config.open.binding,
            next_chunk_seq: 0,
            finish_sent: false,
            cancelled: false,
        })
    }

    pub fn task_id(&self) -> &str {
        &self.session.task_id
    }

    pub fn binding(&self) -> &focaus_contracts::CredentialBinding {
        &self.binding
    }

    /// Validate and hand one durable chunk to N's transport. A sequence gap
    /// is reported for the host to record; it does not block real-time audio.
    pub fn feed_chunk(&mut self, chunk: AudioChunk) -> Result<AudioFeedResult, SafeError> {
        if self.cancelled {
            return Err(cancelled("asr_feed_after_cancel"));
        }
        if self.finish_sent {
            return Err(stale("asr_feed_after_finish"));
        }
        crate::asr::validate_audio_chunk(&chunk)?;
        if chunk.chunk_seq < self.next_chunk_seq {
            return Err(task_error(ErrorCode::IdempotencyMismatch, "audio_chunk_sequence_replayed", false));
        }
        let missing_before = (chunk.chunk_seq > self.next_chunk_seq).then_some((self.next_chunk_seq, chunk.chunk_seq));
        self.next_chunk_seq = chunk.chunk_seq.saturating_add(1);
        Ok(AudioFeedResult {
            chunk_seq: chunk.chunk_seq,
            missing_before,
            audio_span_id: chunk.audio_span.audio_span_id,
            timeline_start_ms: chunk.audio_span.timeline_range.start_ms,
            timeline_end_ms: chunk.audio_span.timeline_range.end_ms,
        })
    }

    /// Decode an official-shaped event and return a candidate for N to submit.
    pub fn ingest_provider_event(&mut self, value: &Value) -> Result<AsrEventOutcome, SafeError> {
        if self.cancelled {
            return Err(cancelled("asr_event_after_cancel"));
        }
        match decode_stream_event(value)? {
            DecodedStreamEvent::TaskStarted { provider_task_id } => {
                self.session.provider_task_started(&provider_task_id)?;
                Ok(AsrEventOutcome::TaskStarted)
            }
            DecodedStreamEvent::Heartbeat | DecodedStreamEvent::SentenceStart => Ok(AsrEventOutcome::Ignored),
            DecodedStreamEvent::Sentence(mut sentence) => {
                sentence.connection_epoch = self.session.connection_epoch;
                match self.session.accept_sentence(sentence)? {
                    SentenceDisposition::New(candidate) => self.classify_candidate(candidate, false),
                    SentenceDisposition::Updated(candidate) => self.classify_candidate(candidate, true),
                    SentenceDisposition::Duplicate => Ok(AsrEventOutcome::Duplicate),
                }
            }
            DecodedStreamEvent::TaskFinished { provider_task_id } => {
                self.session.provider_task_finished(&provider_task_id)?;
                Ok(AsrEventOutcome::TaskFinished)
            }
        }
    }

    fn classify_candidate(&mut self, candidate: AsrRawCandidate, updated: bool) -> Result<AsrEventOutcome, SafeError> {
        match self.gate.classify(&candidate) {
            PublishDecision::Accept => Ok(if updated { AsrEventOutcome::Updated(candidate) } else { AsrEventOutcome::Candidate(candidate) }),
            PublishDecision::Duplicate => Ok(AsrEventOutcome::Duplicate),
            decision => Ok(AsrEventOutcome::Rejected(decision)),
        }
    }

    pub fn finish_task_request(&mut self) -> Result<Value, SafeError> {
        if self.cancelled {
            return Err(cancelled("asr_finish_after_cancel"));
        }
        self.session.request_finish()?;
        self.finish_sent = true;
        build_stream_finish_task(&self.session.provider_task_id)
    }

    pub fn provider_finished(&mut self) -> Result<(), SafeError> {
        if !self.finish_sent {
            return Err(invalid_argument("provider_finished_before_finish_task"));
        }
        self.session.finish();
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.session.cancel();
        self.gate.cancel();
    }

    pub fn retry_plan(&self, error: &SafeError, current_attempt: u64, max_attempts: u64) -> Result<RetryPlan, SafeError> {
        plan_retry(error, current_attempt, max_attempts, &self.binding)
    }
}

/// Replay sanitized provider events through the same formal ASR state machine
/// used by the host.  This is a local loopback entry only; it never opens a
/// socket or reads credentials.
pub fn replay_stream_events(
    service: &mut AsrService,
    events: &[Value],
) -> Result<Vec<AsrEventOutcome>, SafeError> {
    events
        .iter()
        .map(|event| service.ingest_provider_event(event))
        .collect()
}

pub async fn submit_asr_outcome<H: crate::HostServices>(
    host: &H,
    outcome: AsrEventOutcome,
    source_refs: Vec<focaus_contracts::SourceRef>,
) -> Result<Option<focaus_contracts::CommandResult>, SafeError> {
    match outcome {
        AsrEventOutcome::Candidate(candidate) | AsrEventOutcome::Updated(candidate) => Ok(Some(submit_candidate(host, &candidate, source_refs).await?)),
        AsrEventOutcome::TaskStarted
        | AsrEventOutcome::TaskFinished
        | AsrEventOutcome::Ignored
        | AsrEventOutcome::Duplicate
        | AsrEventOutcome::Rejected(_) => Ok(None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPriority {
    Capture = 0,
    StreamingAsr = 10,
    Focaus = 20,
    Progress = 30,
    FileTrans = 40,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkKind {
    AudioChunk,
    StreamingEvent,
    Focaus { request_generation: u64 },
    Progress { sequence: u64 },
    FileTrans { task_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledWork {
    pub id: u64,
    pub priority: WorkPriority,
    pub sequence: u64,
    pub kind: WorkKind,
}

#[derive(Debug, Default, Clone)]
pub struct PriorityScheduler {
    next_id: u64,
    next_sequence: u64,
    queue: Vec<ScheduledWork>,
}

impl PriorityScheduler {
    pub fn enqueue(&mut self, priority: WorkPriority, kind: WorkKind) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.queue.push(ScheduledWork { id: self.next_id, priority, sequence: self.next_sequence, kind });
        self.next_id
    }

    pub fn pop_next(&mut self) -> Option<ScheduledWork> {
        let index = self.queue.iter().enumerate().min_by(|(_, left), (_, right)| {
            (left.priority as u8, left.sequence).cmp(&(right.priority as u8, right.sequence))
        }).map(|(index, _)| index)?;
        Some(self.queue.remove(index))
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressCadence {
    pub interval_ms: u64,
    pub next_due_ms: u64,
    pub last_evidence_cutoff_ms: Option<u64>,
}

impl Default for ProgressCadence {
    fn default() -> Self {
        Self { interval_ms: DEFAULT_PROGRESS_INTERVAL_MS, next_due_ms: 0, last_evidence_cutoff_ms: None }
    }
}

impl ProgressCadence {
    pub fn due(&self, now_ms: u64, evidence_cutoff_ms: Option<u64>) -> bool {
        now_ms >= self.next_due_ms
            && evidence_cutoff_ms.is_some()
            && evidence_cutoff_ms != self.last_evidence_cutoff_ms
    }

    pub fn mark_scheduled(&mut self, now_ms: u64, evidence_cutoff_ms: Option<u64>) {
        self.next_due_ms = now_ms.saturating_add(self.interval_ms);
        self.last_evidence_cutoff_ms = evidence_cutoff_ms;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocausController {
    active_generation: Option<u64>,
    triggered_at_ms: u64,
    cancelled: bool,
}

impl Default for FocausController {
    fn default() -> Self {
        Self { active_generation: None, triggered_at_ms: 0, cancelled: false }
    }
}

impl FocausController {
    pub fn start(&mut self, request_generation: u64, triggered_at_ms: u64) -> Result<(), SafeError> {
        if self.active_generation.is_some() {
            return Err(task_error(ErrorCode::Conflict, "focaus_request_already_active", false));
        }
        self.active_generation = Some(request_generation);
        self.triggered_at_ms = triggered_at_ms;
        self.cancelled = false;
        Ok(())
    }

    pub fn cancel(&mut self, request_generation: u64) -> Result<(), SafeError> {
        if self.active_generation != Some(request_generation) {
            return Err(stale("focaus_cancel_generation_mismatch"));
        }
        self.cancelled = true;
        Ok(())
    }

    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        FOCAUS_DEADLINE_MS.saturating_sub(now_ms.saturating_sub(self.triggered_at_ms))
    }

    pub fn accept(&mut self, result: &CatchupResult, now_ms: u64) -> Result<(), SafeError> {
        if self.active_generation != Some(result.request_generation) {
            return Err(stale("focaus_result_generation_stale"));
        }
        if self.cancelled {
            return Err(cancelled("focaus_result_after_cancel"));
        }
        let elapsed = now_ms.saturating_sub(self.triggered_at_ms);
        validate_publication(result, elapsed)?;
        self.active_generation = None;
        Ok(())
    }
}

pub fn replay_progress_response(
    snapshot: &InputSnapshot,
    evidence: &ProgressEvidence,
    response: &Value,
    current: Option<&ProgressState>,
    recording_state: ProgressStateRecordingState,
) -> Result<ProgressState, SafeError> {
    let choice = parse_system_one_choice(response, "current_position")?;
    let (position_status, page_id, region_id) = if choice == "__unlocated__" {
        (ProgressStatePositionStatus::Unlocated, None, None)
    } else {
        let page = evidence.pages.iter().find(|page| page.page_id == choice).ok_or_else(|| invalid("progress_replay_choice_outside_scope"))?;
        (ProgressStatePositionStatus::Located, Some(page.page_id.clone()), page.region_id.clone())
    };
    let decision = ProgressDecision {
        snapshot_id: snapshot.snapshot_id.clone(),
        lesson_id: evidence.lesson_id.clone(),
        recording_id: evidence.recording_id.clone(),
        assignment_epoch: evidence.assignment_epoch,
        decision_seq: snapshot.decision_seq.unwrap_or(0),
        calibration_seq: snapshot.calibration_seq.unwrap_or(0),
        evidence_cutoff_ms: evidence.evidence_cutoff_ms,
        position_status,
        page_id,
        region_id,
        confidence: None,
    };
    validate_decision(&decision, evidence, current)?;
    to_progress_state(&decision, evidence, snapshot.snapshot_id.clone(), recording_state)
}

pub fn replay_focaus_result(
    controller: &mut FocausController,
    input: &CatchupGenerateInput,
    draft: FocausDraft,
    request_generation: u64,
    triggered_at_ms: u64,
    generated_at: &str,
    now_ms: u64,
) -> Result<(CatchupResult, FocausPublication), SafeError> {
    controller.start(request_generation, triggered_at_ms)?;
    let budget = DeadlineBudget { triggered_at_mono_ms: triggered_at_ms, now_mono_ms: now_ms };
    let (result, publication) = build_result_with_context(
        input.snapshot.target_id.clone(),
        input.snapshot.target_base_revision.clone().unwrap_or_else(|| input.snapshot.snapshot_id.clone()),
        request_generation,
        input,
        draft,
        input.snapshot.available_at.clone(),
        generated_at.to_string(),
        budget,
    )?;
    controller.accept(&result, now_ms)?;
    Ok((result, publication))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::build_stream_run_task;
    use crate::fixtures;
    use focaus_contracts::{AudioSpanAsrCoverage, AudioSpanCoverageStatus, CatchupCoverage, CatchupCoverageRangeStatus, CatchupResult, CatchupResultCompletionStatus, CatchupWindow, CurrentFocus, CurrentFocusActivity, CurrentFocusStatus, FrameRange, MappingSegment, ModelId, PathEdge, PathNode, ProgressBinding, TimeMapping, TimelineRange, TaskKind};

    fn mapping() -> TimeMapping {
        TimeMapping { mapping_revision_id: "00000000-0000-4000-8000-000000000001".into(), derived_asset_id: "00000000-0000-4000-8000-000000000002".into(), segments: vec![MappingSegment { derived_frames: FrameRange { start_frame: 0, end_frame: 1600 }, original_asset_id: "00000000-0000-4000-8000-000000000002".into(), original_track_id: "track".into(), original_frames: FrameRange { start_frame: 0, end_frame: 1600 }, timeline: TimelineRange { start_ms: 0, end_ms: 100 }, scale_num: 1, scale_den: 1, delay_frames: 0, max_error_ms: 0.0 }] }
    }

    fn open() -> AsrStreamOpen {
        AsrStreamOpen { task_id: "00000000-0000-4000-8000-000000000003".into(), snapshot: InputSnapshot { snapshot_id: "00000000-0000-4000-8000-000000000004".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose: TaskKind::AsrStream, target_id: "00000000-0000-4000-8000-000000000005".into(), lesson_id: Some("00000000-0000-4000-8000-000000000006".into()), recording_id: Some("00000000-0000-4000-8000-000000000007".into()), assignment_epoch: Some(1), sources: vec![], attachment_set_revision: "00000000-0000-4000-8000-000000000008".into(), fact_set_revision: None, target_base_revision: None, edit_revision: None, language_tier: None, evidence_cutoff: vec![], available_at: "2026-01-01T00:00:00Z".into(), executor_id: "ai".into(), executor_version: "1".into(), model_id: Some(ModelId::QwenAudio31AsrFlashStreaming), region: Some("cn-beijing".into()), prompt_version: Some(crate::PROMPT_ASR_STREAM_V1.into()), parameters_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(), calibration_seq: None, decision_seq: None }, binding: crate::binding_for_model(ModelId::QwenAudio31AsrFlashStreaming), format: "pcm".into(), sample_rate: 16000, channels: 1, hotwords: vec!["Fourier".into()], context: vec!["lecture".into()], mapping: mapping() }
    }

    #[test]
    fn asr_service_connects_durable_chunk_to_replayed_candidate_without_db() {
        let mut service = AsrService::new(AsrServiceConfig { open: open(), provider_task_id: "fixture-stream-task".into(), connection_epoch: 1, attempt: 1 }).unwrap();
        let feed = service.feed_chunk(AudioChunk { chunk_seq: 0, audio_span: focaus_contracts::AudioSpan { audio_span_id: "00000000-0000-4000-8000-000000000009".into(), recording_id: "00000000-0000-4000-8000-000000000007".into(), asset_id: Some("00000000-0000-4000-8000-000000000002".into()), track_id: Some("track".into()), frame_range: Some(FrameRange { start_frame: 0, end_frame: 1600 }), timeline_range: TimelineRange { start_ms: 0, end_ms: 100 }, mapping_revision_id: Some("00000000-0000-4000-8000-000000000001".into()), coverage_status: AudioSpanCoverageStatus::Captured, asr_coverage: AudioSpanAsrCoverage::Pending }, encoded_bytes: vec![1, 2], mapping: mapping() }).unwrap();
        assert_eq!(feed.audio_span_id, "00000000-0000-4000-8000-000000000009");
        let events = vec![
            fixtures::streaming_task_started(),
            fixtures::streaming_sentence_start(),
            fixtures::streaming_sentence_final(),
        ];
        let outcomes = replay_stream_events(&mut service, &events).unwrap();
        assert!(matches!(&outcomes[0], AsrEventOutcome::TaskStarted));
        assert!(matches!(&outcomes[1], AsrEventOutcome::Ignored));
        assert!(matches!(&outcomes[2], AsrEventOutcome::Candidate(_)));
        service.finish_task_request().unwrap();
        assert!(matches!(&replay_stream_events(&mut service, &[fixtures::streaming_task_finished()]).unwrap()[0], AsrEventOutcome::TaskFinished));
        assert_eq!(build_stream_run_task(&open(), "fixture-stream-task").unwrap()["payload"]["model"], crate::models::STREAMING_ASR);
    }

    #[test]
    fn cadence_priority_and_focaus_cancel_are_local_replay_behaviors() {
        let mut cadence = ProgressCadence::default();
        assert!(cadence.due(0, Some(100)));
        cadence.mark_scheduled(0, Some(100));
        assert!(!cadence.due(9_999, Some(200)));
        assert!(cadence.due(10_000, Some(200)));
        let mut scheduler = PriorityScheduler::default();
        scheduler.enqueue(WorkPriority::Progress, WorkKind::Progress { sequence: 1 });
        scheduler.enqueue(WorkPriority::Focaus, WorkKind::Focaus { request_generation: 1 });
        scheduler.enqueue(WorkPriority::StreamingAsr, WorkKind::StreamingEvent);
        assert_eq!(scheduler.pop_next().unwrap().priority, WorkPriority::StreamingAsr);
        assert_eq!(scheduler.pop_next().unwrap().priority, WorkPriority::Focaus);
        let mut controller = FocausController::default();
        controller.start(1, 100).unwrap();
        controller.cancel(1).unwrap();
        assert!(controller.remaining_ms(200) < FOCAUS_DEADLINE_MS);
    }

    fn focaus_result(generation: u64, status: CatchupResultCompletionStatus) -> CatchupResult {
        CatchupResult {
            result_semantics: crate::catchup::FOCAUS_RESULT_SEMANTICS.into(),
            result_id: "result".into(),
            request_generation: generation,
            snapshot_id: "snapshot".into(),
            body_revision_id: "body".into(),
            requested_window: CatchupWindow { recording_id: None, start_ms: None, end_ms: None, start_source: None },
            effective_window: CatchupWindow { recording_id: None, start_ms: None, end_ms: None, start_source: None },
            estimate_basis: "fixture".into(),
            current_focus: CurrentFocus { topic: "topic".into(), problem: "problem".into(), activity: CurrentFocusActivity::Unknown, step: None, source_refs: vec![], status: CurrentFocusStatus::Unknown },
            path_nodes: vec![PathNode { node_id: "node".into(), role: focaus_contracts::PathNodeRole::Current, fact_ids: vec![], definition_ids: vec![], binding_ids: vec![], body_block_ids: vec![], evidence_role: focaus_contracts::PathNodeEvidenceRole::Unknown, source_refs: vec![] }],
            path_edges: vec![PathEdge { from_node_id: "node".into(), to_node_id: "node".into(), relation: focaus_contracts::PathEdgeRelation::Temporal, source_refs: vec![], certainty: focaus_contracts::PathEdgeCertainty::Unknown }],
            prerequisites: vec![], prerequisites_reason: Some(focaus_contracts::CatchupResultPrerequisitesReasonValue::NoneNeeded), body: vec![], presentation_tier: 3, body_evidence_cutoff: vec![], body_generated_at: "2026-01-01T00:00:00Z".into(), progress_at_trigger: None, progress_unavailable_reason: Some("fixture".into()), progress_binding: ProgressBinding { lesson_id: "lesson".into(), recording_id: None, assignment_epoch: None, no_pointer_reason: Some("fixture".into()) }, coverage: CatchupCoverage { audio_gaps: vec![], asr_gaps: vec![], material_supplement_used: false, range_status: CatchupCoverageRangeStatus::Unknown }, limitations: vec![], completion_status: status, triggered_at: "2026-01-01T00:00:00Z".into(), completed_at: None, displayed_complete_elapsed_ms: None,
        }
    }

    #[test]
    fn focaus_controller_rejects_old_generation_and_late_complete_body() {
        let mut controller = FocausController::default();
        controller.start(2, 0).unwrap();
        assert_eq!(controller.accept(&focaus_result(1, CatchupResultCompletionStatus::Complete), 100).unwrap_err().code, ErrorCode::StaleInput);
        assert_eq!(controller.accept(&focaus_result(2, CatchupResultCompletionStatus::Complete), FOCAUS_DEADLINE_MS + 1).unwrap_err().code, ErrorCode::DeadlineExceeded);
        controller.cancel(2).unwrap();
        assert_eq!(controller.accept(&focaus_result(2, CatchupResultCompletionStatus::Partial), 100).unwrap_err().code, ErrorCode::Cancelled);
    }
}

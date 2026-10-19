//! Dual ASR protocol adapters and local merge gates.
//!
//! The adapters stop at a transport boundary.  A host may use the request
//! builders with its authenticated HTTP/WebSocket implementation, but this
//! module never opens a socket, reads a path, uploads an asset, or stores a
//! provider task.  Provider IDs are only bindings to a host-owned local task.

use crate::{binding_matches, cancelled, compact_text, invalid, invalid_argument, stale, task_error, models};
use focaus_contracts::{
    AsrRawCandidate, AsrRawCandidateState, AsrStreamOpen, AudioChunk, AudioSpan, CredentialBinding,
    ErrorCode, HostServices, InputSnapshot, ModelId, RemoteBinding, ResultSubmitInput, SafeError,
    SourceRef, Task, TaskCheckpointInput, TaskKind, TimeMapping, TimelineRange,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const STREAMING_MODEL_ID: &str = models::STREAMING_ASR;
pub const FILETRANS_MODEL_ID: &str = models::FILETRANS_ASR;
pub const ASR_RESULT_SEMANTICS: &str = "asr_raw_candidate_v1";

/// Provider-neutral event decoded by the host transport.  The wire decoder is
/// intentionally strict about the fields needed for local identity and time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSentence {
    pub provider_task_id: String,
    pub connection_epoch: u64,
    pub sentence_id: String,
    pub finality: AsrRawCandidateState,
    pub text: String,
    pub begin_time_ms: u64,
    pub end_time_ms: u64,
    pub speaker_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SentenceDisposition {
    New(AsrRawCandidate),
    Updated(AsrRawCandidate),
    Duplicate,
}

#[derive(Debug, Clone, PartialEq)]
struct SentenceRecord {
    candidate: AsrRawCandidate,
    finality: AsrRawCandidateState,
}

/// Validates the exact model/credential pair before any network request.
pub fn validate_stream_open(open: &AsrStreamOpen) -> Result<(), SafeError> {
    if open.snapshot.contract_version != focaus_contracts::CONTRACT_VERSION {
        return Err(task_error(
            ErrorCode::UnsupportedVersion,
            "stream_contract_version_unsupported",
            false,
        ));
    }
    if open.snapshot.purpose != TaskKind::AsrStream
        || open.snapshot.model_id != Some(ModelId::QwenAudio31AsrFlashStreaming)
        || open.snapshot.region.as_deref() != Some(models::REGION)
        || open.snapshot.prompt_version.as_deref() != Some(crate::PROMPT_ASR_STREAM_V1)
    {
        return Err(invalid_argument("stream_snapshot_model_or_region_mismatch"));
    }
    binding_matches(&open.binding, ModelId::QwenAudio31AsrFlashStreaming)?;
    if open.task_id.is_empty()
        || open.snapshot.recording_id.is_none()
        || open.snapshot.assignment_epoch.is_none()
        || open.sample_rate == 0
        || open.channels != 1
    {
        return Err(invalid_argument("invalid_stream_open"));
    }
    if open.mapping.segments.is_empty() {
        return Err(invalid_argument("stream_mapping_required"));
    }
    if open.hotwords.len() > 2_000 {
        return Err(invalid_argument("hotword_limit_exceeded"));
    }
    if open.context.iter().map(|s| s.chars().count()).sum::<usize>() > 2_000 {
        return Err(invalid_argument("asr_context_limit_exceeded"));
    }
    validate_mapping(&open.mapping)
}

pub fn validate_audio_chunk(chunk: &AudioChunk) -> Result<(), SafeError> {
    if chunk.encoded_bytes.is_empty() || chunk.audio_span.timeline_range.end_ms <= chunk.audio_span.timeline_range.start_ms {
        return Err(invalid_argument("empty_or_invalid_audio_chunk"));
    }
    if chunk.mapping.segments.is_empty() {
        return Err(invalid_argument("chunk_mapping_required"));
    }
    if chunk.audio_span.mapping_revision_id != Some(chunk.mapping.mapping_revision_id.clone()) {
        return Err(stale("audio_mapping_revision_mismatch"));
    }
    validate_mapping(&chunk.mapping)
}

fn validate_mapping(mapping: &TimeMapping) -> Result<(), SafeError> {
    if mapping.mapping_revision_id.is_empty() || mapping.segments.is_empty() {
        return Err(invalid_argument("invalid_time_mapping"));
    }
    let mut previous_end = None;
    for segment in &mapping.segments {
        if segment.timeline.start_ms >= segment.timeline.end_ms
            || segment.derived_frames.start_frame >= segment.derived_frames.end_frame
            || segment.original_frames.start_frame >= segment.original_frames.end_frame
            || segment.scale_num == 0
            || segment.scale_den == 0
        {
            return Err(invalid_argument("invalid_time_mapping_segment"));
        }
        if previous_end.is_some_and(|end| segment.timeline.start_ms < end) {
            return Err(invalid_argument("overlapping_time_mapping_segments"));
        }
        previous_end = Some(segment.timeline.end_ms);
    }
    Ok(())
}

/// Map provider-relative milliseconds into the recording coordinate.  The
/// provider sees the submitted spans consecutively; gaps are represented by
/// separate mappings and are never silently filled.
pub fn map_relative_ms(mapping: &TimeMapping, relative_ms: u64) -> Result<u64, SafeError> {
    validate_mapping(mapping)?;
    if mapping.segments.iter().any(|segment| segment.scale_num != segment.scale_den) {
        return Err(invalid_argument("sample_rate_required_for_non_identity_time_mapping"));
    }
    let mut remaining = relative_ms;
    for segment in &mapping.segments {
        let duration = segment.timeline.end_ms - segment.timeline.start_ms;
        if remaining < duration {
            return Ok(segment.timeline.start_ms + remaining);
        }
        remaining = remaining.saturating_sub(duration);
    }
    Err(invalid("provider_time_outside_submitted_mapping"))
}

/// Map an end-exclusive interval.  A start exactly at a span boundary is
/// assigned to the next span; an end exactly at the final boundary is valid
/// and maps to the preceding span's end.  Gaps in the recording timeline are
/// preserved because provider-relative time advances only through submitted
/// derived spans.
pub fn map_relative_interval(
    mapping: &TimeMapping,
    start_ms: u64,
    end_ms: u64,
) -> Result<TimelineRange, SafeError> {
    if start_ms >= end_ms {
        return Err(invalid_argument("invalid_provider_interval"));
    }
    let start = map_relative_ms(mapping, start_ms)?;
    let end = map_relative_end_ms(mapping, end_ms)?;
    if start >= end {
        return Err(invalid("mapped_provider_interval_not_positive"));
    }
    Ok(TimelineRange { start_ms: start, end_ms: end })
}

fn map_relative_end_ms(mapping: &TimeMapping, relative_ms: u64) -> Result<u64, SafeError> {
    validate_mapping(mapping)?;
    if mapping.segments.iter().any(|segment| segment.scale_num != segment.scale_den) {
        return Err(invalid_argument("sample_rate_required_for_non_identity_time_mapping"));
    }
    let mut remaining = relative_ms;
    for segment in &mapping.segments {
        let duration = segment.timeline.end_ms - segment.timeline.start_ms;
        if remaining <= duration {
            // `<=` deliberately gives an exact end boundary to this span.
            return Ok(segment.timeline.start_ms + remaining);
        }
        remaining = remaining.saturating_sub(duration);
    }
    if remaining == 0 {
        return mapping.segments.last().map(|segment| segment.timeline.end_ms).ok_or_else(|| invalid("empty_time_mapping"));
    }
    Err(invalid("provider_end_time_outside_submitted_mapping"))
}

/// Frame-aware mapping for transports that provide the derived sample rate.
/// `scale_num/scale_den` is applied from derived-frame offset to original-frame
/// offset before interpolating within the declared timeline segment.  This
/// keeps resampling and non-unity time mappings visible instead of treating
/// every segment as a wall-clock rectangle.
pub fn map_relative_interval_with_sample_rate(
    mapping: &TimeMapping,
    start_ms: u64,
    end_ms: u64,
    sample_rate: u64,
) -> Result<TimelineRange, SafeError> {
    if sample_rate == 0 || start_ms >= end_ms {
        return Err(invalid_argument("invalid_sample_rate_or_provider_interval"));
    }
    validate_mapping(mapping)?;
    let start = map_frame_boundary(mapping, start_ms, sample_rate, false)?;
    let end = map_frame_boundary(mapping, end_ms, sample_rate, true)?;
    if start >= end {
        return Err(invalid("mapped_provider_interval_not_positive"));
    }
    Ok(TimelineRange { start_ms: start, end_ms: end })
}

fn map_frame_boundary(mapping: &TimeMapping, relative_ms: u64, sample_rate: u64, end_boundary: bool) -> Result<u64, SafeError> {
    let relative_frames = if end_boundary {
        relative_ms.saturating_mul(sample_rate).saturating_add(999) / 1000
    } else {
        relative_ms.saturating_mul(sample_rate) / 1000
    };
    let mut remaining = relative_frames;
    for segment in &mapping.segments {
        let derived_len = segment.derived_frames.end_frame - segment.derived_frames.start_frame;
        if remaining < derived_len || (end_boundary && remaining == derived_len) {
            let derived_offset = remaining.min(derived_len);
            let scaled_original_offset = derived_offset
                .saturating_mul(segment.scale_num)
                / segment.scale_den;
            let original_len = segment.original_frames.end_frame - segment.original_frames.start_frame;
            if scaled_original_offset > original_len {
                return Err(invalid("time_mapping_scale_exceeds_original_frames"));
            }
            let timeline_len = segment.timeline.end_ms - segment.timeline.start_ms;
            let timeline_offset = if original_len == 0 { 0 } else { scaled_original_offset.saturating_mul(timeline_len) / original_len };
            return Ok(segment.timeline.start_ms + timeline_offset);
        }
        remaining = remaining.saturating_sub(derived_len);
    }
    if end_boundary && remaining == 0 {
        return mapping.segments.last().map(|segment| segment.timeline.end_ms).ok_or_else(|| invalid("empty_time_mapping"));
    }
    Err(invalid("provider_frame_time_outside_submitted_mapping"))
}

/// State for one streaming connection.  The connection epoch prevents a
/// reconnect from replacing a newer sentence with an old callback.
#[derive(Debug, Clone)]
pub struct StreamingSession {
    pub task_id: String,
    pub attempt: u64,
    pub snapshot_id: String,
    pub provider_task_id: String,
    pub connection_epoch: u64,
    pub mapping_revision_id: String,
    cancelled: bool,
    provider_started: bool,
    finish_requested: bool,
    provider_finished: bool,
    terminal: bool,
    sentences: BTreeMap<String, SentenceRecord>,
}

impl StreamingSession {
    pub fn open(open: &AsrStreamOpen, provider_task_id: impl Into<String>, connection_epoch: u64, attempt: u64) -> Result<Self, SafeError> {
        validate_stream_open(open)?;
        let provider_task_id = provider_task_id.into();
        if provider_task_id.is_empty() {
            return Err(invalid_argument("provider_task_id_required"));
        }
        crate::transport::validate_remote_task_id(&provider_task_id)?;
        Ok(Self {
            task_id: open.task_id.clone(),
            attempt,
            snapshot_id: open.snapshot.snapshot_id.clone(),
            provider_task_id,
            connection_epoch,
            mapping_revision_id: open.mapping.mapping_revision_id.clone(),
            cancelled: false,
            provider_started: false,
            finish_requested: false,
            provider_finished: false,
            terminal: false,
            sentences: BTreeMap::new(),
        })
    }

    pub fn provider_task_started(&mut self, provider_task_id: &str) -> Result<(), SafeError> {
        if provider_task_id != self.provider_task_id {
            return Err(stale("stream_connection_binding_mismatch"));
        }
        if self.cancelled || self.provider_finished || self.terminal {
            return Err(stale("stream_task_started_after_terminal"));
        }
        if self.provider_started {
            return Err(task_error(ErrorCode::Conflict, "duplicate_stream_task_started", false));
        }
        self.provider_started = true;
        Ok(())
    }

    pub fn request_finish(&mut self) -> Result<(), SafeError> {
        if self.cancelled {
            return Err(cancelled("stream_finish_after_cancel"));
        }
        if !self.provider_started {
            return Err(invalid_argument("stream_finish_before_task_started"));
        }
        if self.provider_finished || self.terminal {
            return Err(stale("stream_finish_after_provider_finished"));
        }
        if self.finish_requested {
            return Err(task_error(ErrorCode::IdempotencyMismatch, "duplicate_stream_finish", false));
        }
        self.finish_requested = true;
        Ok(())
    }

    pub fn provider_task_finished(&mut self, provider_task_id: &str) -> Result<(), SafeError> {
        if provider_task_id != self.provider_task_id {
            return Err(stale("stream_connection_binding_mismatch"));
        }
        if self.cancelled {
            return Err(cancelled("stream_finished_after_cancel"));
        }
        if !self.provider_started || !self.finish_requested {
            return Err(invalid("provider_task_finished_before_finish_request"));
        }
        if self.provider_finished {
            return Err(task_error(ErrorCode::Conflict, "duplicate_stream_task_finished", false));
        }
        self.provider_finished = true;
        self.terminal = true;
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn finish(&mut self) {
        if !self.cancelled && self.finish_requested {
            self.provider_finished = true;
            self.terminal = true;
        }
    }

    pub fn accept_sentence(&mut self, sentence: RemoteSentence) -> Result<SentenceDisposition, SafeError> {
        if self.cancelled {
            return Err(cancelled("stream_result_after_cancel"));
        }
        if self.terminal {
            return Err(stale("stream_result_after_finish"));
        }
        if !self.provider_started {
            return Err(invalid("stream_result_before_task_started"));
        }
        if sentence.provider_task_id != self.provider_task_id
            || sentence.connection_epoch != self.connection_epoch
        {
            return Err(stale("stream_connection_binding_mismatch"));
        }
        if sentence.sentence_id.is_empty()
            || sentence.end_time_ms <= sentence.begin_time_ms
            || sentence.text.trim().is_empty()
        {
            return Err(invalid("invalid_provider_sentence"));
        }
        let remote_binding = RemoteBinding {
            provider: "bailian".to_string(),
            provider_task_id: sentence.provider_task_id,
            connection_epoch: sentence.connection_epoch,
            provider_sentence_id: Some(sentence.sentence_id.clone()),
            local_request_segment_id: self.task_id.clone(),
        };
        let candidate = AsrRawCandidate {
            task_id: self.task_id.clone(),
            attempt: self.attempt,
            snapshot_id: self.snapshot_id.clone(),
            remote_binding,
            state: sentence.finality.clone(),
            text: sentence.text,
            relative_start_ms: sentence.begin_time_ms,
            relative_end_ms: sentence.end_time_ms,
            speaker_ref: sentence.speaker_ref,
            mapping_revision_id: self.mapping_revision_id.clone(),
        };
        match self.sentences.get(&sentence.sentence_id) {
            None => {
                self.sentences.insert(
                    sentence.sentence_id,
                    SentenceRecord { finality: candidate.state.clone(), candidate: candidate.clone() },
                );
                Ok(SentenceDisposition::New(candidate))
            }
            Some(previous) => {
                if previous.finality == AsrRawCandidateState::Final
                    && candidate.state == AsrRawCandidateState::Provisional
                {
                    return Ok(SentenceDisposition::Duplicate);
                }
                if previous.candidate == candidate {
                    return Ok(SentenceDisposition::Duplicate);
                }
                if previous.finality == AsrRawCandidateState::Final
                    && candidate.state == AsrRawCandidateState::Final
                {
                    // A provider may repeat a final after reconnect.  A
                    // changed final is a conflict, never a silent overwrite.
                    return Err(task_error(ErrorCode::Conflict, "conflicting_final_sentence", false));
                }
                self.sentences.insert(
                    sentence.sentence_id,
                    SentenceRecord { finality: candidate.state.clone(), candidate: candidate.clone() },
                );
                Ok(SentenceDisposition::Updated(candidate))
            }
        }
    }

    pub fn candidates(&self) -> impl Iterator<Item = &AsrRawCandidate> {
        self.sentences.values().map(|record| &record.candidate)
    }

    pub fn mapping_revision_id(&self) -> &str {
        &self.mapping_revision_id
    }
}

/// Filetrans is intentionally a separate binding and task namespace.  It can
/// fill historical holes but cannot be used to overwrite stable streaming
/// sentences.
#[derive(Debug, Clone, PartialEq)]
pub struct FileTransRequest {
    pub local_task_id: String,
    pub attempt: u64,
    pub snapshot: InputSnapshot,
    pub binding: CredentialBinding,
    pub local_request_segment_id: String,
    pub audio_span: AudioSpan,
    pub file_url: String,
}

pub fn validate_filetrans_request(request: &FileTransRequest) -> Result<(), SafeError> {
    if request.snapshot.contract_version != focaus_contracts::CONTRACT_VERSION
        || request.snapshot.purpose != TaskKind::AsrFile
        || request.snapshot.model_id != Some(ModelId::QwenAudio31AsrFlashFiletrans)
        || request.snapshot.region.as_deref() != Some(models::REGION)
        || request.snapshot.prompt_version.as_deref() != Some(crate::PROMPT_ASR_FILE_V1)
    {
        return Err(invalid_argument("filetrans_snapshot_model_or_region_mismatch"));
    }
    binding_matches(&request.binding, ModelId::QwenAudio31AsrFlashFiletrans)?;
    if request.attempt == 0
        || request.snapshot.recording_id.is_none()
        || request.snapshot.assignment_epoch.is_none()
        || request.local_task_id.is_empty()
        || request.local_request_segment_id.is_empty()
    {
        return Err(invalid_argument("filetrans_local_task_identity_required"));
    }
    if !(request.file_url.starts_with("https://") || request.file_url.starts_with("http://") || request.file_url.starts_with("oss://")) {
        return Err(invalid_argument("filetrans_requires_host_authorized_url"));
    }
    if request.audio_span.timeline_range.start_ms >= request.audio_span.timeline_range.end_ms {
        return Err(invalid_argument("filetrans_audio_span_invalid"));
    }
    Ok(())
}

/// Build the non-secret HTTP request body.  The host owns uploading and
/// replacing the URL with its authorized temporary object URL.
pub fn build_filetrans_submit(request: &FileTransRequest) -> Result<Value, SafeError> {
    validate_filetrans_request(request)?;
    Ok(json!({
        "model": FILETRANS_MODEL_ID,
        "input": { "file_urls": [request.file_url] },
        "parameters": {}
    }))
}

pub fn filetrans_required_headers() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-DashScope-Async".to_string(), "enable".to_string()),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTransStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Unknown(String),
}

pub fn parse_filetrans_status(value: &Value) -> Result<(String, FileTransStatus), SafeError> {
    let output = value.get("output").ok_or_else(|| invalid("filetrans_output_missing"))?;
    let task_id = output.get("task_id").and_then(Value::as_str).filter(|id| !id.is_empty()).ok_or_else(|| invalid("filetrans_task_id_missing"))?.to_string();
    let status = output.get("task_status").and_then(Value::as_str).ok_or_else(|| invalid("filetrans_task_status_missing"))?;
    let status = match status {
        "PENDING" => FileTransStatus::Pending,
        "RUNNING" => FileTransStatus::Running,
        "SUCCEEDED" => FileTransStatus::Succeeded,
        "FAILED" => FileTransStatus::Failed,
        "CANCELLED" | "CANCELED" => FileTransStatus::Cancelled,
        other => FileTransStatus::Unknown(other.to_string()),
    };
    Ok((task_id, status))
}

pub fn filetrans_cancel_allowed(status: &FileTransStatus) -> bool {
    matches!(status, FileTransStatus::Pending)
}

pub fn build_stream_run_task(open: &AsrStreamOpen, provider_task_id: &str) -> Result<Value, SafeError> {
    validate_stream_open(open)?;
    crate::transport::validate_remote_task_id(provider_task_id)?;
    let context = open.context.iter().map(|text| json!({
        "role": "user",
        "content": [{ "type": "input_text", "text": text }]
    })).collect::<Vec<_>>();
    let vocabulary = open.hotwords.iter().map(|word| (word.clone(), json!(5))).collect::<serde_json::Map<_, _>>();
    let mut input = json!({});
    if !context.is_empty() {
        input["context"] = Value::Array(context);
    }
    let mut parameters = json!({
        "format": open.format,
        "sample_rate": open.sample_rate
    });
    if !vocabulary.is_empty() {
        parameters["vocabulary"] = Value::Object(vocabulary);
    }
    Ok(json!({
        "header": { "action": "run-task", "task_id": provider_task_id, "streaming": "duplex" },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": STREAMING_MODEL_ID,
            "parameters": parameters,
            "input": input
        }
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamingRequestEnvelope {
    pub local_task_id: String,
    pub attempt: u64,
    pub snapshot_id: String,
    pub provider_task_id: String,
    pub binding: CredentialBinding,
    pub provider_message: Value,
}

/// Typed host-facing envelope for the provider request plus the local task
/// identity that must accompany it in TaskManager checkpoints.
pub fn build_stream_request_envelope(
    open: &AsrStreamOpen,
    provider_task_id: &str,
    attempt: u64,
) -> Result<StreamingRequestEnvelope, SafeError> {
    if attempt == 0 {
        return Err(invalid_argument("asr_attempt_must_be_positive"));
    }
    Ok(StreamingRequestEnvelope {
        local_task_id: open.task_id.clone(),
        attempt,
        snapshot_id: open.snapshot.snapshot_id.clone(),
        provider_task_id: provider_task_id.to_owned(),
        binding: open.binding.clone(),
        provider_message: build_stream_run_task(open, provider_task_id)?,
    })
}

pub fn build_stream_finish_task(provider_task_id: &str) -> Result<Value, SafeError> {
    crate::transport::validate_remote_task_id(provider_task_id)?;
    Ok(json!({
        "header": { "action": "finish-task", "task_id": provider_task_id, "streaming": "duplex" },
        "payload": { "input": {} }
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetryPlan {
    Retry {
        next_attempt: u64,
        backoff_ms: u64,
        binding: CredentialBinding,
    },
    KeepWaiting,
    Fail,
}

/// Retry only transient provider failures and carry the same exact binding
/// forward.  There is intentionally no fallback model/key branch here.
pub fn plan_retry(
    error: &SafeError,
    current_attempt: u64,
    max_attempts: u64,
    binding: &CredentialBinding,
) -> Result<RetryPlan, SafeError> {
    binding_matches(binding, binding.model_id.clone())?;
    if current_attempt == 0 || max_attempts == 0 || current_attempt > max_attempts {
        return Err(invalid_argument("asr_retry_attempt_bounds_invalid"));
    }
    if !error.retryable || matches!(error.code, ErrorCode::AuthRequired | ErrorCode::InvalidArgument | ErrorCode::UnsupportedVersion) {
        return Ok(RetryPlan::Fail);
    }
    if matches!(error.code, ErrorCode::NetworkUnavailable | ErrorCode::RateLimited | ErrorCode::ProviderError) {
        if current_attempt >= max_attempts {
            return Ok(RetryPlan::Fail);
        }
        let exponent = current_attempt.saturating_sub(1).min(8);
        let backoff_ms = 500_u64.saturating_mul(1_u64 << exponent).min(30_000);
        return Ok(RetryPlan::Retry { next_attempt: current_attempt + 1, backoff_ms, binding: binding.clone() });
    }
    Ok(RetryPlan::KeepWaiting)
}

pub async fn checkpoint<H: HostServices>(
    host: &H,
    task_id: impl Into<String>,
    attempt: u64,
    cursor: impl Into<String>,
    safe_state: BTreeMap<String, Value>,
) -> Result<Task, SafeError> {
    host.task_checkpoint(TaskCheckpointInput { task_id: task_id.into(), attempt, cursor: cursor.into(), safe_state }).await
}

pub async fn submit_candidate<H: HostServices>(
    host: &H,
    candidate: &AsrRawCandidate,
    source_refs: Vec<SourceRef>,
) -> Result<focaus_contracts::CommandResult, SafeError> {
    host.result_submit(build_asr_result_submission(candidate, source_refs)?).await
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrResultCandidateEnvelope {
    pub result_semantics: String,
    pub submission_key: String,
    pub task_id: String,
    pub attempt: u64,
    pub snapshot_id: String,
    pub source_refs: Vec<SourceRef>,
    pub candidate: AsrRawCandidate,
}

pub fn build_asr_result_submission(
    candidate: &AsrRawCandidate,
    source_refs: Vec<SourceRef>,
) -> Result<ResultSubmitInput, SafeError> {
    if candidate.task_id.is_empty()
        || candidate.snapshot_id.is_empty()
        || candidate.attempt == 0
        || candidate.remote_binding.provider != "bailian"
        || candidate.remote_binding.provider_task_id.is_empty()
        || candidate.remote_binding.local_request_segment_id != candidate.task_id
    {
        return Err(invalid_argument("asr_result_identity_required"));
    }
    if source_refs.is_empty() {
        return Err(invalid_argument("asr_result_source_refs_required"));
    }
    let envelope = AsrResultCandidateEnvelope {
        result_semantics: ASR_RESULT_SEMANTICS.into(),
        submission_key: format!("asr:{}:{}:{}", candidate.task_id, candidate.attempt, candidate.snapshot_id),
        task_id: candidate.task_id.clone(),
        attempt: candidate.attempt,
        snapshot_id: candidate.snapshot_id.clone(),
        source_refs,
        candidate: candidate.clone(),
    };
    let candidate_value = serde_json::to_value(&envelope)
        .map_err(|_| invalid("asr_candidate_serialization_failed"))?;
    let submission = ResultSubmitInput {
        task_id: candidate.task_id.clone(),
        attempt: candidate.attempt,
        snapshot_id: candidate.snapshot_id.clone(),
        candidate: candidate_value,
        source_refs: envelope.source_refs,
        resources: vec![],
    };
    validate_asr_result_submission(&submission)?;
    Ok(submission)
}

pub fn build_asr_result_submission_for_open(
    open: &AsrStreamOpen,
    candidate: &AsrRawCandidate,
    source_refs: Vec<SourceRef>,
) -> Result<ResultSubmitInput, SafeError> {
    validate_stream_open(open)?;
    if candidate.task_id != open.task_id
        || candidate.snapshot_id != open.snapshot.snapshot_id
        || candidate.mapping_revision_id != open.mapping.mapping_revision_id
    {
        return Err(stale("asr_result_open_identity_mismatch"));
    }
    build_asr_result_submission(candidate, source_refs)
}

pub fn decode_asr_result_candidate(value: &Value) -> Result<AsrResultCandidateEnvelope, SafeError> {
    let envelope: AsrResultCandidateEnvelope = serde_json::from_value(value.clone())
        .map_err(|_| invalid("asr_result_candidate_envelope_invalid"))?;
    if envelope.result_semantics != ASR_RESULT_SEMANTICS {
        return Err(invalid("unknown_asr_result_semantics"));
    }
    let expected_submission_key = format!("asr:{}:{}:{}", envelope.task_id, envelope.attempt, envelope.snapshot_id);
    if envelope.submission_key != expected_submission_key
        || envelope.task_id.is_empty()
        || envelope.snapshot_id.is_empty()
        || envelope.attempt == 0
        || envelope.source_refs.is_empty()
        || envelope.candidate.task_id != envelope.task_id
        || envelope.candidate.snapshot_id != envelope.snapshot_id
        || envelope.candidate.attempt != envelope.attempt
    {
        return Err(stale("asr_result_candidate_identity_mismatch"));
    }
    Ok(envelope)
}

pub fn validate_asr_result_submission(
    submission: &ResultSubmitInput,
) -> Result<AsrResultCandidateEnvelope, SafeError> {
    let envelope = decode_asr_result_candidate(&submission.candidate)?;
    if submission.task_id != envelope.task_id
        || submission.attempt != envelope.attempt
        || submission.snapshot_id != envelope.snapshot_id
        || submission.source_refs != envelope.source_refs
    {
        return Err(stale("asr_result_submission_outer_identity_mismatch"));
    }
    Ok(envelope)
}

/// Exact provider-shaped stream events are decoded before entering the local
/// session state machine. The empty `sentence_begin` result and heartbeat are
/// intentionally not candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedStreamEvent {
    TaskStarted { provider_task_id: String },
    Sentence(RemoteSentence),
    Heartbeat,
    SentenceStart,
    TaskFinished { provider_task_id: String },
}

pub fn decode_stream_event(value: &Value) -> Result<DecodedStreamEvent, SafeError> {
    let header = value.get("header").unwrap_or(value);
    let payload = value.get("payload").unwrap_or(value);
    let event = string_field(header, "event");
    if event.as_deref() == Some("task-failed") {
        return Err(task_error(ErrorCode::ProviderError, "provider_stream_task_failed", false));
    }
    let provider_task_id = string_field(header, "task_id")
        .or_else(|| string_field(payload, "task_id"))
        .or_else(|| string_field(value, "task_id"));
    if matches!(event.as_deref(), Some("task-started") | Some("task-finished")) {
        let provider_task_id = provider_task_id
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("provider_task_id_missing"))?;
        return Ok(match event.as_deref() {
            Some("task-started") => DecodedStreamEvent::TaskStarted { provider_task_id },
            Some("task-finished") => DecodedStreamEvent::TaskFinished { provider_task_id },
            _ => unreachable!(),
        });
    }
    let output = payload.get("output").unwrap_or(payload);
    let sentence = output.get("sentence").unwrap_or(output);
    if sentence.get("heartbeat").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(DecodedStreamEvent::Heartbeat);
    }
    let sentence_id = id_field(sentence, "sentence_id").or_else(|| id_field(sentence, "id"));
    let text = string_field(sentence, "text");
    let begin = u64_field(sentence, "begin_time").or_else(|| u64_field(sentence, "begin_time_ms"));
    let end = u64_field(sentence, "end_time").or_else(|| u64_field(sentence, "end_time_ms"));
    let finality = sentence.get("sentence_end").and_then(Value::as_bool).map(|is_end| if is_end { AsrRawCandidateState::Final } else { AsrRawCandidateState::Provisional }).or_else(|| {
        let state = string_field(sentence, "status").or_else(|| string_field(sentence, "state"));
        match state.as_deref() {
            Some("final") | Some("completed") => Some(AsrRawCandidateState::Final),
            Some("provisional") | Some("partial") | Some("running") => Some(AsrRawCandidateState::Provisional),
            _ => None,
        }
    }).ok_or_else(|| invalid("provider_sentence_state_missing"))?;
    if finality == AsrRawCandidateState::Provisional && (end.is_none() || text.as_deref().unwrap_or_default().is_empty()) {
        return Ok(DecodedStreamEvent::SentenceStart);
    }
    let (provider_task_id, sentence_id, text, begin, end) = match (provider_task_id, sentence_id, text, begin, end) {
        (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a, b, c, d, e),
        _ => return Err(invalid("provider_sentence_required_fields_missing")),
    };
    if end <= begin {
        return Err(invalid("provider_sentence_time_invalid"));
    }
    Ok(DecodedStreamEvent::Sentence(RemoteSentence {
        provider_task_id,
        connection_epoch: u64_field(payload, "connection_epoch").or_else(|| u64_field(value, "connection_epoch")).unwrap_or(0),
        sentence_id,
        finality,
        text,
        begin_time_ms: begin,
        end_time_ms: end,
        speaker_ref: string_field(sentence, "speaker_id").or_else(|| string_field(sentence, "speaker_ref")),
    }))
}

pub fn decode_remote_sentence(value: &Value) -> Result<RemoteSentence, SafeError> {
    match decode_stream_event(value)? {
        DecodedStreamEvent::Sentence(sentence) => Ok(sentence),
        DecodedStreamEvent::TaskStarted { .. }
        | DecodedStreamEvent::Heartbeat
        | DecodedStreamEvent::SentenceStart
        | DecodedStreamEvent::TaskFinished { .. } => Err(invalid("provider_event_is_not_complete_sentence")),
    }
}

pub fn decode_remote_sentence_for_epoch(value: &Value, connection_epoch: u64) -> Result<RemoteSentence, SafeError> {
    let mut sentence = decode_remote_sentence(value)?;
    sentence.connection_epoch = connection_epoch;
    Ok(sentence)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn id_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| field.as_str().map(str::to_owned).or_else(|| field.as_u64().map(|id| id.to_string())))
}

/// A provider task map never treats a remote ID as the local task ID.
#[derive(Debug, Default, Clone)]
pub struct RemoteTaskMap {
    local_to_remote: BTreeMap<String, String>,
    remote_to_local: BTreeMap<String, String>,
}

impl RemoteTaskMap {
    pub fn bind(&mut self, local: &str, remote: &str) -> Result<(), SafeError> {
        if local.is_empty() || remote.is_empty() {
            return Err(invalid_argument("task_ids_required"));
        }
        crate::transport::validate_remote_task_id(remote)?;
        if self.local_to_remote.get(local).is_some_and(|current| current != remote)
            || self.remote_to_local.get(remote).is_some_and(|current| current != local)
        {
            return Err(task_error(ErrorCode::IdempotencyMismatch, "remote_task_binding_conflict", false));
        }
        self.local_to_remote.insert(local.to_owned(), remote.to_owned());
        self.remote_to_local.insert(remote.to_owned(), local.to_owned());
        Ok(())
    }

    pub fn local_for_remote(&self, remote: &str) -> Option<&str> {
        self.remote_to_local.get(remote).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedText {
    pub local_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub source: MergeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeSource {
    Streaming,
    FileTrans,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeDecision {
    InsertHole(TimedText),
    DuplicateOf(String),
    BoundaryReview(TimedText),
}

/// Decide how a historical filetrans sentence interacts with stable streaming
/// sentences.  Text equality is only a duplicate proof when its time range is
/// fully covered; overlap alone is kept for review so normal repeated speech
/// is never deleted.
pub fn merge_filetrans_sentence(candidate: TimedText, stable: &[TimedText]) -> Result<MergeDecision, SafeError> {
    if candidate.source != MergeSource::FileTrans || candidate.start_ms >= candidate.end_ms || candidate.text.trim().is_empty() {
        return Err(invalid_argument("invalid_filetrans_merge_candidate"));
    }
    let normalized = compact_text(&candidate.text);
    let mut overlaps = Vec::new();
    for current in stable {
        if current.start_ms < candidate.end_ms && candidate.start_ms < current.end_ms {
            overlaps.push(current);
            if current.start_ms <= candidate.start_ms
                && current.end_ms >= candidate.end_ms
                && compact_text(&current.text) == normalized
            {
                return Ok(MergeDecision::DuplicateOf(current.local_id.clone()));
            }
        }
    }
    if overlaps.is_empty() {
        Ok(MergeDecision::InsertHole(candidate))
    } else {
        Ok(MergeDecision::BoundaryReview(candidate))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishDecision {
    Accept,
    Duplicate,
    DropCancelled,
    StaleTask,
    StaleAttempt,
    StaleSnapshot,
    StaleAssignment,
}

#[derive(Debug, Clone)]
pub struct ResultGate {
    pub task_id: String,
    pub current_attempt: u64,
    pub snapshot_id: String,
    pub assignment_epoch: Option<u64>,
    pub current_assignment_epoch: Option<u64>,
    cancelled: bool,
    seen: BTreeMap<String, AsrRawCandidateState>,
}

impl ResultGate {
    pub fn new(task_id: impl Into<String>, attempt: u64, snapshot_id: impl Into<String>, assignment_epoch: Option<u64>, current_assignment_epoch: Option<u64>) -> Self {
        Self { task_id: task_id.into(), current_attempt: attempt, snapshot_id: snapshot_id.into(), assignment_epoch, current_assignment_epoch, cancelled: false, seen: BTreeMap::new() }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn update_current_assignment_epoch(&mut self, current_assignment_epoch: Option<u64>) {
        self.current_assignment_epoch = current_assignment_epoch;
    }

    pub fn classify(&mut self, candidate: &AsrRawCandidate) -> PublishDecision {
        if self.cancelled {
            return PublishDecision::DropCancelled;
        }
        if candidate.task_id != self.task_id {
            return PublishDecision::StaleTask;
        }
        if candidate.attempt != self.current_attempt {
            return PublishDecision::StaleAttempt;
        }
        if candidate.snapshot_id != self.snapshot_id {
            return PublishDecision::StaleSnapshot;
        }
        if self.assignment_epoch != self.current_assignment_epoch {
            return PublishDecision::StaleAssignment;
        }
        let key = format!("{}:{}:{}", candidate.remote_binding.provider_task_id, candidate.remote_binding.connection_epoch, candidate.remote_binding.provider_sentence_id.as_deref().unwrap_or(""));
        match self.seen.get(&key) {
            Some(AsrRawCandidateState::Provisional) if candidate.state == AsrRawCandidateState::Final => {
                self.seen.insert(key, candidate.state.clone());
                PublishDecision::Accept
            }
            Some(_) => PublishDecision::Duplicate,
            None => {
                self.seen.insert(key, candidate.state.clone());
                PublishDecision::Accept
            }
        }
    }
}

fn _assert_model_constants_are_used() {
    let _ = (STREAMING_MODEL_ID, FILETRANS_MODEL_ID);
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{
        AsrRawCandidateState, AudioSpanAsrCoverage, AudioSpanCoverageStatus, FrameRange,
        InputSnapshot, Selector, SourceRef, TaskKind, TimelineRange, WholeSelector,
    };

    fn mapping() -> TimeMapping {
        TimeMapping {
            mapping_revision_id: "map-1".into(),
            derived_asset_id: "asset".into(),
            segments: vec![focaus_contracts::MappingSegment {
                derived_frames: FrameRange { start_frame: 0, end_frame: 100 },
                original_asset_id: "asset".into(),
                original_track_id: "track".into(),
                original_frames: FrameRange { start_frame: 0, end_frame: 100 },
                timeline: TimelineRange { start_ms: 10_000, end_ms: 20_000 },
                scale_num: 1,
                scale_den: 1,
                delay_frames: 0,
                max_error_ms: 0.1,
            }],
        }
    }

    fn snapshot(model: ModelId, purpose: TaskKind) -> InputSnapshot {
        InputSnapshot {
            snapshot_id: "snap".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose,
            target_id: "target".into(), lesson_id: None, recording_id: Some("rec".into()), assignment_epoch: Some(1),
            sources: vec![], attachment_set_revision: "attach".into(), fact_set_revision: None,
            target_base_revision: None, edit_revision: None, language_tier: None, evidence_cutoff: vec![],
            available_at: "2026-01-01T00:00:00Z".into(), executor_id: "test".into(), executor_version: "1".into(),
            model_id: Some(model), region: Some("cn-beijing".into()), prompt_version: Some(crate::PROMPT_ASR_STREAM_V1.into()), parameters_hash: "hash".into(), calibration_seq: None, decision_seq: None,
        }
    }

    fn open() -> AsrStreamOpen {
        AsrStreamOpen { task_id: "local".into(), snapshot: snapshot(ModelId::QwenAudio31AsrFlashStreaming, TaskKind::AsrStream), binding: crate::binding_for_model(ModelId::QwenAudio31AsrFlashStreaming), format: "pcm".into(), sample_rate: 16_000, channels: 1, hotwords: vec!["Fourier".into()], context: vec!["lecture".into()], mapping: mapping() }
    }

    #[test]
    fn streaming_final_replaces_provisional_but_never_final() {
        let mut session = StreamingSession::open(&open(), "remote-123", 2, 1).unwrap();
        session.provider_task_started("remote-123").unwrap();
        let provisional = RemoteSentence { provider_task_id: "remote-123".into(), connection_epoch: 2, sentence_id: "s1".into(), finality: AsrRawCandidateState::Provisional, text: "hello".into(), begin_time_ms: 0, end_time_ms: 100, speaker_ref: None };
        assert!(matches!(session.accept_sentence(provisional.clone()).unwrap(), SentenceDisposition::New(_)));
        let mut final_sentence = provisional;
        final_sentence.finality = AsrRawCandidateState::Final;
        final_sentence.text = "hello world".into();
        assert!(matches!(session.accept_sentence(final_sentence.clone()).unwrap(), SentenceDisposition::Updated(_)));
        let mut conflicting = final_sentence;
        conflicting.text = "different".into();
        assert_eq!(session.accept_sentence(conflicting).unwrap_err().code, ErrorCode::Conflict);
    }

    #[test]
    fn streaming_event_sequence_rejects_early_and_late_events() {
        let mut session = StreamingSession::open(&open(), "remote-123", 2, 1).unwrap();
        let sentence = RemoteSentence { provider_task_id: "remote-123".into(), connection_epoch: 2, sentence_id: "s1".into(), finality: AsrRawCandidateState::Final, text: "hello".into(), begin_time_ms: 0, end_time_ms: 100, speaker_ref: None };
        assert_eq!(session.accept_sentence(sentence.clone()).unwrap_err().code, ErrorCode::InvalidResult);
        assert_eq!(session.request_finish().unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(session.provider_task_started("wrong-task").unwrap_err().code, ErrorCode::StaleInput);
        session.provider_task_started("remote-123").unwrap();
        session.request_finish().unwrap();
        session.provider_task_finished("remote-123").unwrap();
        assert_eq!(session.accept_sentence(sentence).unwrap_err().code, ErrorCode::StaleInput);
        assert_eq!(session.provider_task_finished("remote-123").unwrap_err().code, ErrorCode::Conflict);
    }

    #[test]
    fn streaming_refuses_non_streaming_model_or_slot() {
        let mut wrong_model = open();
        wrong_model.snapshot.model_id = Some(ModelId::QwenAudio31AsrFlashFiletrans);
        assert_eq!(validate_stream_open(&wrong_model).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut wrong_slot = open();
        wrong_slot.binding = crate::binding_for_model(ModelId::Qwen38Flash);
        assert_eq!(validate_stream_open(&wrong_slot).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut wrong_prompt = open();
        wrong_prompt.snapshot.prompt_version = Some("old_prompt".into());
        assert_eq!(validate_stream_open(&wrong_prompt).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut old_contract = open();
        old_contract.snapshot.contract_version = "1.0.0".into();
        assert_eq!(validate_stream_open(&old_contract).unwrap_err().code, ErrorCode::UnsupportedVersion);
        let mut missing_recording = open();
        missing_recording.snapshot.recording_id = None;
        assert_eq!(validate_stream_open(&missing_recording).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut missing_assignment = open();
        missing_assignment.snapshot.assignment_epoch = None;
        assert_eq!(validate_stream_open(&missing_assignment).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn filetrans_uses_only_its_own_model_and_slot() {
        let mut file_snapshot = snapshot(ModelId::QwenAudio31AsrFlashFiletrans, TaskKind::AsrFile);
        file_snapshot.prompt_version = Some(crate::PROMPT_ASR_FILE_V1.into());
        let request = FileTransRequest {
            local_task_id: "file-task".into(),
            attempt: 1,
            snapshot: file_snapshot,
            binding: crate::binding_for_model(ModelId::QwenAudio31AsrFlashFiletrans),
            local_request_segment_id: "segment".into(),
            audio_span: AudioSpan {
                audio_span_id: "span".into(),
                recording_id: "rec".into(),
                asset_id: Some("asset".into()),
                track_id: Some("track".into()),
                frame_range: Some(FrameRange { start_frame: 0, end_frame: 10 }),
                timeline_range: TimelineRange { start_ms: 0, end_ms: 100 },
                mapping_revision_id: None,
                coverage_status: AudioSpanCoverageStatus::Captured,
                asr_coverage: AudioSpanAsrCoverage::Pending,
            },
            file_url: "https://host.invalid/fixture.wav".into(),
        };
        assert_eq!(build_filetrans_submit(&request).unwrap()["model"], FILETRANS_MODEL_ID);
        let mut wrong_slot = request.clone();
        wrong_slot.binding = crate::binding_for_model(ModelId::QwenAudio31AsrFlashStreaming);
        assert_eq!(validate_filetrans_request(&wrong_slot).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut wrong_prompt = request.clone();
        wrong_prompt.snapshot.prompt_version = Some(crate::PROMPT_ASR_STREAM_V1.into());
        assert_eq!(validate_filetrans_request(&wrong_prompt).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn filetrans_only_fills_holes_and_keeps_ambiguous_boundaries() {
        let stable = vec![TimedText { local_id: "s".into(), start_ms: 0, end_ms: 1000, text: "same words".into(), source: MergeSource::Streaming }];
        let duplicate = TimedText { local_id: "f".into(), start_ms: 100, end_ms: 900, text: "same  words".into(), source: MergeSource::FileTrans };
        assert_eq!(merge_filetrans_sentence(duplicate, &stable).unwrap(), MergeDecision::DuplicateOf("s".into()));
        let ambiguous = TimedText { local_id: "f2".into(), start_ms: 100, end_ms: 900, text: "different".into(), source: MergeSource::FileTrans };
        assert!(matches!(merge_filetrans_sentence(ambiguous, &stable).unwrap(), MergeDecision::BoundaryReview(_)));
        let hole = TimedText { local_id: "f3".into(), start_ms: 1100, end_ms: 1500, text: "hole".into(), source: MergeSource::FileTrans };
        assert!(matches!(merge_filetrans_sentence(hole, &stable).unwrap(), MergeDecision::InsertHole(_)));
    }

    #[test]
    fn late_and_cancelled_results_are_blocked() {
        let candidate = AsrRawCandidate { task_id: "local".into(), attempt: 1, snapshot_id: "snap".into(), remote_binding: RemoteBinding { provider: "bailian".into(), provider_task_id: "r".into(), connection_epoch: 1, provider_sentence_id: Some("s".into()), local_request_segment_id: "local".into() }, state: AsrRawCandidateState::Final, text: "x".into(), relative_start_ms: 0, relative_end_ms: 1, speaker_ref: None, mapping_revision_id: "map-1".into() };
        let mut gate = ResultGate::new("local", 1, "snap", Some(1), Some(1));
        assert_eq!(gate.classify(&candidate), PublishDecision::Accept);
        assert_eq!(gate.classify(&candidate), PublishDecision::Duplicate);
        let mut wrong_task = candidate.clone();
        wrong_task.task_id = "other-task".into();
        assert_eq!(gate.classify(&wrong_task), PublishDecision::StaleTask);
        gate.cancel();
        assert_eq!(gate.classify(&candidate), PublishDecision::DropCancelled);
        let mut assignment_gate = ResultGate::new("local", 1, "snap", Some(1), Some(1));
        assignment_gate.update_current_assignment_epoch(Some(2));
        assert_eq!(assignment_gate.classify(&candidate), PublishDecision::StaleAssignment);
        let source = SourceRef { entity_id: "raw".into(), revision_id: "raw-rev".into(), content_hash: "hash".into(), selector: Selector::V4(WholeSelector { kind: "whole".into() }) };
        let submission = build_asr_result_submission_for_open(&open(), &candidate, vec![source.clone()]).unwrap();
        assert_eq!(submission.candidate["result_semantics"], ASR_RESULT_SEMANTICS);
        assert_eq!(validate_asr_result_submission(&submission).unwrap().candidate, candidate);
        let mut outer_mismatch = submission.clone();
        outer_mismatch.attempt = 2;
        assert_eq!(validate_asr_result_submission(&outer_mismatch).unwrap_err().code, ErrorCode::StaleInput);
        let mut wrong_semantics = submission.candidate.clone();
        wrong_semantics["result_semantics"] = serde_json::json!("focaus_context_v2");
        assert_eq!(decode_asr_result_candidate(&wrong_semantics).unwrap_err().code, ErrorCode::InvalidResult);
        let mut wrong_key = submission.candidate.clone();
        wrong_key["submission_key"] = serde_json::json!("asr:other:1:other");
        assert_eq!(decode_asr_result_candidate(&wrong_key).unwrap_err().code, ErrorCode::StaleInput);
        let mut wrong_open = open();
        wrong_open.binding = crate::binding_for_model(ModelId::Qwen38Flash);
        assert_eq!(build_asr_result_submission_for_open(&wrong_open, &candidate, vec![source]).unwrap_err().code, ErrorCode::InvalidArgument);
        let mut wrong_provider = candidate.clone();
        wrong_provider.remote_binding.provider = "other-provider".into();
        assert_eq!(build_asr_result_submission_for_open(&open(), &wrong_provider, vec![]).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn final_after_provisional_is_retried_as_an_update_with_same_binding() {
        let binding = crate::binding_for_model(ModelId::QwenAudio31AsrFlashStreaming);
        let mut gate = ResultGate::new("local", 1, "snap", Some(1), Some(1));
        let mut candidate = AsrRawCandidate { task_id: "local".into(), attempt: 1, snapshot_id: "snap".into(), remote_binding: RemoteBinding { provider: "bailian".into(), provider_task_id: "r".into(), connection_epoch: 1, provider_sentence_id: Some("s".into()), local_request_segment_id: "local".into() }, state: AsrRawCandidateState::Provisional, text: "x".into(), relative_start_ms: 0, relative_end_ms: 1, speaker_ref: None, mapping_revision_id: "map".into() };
        assert_eq!(gate.classify(&candidate), PublishDecision::Accept);
        candidate.state = AsrRawCandidateState::Final;
        assert_eq!(gate.classify(&candidate), PublishDecision::Accept);
        let retry = plan_retry(&task_error(ErrorCode::NetworkUnavailable, "temporary", true), 1, 3, &binding).unwrap();
        assert!(matches!(retry, RetryPlan::Retry { next_attempt: 2, .. }));
        if let RetryPlan::Retry { binding: carried, .. } = retry { assert_eq!(carried, binding); }
        let limited = plan_retry(&task_error(ErrorCode::RateLimited, "limited", true), 1, 3, &binding).unwrap();
        assert!(matches!(limited, RetryPlan::Retry { binding: carried, .. } if carried == binding));
        let auth = plan_retry(&task_error(ErrorCode::AuthRequired, "missing", false), 1, 3, &binding).unwrap();
        assert_eq!(auth, RetryPlan::Fail);
        let auth_marked_retryable = plan_retry(&task_error(ErrorCode::AuthRequired, "marked", true), 1, 3, &binding).unwrap();
        assert_eq!(auth_marked_retryable, RetryPlan::Fail);
        let mut wrong_binding = binding.clone();
        wrong_binding.credential_ref = focaus_contracts::CredentialRef::BailianBeijingMultimodal;
        assert_eq!(plan_retry(&task_error(ErrorCode::NetworkUnavailable, "temporary", true), 1, 3, &wrong_binding).unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(plan_retry(&task_error(ErrorCode::NetworkUnavailable, "bad-attempt", true), 0, 3, &binding).unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(plan_retry(&task_error(ErrorCode::NetworkUnavailable, "bad-max", true), 1, 0, &binding).unwrap_err().code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn mapping_and_wire_requests_are_strict_and_secret_free() {
        assert_eq!(map_relative_ms(&mapping(), 250).unwrap(), 10_250);
        assert_eq!(map_relative_interval(&mapping(), 0, 10_000).unwrap(), TimelineRange { start_ms: 10_000, end_ms: 20_000 });
        let two_spans = TimeMapping { mapping_revision_id: "m".into(), derived_asset_id: "a".into(), segments: vec![
            focaus_contracts::MappingSegment { derived_frames: FrameRange { start_frame: 0, end_frame: 100 }, original_asset_id: "a".into(), original_track_id: "t".into(), original_frames: FrameRange { start_frame: 0, end_frame: 100 }, timeline: TimelineRange { start_ms: 0, end_ms: 1000 }, scale_num: 1, scale_den: 1, delay_frames: 0, max_error_ms: 0.0 },
            focaus_contracts::MappingSegment { derived_frames: FrameRange { start_frame: 100, end_frame: 200 }, original_asset_id: "a".into(), original_track_id: "t".into(), original_frames: FrameRange { start_frame: 100, end_frame: 200 }, timeline: TimelineRange { start_ms: 2000, end_ms: 3000 }, scale_num: 1, scale_den: 1, delay_frames: 0, max_error_ms: 0.0 },
        ] };
        assert_eq!(map_relative_interval(&two_spans, 0, 1000).unwrap(), TimelineRange { start_ms: 0, end_ms: 1000 });
        assert_eq!(map_relative_interval(&two_spans, 1000, 2000).unwrap(), TimelineRange { start_ms: 2000, end_ms: 3000 });
        assert_eq!(map_relative_interval(&two_spans, 1000, 1001).unwrap(), TimelineRange { start_ms: 2000, end_ms: 2001 });
        assert_eq!(map_relative_interval_with_sample_rate(&two_spans, 0, 1000, 100).unwrap(), TimelineRange { start_ms: 0, end_ms: 1000 });
        let scaled = TimeMapping { mapping_revision_id: "scaled".into(), derived_asset_id: "a".into(), segments: vec![focaus_contracts::MappingSegment { derived_frames: FrameRange { start_frame: 0, end_frame: 100 }, original_asset_id: "a".into(), original_track_id: "t".into(), original_frames: FrameRange { start_frame: 0, end_frame: 200 }, timeline: TimelineRange { start_ms: 0, end_ms: 1000 }, scale_num: 2, scale_den: 1, delay_frames: 0, max_error_ms: 0.0 }] };
        assert!(map_relative_interval(&scaled, 0, 1000).is_err());
        assert_eq!(map_relative_interval_with_sample_rate(&scaled, 0, 1000, 100).unwrap(), TimelineRange { start_ms: 0, end_ms: 1000 });
        let body = build_stream_run_task(&open(), "remote-123").unwrap();
        assert_eq!(body["payload"]["model"], STREAMING_MODEL_ID);
        let envelope = build_stream_request_envelope(&open(), "remote-123", 2).unwrap();
        assert_eq!(envelope.local_task_id, open().task_id);
        assert_eq!(envelope.attempt, 2);
        assert_eq!(envelope.binding.credential_ref, crate::binding_for_model(ModelId::QwenAudio31AsrFlashStreaming).credential_ref);
        assert_eq!(envelope.provider_message["payload"]["model"], STREAMING_MODEL_ID);
        assert_eq!(build_stream_request_envelope(&open(), "remote-123", 0).unwrap_err().code, ErrorCode::InvalidArgument);
        assert_eq!(body["payload"]["parameters"]["sample_rate"], 16_000);
        assert!(body["payload"]["parameters"].get("channels").is_none());
        assert_eq!(build_stream_finish_task("remote-123").unwrap()["payload"]["input"], serde_json::json!({}));
        assert_eq!(filetrans_required_headers()["X-DashScope-Async"], "enable");
        assert!(body.to_string().find("credential").is_none());
        assert!(validate_audio_chunk(&AudioChunk { chunk_seq: 0, audio_span: AudioSpan { audio_span_id: "span".into(), recording_id: "rec".into(), asset_id: Some("asset".into()), track_id: Some("track".into()), frame_range: Some(FrameRange { start_frame: 0, end_frame: 10 }), timeline_range: TimelineRange { start_ms: 10_000, end_ms: 10_100 }, mapping_revision_id: Some("map-1".into()), coverage_status: AudioSpanCoverageStatus::Captured, asr_coverage: AudioSpanAsrCoverage::Pending }, encoded_bytes: vec![1], mapping: mapping() }).is_ok());
    }
}

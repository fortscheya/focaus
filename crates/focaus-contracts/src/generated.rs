// Generated from schemas/source.py; do not edit.

pub type JsonValue = serde_json::Value;

pub type EntityId = String;

pub type ContentHash = String;

pub type Timestamp = String;

pub type Version = String;

pub type LanguageTier = u64;

pub type BBox = Vec<f64>;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRange {
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineRange {
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Lifecycle {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "removed")]
    Removed,
    #[serde(rename = "deleted")]
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecordingState {
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "recording")]
    Recording,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskState {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "waiting_network")]
    WaitingNetwork,
    #[serde(rename = "waiting_input")]
    WaitingInput,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "retry_wait")]
    RetryWait,
    #[serde(rename = "cancel_requested")]
    CancelRequested,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "superseded")]
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskKind {
    #[serde(rename = "asr_stream")]
    AsrStream,
    #[serde(rename = "asr_file")]
    AsrFile,
    #[serde(rename = "secondary")]
    Secondary,
    #[serde(rename = "note_incremental")]
    NoteIncremental,
    #[serde(rename = "note_reorganize")]
    NoteReorganize,
    #[serde(rename = "language_render")]
    LanguageRender,
    #[serde(rename = "catchup")]
    Catchup,
    #[serde(rename = "progress")]
    Progress,
    #[serde(rename = "qa")]
    Qa,
    #[serde(rename = "material_convert")]
    MaterialConvert,
    #[serde(rename = "material_reparse")]
    MaterialReparse,
    #[serde(rename = "material_understand")]
    MaterialUnderstand,
    #[serde(rename = "export")]
    Export,
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "visualization_discover")]
    VisualizationDiscover,
    #[serde(rename = "visualization_build")]
    VisualizationBuild,
    #[serde(rename = "visualization_validate")]
    VisualizationValidate,
    #[serde(rename = "visualization_evaluate")]
    VisualizationEvaluate,
    #[serde(rename = "search_index")]
    SearchIndex,
    #[serde(rename = "search_query")]
    SearchQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "INVALID_ARGUMENT")]
    InvalidArgument,
    #[serde(rename = "UNSUPPORTED_VERSION")]
    UnsupportedVersion,
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "SOURCE_UNAVAILABLE")]
    SourceUnavailable,
    #[serde(rename = "SOURCE_DELETED")]
    SourceDeleted,
    #[serde(rename = "CONFLICT")]
    Conflict,
    #[serde(rename = "STALE_INPUT")]
    StaleInput,
    #[serde(rename = "IDEMPOTENCY_MISMATCH")]
    IdempotencyMismatch,
    #[serde(rename = "CURSOR_EXPIRED")]
    CursorExpired,
    #[serde(rename = "RECORDING_BUSY")]
    RecordingBusy,
    #[serde(rename = "LANGUAGE_LOCKED")]
    LanguageLocked,
    #[serde(rename = "PLAYBACK_BLOCKED")]
    PlaybackBlocked,
    #[serde(rename = "GRID_FULL")]
    GridFull,
    #[serde(rename = "EDIT_PROTECTED")]
    EditProtected,
    #[serde(rename = "PERMISSION_DENIED")]
    PermissionDenied,
    #[serde(rename = "DEVICE_LOST")]
    DeviceLost,
    #[serde(rename = "DISK_FULL")]
    DiskFull,
    #[serde(rename = "CORRUPT_ASSET")]
    CorruptAsset,
    #[serde(rename = "NETWORK_UNAVAILABLE")]
    NetworkUnavailable,
    #[serde(rename = "AUTH_REQUIRED")]
    AuthRequired,
    #[serde(rename = "RATE_LIMITED")]
    RateLimited,
    #[serde(rename = "PROVIDER_ERROR")]
    ProviderError,
    #[serde(rename = "DEADLINE_EXCEEDED")]
    DeadlineExceeded,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(rename = "INVALID_RESULT")]
    InvalidResult,
    #[serde(rename = "UNSUPPORTED_CAPABILITY")]
    UnsupportedCapability,
    #[serde(rename = "SEARCH_INVALIDATED")]
    SearchInvalidated,
    #[serde(rename = "INDEX_NOT_READY")]
    IndexNotReady,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelId {
    #[serde(rename = "qwen-audio-3.1-asr-flash-streaming")]
    QwenAudio31AsrFlashStreaming,
    #[serde(rename = "qwen-audio-3.1-asr-flash-filetrans")]
    QwenAudio31AsrFlashFiletrans,
    #[serde(rename = "decision-model-preview")]
    DecisionModelPreview,
    #[serde(rename = "qwen3.8-flash")]
    Qwen38Flash,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CredentialRef {
    #[serde(rename = "bailian_beijing_asr_streaming")]
    BailianBeijingAsrStreaming,
    #[serde(rename = "bailian_beijing_asr_filetrans")]
    BailianBeijingAsrFiletrans,
    #[serde(rename = "bailian_beijing_progress")]
    BailianBeijingProgress,
    #[serde(rename = "bailian_beijing_multimodal")]
    BailianBeijingMultimodal,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialBinding {
    pub provider: String,
    pub region: String,
    pub model_id: ModelId,
    pub credential_ref: CredentialRef,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafeError {
    pub code: ErrorCode,
    pub retryable: bool,
    pub scope: SafeErrorScope,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextAnchor {
    pub left_element_id: Option<EntityId>,
    pub right_element_id: Option<EntityId>,
    pub element_ids: Vec<EntityId>,
    pub scalar_range: ScalarRange,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSelector {
    pub kind: String,
    pub recording_id: EntityId,
    pub audio_span_id: EntityId,
    pub track_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub mapping_revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSelector {
    pub kind: String,
    pub block_id: EntityId,
    pub anchor: TextAnchor,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialSelector {
    pub kind: String,
    pub material_id: EntityId,
    pub source_version_id: EntityId,
    pub parse_version_id: EntityId,
    pub page_id: EntityId,
    pub region_id: EntityId,
    pub bbox: BBox,
    pub coordinate_space: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectSelector {
    pub kind: String,
    pub object_id: EntityId,
    pub element_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WholeSelector {
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Selector {
    V0(AudioSelector),
    V1(TextSelector),
    V2(MaterialSelector),
    V3(ObjectSelector),
    V4(WholeSelector),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub entity_id: EntityId,
    pub revision_id: EntityId,
    pub content_hash: ContentHash,
    pub selector: Selector,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCutoff {
    pub recording_id: EntityId,
    pub end_ms: Option<u64>,
    pub status: EvidenceCutoffStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSnapshot {
    pub snapshot_id: EntityId,
    pub parent_snapshot_id: Option<EntityId>,
    pub contract_version: String,
    pub purpose: TaskKind,
    pub target_id: EntityId,
    pub lesson_id: Option<EntityId>,
    pub recording_id: Option<EntityId>,
    pub assignment_epoch: Option<u64>,
    pub sources: Vec<SourceRef>,
    pub attachment_set_revision: EntityId,
    pub fact_set_revision: Option<EntityId>,
    pub target_base_revision: Option<EntityId>,
    pub edit_revision: Option<EntityId>,
    pub language_tier: Option<LanguageTier>,
    pub evidence_cutoff: Vec<EvidenceCutoff>,
    pub available_at: Timestamp,
    pub executor_id: String,
    pub executor_version: Version,
    pub model_id: Option<ModelId>,
    pub region: Option<String>,
    pub prompt_version: Option<Version>,
    pub parameters_hash: ContentHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Course {
    pub course_id: EntityId,
    pub name: String,
    pub revision_id: EntityId,
    pub entity_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub lesson_id: EntityId,
    pub course_id: EntityId,
    pub title: String,
    pub actual_date: Option<String>,
    pub timezone: String,
    pub requested_language_tier: LanguageTier,
    pub last_recording_tier: Option<LanguageTier>,
    pub material_scope: Vec<EntityId>,
    pub revision_id: EntityId,
    pub entity_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingSession {
    pub recording_id: EntityId,
    pub lesson_id: Option<EntityId>,
    pub assignment_epoch: u64,
    pub state: RecordingState,
    pub capture_language_tier: LanguageTier,
    pub timeline_origin: Timestamp,
    pub durable_watermark_ms: u64,
    pub revision_id: EntityId,
    pub entity_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTrack {
    pub track_id: String,
    pub codec: String,
    pub sample_rate: u64,
    pub channels: u64,
    pub time_base_num: u64,
    pub time_base_den: u64,
    pub pts_origin: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAsset {
    pub audio_asset_id: EntityId,
    pub recording_id: EntityId,
    pub resource_id: EntityId,
    pub container: String,
    pub tracks: Vec<AudioTrack>,
    pub origin: AudioAssetOrigin,
    pub decode_status: AudioAssetDecodeStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingSegment {
    pub derived_frames: FrameRange,
    pub original_asset_id: EntityId,
    pub original_track_id: String,
    pub original_frames: FrameRange,
    pub timeline: TimelineRange,
    pub scale_num: u64,
    pub scale_den: u64,
    pub delay_frames: i64,
    pub max_error_ms: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeMapping {
    pub mapping_revision_id: EntityId,
    pub derived_asset_id: EntityId,
    pub segments: Vec<MappingSegment>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSpan {
    pub audio_span_id: EntityId,
    pub recording_id: EntityId,
    pub asset_id: Option<EntityId>,
    pub track_id: Option<String>,
    pub frame_range: Option<FrameRange>,
    pub timeline_range: TimelineRange,
    pub mapping_revision_id: Option<EntityId>,
    pub coverage_status: AudioSpanCoverageStatus,
    pub asr_coverage: AudioSpanAsrCoverage,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteBinding {
    pub provider: String,
    pub provider_task_id: String,
    pub connection_epoch: u64,
    pub provider_sentence_id: Option<String>,
    pub local_request_segment_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawUtterance {
    pub utterance_id: EntityId,
    pub recording_id: EntityId,
    pub audio_spans: Vec<EntityId>,
    pub ordinal: u64,
    pub speaker_ref: Option<String>,
    pub state: RawUtteranceState,
    pub text: String,
    pub revision_id: EntityId,
    pub entity_seq: u64,
    pub remote_binding: RemoteBinding,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEdge {
    pub edge_id: EntityId,
    pub from_entity_id: EntityId,
    pub from_revision_id: EntityId,
    pub from_selector: Selector,
    pub to: SourceRef,
    pub role: SourceEdgeRole,
    pub created_by: SourceEdgeCreatedBy,
    pub snapshot_id: EntityId,
    pub availability: SourceEdgeAvailability,
    pub freshness: SourceEdgeFreshness,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPatch {
    pub patch_id: EntityId,
    pub target_id: EntityId,
    pub base_revision: EntityId,
    pub operation: UserPatchOperation,
    pub selector: Selector,
    pub before_hash: ContentHash,
    pub after_value: JsonValue,
    pub author: String,
    pub intent: String,
    pub undo_group_id: EntityId,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteIntent {
    pub intent_id: EntityId,
    pub target_id: EntityId,
    pub deleted_ids: Vec<EntityId>,
    pub semantic_key: Option<String>,
    pub scope: String,
    pub base_source_revisions: Vec<EntityId>,
    pub undo_group_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditLease {
    pub view_id: EntityId,
    pub target_id: EntityId,
    pub selectors: Vec<Selector>,
    pub lease_epoch: u64,
    pub composition_active: bool,
    pub draft_revision: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionSuggestion {
    pub suggestion_id: EntityId,
    pub secondary_id: EntityId,
    pub source_revision: EntityId,
    pub anchor: TextAnchor,
    pub original_text: String,
    pub suggested_text: String,
    pub reason: String,
    pub semantic_risk: CorrectionSuggestionSemanticRisk,
    pub state: CorrectionSuggestionState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondarySegment {
    pub secondary_id: EntityId,
    pub recording_id: EntityId,
    pub raw_refs: Vec<SourceRef>,
    pub ordered_spans: Vec<EntityId>,
    pub text: String,
    pub suggestions: Vec<CorrectionSuggestion>,
    pub generation_revision: EntityId,
    pub revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposition {
    pub kind: PropositionKind,
    pub text: String,
    pub polarity: PropositionPolarity,
    pub qualifiers: Vec<String>,
    pub values: std::collections::BTreeMap<String, JsonValue>,
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub fact_id: EntityId,
    pub proposition: Proposition,
    pub source_edges: Vec<SourceEdge>,
    pub revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub note_id: EntityId,
    pub lesson_id: EntityId,
    pub fact_set_revision: EntityId,
    pub fact_ids: Vec<EntityId>,
    pub objects: Vec<MaterialObject>,
    pub revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageVariant {
    pub variant_id: EntityId,
    pub note_id: EntityId,
    pub tier: LanguageTier,
    pub fact_set_revision: EntityId,
    pub fact_ids: Vec<EntityId>,
    pub presentation_revision: EntityId,
    pub sync_status: LanguageVariantSyncStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub proposal_id: EntityId,
    pub snapshot_id: EntityId,
    pub base_revision: EntityId,
    pub operations: Vec<ProposedOperation>,
    pub source_edges: Vec<SourceEdge>,
    pub touched_selectors: Vec<Selector>,
    pub protected_intersections: Vec<Selector>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub definition_id: EntityId,
    pub concept_id: EntityId,
    pub lesson_id: EntityId,
    pub scope_id: EntityId,
    pub valid_from_anchor: SourceRef,
    pub valid_to_anchor: Option<SourceRef>,
    pub source_refs: Vec<SourceRef>,
    pub definition_fact_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolBinding {
    pub binding_id: EntityId,
    pub symbol: String,
    pub scope_id: EntityId,
    pub definition_id: EntityId,
    pub valid_from_anchor: SourceRef,
    pub valid_to_anchor: Option<SourceRef>,
    pub meaning: String,
    pub expression: String,
    pub constraints: Vec<String>,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub task_id: EntityId,
    pub kind: TaskKind,
    pub target_id: EntityId,
    pub snapshot_id: EntityId,
    pub state: TaskState,
    pub priority: u64,
    pub attempt: u64,
    pub deadline_at: Option<Timestamp>,
    pub idempotency_key: String,
    pub checkpoint: std::collections::BTreeMap<String, JsonValue>,
    pub error: Option<SafeError>,
    pub provider_bindings: Vec<RemoteBinding>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub model_id: Option<ModelId>,
    pub region: Option<String>,
    pub prompt_version: Option<Version>,
    pub parameters_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Scope {
    V0(ScopeVariant0),
    V1(ScopeVariant1),
    V2(ScopeVariant2),
    V3(ScopeVariant3),
    V4(ScopeVariant4),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub event_id: EntityId,
    pub contract_version: String,
    pub global_seq: u64,
    pub scope: Scope,
    pub event_type: String,
    pub entity_id: EntityId,
    pub entity_seq: u64,
    pub revision_id: EntityId,
    pub cause_command_id: Option<EntityId>,
    pub task_id: Option<EntityId>,
    pub committed_at: Timestamp,
    pub payload: std::collections::BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventBatch {
    pub subscription_id: EntityId,
    pub after_seq: u64,
    pub through_seq: u64,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSnapshot {
    pub contract_version: String,
    pub host_run_id: EntityId,
    pub snapshot_seq: u64,
    pub scopes: Vec<Scope>,
    pub global: std::collections::BTreeMap<String, JsonValue>,
    pub records: std::collections::BTreeMap<String, std::collections::BTreeMap<String, JsonValue>>,
    pub continuations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub supported_contract_majors: Vec<u64>,
    pub supported_material_majors: Vec<u64>,
    pub visualization_protocol_majors: Vec<u64>,
    pub templates: std::collections::BTreeMap<String, Vec<Version>>,
    pub search_projection_versions: Vec<u64>,
    pub focaus_result_semantics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegotiatedCapabilities {
    pub contract_version: String,
    pub material_version: String,
    pub visualization_version: String,
    pub write_allowed: bool,
    pub unsupported: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionTarget {
    pub kind: String,
    pub lesson_id: EntityId,
    pub content_kind: CollectionTargetContentKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityTarget {
    pub kind: String,
    pub entity_id: EntityId,
    pub revision_id: Option<EntityId>,
    pub selector: Option<Selector>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ViewTarget {
    V0(CollectionTarget),
    V1(EntityTarget),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentView {
    pub view_id: EntityId,
    pub lesson_id: EntityId,
    pub target: ViewTarget,
    pub host_window_id: EntityId,
    pub view_revision: EntityId,
    pub current_owner_lesson_id: Option<EntityId>,
    pub state: ContentViewState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewTransfer {
    pub transfer_id: EntityId,
    pub view_id: EntityId,
    pub source_window_id: EntityId,
    pub target_window_id: EntityId,
    pub base_view_revision: EntityId,
    pub lease_epoch: u64,
    pub state: ViewTransferState,
    pub draft_revision: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub columns: Vec<Vec<EntityId>>,
    pub column_ratios: Vec<f64>,
    pub row_ratios: Vec<Vec<f64>>,
    pub maximized_view_id: Option<EntityId>,
    pub revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub resource_id: EntityId,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: ContentHash,
    pub relative_path: String,
    pub role: String,
    pub owner_refs: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub region_id: EntityId,
    pub page_id: EntityId,
    pub bbox: BBox,
    pub object_ids: Vec<EntityId>,
    pub semantic_role: String,
    pub parent_region_id: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub page_id: EntityId,
    pub revision_id: EntityId,
    pub source_page_index: u64,
    pub display_label: String,
    pub width: f64,
    pub height: f64,
    pub unit: String,
    pub rotation: u64,
    pub source_transform: Vec<f64>,
    pub preview_resource_id: Option<EntityId>,
    pub reading_order: Vec<EntityId>,
    pub region_ids: Vec<EntityId>,
    pub status: PageStatus,
    pub errors: Vec<SafeError>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectLayout {
    pub bounds: BBox,
    pub z_order: u64,
    pub group_id: Option<EntityId>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRun {
    pub element_id: EntityId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextContent {
    pub block_id: EntityId,
    pub runs: Vec<TextRun>,
    pub level: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormulaContent {
    pub latex: String,
    pub display_mode: bool,
    pub symbol_binding_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageContent {
    pub resource_id: EntityId,
    pub alt: String,
    pub crop: BBox,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableCell {
    pub cell_id: EntityId,
    pub row_id: EntityId,
    pub column_id: EntityId,
    pub rowspan: u64,
    pub colspan: u64,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub formula: Option<String>,
    pub status: TableCellStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableContent {
    pub row_ids: Vec<EntityId>,
    pub column_ids: Vec<EntityId>,
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartPoint {
    pub point_id: EntityId,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub value_origin: ChartPointValueOrigin,
    pub error_bound: Option<f64>,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartSeries {
    pub series_id: EntityId,
    pub name: String,
    pub points: Vec<ChartPoint>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartAxis {
    pub axis_id: EntityId,
    pub label: String,
    pub scale: ChartAxisScale,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartContent {
    pub chart_kind: ChartContentChartKind,
    pub axes: Vec<ChartAxis>,
    pub series: Vec<ChartSeries>,
    pub legend: Vec<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramNode {
    pub node_id: EntityId,
    pub label: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramEdge {
    pub edge_id: EntityId,
    pub from_node_id: EntityId,
    pub to_node_id: EntityId,
    pub directed: bool,
    pub label: String,
    pub status: DiagramEdgeStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramContent {
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupContent {
    pub reading_order: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueBinding {
    pub binding_id: EntityId,
    pub parameter_path: String,
    pub value_origin: ValueBindingValueOrigin,
    pub value: JsonValue,
    pub unit: Option<String>,
    pub source_refs: Vec<SourceRef>,
    pub dependency_ids: Vec<EntityId>,
    pub formula_id: Option<String>,
    pub scope_id: EntityId,
    pub unknown_reason: Option<String>,
    pub user_override: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conv2dParameters {
    pub operator_dimension: u64,
    pub scene_dimension: u64,
    pub operator_semantics: Conv2dParametersOperatorSemantics,
    pub axis_order: String,
    pub input: Vec<Vec<Vec<f64>>>,
    pub kernel: Vec<Vec<Vec<f64>>>,
    pub stride: Vec<u64>,
    pub padding: Vec<u64>,
    pub dilation: Vec<u64>,
    pub bias: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveParameters {
    pub formula_id: String,
    pub slope: f64,
    pub intercept: f64,
    pub x_min: f64,
    pub x_max: f64,
    pub samples: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Computation {
    pub algorithm_id: String,
    pub algorithm_version: Version,
    pub operator_semantics: String,
    pub formula_id: String,
    pub input_shape: Vec<u64>,
    pub output_shape: Vec<u64>,
    pub precision_policy_id: String,
    pub dependency_binding_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub position: Vec<f64>,
    pub target: Vec<f64>,
    pub zoom: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionState {
    pub selected_input_ids: Vec<EntityId>,
    pub selected_output_ids: Vec<EntityId>,
    pub step_index: u64,
    pub camera: Option<Camera>,
    pub user_parameter_overrides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CheckState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationChecks {
    pub schema: CheckState,
    pub source: CheckState,
    pub semantic: CheckState,
    pub math: CheckState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationReport {
    pub checks: ValidationChecks,
    pub validator_version: Version,
    pub parameters_hash: ContentHash,
    pub validated_revision: EntityId,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierPresentation {
    pub tier: LanguageTier,
    pub label: String,
    pub description: String,
    pub sync_status: TierPresentationSyncStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fallback {
    pub preview_resource_id: Option<EntityId>,
    pub description: String,
    pub table: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationContent {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub parent_revision_ids: Vec<EntityId>,
    pub protocol: String,
    pub protocol_version: String,
    pub template_id: String,
    pub template_version: Version,
    pub registry_revision: String,
    pub concept_fact_ids: Vec<EntityId>,
    pub definition_ids: Vec<EntityId>,
    pub symbol_binding_ids: Vec<EntityId>,
    pub scope_id: EntityId,
    pub semantic_key: String,
    pub purpose: String,
    pub placement_anchor: Selector,
    pub parameters: JsonValue,
    pub parameter_schema_version: String,
    pub value_bindings: Vec<ValueBinding>,
    pub computation: Computation,
    pub resources: Vec<Resource>,
    pub source_refs: Vec<SourceRef>,
    pub snapshot_id: EntityId,
    pub fact_set_revision: EntityId,
    pub interaction_state: InteractionState,
    pub presentation_by_tier: Vec<TierPresentation>,
    pub generation_status: VisualizationContentGenerationStatus,
    pub validation: ValidationReport,
    pub fallback: Fallback,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: TextContentObjectExtraction,
    pub editability: TextContentObjectEditability,
    pub status: TextContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: TextContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormulaContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: FormulaContentObjectExtraction,
    pub editability: FormulaContentObjectEditability,
    pub status: FormulaContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: FormulaContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: ImageContentObjectExtraction,
    pub editability: ImageContentObjectEditability,
    pub status: ImageContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: ImageContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: TableContentObjectExtraction,
    pub editability: TableContentObjectEditability,
    pub status: TableContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: TableContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: ChartContentObjectExtraction,
    pub editability: ChartContentObjectEditability,
    pub status: ChartContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: ChartContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: DiagramContentObjectExtraction,
    pub editability: DiagramContentObjectEditability,
    pub status: DiagramContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: DiagramContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: GroupContentObjectExtraction,
    pub editability: GroupContentObjectEditability,
    pub status: GroupContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: GroupContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationContentObject {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub source_refs: Vec<SourceRef>,
    pub region_ids: Vec<EntityId>,
    pub layout: ObjectLayout,
    pub extraction: VisualizationContentObjectExtraction,
    pub editability: VisualizationContentObjectEditability,
    pub status: VisualizationContentObjectStatus,
    pub children: Vec<EntityId>,
    pub r#type: String,
    pub content: VisualizationContent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MaterialObject {
    V0(TextContentObject),
    V1(FormulaContentObject),
    V2(ImageContentObject),
    V3(TableContentObject),
    V4(ChartContentObject),
    V5(DiagramContentObject),
    V6(GroupContentObject),
    V7(VisualizationContentObject),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchAnchor {
    pub owner_entity_id: EntityId,
    pub revision_id: EntityId,
    pub selector: Selector,
    pub field_path: String,
    pub scalar_range: ScalarRange,
    pub audio_spans: Vec<EntityId>,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedSearchUnit {
    pub unit_key: String,
    pub owner_entity_id: EntityId,
    pub revision_id: EntityId,
    pub content_hash: ContentHash,
    pub display_text: String,
    pub field_path: String,
    pub anchor: SearchAnchor,
    pub equivalence_group_id: Option<String>,
    pub alternate_anchors: Vec<SearchAnchor>,
    pub source_role: String,
    pub extraction_version: Version,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageArea {
    pub page_id: Option<EntityId>,
    pub region_id: Option<EntityId>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionCoverage {
    pub requested_pages: Vec<EntityId>,
    pub processed_pages: Vec<EntityId>,
    pub unprocessed: Vec<CoverageArea>,
    pub failed: Vec<CoverageArea>,
    pub state: ExtractionCoverageState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OffsetEntry {
    pub normalized_range: ScalarRange,
    pub original_range: ScalarRange,
    pub element_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchUnit {
    pub search_unit_id: EntityId,
    pub owner_entity_id: EntityId,
    pub revision_id: EntityId,
    pub content_hash: ContentHash,
    pub kind: SearchUnitKind,
    pub visible_tier: Option<LanguageTier>,
    pub field_path: String,
    pub display_text: String,
    pub normalized_text: String,
    pub normalization_profile: String,
    pub offset_map: Vec<OffsetEntry>,
    pub anchor: SearchAnchor,
    pub equivalence_group_id: Option<String>,
    pub source_role: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchScope {
    pub kind: SearchScopeKind,
    pub view_id: EntityId,
    pub lesson_id: EntityId,
    pub target_ids: Vec<EntityId>,
    pub revision_ids: Vec<EntityId>,
    pub actual_visible_tier: Option<LanguageTier>,
    pub version_mode: SearchScopeVersionMode,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCoverage {
    pub scope_snapshot_id: EntityId,
    pub required_units: u64,
    pub indexed_units: u64,
    pub required_pages: Vec<EntityId>,
    pub indexed_pages: Vec<EntityId>,
    pub unprocessed: Vec<CoverageArea>,
    pub failed: Vec<CoverageArea>,
    pub stale: Vec<CoverageArea>,
    pub state: SearchCoverageState,
    pub source_extraction_revision: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchIndexState {
    pub projection_version: u64,
    pub index_generation: u64,
    pub head_manifest_hash: ContentHash,
    pub dirty_ranges: Vec<EntityId>,
    pub coverage: SearchCoverage,
    pub visibility_epoch: u64,
    pub last_committed_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSession {
    pub search_id: EntityId,
    pub view_id: EntityId,
    pub query_seq: u64,
    pub query: String,
    pub case_sensitive: bool,
    pub scope: SearchScope,
    pub scope_snapshot_id: EntityId,
    pub index_generation: u64,
    pub visibility_epoch: u64,
    pub selected_hit_id: Option<String>,
    pub cursor: Option<String>,
    pub status: SearchSessionStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    pub hit_id: String,
    pub search_unit_id: EntityId,
    pub revision_id: EntityId,
    pub matched_anchor: SearchAnchor,
    pub ranges: Vec<ScalarRange>,
    pub display_snippet: String,
    pub equivalence_group_id: Option<String>,
    pub nav_kind: SearchHitNavKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchPage {
    pub search_id: EntityId,
    pub query_seq: u64,
    pub scope_snapshot_id: EntityId,
    pub hits: Vec<SearchHit>,
    pub total_count: u64,
    pub count_status: SearchPageCountStatus,
    pub coverage: SearchCoverage,
    pub index_generation: u64,
    pub visibility_epoch: u64,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialCorrection {
    pub correction_id: EntityId,
    pub source_version_id: EntityId,
    pub parse_version_id: EntityId,
    pub selector: Selector,
    pub before_hash: ContentHash,
    pub replacement: JsonValue,
    pub scope: String,
    pub replay_status: MaterialCorrectionReplayStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialManifest {
    pub format: String,
    pub format_version: String,
    pub material_id: EntityId,
    pub source_version_id: EntityId,
    pub parse_version_id: EntityId,
    pub parent_parse_version_id: Option<EntityId>,
    pub source_hash: ContentHash,
    pub engine_build: String,
    pub parse_profile: MaterialManifestParseProfile,
    pub requested_pages: Vec<u64>,
    pub pages: Vec<Page>,
    pub regions: Vec<Region>,
    pub objects: Vec<MaterialObject>,
    pub resources: Vec<Resource>,
    pub corrections_applied: Vec<MaterialCorrection>,
    pub unresolved_conflicts: Vec<String>,
    pub provenance: std::collections::BTreeMap<String, JsonValue>,
    pub completion: MaterialManifestCompletion,
    pub search_units: Vec<ExtractedSearchUnit>,
    pub search_coverage: ExtractionCoverage,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressState {
    pub lesson_id: EntityId,
    pub recording_id: EntityId,
    pub assignment_epoch: u64,
    pub snapshot_id: EntityId,
    pub position_status: ProgressStatePositionStatus,
    pub page_id: Option<EntityId>,
    pub region_id: Option<EntityId>,
    pub evidence_cutoff_ms: Option<u64>,
    pub calibration_seq: u64,
    pub decision_seq: u64,
    pub recording_state: ProgressStateRecordingState,
    pub freshness: ProgressStateFreshness,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupWindow {
    pub recording_id: Option<EntityId>,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub start_source: Option<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentFocus {
    pub topic: String,
    pub problem: String,
    pub activity: CurrentFocusActivity,
    pub step: Option<String>,
    pub source_refs: Vec<SourceRef>,
    pub status: CurrentFocusStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathNode {
    pub node_id: String,
    pub role: PathNodeRole,
    pub fact_ids: Vec<EntityId>,
    pub definition_ids: Vec<EntityId>,
    pub binding_ids: Vec<EntityId>,
    pub body_block_ids: Vec<EntityId>,
    pub evidence_role: PathNodeEvidenceRole,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation: PathEdgeRelation,
    pub source_refs: Vec<SourceRef>,
    pub certainty: PathEdgeCertainty,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prerequisite {
    pub definition_id: Option<EntityId>,
    pub binding_id: Option<EntityId>,
    pub fact_id: Option<EntityId>,
    pub reason: String,
    pub source_refs: Vec<SourceRef>,
    pub scope_id: EntityId,
    pub outside_window: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyBlock {
    pub block_id: EntityId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressBinding {
    pub lesson_id: EntityId,
    pub recording_id: Option<EntityId>,
    pub assignment_epoch: Option<u64>,
    pub no_pointer_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupCoverage {
    pub audio_gaps: Vec<AudioSpan>,
    pub asr_gaps: Vec<AudioSpan>,
    pub material_supplement_used: bool,
    pub range_status: CatchupCoverageRangeStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupResult {
    pub result_semantics: String,
    pub result_id: EntityId,
    pub request_generation: u64,
    pub snapshot_id: EntityId,
    pub body_revision_id: EntityId,
    pub requested_window: CatchupWindow,
    pub effective_window: CatchupWindow,
    pub estimate_basis: String,
    pub current_focus: CurrentFocus,
    pub path_nodes: Vec<PathNode>,
    pub path_edges: Vec<PathEdge>,
    pub prerequisites: Vec<Prerequisite>,
    pub prerequisites_reason: Option<CatchupResultPrerequisitesReasonValue>,
    pub body: Vec<BodyBlock>,
    pub presentation_tier: LanguageTier,
    pub body_evidence_cutoff: Vec<EvidenceCutoff>,
    pub body_generated_at: Timestamp,
    pub progress_at_trigger: Option<ProgressState>,
    pub progress_unavailable_reason: Option<String>,
    pub progress_binding: ProgressBinding,
    pub coverage: CatchupCoverage,
    pub limitations: Vec<String>,
    pub completion_status: CatchupResultCompletionStatus,
    pub triggered_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub displayed_complete_elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySummary {
    pub result_semantics: String,
    pub original: JsonValue,
    pub completion_status: String,
    pub new_acceptance_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationStep {
    pub output_index: Vec<u64>,
    pub input_indices: Vec<Vec<i64>>,
    pub input_values: Vec<f64>,
    pub kernel_values: Vec<f64>,
    pub products: Vec<f64>,
    pub bias: f64,
    pub sum: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationResult {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub interaction_seq: u64,
    pub parameters_hash: ContentHash,
    pub algorithm_version: Version,
    pub shape: Vec<u64>,
    pub values: Vec<f64>,
    pub steps: Vec<EvaluationStep>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextResult {
    pub snapshot: InputSnapshot,
    pub definitions: Vec<Definition>,
    pub symbol_bindings: Vec<SymbolBinding>,
    pub source_refs: Vec<SourceRef>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateOpenInput {
    pub scopes: Vec<Scope>,
    pub last_seq: Option<u64>,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateAckInput {
    pub subscription_id: EntityId,
    pub upto_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateReplayInput {
    pub subscription_id: EntityId,
    pub after_seq: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateCloseInput {
    pub subscription_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseCreateInput {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessonCreateInput {
    pub course_id: EntityId,
    pub title: String,
    pub actual_date: Option<String>,
    pub timezone: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessonUpdateInput {
    pub lesson_id: EntityId,
    pub title: String,
    pub material_scope: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingStartInput {
    pub lesson_id: Option<EntityId>,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingPauseInput {
    pub recording_id: EntityId,
    pub expected_state: RecordingState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingResumeInput {
    pub recording_id: EntityId,
    pub expected_state: RecordingState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingStopInput {
    pub recording_id: EntityId,
    pub expected_state: RecordingState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingTransferInput {
    pub recording_id: EntityId,
    pub target_lesson_id: EntityId,
    pub expected_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingImportInput {
    pub file_handle: String,
    pub lesson_id: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingAlignInput {
    pub recording_id: EntityId,
    pub timeline: TimelineRange,
    pub lesson_id: Option<EntityId>,
    pub evidence_refs: Vec<SourceRef>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentGetInput {
    pub entity_id: EntityId,
    pub revision_id: Option<EntityId>,
    pub selector: Option<Selector>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentPatchInput {
    pub patch: UserPatch,
    pub lease_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentUndoInput {
    pub undo_group_id: EntityId,
    pub current_revision: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditBeginInput {
    pub lease: EditLease,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditEndInput {
    pub view_id: EntityId,
    pub target_id: EntityId,
    pub lease_epoch: u64,
    pub draft_revision: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionDraftInput {
    pub suggestion_id: EntityId,
    pub draft: String,
    pub composition_active: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionAcceptInput {
    pub suggestion_id: EntityId,
    pub base_revision: EntityId,
    pub final_text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionRejectInput {
    pub suggestion_id: EntityId,
    pub base_revision: EntityId,
    pub final_text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionAcceptBatchInput {
    pub base_revision: EntityId,
    pub suggestion_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteReorganizeInput {
    pub note_id: EntityId,
    pub selectors: Vec<Selector>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteAddSelectionInput {
    pub note_id: EntityId,
    pub sources: Vec<SourceRef>,
    pub object_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageSelectInput {
    pub lesson_id: EntityId,
    pub tier: LanguageTier,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceResolveInput {
    pub edge_id: EntityId,
    pub return_anchor: Selector,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDiffInput {
    pub source: SourceRef,
    pub new_revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionResolveInput {
    pub definition_id: EntityId,
    pub visible_tier: LanguageTier,
    pub scope_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPlayInput {
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSeekInput {
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStopInput {
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCancelInput {
    pub task_id: EntityId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRetryInput {
    pub task_id: EntityId,
    pub snapshot_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityReserveInput {
    pub entity_type: String,
    pub scope_id: EntityId,
    pub candidate_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBuildInput {
    pub purpose: TaskKind,
    pub target_id: EntityId,
    pub evidence_cutoff: Vec<EvidenceCutoff>,
    pub language_tier: Option<LanguageTier>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEnqueueInput {
    pub task: Task,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCheckpointInput {
    pub task_id: EntityId,
    pub attempt: u64,
    pub cursor: String,
    pub safe_state: std::collections::BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultSubmitInput {
    pub task_id: EntityId,
    pub attempt: u64,
    pub snapshot_id: EntityId,
    pub candidate: JsonValue,
    pub source_refs: Vec<SourceRef>,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalApplyInput {
    pub proposal_id: EntityId,
    pub base_revisions: std::collections::BTreeMap<String, EntityId>,
    pub selectors: Vec<Selector>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReadInput {
    pub resource_id: EntityId,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceOpenCourseInput {
    pub course_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewOpenInput {
    pub lesson_id: EntityId,
    pub target: ViewTarget,
    pub placement: ViewOpenInputPlacement,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSaveStateInput {
    pub view_id: EntityId,
    pub revision_id: EntityId,
    pub draft_revision: Option<EntityId>,
    pub selection: Option<Selector>,
    pub scroll: f64,
    pub camera: Option<Camera>,
    pub search: Option<SearchSession>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMoveInput {
    pub view_id: EntityId,
    pub target_window_id: Option<EntityId>,
    pub base_view_revision: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewReadyInput {
    pub transfer_id: EntityId,
    pub view_revision: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMaximizeInput {
    pub view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRestoreInput {
    pub view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewMinimizeInput {
    pub view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewCloseInput {
    pub view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutResizeInput {
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialImportInput {
    pub file_handle: String,
    pub lesson_id: EntityId,
    pub purpose: MaterialImportInputPurpose,
    pub pages: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialConvertInput {
    pub snapshot: InputSnapshot,
    pub source_handle: String,
    pub staging_handle: String,
    pub identity_pool: std::collections::BTreeMap<String, EntityId>,
    pub profile: MaterialConvertInputProfile,
    pub pages: Vec<u64>,
    pub corrections: Vec<MaterialCorrection>,
    pub budget_ms: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialPageResultInput {
    pub task_id: EntityId,
    pub snapshot_id: EntityId,
    pub manifest: MaterialManifest,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialReparseInput {
    pub material_id: EntityId,
    pub base_parse_version_id: EntityId,
    pub pages: Vec<u64>,
    pub corrections: Vec<MaterialCorrection>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialCorrectInput {
    pub correction: MaterialCorrection,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialDetachInput {
    pub attachment_id: EntityId,
    pub base_revision: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialDeletePreviewInput {
    pub material_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialDeleteConfirmInput {
    pub material_id: EntityId,
    pub impact_revision: EntityId,
    pub delete_copies: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialUnderstandInput {
    pub snapshot: InputSnapshot,
    pub resource_ids: Vec<EntityId>,
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationRequestInput {
    pub note_id: EntityId,
    pub changed_fact_ids: Vec<EntityId>,
    pub trigger: VisualizationRequestInputTrigger,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationEvaluateInput {
    pub object_id: EntityId,
    pub revision_id: EntityId,
    pub parameters: JsonValue,
    pub interaction_seq: u64,
    pub algorithm_version: Version,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationValidateInput {
    pub candidate: VisualizationContent,
    pub semantic_check: CheckState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationPatchInput {
    pub patch: UserPatch,
    pub lease_epoch: u64,
    pub validated_parameters_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationRetryInput {
    pub object_id: EntityId,
    pub base_revision: EntityId,
    pub failed_stage: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchOpenInput {
    pub view_id: EntityId,
    pub target: ViewTarget,
    pub version_mode: SearchOpenInputVersionMode,
    pub actual_visible_tier: Option<LanguageTier>,
    pub revision_ids: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchQueryInput {
    pub search_id: EntityId,
    pub query_seq: u64,
    pub query: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchPageInput {
    pub search_id: EntityId,
    pub query_seq: u64,
    pub scope_snapshot_id: EntityId,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchLocateInput {
    pub search_id: EntityId,
    pub hit_id: String,
    pub view_id: EntityId,
    pub expected_revision: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRefreshInput {
    pub search_id: EntityId,
    pub query_seq: u64,
    pub selected_hit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCloseInput {
    pub search_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupRequestInput {
    pub view_id: EntityId,
    pub lesson_id: EntityId,
    pub recording_id: Option<EntityId>,
    pub assignment_epoch: Option<u64>,
    pub request_generation: u64,
    pub triggered_at: Timestamp,
    pub remaining_deadline_ms: u64,
    pub requested_window: CatchupWindow,
    pub estimate_basis: String,
    pub actual_tier: LanguageTier,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRetrieveInput {
    pub snapshot: InputSnapshot,
    pub definition_ids: Vec<EntityId>,
    pub binding_ids: Vec<EntityId>,
    pub course_id: EntityId,
    pub remaining_budget_ms: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupDisplayAckInput {
    pub result_id: EntityId,
    pub request_generation: u64,
    pub body_revision_id: EntityId,
    pub elapsed_ms: u64,
    pub displayed_complete: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressCalibrateInput {
    pub lesson_id: EntityId,
    pub recording_id: EntityId,
    pub page_id: EntityId,
    pub region_id: Option<EntityId>,
    pub evidence: SourceRef,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QaAskInput {
    pub lesson_id: EntityId,
    pub question: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsUpdateInput {
    pub default_language_tier: LanguageTier,
    pub theme: SettingsUpdateInputTheme,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleUpsertInput {
    pub schedule_id: EntityId,
    pub course_id: EntityId,
    pub timezone: String,
    pub start_minute: u64,
    pub duration_minutes: u64,
    pub every_n_weeks: u64,
    pub exceptions: Vec<String>,
    pub reminder: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageExportInput {
    pub lesson_id: EntityId,
    pub format: PackageExportInputFormat,
    pub destination_handle: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageRestoreInput {
    pub file_handle: String,
    pub confirmed_impact_revision: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppExitInput {
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub contract_version: String,
    pub command_id: EntityId,
    pub name: CommandName,
    pub actor: CommandActor,
    pub window_id: Option<EntityId>,
    pub payload: JsonValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revisions: Option<std::collections::BTreeMap<String, EntityId>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResult {
    pub contract_version: String,
    pub command_id: EntityId,
    pub status: CommandResultStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_revisions: Option<std::collections::BTreeMap<String, EntityId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_state: Option<TaskState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SafeError>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateOpenResult {
    pub subscription_id: EntityId,
    pub snapshot: AppSnapshot,
    pub snapshot_seq: u64,
    pub negotiated: NegotiatedCapabilities,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedOperation {
    pub target_id: EntityId,
    pub base_revision: EntityId,
    pub operation: ProposedOperationOperation,
    pub selector: Selector,
    pub before_hash: ContentHash,
    pub after_value: JsonValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrStreamOpen {
    pub task_id: EntityId,
    pub snapshot: InputSnapshot,
    pub binding: CredentialBinding,
    pub format: String,
    pub sample_rate: u64,
    pub channels: u64,
    pub hotwords: Vec<String>,
    pub context: Vec<String>,
    pub mapping: TimeMapping,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioChunk {
    pub chunk_seq: u64,
    pub audio_span: AudioSpan,
    pub encoded_bytes: Vec<u64>,
    pub mapping: TimeMapping,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrRawCandidate {
    pub task_id: EntityId,
    pub attempt: u64,
    pub snapshot_id: EntityId,
    pub remote_binding: RemoteBinding,
    pub state: AsrRawCandidateState,
    pub text: String,
    pub relative_start_ms: u64,
    pub relative_end_ms: u64,
    pub speaker_ref: Option<String>,
    pub mapping_revision_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineRequest {
    pub contract_version: String,
    pub request_id: EntityId,
    pub name: EngineRequestName,
    pub task_id: Option<EntityId>,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineResponse {
    pub contract_version: String,
    pub request_id: EntityId,
    pub task_id: Option<EntityId>,
    pub kind: EngineResponseKind,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationDiscoverInput {
    pub snapshot: InputSnapshot,
    pub existing_keys: Vec<String>,
    pub deleted_keys: Vec<String>,
    pub supported_templates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizationBuildInput {
    pub snapshot: InputSnapshot,
    pub candidate: JsonValue,
    pub identity_map: std::collections::BTreeMap<String, EntityId>,
    pub registry_revision: Version,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchProjectionSubmitInput {
    pub units: Vec<ExtractedSearchUnit>,
    pub coverage: ExtractionCoverage,
    pub owner_revisions: std::collections::BTreeMap<String, EntityId>,
    pub source_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRebuildInput {
    pub scope: SearchScope,
    pub projection_version: u64,
    pub normalization_profile: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupGenerateInput {
    pub snapshot: InputSnapshot,
    pub progress_at_trigger: Option<ProgressState>,
    pub window: CatchupWindow,
    pub context: ContextResult,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchupValidateInput {
    pub result: CatchupResult,
    pub snapshot: InputSnapshot,
    pub remaining_budget_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SafeErrorScope {
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "task")]
    Task,
    #[serde(rename = "asset")]
    Asset,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "recording")]
    Recording,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EvidenceCutoffStatus {
    #[serde(rename = "known")]
    Known,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioAssetOrigin {
    #[serde(rename = "original")]
    Original,
    #[serde(rename = "derived")]
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioAssetDecodeStatus {
    #[serde(rename = "unverified")]
    Unverified,
    #[serde(rename = "decodable")]
    Decodable,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioSpanCoverageStatus {
    #[serde(rename = "captured")]
    Captured,
    #[serde(rename = "silence_observed")]
    SilenceObserved,
    #[serde(rename = "capture_gap")]
    CaptureGap,
    #[serde(rename = "decode_unknown")]
    DecodeUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioSpanAsrCoverage {
    #[serde(rename = "unsubmitted")]
    Unsubmitted,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "provisional")]
    Provisional,
    #[serde(rename = "final")]
    Final,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "uncertain")]
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RawUtteranceState {
    #[serde(rename = "provisional")]
    Provisional,
    #[serde(rename = "final")]
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceEdgeRole {
    #[serde(rename = "teacher_said")]
    TeacherSaid,
    #[serde(rename = "material_supplement")]
    MaterialSupplement,
    #[serde(rename = "ai_background")]
    AiBackground,
    #[serde(rename = "derived")]
    Derived,
    #[serde(rename = "teacher_path")]
    TeacherPath,
    #[serde(rename = "system_explanation")]
    SystemExplanation,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceEdgeCreatedBy {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "ai")]
    Ai,
    #[serde(rename = "engine")]
    Engine,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceEdgeAvailability {
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "source_removed")]
    SourceRemoved,
    #[serde(rename = "source_deleted")]
    SourceDeleted,
    #[serde(rename = "intermediate_unavailable")]
    IntermediateUnavailable,
    #[serde(rename = "unresolved")]
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceEdgeFreshness {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "update_available")]
    UpdateAvailable,
    #[serde(rename = "new_source_available")]
    NewSourceAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UserPatchOperation {
    #[serde(rename = "text_insert")]
    TextInsert,
    #[serde(rename = "text_delete")]
    TextDelete,
    #[serde(rename = "text_replace")]
    TextReplace,
    #[serde(rename = "cell_set")]
    CellSet,
    #[serde(rename = "node_set")]
    NodeSet,
    #[serde(rename = "edge_set")]
    EdgeSet,
    #[serde(rename = "move")]
    Move,
    #[serde(rename = "resize")]
    Resize,
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "parameter_set")]
    ParameterSet,
    #[serde(rename = "view_default_set")]
    ViewDefaultSet,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorrectionSuggestionSemanticRisk {
    #[serde(rename = "ordinary")]
    Ordinary,
    #[serde(rename = "critical")]
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorrectionSuggestionState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "obsolete")]
    Obsolete,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PropositionKind {
    #[serde(rename = "statement")]
    Statement,
    #[serde(rename = "definition")]
    Definition,
    #[serde(rename = "formula")]
    Formula,
    #[serde(rename = "assignment")]
    Assignment,
    #[serde(rename = "deadline")]
    Deadline,
    #[serde(rename = "exam")]
    Exam,
    #[serde(rename = "relationship")]
    Relationship,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PropositionPolarity {
    #[serde(rename = "positive")]
    Positive,
    #[serde(rename = "negative")]
    Negative,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LanguageVariantSyncStatus {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "generating")]
    Generating,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVariant0 {
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVariant1 {
    pub kind: String,
    pub course_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVariant2 {
    pub kind: String,
    pub lesson_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVariant3 {
    pub kind: String,
    pub entity_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeVariant4 {
    pub kind: String,
    pub view_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CollectionTargetContentKind {
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "raw")]
    Raw,
    #[serde(rename = "secondary")]
    Secondary,
    #[serde(rename = "material")]
    Material,
    #[serde(rename = "qa")]
    Qa,
    #[serde(rename = "arrangements")]
    Arrangements,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ContentViewState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "minimized")]
    Minimized,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "transferring")]
    Transferring,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewTransferState {
    #[serde(rename = "preparing")]
    Preparing,
    #[serde(rename = "committed")]
    Committed,
    #[serde(rename = "rolled_back")]
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PageStatus {
    #[serde(rename = "basic_ready")]
    BasicReady,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TableCellStatus {
    #[serde(rename = "known")]
    Known,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartPointValueOrigin {
    #[serde(rename = "explicit")]
    Explicit,
    #[serde(rename = "estimated")]
    Estimated,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartAxisScale {
    #[serde(rename = "linear")]
    Linear,
    #[serde(rename = "log")]
    Log,
    #[serde(rename = "category")]
    Category,
    #[serde(rename = "date")]
    Date,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartContentChartKind {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "bar")]
    Bar,
    #[serde(rename = "scatter")]
    Scatter,
    #[serde(rename = "area")]
    Area,
    #[serde(rename = "pie")]
    Pie,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagramEdgeStatus {
    #[serde(rename = "known")]
    Known,
    #[serde(rename = "unresolved")]
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ValueBindingValueOrigin {
    #[serde(rename = "classroom")]
    Classroom,
    #[serde(rename = "derived")]
    Derived,
    #[serde(rename = "demonstration")]
    Demonstration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Conv2dParametersOperatorSemantics {
    #[serde(rename = "cross_correlation")]
    CrossCorrelation,
    #[serde(rename = "convolution")]
    Convolution,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TierPresentationSyncStatus {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "stale")]
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisualizationContentGenerationStatus {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "validating")]
    Validating,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "needs_evidence")]
    NeedsEvidence,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FormulaContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FormulaContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FormulaContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImageContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImageContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImageContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TableContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TableContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TableContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagramContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagramContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagramContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GroupContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GroupContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GroupContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisualizationContentObjectExtraction {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisualizationContentObjectEditability {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "image_only")]
    ImageOnly,
    #[serde(rename = "read_only")]
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisualizationContentObjectStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unresolved")]
    Unresolved,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExtractionCoverageState {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchUnitKind {
    #[serde(rename = "material")]
    Material,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "raw")]
    Raw,
    #[serde(rename = "secondary")]
    Secondary,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchScopeKind {
    #[serde(rename = "material")]
    Material,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "raw")]
    Raw,
    #[serde(rename = "secondary")]
    Secondary,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchScopeVersionMode {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "historical")]
    Historical,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchCoverageState {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "indexing")]
    Indexing,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchSessionStatus {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "updating")]
    Updating,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "closed")]
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchHitNavKind {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "page_region")]
    PageRegion,
    #[serde(rename = "object_focus")]
    ObjectFocus,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchPageCountStatus {
    #[serde(rename = "exact")]
    Exact,
    #[serde(rename = "provisional")]
    Provisional,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialCorrectionReplayStatus {
    #[serde(rename = "applied")]
    Applied,
    #[serde(rename = "conflict")]
    Conflict,
    #[serde(rename = "unmatched")]
    Unmatched,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialManifestParseProfile {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "deep")]
    Deep,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialManifestCompletion {
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProgressStatePositionStatus {
    #[serde(rename = "located")]
    Located,
    #[serde(rename = "unlocated")]
    Unlocated,
    #[serde(rename = "off_material")]
    OffMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProgressStateRecordingState {
    #[serde(rename = "recording")]
    Recording,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "interrupted")]
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProgressStateFreshness {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "pending_update")]
    PendingUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CurrentFocusActivity {
    #[serde(rename = "derivation")]
    Derivation,
    #[serde(rename = "example")]
    Example,
    #[serde(rename = "transition")]
    Transition,
    #[serde(rename = "explanation")]
    Explanation,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CurrentFocusStatus {
    #[serde(rename = "supported")]
    Supported,
    #[serde(rename = "uncertain")]
    Uncertain,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathNodeRole {
    #[serde(rename = "missed_key")]
    MissedKey,
    #[serde(rename = "bridge")]
    Bridge,
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "prerequisite")]
    Prerequisite,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathNodeEvidenceRole {
    #[serde(rename = "teacher_path")]
    TeacherPath,
    #[serde(rename = "system_explanation")]
    SystemExplanation,
    #[serde(rename = "material_supplement")]
    MaterialSupplement,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathEdgeRelation {
    #[serde(rename = "temporal")]
    Temporal,
    #[serde(rename = "dependency")]
    Dependency,
    #[serde(rename = "topic_bridge")]
    TopicBridge,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathEdgeCertainty {
    #[serde(rename = "supported")]
    Supported,
    #[serde(rename = "uncertain")]
    Uncertain,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CatchupCoverageRangeStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CatchupResultPrerequisitesReasonValue {
    #[serde(rename = "none_needed")]
    NoneNeeded,
    #[serde(rename = "insufficient_evidence")]
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CatchupResultCompletionStatus {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewOpenInputPlacement {
    #[serde(rename = "grid")]
    Grid,
    #[serde(rename = "standalone")]
    Standalone,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialImportInputPurpose {
    #[serde(rename = "textbook")]
    Textbook,
    #[serde(rename = "courseware")]
    Courseware,
    #[serde(rename = "reference")]
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialConvertInputProfile {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "deep")]
    Deep,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisualizationRequestInputTrigger {
    #[serde(rename = "automatic")]
    Automatic,
    #[serde(rename = "explicit")]
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchOpenInputVersionMode {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "historical")]
    Historical,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SettingsUpdateInputTheme {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PackageExportInputFormat {
    #[serde(rename = "classroom")]
    Classroom,
    #[serde(rename = "pdf")]
    Pdf,
    #[serde(rename = "markdown")]
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommandName {
    #[serde(rename = "state.open")]
    StateOpen,
    #[serde(rename = "state.ack")]
    StateAck,
    #[serde(rename = "state.replay")]
    StateReplay,
    #[serde(rename = "state.close")]
    StateClose,
    #[serde(rename = "course.create")]
    CourseCreate,
    #[serde(rename = "lesson.create")]
    LessonCreate,
    #[serde(rename = "lesson.update")]
    LessonUpdate,
    #[serde(rename = "recording.start")]
    RecordingStart,
    #[serde(rename = "recording.pause")]
    RecordingPause,
    #[serde(rename = "recording.resume")]
    RecordingResume,
    #[serde(rename = "recording.stop")]
    RecordingStop,
    #[serde(rename = "recording.transfer")]
    RecordingTransfer,
    #[serde(rename = "recording.import")]
    RecordingImport,
    #[serde(rename = "recording.align")]
    RecordingAlign,
    #[serde(rename = "content.get")]
    ContentGet,
    #[serde(rename = "content.patch")]
    ContentPatch,
    #[serde(rename = "content.undo")]
    ContentUndo,
    #[serde(rename = "edit.begin")]
    EditBegin,
    #[serde(rename = "edit.end")]
    EditEnd,
    #[serde(rename = "correction.draft")]
    CorrectionDraft,
    #[serde(rename = "correction.accept")]
    CorrectionAccept,
    #[serde(rename = "correction.reject")]
    CorrectionReject,
    #[serde(rename = "correction.accept_batch")]
    CorrectionAcceptBatch,
    #[serde(rename = "note.reorganize")]
    NoteReorganize,
    #[serde(rename = "note.add_selection")]
    NoteAddSelection,
    #[serde(rename = "language.select")]
    LanguageSelect,
    #[serde(rename = "source.resolve")]
    SourceResolve,
    #[serde(rename = "source.diff")]
    SourceDiff,
    #[serde(rename = "definition.resolve")]
    DefinitionResolve,
    #[serde(rename = "audio.play")]
    AudioPlay,
    #[serde(rename = "audio.seek")]
    AudioSeek,
    #[serde(rename = "audio.stop")]
    AudioStop,
    #[serde(rename = "task.cancel")]
    TaskCancel,
    #[serde(rename = "task.retry")]
    TaskRetry,
    #[serde(rename = "identity.reserve")]
    IdentityReserve,
    #[serde(rename = "snapshot.build")]
    SnapshotBuild,
    #[serde(rename = "task.enqueue")]
    TaskEnqueue,
    #[serde(rename = "task.checkpoint")]
    TaskCheckpoint,
    #[serde(rename = "result.submit")]
    ResultSubmit,
    #[serde(rename = "proposal.apply")]
    ProposalApply,
    #[serde(rename = "resource.read")]
    ResourceRead,
    #[serde(rename = "workspace.open_course")]
    WorkspaceOpenCourse,
    #[serde(rename = "view.open")]
    ViewOpen,
    #[serde(rename = "view.save_state")]
    ViewSaveState,
    #[serde(rename = "view.move")]
    ViewMove,
    #[serde(rename = "view.ready")]
    ViewReady,
    #[serde(rename = "view.maximize")]
    ViewMaximize,
    #[serde(rename = "view.restore")]
    ViewRestore,
    #[serde(rename = "view.minimize")]
    ViewMinimize,
    #[serde(rename = "view.close")]
    ViewClose,
    #[serde(rename = "layout.resize")]
    LayoutResize,
    #[serde(rename = "material.import")]
    MaterialImport,
    #[serde(rename = "material.convert")]
    MaterialConvert,
    #[serde(rename = "material.page_result")]
    MaterialPageResult,
    #[serde(rename = "material.reparse")]
    MaterialReparse,
    #[serde(rename = "material.correct")]
    MaterialCorrect,
    #[serde(rename = "material.detach")]
    MaterialDetach,
    #[serde(rename = "material.delete_preview")]
    MaterialDeletePreview,
    #[serde(rename = "material.delete_confirm")]
    MaterialDeleteConfirm,
    #[serde(rename = "material_understand")]
    MaterialUnderstand,
    #[serde(rename = "visualization.request")]
    VisualizationRequest,
    #[serde(rename = "visualization.evaluate")]
    VisualizationEvaluate,
    #[serde(rename = "visualization.validate")]
    VisualizationValidate,
    #[serde(rename = "visualization.patch")]
    VisualizationPatch,
    #[serde(rename = "visualization.retry")]
    VisualizationRetry,
    #[serde(rename = "search.open")]
    SearchOpen,
    #[serde(rename = "search.query")]
    SearchQuery,
    #[serde(rename = "search.page")]
    SearchPage,
    #[serde(rename = "search.locate")]
    SearchLocate,
    #[serde(rename = "search.refresh")]
    SearchRefresh,
    #[serde(rename = "search.close")]
    SearchClose,
    #[serde(rename = "catchup.request")]
    CatchupRequest,
    #[serde(rename = "context.retrieve")]
    ContextRetrieve,
    #[serde(rename = "catchup.display_ack")]
    CatchupDisplayAck,
    #[serde(rename = "progress.calibrate")]
    ProgressCalibrate,
    #[serde(rename = "qa.ask")]
    QaAsk,
    #[serde(rename = "settings.update")]
    SettingsUpdate,
    #[serde(rename = "schedule.upsert")]
    ScheduleUpsert,
    #[serde(rename = "package.export")]
    PackageExport,
    #[serde(rename = "package.restore")]
    PackageRestore,
    #[serde(rename = "app.exit")]
    AppExit,
    #[serde(rename = "visualization.discover")]
    VisualizationDiscover,
    #[serde(rename = "visualization.build")]
    VisualizationBuild,
    #[serde(rename = "search.projection_submit")]
    SearchProjectionSubmit,
    #[serde(rename = "search.rebuild")]
    SearchRebuild,
    #[serde(rename = "catchup.generate")]
    CatchupGenerate,
    #[serde(rename = "catchup.validate")]
    CatchupValidate,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommandActor {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "ai")]
    Ai,
    #[serde(rename = "engine")]
    Engine,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommandResultStatus {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProposedOperationOperation {
    #[serde(rename = "text_insert")]
    TextInsert,
    #[serde(rename = "text_delete")]
    TextDelete,
    #[serde(rename = "text_replace")]
    TextReplace,
    #[serde(rename = "cell_set")]
    CellSet,
    #[serde(rename = "node_set")]
    NodeSet,
    #[serde(rename = "edge_set")]
    EdgeSet,
    #[serde(rename = "move")]
    Move,
    #[serde(rename = "resize")]
    Resize,
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "parameter_set")]
    ParameterSet,
    #[serde(rename = "view_default_set")]
    ViewDefaultSet,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AsrRawCandidateState {
    #[serde(rename = "provisional")]
    Provisional,
    #[serde(rename = "final")]
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EngineRequestName {
    #[serde(rename = "handshake")]
    Handshake,
    #[serde(rename = "convert")]
    Convert,
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "resume")]
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EngineResponseKind {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "checkpoint")]
    Checkpoint,
    #[serde(rename = "page_result")]
    PageResult,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "error")]
    Error,
}

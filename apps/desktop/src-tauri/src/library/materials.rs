//! Managed originals and atomic acceptance of the material sidecar's immutable
//! page/object package. Only host-issued paths enter the sidecar input.
use super::{resources, snapshots};
use crate::{
    search::{self, PreparedProjection},
    storage::{
        self, commit_entity, commit_version, emit, encode, head, io_error, lifecycle, sql_error,
        Result, Store,
    },
    tasks::TaskManager,
};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};
struct CopiedFiles {
    root: PathBuf,
    paths: Vec<String>,
    keep: bool,
}
impl Drop for CopiedFiles {
    fn drop(&mut self) {
        if !self.keep {
            for path in &self.paths {
                let _ = fs::remove_file(self.root.join(path));
            }
        }
    }
}

/// Host-to-engine capability. Do not forward input paths to the WebView, outbox,
/// snapshots or backup. UI receives the task/material identifiers only.
#[derive(Clone)]
pub struct MaterialJob {
    pub task: Task,
    pub input: MaterialConvertInput,
    pub material_id: String,
    pub source_version_id: String,
    pub parse_version_id: String,
    pub parent_parse_version_id: Option<String>,
}
impl MaterialJob {
    /// E's JSONL envelope carries host-derived identity coordinates alongside
    /// the common conversion input. No identity is derived from a filename.
    pub fn engine_request(&self) -> Result<EngineRequest> {
        let mut payload = storage::value(&self.input)?;
        let object = payload
            .as_object_mut()
            .ok_or_else(|| error(ErrorCode::InvalidResult, "Invalid engine conversion input"))?;
        object.insert("material_id".into(), json!(self.material_id));
        object.insert("source_version_id".into(), json!(self.source_version_id));
        object.insert("parse_version_id".into(), json!(self.parse_version_id));
        object.insert(
            "parent_parse_version_id".into(),
            json!(self.parent_parse_version_id),
        );
        object.insert(
            "source_hash".into(),
            json!(self
                .input
                .snapshot
                .sources
                .first()
                .map(|s| s.content_hash.clone())),
        );
        Ok(EngineRequest {
            contract_version: CONTRACT_VERSION.into(),
            request_id: storage::id(),
            name: EngineRequestName::Convert,
            task_id: Some(self.task.task_id.clone()),
            payload,
        })
    }
}
pub struct MaterialService {
    store: Store,
    accept_gate: Mutex<()>,
    jobs: Mutex<BTreeMap<String, MaterialJob>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StagedPublication {
    sequence: u64,
    parse_version_id: String,
    manifest_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StagedResource {
    relative_path: String,
    sha256: String,
    media_type: String,
    byte_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MaterialBridgeState {
    material_id: String,
    source_version_id: String,
    source_hash: String,
    final_parse_version_id: String,
    base_parent_parse_version_id: Option<String>,
    requested_pages: Vec<u64>,
    next_publication_sequence: u64,
    head_parse_version_id: Option<String>,
    state: String,
    accepted_publications: Vec<StagedPublication>,
    resources: BTreeMap<String, StagedResource>,
    search_coverage: Option<ExtractionCoverage>,
    progress: Value,
    completed_progress_hash: Option<String>,
}

fn required_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            let _ = key;
            error(ErrorCode::InvalidResult, "Material progress string is missing")
        })
}

fn optional_string(value: &Value, key: &str) -> Result<Option<String>> {
    match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                let _ = key;
                error(ErrorCode::InvalidResult, "Material progress string is invalid")
            }),
    }
}

fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            let _ = key;
            error(ErrorCode::InvalidResult, "Material progress number is missing")
        })
}

fn manifest_from_payload(payload: &Value) -> Result<MaterialManifest> {
    let value = payload
        .get("manifest")
        .cloned()
        .ok_or_else(|| error(ErrorCode::InvalidResult, "Material page result has no manifest"))?;
    let manifest: MaterialManifest = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            ErrorCode::InvalidResult,
            "Material page result manifest is invalid",
        )
    })?;
    validate("MaterialManifest", &storage::value(&manifest)?)?;
    Ok(manifest)
}

fn progress_from_payload(payload: &Value) -> Result<Value> {
    let progress = payload
        .get("progress")
        .cloned()
        .ok_or_else(|| error(ErrorCode::InvalidResult, "Material response has no progress"))?;
    if !progress.is_object() {
        return Err(error(
            ErrorCode::InvalidResult,
            "Material response progress is not an object",
        ));
    }
    Ok(progress)
}

fn bridge_from_task(task: &Task, snapshot: &InputSnapshot) -> Result<MaterialBridgeState> {
    let source = snapshot
        .sources
        .iter()
        .find(|source| source.entity_id == task.target_id)
        .ok_or_else(|| error(ErrorCode::InvalidResult, "Material snapshot source is missing"))?;
    let final_parse_version_id = task
        .checkpoint
        .get("parse_version_id")
        .and_then(Value::as_str)
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Material task parse identity is missing"))?;
    let base_parent_parse_version_id = match task.checkpoint.get("parent_parse_version_id") {
        Some(Value::Null) | None => None,
        Some(value) => Some(value.as_str().ok_or_else(|| {
            error(
                ErrorCode::CorruptAsset,
                "Material task parent parse identity is invalid",
            )
        })?.to_owned()),
    };
    if let Some(value) = task.checkpoint.get("material_bridge") {
        let bridge: MaterialBridgeState = serde_json::from_value(value.clone()).map_err(|_| {
            error(
                ErrorCode::CorruptAsset,
                "Material task bridge checkpoint is invalid",
            )
        })?;
        if bridge.material_id != task.target_id
            || bridge.source_version_id != source.revision_id
            || bridge.source_hash != source.content_hash
            || bridge.final_parse_version_id != final_parse_version_id
            || bridge.base_parent_parse_version_id != base_parent_parse_version_id
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material task bridge identity changed",
            ));
        }
        return Ok(bridge);
    }
    Ok(MaterialBridgeState {
        material_id: task.target_id.clone(),
        source_version_id: source.revision_id.clone(),
        source_hash: source.content_hash.clone(),
        final_parse_version_id: final_parse_version_id.into(),
        base_parent_parse_version_id,
        requested_pages: Vec::new(),
        next_publication_sequence: 0,
        head_parse_version_id: task
            .checkpoint
            .get("parent_parse_version_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        state: "running".into(),
        accepted_publications: Vec::new(),
        resources: BTreeMap::new(),
        search_coverage: None,
        progress: json!({}),
        completed_progress_hash: None,
    })
}

fn checkpoint_with_bridge(
    task: &Task,
    bridge: &MaterialBridgeState,
) -> Result<BTreeMap<String, Value>> {
    let mut checkpoint = task.checkpoint.clone();
    checkpoint.insert(
        "material_bridge".into(),
        serde_json::to_value(bridge).map_err(|_| {
            error(
                ErrorCode::InvalidResult,
                "Material bridge checkpoint could not be encoded",
            )
        })?,
    );
    Ok(checkpoint)
}

fn validate_coverage(
    previous: Option<&ExtractionCoverage>,
    current: &ExtractionCoverage,
) -> Result<()> {
    if let Some(previous) = previous {
        if previous.requested_pages != current.requested_pages
            || previous
                .processed_pages
                .iter()
                .any(|page| !current.processed_pages.contains(page))
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material search coverage moved backwards",
            ));
        }
        if previous.state == ExtractionCoverageState::Complete
            && current.state != ExtractionCoverageState::Complete
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material search coverage was downgraded",
            ));
        }
    }
    if current.state == ExtractionCoverageState::Complete
        && current
            .requested_pages
            .iter()
            .any(|page| !current.processed_pages.contains(page))
    {
        return Err(error(
            ErrorCode::InvalidResult,
            "Complete material search coverage omits a requested page",
        ));
    }
    Ok(())
}

fn verify_staged_resource(stage: &Path, resource: &Resource) -> Result<StagedResource> {
    let mut file = resources::open_relative(stage, Path::new(&resource.relative_path))?;
    validate_preview(&mut file, &resource.media_type)?;
    let mut digest = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .ok_or_else(|| error(ErrorCode::CorruptAsset, "Material resource length overflow"))?;
        if length > 64 * 1024 * 1024 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Material resource exceeds the staging budget",
            ));
        }
        digest.update(&buffer[..count]);
    }
    let sha256 = format!("{:x}", digest.finalize());
    if length != resource.byte_length || sha256 != resource.sha256 {
        return Err(error(
            ErrorCode::CorruptAsset,
            "Material staged resource hash or size mismatch",
        ));
    }
    Ok(StagedResource {
        relative_path: resource.relative_path.clone(),
        sha256: resource.sha256.clone(),
        media_type: resource.media_type.clone(),
        byte_length: resource.byte_length,
    })
}

fn validate_publication_list(
    progress: &Value,
    bridge: &MaterialBridgeState,
    final_parse_version_id: Option<&str>,
) -> Result<()> {
    let list = progress
        .get("published_parse_versions")
        .and_then(Value::as_array)
        .ok_or_else(|| error(ErrorCode::InvalidResult, "Material publication list is missing"))?;
    let expected = bridge.accepted_publications.len() + usize::from(final_parse_version_id.is_some());
    if list.len() != expected {
        return Err(error(
            ErrorCode::StaleInput,
            "Material publication list is out of order",
        ));
    }
    for (value, publication) in list.iter().zip(&bridge.accepted_publications) {
        if value.as_str() != Some(publication.parse_version_id.as_str()) {
            return Err(error(
                ErrorCode::StaleInput,
                "Material publication parent chain changed",
            ));
        }
    }
    if let Some(final_parse_version_id) = final_parse_version_id {
        if list.last().and_then(Value::as_str) != Some(final_parse_version_id) {
            return Err(error(
                ErrorCode::StaleInput,
                "Material final publication identity changed",
            ));
        }
    }
    Ok(())
}

fn validate_progress_base(
    progress: &Value,
    task: &Task,
    snapshot: &InputSnapshot,
    bridge: &MaterialBridgeState,
    state: &str,
) -> Result<u64> {
    if progress.get("contract_version").and_then(Value::as_str) != Some(CONTRACT_VERSION)
        || required_string(progress, "task_id")? != task.task_id
        || required_string(progress, "material_id")? != task.target_id
        || required_string(progress, "source_version_id")? != bridge.source_version_id
        || required_string(progress, "source_hash")? != bridge.source_hash
        || required_string(progress, "parse_version_id")? != bridge.final_parse_version_id
        || optional_string(progress, "parent_parse_version_id")?
            != bridge.base_parent_parse_version_id
        || required_string(progress, "state")? != state
    {
        return Err(error(
            ErrorCode::StaleInput,
            "Material progress identity does not match the frozen task",
        ));
    }
    if !snapshot.sources.iter().any(|source| {
        source.entity_id == task.target_id
            && source.revision_id == bridge.source_version_id
            && source.content_hash == bridge.source_hash
    }) {
        return Err(error(
            ErrorCode::InvalidResult,
            "Material progress source changed",
        ));
    }
    Ok(required_u64(progress, "next_publication_sequence")?)
}

fn validate_publication_parent(
    current_parse_version: Option<&str>,
    parent_parse_version_id: Option<&str>,
    parent_exists: bool,
) -> Result<()> {
    match (current_parse_version, parent_parse_version_id) {
        (None, None) => Ok(()),
        (Some(current), Some(parent)) if current == parent && parent_exists => Ok(()),
        (Some(_), None) => Err(error(
            ErrorCode::StaleInput,
            "Published material requires its current parse parent",
        )),
        _ => Err(error(
            ErrorCode::StaleInput,
            "Material parse parent is not the current published version",
        )),
    }
}

fn validate_existing_parse_identity(
    existing: (&str, &str, Option<&str>, &str),
    material_id: &str,
    source_version_id: &str,
    parent_parse_version_id: Option<&str>,
    content_hash: &str,
) -> Result<()> {
    if existing.0 == material_id
        && existing.1 == source_version_id
        && existing.2 == parent_parse_version_id
        && existing.3 == content_hash
    {
        return Ok(());
    }
    Err(error(
        ErrorCode::IdempotencyMismatch,
        "Parse version ID reused for different package",
    ))
}

fn current_publication_parent(
    c: &Connection,
    material_id: &str,
    source_version_id: &str,
) -> Result<(Option<String>, bool)> {
    let current = head(c, material_id)?;
    let current_parse = current["parse_version_id"].as_str().map(str::to_owned);
    let parent_exists = if let Some(parent) = current_parse.as_deref() {
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM parse_versions WHERE parse_version_id=?1 AND material_id=?2 AND source_version_id=?3)",
            params![parent, material_id, source_version_id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(sql_error)?
    } else {
        false
    };
    Ok((current_parse, parent_exists))
}

impl MaterialService {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            accept_gate: Mutex::new(()),
            jobs: Mutex::new(BTreeMap::new()),
        }
    }
    /// C calls this after the native file picker grants access. This is not a
    /// WebView command that accepts arbitrary filesystem paths.
    pub fn import_original(
        &self,
        path: &Path,
        lesson: &str,
        purpose: MaterialImportInputPurpose,
        pages: Vec<u64>,
    ) -> Result<MaterialJob> {
        let source = File::open(path).map_err(io_error)?;
        let display_name = path.file_name().and_then(|x| x.to_str()).ok_or_else(|| {
            error(
                ErrorCode::InvalidArgument,
                "Material source name is invalid",
            )
        })?;
        self.import_original_file(source, display_name, lesson, purpose, pages)
    }
    /// Import from a descriptor opened by the native picker while its
    /// security-scoped access was active. The descriptor, rather than a path,
    /// crosses into the worker so the WebView never supplies filesystem input.
    pub fn import_original_file(
        &self,
        source: File,
        display_name: &str,
        lesson: &str,
        purpose: MaterialImportInputPurpose,
        pages: Vec<u64>,
    ) -> Result<MaterialJob> {
        if self.store.read(|c| lifecycle(c, lesson))? != "active" {
            return Err(error(ErrorCode::SourceDeleted, "Import lesson unavailable"));
        }
        if display_name.is_empty()
            || Path::new(display_name).file_name().and_then(|x| x.to_str()) != Some(display_name)
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Material source name is invalid",
            ));
        }
        let extension = Path::new(display_name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let media = match extension.as_str() {
            "pdf" => "application/pdf",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "txt" | "md" | "markdown" => "text/plain",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => {
                return Err(error(
                    ErrorCode::UnsupportedCapability,
                    "Material type is unsupported by the local importer",
                ))
            }
        };
        if !source.metadata().map_err(io_error)?.is_file() {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Material source is not a regular file",
            ));
        }
        let material = storage::id();
        let source_version = storage::id();
        let resource = storage::id();
        let attachment = storage::id();
        let relative = format!("resources/{resource}.{extension}");
        let (sha, length) =
            resources::persist_bytes(source, self.store.root(), &relative, 512 * 1024 * 1024)?;
        let display = display_name.to_owned();
        let result=self.store.write(|t| {
            if lifecycle(t,lesson)?!="active"{return Err(error(ErrorCode::SourceDeleted,"Import lesson was deleted"));}
            let body=commit_entity(t,&material,"material",json!({"material_id":material,"source_version_id":source_version,"resource_id":resource,"file_type":extension,"display_name":display,"parse_version_id":null,"revision_id":storage::id()}),"content.committed",json!({"kind":"lesson","lesson_id":lesson}),None,None)?;
            t.execute("INSERT INTO resources VALUES(?1,?2,?3,?4,?5,'ready')",params![resource,relative,sha,media,length]).map_err(sql_error)?;
            t.execute("INSERT INTO materials VALUES(?1,?2,?3,?4,?5)",params![material,source_version,resource,extension,display]).map_err(sql_error)?;t.execute("INSERT INTO resource_owners VALUES(?1,?2)",params![resource,material]).map_err(sql_error)?;
            commit_entity(t,&attachment,"material_attachment",json!({"attachment_id":attachment,"material_id":material,"lesson_id":lesson,"purpose":purpose,"pages":pages,"lifecycle":"active","revision_id":storage::id()}),"content.committed",json!({"kind":"lesson","lesson_id":lesson}),None,None)?;
            t.execute("INSERT INTO material_attachments VALUES(?1,?2,?3,NULL,?4,?5,'active')",params![attachment,material,lesson,storage::value(&purpose)?.as_str().unwrap_or(""),encode(&pages)?]).map_err(sql_error)?;
            let mut lesson_body=head(t,lesson)?;lesson_body["material_scope"].as_array_mut().ok_or_else(||error(ErrorCode::CorruptAsset,"Invalid lesson material scope"))?.push(json!(attachment));
            let updated=commit_entity(t,lesson,"lesson",lesson_body,"content.committed",json!({"kind":"lesson","lesson_id":lesson}),None,None)?;t.execute("UPDATE lessons SET attachment_set_revision=?2 WHERE lesson_id=?1",params![lesson,updated["revision_id"].as_str()]).map_err(sql_error)?;
            Ok(body)
        });
        if result.is_err() {
            let _ = fs::remove_file(self.store.root().join(&relative));
            result?;
        }
        self.prepare(
            &material,
            if purpose == MaterialImportInputPurpose::Textbook {
                MaterialConvertInputProfile::Light
            } else {
                MaterialConvertInputProfile::Deep
            },
            pages,
            Vec::new(),
            600_000,
        )
    }
    pub fn prepare(
        &self,
        material: &str,
        profile: MaterialConvertInputProfile,
        pages: Vec<u64>,
        corrections: Vec<MaterialCorrection>,
        budget_ms: u64,
    ) -> Result<MaterialJob> {
        if budget_ms == 0 || budget_ms > 900_000 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Invalid conversion budget",
            ));
        }
        let (source_version,relative,sha,parent_parse_version_id)=self.store.read(|c|{
            let (source_version,relative,sha)=c.query_row("SELECT m.source_version_id,r.relative_path,r.sha256 FROM materials m JOIN resources r ON r.resource_id=m.resource_id JOIN entities e ON e.entity_id=m.material_id WHERE m.material_id=?1 AND r.state='ready' AND e.lifecycle='active'",[material],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional().map_err(sql_error)?.ok_or_else(||error(ErrorCode::SourceUnavailable,"Managed original unavailable"))?;
            let current=head(c,material)?;
            let parent=current["parse_version_id"].as_str().map(str::to_owned);
            Ok((source_version,relative,sha,parent))
        })?;
        let build = SnapshotBuildInput {
            purpose: TaskKind::MaterialConvert,
            target_id: material.into(),
            evidence_cutoff: Vec::new(),
            language_tier: None,
        };
        let mut snapshot = self.store.read(|c| snapshots::assemble(c, &build, None))?;
        snapshot.executor_id = "focaus.material".into();
        snapshot.executor_version = "0.1.0".into();
        snapshot.parameters_hash = canonical_hash(&json!([profile, pages, corrections]));
        snapshot.sources = vec![SourceRef {
            entity_id: material.into(),
            revision_id: source_version.clone(),
            content_hash: sha,
            selector: Selector::V4(WholeSelector {
                kind: "whole".into(),
            }),
        }];
        validate("InputSnapshot", &storage::value(&snapshot)?)?;
        self.store.write(|t| {
            snapshots::check_frozen(t, &snapshot)?;
            snapshots::persist(t, &snapshot)
        })?;
        let task_id = storage::id();
        let parse = storage::id();
        let staging = resources::staging_root(self.store.root(), &task_id)?;
        let task = Task {
            task_id: task_id.clone(),
            kind: TaskKind::MaterialConvert,
            target_id: material.into(),
            snapshot_id: snapshot.snapshot_id.clone(),
            state: TaskState::Queued,
            priority: 4,
            attempt: 1,
            deadline_at: None,
            idempotency_key: storage::id(),
            checkpoint: BTreeMap::from([
                ("parse_version_id".into(), json!(parse)),
                ("source_version_id".into(), json!(source_version)),
                (
                    "parent_parse_version_id".into(),
                    json!(parent_parse_version_id),
                ),
                ("profile".into(), json!(profile)),
                ("pages".into(), json!(pages)),
            ]),
            error: None,
            provider_bindings: Vec::new(),
            created_at: storage::now(),
            updated_at: storage::now(),
            model_id: None,
            region: None,
            prompt_version: None,
            parameters_hash: snapshot.parameters_hash.clone(),
        };
        TaskManager::new(self.store.clone()).enqueue(task.clone())?;
        let input = MaterialConvertInput {
            snapshot,
            source_handle: self
                .store
                .root()
                .join(relative)
                .to_string_lossy()
                .into_owned(),
            staging_handle: staging.to_string_lossy().into_owned(),
            identity_pool: BTreeMap::from([("parse_version:current".into(), parse.clone())]),
            profile,
            pages,
            corrections,
            budget_ms,
        };
        let job = MaterialJob {
            task,
            input,
            material_id: material.into(),
            source_version_id: source_version,
            parse_version_id: parse,
            parent_parse_version_id,
        };
        self.jobs
            .lock()
            .map_err(|_| {
                error(
                    ErrorCode::InvalidResult,
                    "Material job registry unavailable",
                )
            })?
            .insert(job.task.task_id.clone(), job.clone());
        Ok(job)
    }
    /// Return a host-owned job for the safe sidecar command. The caller only
    /// supplies the task ID; source and staging paths remain inside this map.
    pub fn job(&self, task_id: &str) -> Result<MaterialJob> {
        self.jobs
            .lock()
            .map_err(|_| {
                error(
                    ErrorCode::InvalidResult,
                    "Material job registry unavailable",
                )
            })?
            .get(task_id)
            .cloned()
            .ok_or_else(|| {
                error(
                    ErrorCode::NotFound,
                    "Material task is not attached to this host",
                )
            })
    }
    fn material_context(
        &self,
        task_id: &str,
        snapshot_id: &str,
    ) -> Result<(Task, InputSnapshot, MaterialBridgeState, PathBuf)> {
        let task = TaskManager::new(self.store.clone()).get(task_id)?;
        if task.kind != TaskKind::MaterialConvert && task.kind != TaskKind::MaterialReparse {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Task is not a material conversion",
            ));
        }
        if task.snapshot_id != snapshot_id
            || !matches!(
                task.state,
                TaskState::Queued | TaskState::Running | TaskState::Interrupted
            )
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material task input or state changed",
            ));
        }
        let snapshot = self.store.read(|c| {
            let snapshot = snapshots::load(c, snapshot_id)?;
            snapshots::check_frozen(c, &snapshot)?;
            Ok(snapshot)
        })?;
        let bridge = bridge_from_task(&task, &snapshot)?;
        let stage = resources::staging_root(self.store.root(), task_id)?;
        Ok((task, snapshot, bridge, stage))
    }

    fn validate_staged_resources(
        &self,
        stage: &Path,
        manifest: &MaterialManifest,
        bridge: &MaterialBridgeState,
    ) -> Result<BTreeMap<String, StagedResource>> {
        let mut resources_by_id = BTreeMap::new();
        for resource in &manifest.resources {
            let staged = verify_staged_resource(stage, resource)?;
            if resources_by_id
                .insert(resource.resource_id.clone(), staged.clone())
                .is_some()
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Material page result repeats a resource identity",
                ));
            }
            if let Some(previous) = bridge.resources.get(&resource.resource_id) {
                if previous != &staged {
                    return Err(error(
                        ErrorCode::Conflict,
                        "Material page result overwrites a staged resource",
                    ));
                }
            }
            if resources_by_id.iter().any(|(other_id, other)| {
                other_id != &resource.resource_id && other.relative_path == staged.relative_path
            }) || bridge.resources.iter().any(|(other_id, other)| {
                other_id != &resource.resource_id && other.relative_path == staged.relative_path
            }) {
                return Err(error(
                    ErrorCode::Conflict,
                    "Material page result aliases a resource path",
                ));
            }
            let existing = self.store.read(|c| {
                c.query_row(
                    "SELECT sha256,byte_length,relative_path,media_type FROM resources WHERE resource_id=?1",
                    [&resource.resource_id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, u64>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(sql_error)
            })?;
            if existing.is_some_and(|existing| {
                existing
                    != (
                        resource.sha256.clone(),
                        resource.byte_length,
                        resource.relative_path.clone(),
                        resource.media_type.clone(),
                    )
            }) {
                return Err(error(
                    ErrorCode::Conflict,
                    "Material page result conflicts with a managed resource",
                ));
            }
            let path_owner: Option<String> = self.store.read(|c| {
                c.query_row(
                    "SELECT resource_id FROM resources WHERE relative_path=?1 AND resource_id<>?2 LIMIT 1",
                    params![resource.relative_path, resource.resource_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql_error)
            })?;
            if path_owner.is_some() {
                return Err(error(
                    ErrorCode::Conflict,
                    "Material page result reuses a managed resource path",
                ));
            }
        }
        for (resource_id, resource) in &bridge.resources {
            if let Some(current) = resources_by_id.get(resource_id) {
                if current != resource {
                    return Err(error(
                        ErrorCode::Conflict,
                        "Material page result changed an earlier resource",
                    ));
                }
            }
        }
        Ok(resources_by_id)
    }

    /// Validate and stage one sidecar page snapshot. This updates only the
    /// task checkpoint; material entities, parse_versions and search indexes
    /// remain untouched until the final completed manifest is accepted.
    pub fn accept_page_result(
        &self,
        task_id: &str,
        snapshot_id: &str,
        payload: &Value,
    ) -> Result<Value> {
        let _gate = self
            .accept_gate
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Material acceptance unavailable"))?;
        let manifest = manifest_from_payload(payload)?;
        let page: Page = serde_json::from_value(
            payload
                .get("page")
                .cloned()
                .ok_or_else(|| error(ErrorCode::InvalidResult, "Material page result has no page"))?,
        )
        .map_err(|_| error(ErrorCode::InvalidResult, "Material page result page is invalid"))?;
        let progress = progress_from_payload(payload)?;
        let publication_sequence = required_u64(payload, "publication_sequence")?;
        let snapshot_parse_version_id = required_string(payload, "snapshot_parse_version_id")?;
        if required_string(payload, "parse_version_id")? != snapshot_parse_version_id
            || manifest.parse_version_id != snapshot_parse_version_id
        {
            return Err(error(
                ErrorCode::InvalidResult,
                "Material page snapshot identity is inconsistent",
            ));
        }
        let (task, snapshot, mut bridge, stage) = self.material_context(task_id, snapshot_id)?;
        if manifest.material_id != task.target_id
            || manifest.source_version_id != bridge.source_version_id
            || manifest.source_hash != bridge.source_hash
        {
            return Err(error(
                ErrorCode::InvalidResult,
                "Material page result changed the immutable original source",
            ));
        }
        if bridge.state != "running" {
            return Err(error(
                ErrorCode::StaleInput,
                "Material page result arrived after completion",
            ));
        }
        if publication_sequence < bridge.next_publication_sequence {
            let hash = canonical_hash(&storage::value(&manifest)?);
            if bridge.accepted_publications.iter().any(|publication| {
                publication.sequence == publication_sequence
                    && publication.parse_version_id == manifest.parse_version_id
                    && publication.manifest_hash == hash
            }) {
                return Ok(json!({
                    "accepted":true,
                    "duplicate":true,
                    "publication_sequence":publication_sequence,
                    "snapshot_parse_version_id":manifest.parse_version_id
                }));
            }
            return Err(error(
                ErrorCode::StaleInput,
                "Material page result sequence is older than staged progress",
            ));
        }
        if publication_sequence > bridge.next_publication_sequence {
            return Err(error(
                ErrorCode::StaleInput,
                "Material page result sequence skipped staged progress",
            ));
        }
        let next_sequence = validate_progress_base(&progress, &task, &snapshot, &bridge, "running")?;
        if next_sequence != publication_sequence + 1
            || required_string(&progress, "publication_parse_version_id")?
                != manifest.parse_version_id
            || optional_string(&progress, "publication_parent_parse_version_id")?
                != manifest.parent_parse_version_id
            || optional_string(&progress, "head_parse_version_id")?
                != Some(manifest.parse_version_id.clone())
            || manifest.parent_parse_version_id != bridge.head_parse_version_id
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material page result parent chain changed",
            ));
        }
        let manifest_hash = canonical_hash(&storage::value(&manifest)?);
        let mut publication_chain = bridge.clone();
        publication_chain.accepted_publications.push(StagedPublication {
            sequence: publication_sequence,
            parse_version_id: manifest.parse_version_id.clone(),
            manifest_hash: manifest_hash.clone(),
        });
        validate_publication_list(&progress, &publication_chain, None)?;
        if bridge.requested_pages.is_empty() {
            bridge.requested_pages = manifest.requested_pages.clone();
        } else if bridge.requested_pages != manifest.requested_pages {
            return Err(error(
                ErrorCode::StaleInput,
                "Material requested page set changed",
            ));
        }
        validate_coverage(bridge.search_coverage.as_ref(), &manifest.search_coverage)?;
        let resources_by_id = self.validate_staged_resources(&stage, &manifest, &bridge)?;
        if !manifest.pages.iter().any(|candidate| candidate == &page) {
            return Err(error(
                ErrorCode::InvalidResult,
                "Material page result is outside its manifest",
            ));
        }
        bridge.accepted_publications.push(StagedPublication {
            sequence: publication_sequence,
            parse_version_id: manifest.parse_version_id.clone(),
            manifest_hash,
        });
        bridge.next_publication_sequence = next_sequence;
        bridge.head_parse_version_id = Some(manifest.parse_version_id.clone());
        bridge.resources = resources_by_id;
        bridge.search_coverage = Some(manifest.search_coverage.clone());
        bridge.progress = progress;
        let safe_state = checkpoint_with_bridge(&task, &bridge)?;
        TaskManager::new(self.store.clone()).checkpoint_material(TaskCheckpointInput {
            task_id: task.task_id,
            attempt: task.attempt,
            cursor: format!("material-page:{publication_sequence}:{}", manifest.parse_version_id),
            safe_state,
        })?;
        Ok(json!({
            "accepted":true,
            "duplicate":false,
            "publication_sequence":publication_sequence,
            "snapshot_parse_version_id":manifest.parse_version_id,
            "search_projection":"staged"
        }))
    }

    /// Persist a sidecar checkpoint after validating its monotonic relation to
    /// the last staged page. A completed checkpoint is still only a marker;
    /// it never publishes a material head or search projection.
    pub fn record_checkpoint(
        &self,
        task_id: &str,
        attempt: u64,
        payload: &Value,
    ) -> Result<Task> {
        let _gate = self
            .accept_gate
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Material acceptance unavailable"))?;
        let progress = if payload.get("progress").is_some() {
            progress_from_payload(payload)?
        } else {
            payload.clone()
        };
        let task_snapshot_id = TaskManager::new(self.store.clone()).get(task_id)?.snapshot_id;
        let (task, snapshot, mut bridge, _) = self.material_context(task_id, &task_snapshot_id)?;
        if task.attempt != attempt {
            return Err(error(ErrorCode::StaleInput, "Material checkpoint attempt changed"));
        }
        let state = required_string(&progress, "state")?;
        let next_sequence = validate_progress_base(&progress, &task, &snapshot, &bridge, &state)?;
        if state == "running" {
            if bridge.state != "running"
                || next_sequence != bridge.next_publication_sequence
                || optional_string(&progress, "head_parse_version_id")?
                    != bridge.head_parse_version_id
            {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Material running checkpoint is out of order",
                ));
            }
            let expected_publication = bridge.head_parse_version_id.as_deref().ok_or_else(|| {
                error(
                    ErrorCode::InvalidResult,
                    "Material running checkpoint has no staged publication",
                )
            })?;
            if required_string(&progress, "publication_parse_version_id")?
                != expected_publication
                || optional_string(&progress, "publication_parent_parse_version_id")?
                    != bridge
                        .accepted_publications
                        .last()
                        .and_then(|publication| {
                            bridge
                                .accepted_publications
                                .iter()
                                .find(|candidate| candidate.sequence + 1 == publication.sequence)
                                .map(|candidate| candidate.parse_version_id.clone())
                        })
            {
                // The page result already checked the parent chain. Keep this
                // checkpoint check narrow: the head and sequence are the
                // authoritative replay coordinates.
                return Err(error(
                    ErrorCode::StaleInput,
                    "Material running checkpoint publication changed",
                ));
            }
            validate_publication_list(&progress, &bridge, None)?;
            bridge.progress = progress;
        } else if state == "completed" {
            let progress_hash = canonical_hash(&progress);
            if bridge.state == "completed" {
                if bridge.next_publication_sequence == next_sequence
                    && bridge.head_parse_version_id.as_deref()
                        == Some(bridge.final_parse_version_id.as_str())
                    && bridge.completed_progress_hash.as_deref() == Some(progress_hash.as_str())
                {
                    return Ok(task);
                }
                return Err(error(
                    ErrorCode::StaleInput,
                    "Material completion checkpoint is not idempotent",
                ));
            }
            if next_sequence != bridge.next_publication_sequence + 1
                || optional_string(&progress, "head_parse_version_id")?
                    != Some(bridge.final_parse_version_id.clone())
                || required_string(&progress, "publication_parse_version_id")?
                    != bridge.final_parse_version_id
                || optional_string(&progress, "publication_parent_parse_version_id")?
                    != bridge.base_parent_parse_version_id
            {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Material completion checkpoint is out of order",
                ));
            }
            validate_publication_list(
                &progress,
                &bridge,
                Some(&bridge.final_parse_version_id),
            )?;
            bridge.next_publication_sequence = next_sequence;
            bridge.head_parse_version_id = Some(bridge.final_parse_version_id.clone());
            bridge.state = "completed".into();
            bridge.progress = progress;
            bridge.completed_progress_hash = Some(progress_hash);
        } else {
            return Err(error(
                ErrorCode::InvalidResult,
                "Material checkpoint state is invalid",
            ));
        }
        let safe_state = checkpoint_with_bridge(&task, &bridge)?;
        TaskManager::new(self.store.clone()).checkpoint_material(TaskCheckpointInput {
            task_id: task.task_id,
            attempt,
            cursor: format!("material-checkpoint:{}", bridge.next_publication_sequence),
            safe_state,
        })
    }

    /// Confirm the completed response against the staged checkpoint, then use
    /// the existing atomic publication gate for the final immutable manifest.
    pub fn accept_completed(
        &self,
        input: MaterialPageResultInput,
        payload: &Value,
    ) -> Result<Value> {
        let progress = progress_from_payload(payload)?;
        let (task, snapshot, bridge, _) = self.material_context(&input.task_id, &input.snapshot_id)?;
        if bridge.state != "completed"
            || bridge.completed_progress_hash.as_deref() != Some(canonical_hash(&progress).as_str())
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Completed material response has no matching checkpoint",
            ));
        }
        let next_sequence = validate_progress_base(&progress, &task, &snapshot, &bridge, "completed")?;
        if next_sequence != bridge.next_publication_sequence
            || optional_string(&progress, "head_parse_version_id")?
                != Some(bridge.final_parse_version_id.clone())
            || input.manifest.parse_version_id != bridge.final_parse_version_id
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Completed material manifest identity changed",
            ));
        }
        validate_publication_list(
            &progress,
            &bridge,
            Some(&bridge.final_parse_version_id),
        )?;
        self.accept(input)
    }

    /// The JSONL adapter passes the manifest, never a caller-supplied staging path.
    /// Resources are opened under the task's pinned staging directory capability.
    pub fn accept(&self, input: MaterialPageResultInput) -> Result<Value> {
        validate("MaterialPageResultInput", &storage::value(&input)?)?;
        let _gate = self
            .accept_gate
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Material acceptance unavailable"))?;
        let manifest = input.manifest;
        let task = TaskManager::new(self.store.clone()).get(&input.task_id)?;
        if task.kind != TaskKind::MaterialConvert && task.kind != TaskKind::MaterialReparse {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Task is not a material conversion",
            ));
        }
        if task.target_id != manifest.material_id
            || task.snapshot_id != input.snapshot_id
            || !matches!(
                task.state,
                TaskState::Queued | TaskState::Running | TaskState::Interrupted
            )
        {
            return Err(error(
                ErrorCode::StaleInput,
                "Material task input or state changed",
            ));
        }
        let stage = resources::staging_root(self.store.root(), &task.task_id)?;
        let snapshot = self.store.read(|c| {
            let snapshot = snapshots::load(c, &input.snapshot_id)?;
            snapshots::check_frozen(c, &snapshot)?;
            Ok(snapshot)
        })?;
        if !snapshot.sources.iter().any(|s| {
            s.entity_id == manifest.material_id
                && s.revision_id == manifest.source_version_id
                && s.content_hash == manifest.source_hash
        }) {
            return Err(error(
                ErrorCode::InvalidResult,
                "Manifest changed immutable original source",
            ));
        }
        if manifest.format != "focaus.material" || manifest.format_version != "2.0.0" {
            return Err(error(
                ErrorCode::UnsupportedVersion,
                "Material package format is unsupported",
            ));
        }
        let duplicate = self.store.read(|c| {
            c.query_row(
                "SELECT material_id,source_version_id,parent_parse_version_id,content_hash FROM parse_versions WHERE parse_version_id=?1",
                [&manifest.parse_version_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)
        })?;
        let manifest_hash = canonical_hash(&storage::value(&manifest)?);
        if let Some((material_id, source_version_id, parent_parse_version_id, hash)) = duplicate {
            validate_existing_parse_identity(
                (
                    &material_id,
                    &source_version_id,
                    parent_parse_version_id.as_deref(),
                    &hash,
                ),
                &manifest.material_id,
                &manifest.source_version_id,
                manifest.parent_parse_version_id.as_deref(),
                &manifest_hash,
            )?;
            return Ok(
                json!({"material_id":manifest.material_id,"parse_version_id":manifest.parse_version_id,"publication":"committed","duplicate":true}),
            );
        }
        let (current_parse_version, parent_exists) = self.store.read(|c| {
            current_publication_parent(c, &manifest.material_id, &manifest.source_version_id)
        })?;
        validate_publication_parent(
            current_parse_version.as_deref(),
            manifest.parent_parse_version_id.as_deref(),
            parent_exists,
        )?;
        let mut ids = BTreeSet::new();
        for id in manifest
            .pages
            .iter()
            .map(|p| &p.page_id)
            .chain(manifest.regions.iter().map(|r| &r.region_id))
            .chain(manifest.objects.iter().map(|o| object_id(o)))
            .chain(manifest.resources.iter().map(|r| &r.resource_id))
        {
            if !ids.insert(id.clone()) {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Duplicate material package identity",
                ));
            }
        }
        let object_values = manifest
            .objects
            .iter()
            .map(storage::value)
            .collect::<Result<Vec<_>>>()?;
        let object_ids = object_values
            .iter()
            .map(|o| o["object_id"].as_str().unwrap_or("").to_owned())
            .collect::<BTreeSet<_>>();
        let region_ids = manifest
            .regions
            .iter()
            .map(|r| r.region_id.clone())
            .collect::<BTreeSet<_>>();
        let page_ids = manifest
            .pages
            .iter()
            .map(|p| p.page_id.clone())
            .collect::<BTreeSet<_>>();
        let resource_ids = manifest
            .resources
            .iter()
            .map(|r| r.resource_id.clone())
            .collect::<BTreeSet<_>>();
        for page in &manifest.pages {
            if !page.width.is_finite()
                || page.width <= 0.
                || !page.height.is_finite()
                || page.height <= 0.
                || page.reading_order.iter().any(|id| !object_ids.contains(id))
                || page.region_ids.iter().any(|id| !region_ids.contains(id))
                || page
                    .preview_resource_id
                    .as_ref()
                    .is_some_and(|id| !resource_ids.contains(id))
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Material page closure is incomplete",
                ));
            }
        }
        for region in &manifest.regions {
            if !page_ids.contains(&region.page_id)
                || region.object_ids.iter().any(|id| !object_ids.contains(id))
                || region
                    .parent_region_id
                    .as_ref()
                    .is_some_and(|id| !region_ids.contains(id))
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Material region closure is incomplete",
                ));
            }
        }
        for object in &object_values {
            if object["children"].as_array().is_some_and(|ids| {
                ids.iter()
                    .any(|id| !id.as_str().is_some_and(|id| object_ids.contains(id)))
            }) || object["region_ids"].as_array().is_some_and(|ids| {
                ids.iter()
                    .any(|id| !id.as_str().is_some_and(|id| region_ids.contains(id)))
            }) {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Material object closure is incomplete",
                ));
            }
            for source in super::source_refs_in(object)? {
                if source.entity_id != manifest.material_id
                    || source.revision_id != manifest.source_version_id
                    || source.content_hash != manifest.source_hash
                {
                    return Err(error(
                        ErrorCode::InvalidResult,
                        "Material object source escaped its original",
                    ));
                }
                if let Selector::V2(selector) = source.selector {
                    if selector.parse_version_id != manifest.parse_version_id
                        || !page_ids.contains(&selector.page_id)
                        || !region_ids.contains(&selector.region_id)
                    {
                        return Err(error(
                            ErrorCode::InvalidResult,
                            "Material source anchor closure is incomplete",
                        ));
                    }
                }
            }
        }
        let mut groups: BTreeMap<String, Vec<SearchUnit>> = BTreeMap::new();
        for u in &manifest.search_units {
            let object = object_values
                .iter()
                .find(|o| o["object_id"] == u.owner_entity_id && o["revision_id"] == u.revision_id)
                .ok_or_else(|| {
                    error(
                        ErrorCode::InvalidResult,
                        "Search unit owner revision is outside package",
                    )
                })?;
            let _ = object;
            if u.content_hash != search_text_hash(&u.display_text) {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Extracted field UTF8 hash mismatch",
                ));
            }
            for anchor in std::iter::once(&u.anchor).chain(&u.alternate_anchors) {
                if anchor.owner_entity_id != u.owner_entity_id
                    || anchor.revision_id != u.revision_id
                {
                    return Err(error(
                        ErrorCode::InvalidResult,
                        "Alternate search anchor owner mismatch",
                    ));
                }
            }
            groups
                .entry(u.owner_entity_id.clone())
                .or_default()
                .push(SearchUnit {
                    search_unit_id: storage::id(),
                    owner_entity_id: u.owner_entity_id.clone(),
                    revision_id: u.revision_id.clone(),
                    content_hash: u.content_hash.clone(),
                    kind: SearchUnitKind::Material,
                    visible_tier: None,
                    field_path: u.field_path.clone(),
                    display_text: u.display_text.clone(),
                    normalized_text: String::new(),
                    normalization_profile: search::PROFILE.into(),
                    offset_map: Vec::new(),
                    anchor: u
                        .alternate_anchors
                        .iter()
                        .find(|a| matches!(a.selector, Selector::V2(_)))
                        .unwrap_or(&u.anchor)
                        .clone(),
                    equivalence_group_id: u.equivalence_group_id.clone(),
                    source_role: u.source_role.clone(),
                });
        }
        let mut projections: Vec<PreparedProjection> = Vec::new();
        for object in &object_values {
            let owner = object["object_id"].as_str().unwrap_or("");
            let revision = object["revision_id"].as_str().unwrap_or("");
            let units = groups.remove(owner).unwrap_or_default();
            let coverage = coverage(
                units.len() as u64,
                ExtractionCoverageState::Complete,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            projections.push(search::prepare_projection(
                owner, revision, units, coverage, None,
            )?);
        }
        let extracted = &manifest.search_coverage;
        projections.push(search::prepare_projection(
            &manifest.material_id,
            &manifest.parse_version_id,
            Vec::new(),
            coverage(
                0,
                extracted.state.clone(),
                extracted.requested_pages.clone(),
                extracted.processed_pages.clone(),
                extracted.unprocessed.clone(),
                extracted.failed.clone(),
            ),
            None,
        )?);
        let mut copied = CopiedFiles {
            root: self.store.root().into(),
            paths: Vec::new(),
            keep: false,
        };
        let mut resource_records = Vec::new();
        for resource in &manifest.resources {
            if resource
                .owner_refs
                .iter()
                .any(|id| !ids.contains(id) && id != &manifest.material_id)
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Resource owner is outside package",
                ));
            }
            let mut file = resources::open_relative(&stage, Path::new(&resource.relative_path))?;
            validate_preview(&mut file, &resource.media_type)?;
            let destination = format!("resources/{}.asset", resource.resource_id);
            let existing = self.store.read(|c| {
                c.query_row(
                    "SELECT sha256,byte_length,relative_path,media_type FROM resources WHERE resource_id=?1",
                    [&resource.resource_id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, u64>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(sql_error)
            })?;
            if let Some((hash, length, path, media)) = existing {
                let owned=self.store.read(|c|c.query_row("SELECT EXISTS(SELECT 1 FROM resource_owners WHERE resource_id=?1 AND owner_entity_id=?2)",params![resource.resource_id,manifest.material_id],|r|r.get::<_,bool>(0)).map_err(sql_error))?;
                if hash != resource.sha256
                    || length != resource.byte_length
                    || media != resource.media_type
                    || !owned
                {
                    return Err(error(
                        ErrorCode::Conflict,
                        "Immutable resource identity conflicts",
                    ));
                }
                resource_records.push((resource.clone(), path));
                continue;
            }
            let (hash, length) =
                resources::persist_bytes(file, self.store.root(), &destination, 64 * 1024 * 1024)?;
            copied.paths.push(destination.clone());
            if hash != resource.sha256 || length != resource.byte_length {
                return Err(error(
                    ErrorCode::CorruptAsset,
                    "Material resource hash or size mismatch",
                ));
            }
            resource_records.push((resource.clone(), destination));
        }
        let result=self.store.write(|t| {
            snapshots::check_frozen(t,&snapshot)?;let current_task=crate::tasks::task(t,&task.task_id)?;if current_task.attempt!=task.attempt||!matches!(current_task.state,TaskState::Queued|TaskState::Running|TaskState::Interrupted){return Err(error(ErrorCode::StaleInput,"Material task was cancelled or superseded"));}
            let final_duplicate:Option<(String,String,Option<String>,String)>=t.query_row("SELECT material_id,source_version_id,parent_parse_version_id,content_hash FROM parse_versions WHERE parse_version_id=?1",[&manifest.parse_version_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(sql_error)?;
            if let Some((material_id,source_version_id,parent_parse_version_id,hash))=final_duplicate {
                validate_existing_parse_identity((&material_id,&source_version_id,parent_parse_version_id.as_deref(),&hash),&manifest.material_id,&manifest.source_version_id,manifest.parent_parse_version_id.as_deref(),&manifest_hash)?;
                return Ok(json!({"material_id":manifest.material_id,"parse_version_id":manifest.parse_version_id,"publication":"committed","duplicate":true}));
            }
            let (current_parse_version,parent_exists)=current_publication_parent(t,&manifest.material_id,&manifest.source_version_id)?;
            validate_publication_parent(current_parse_version.as_deref(),manifest.parent_parse_version_id.as_deref(),parent_exists)?;
            let scope_hash=canonical_hash(&json!([manifest.material_id,manifest.source_version_id]));
            for identity in manifest.pages.iter().map(|p|p.page_id.as_str()).chain(manifest.regions.iter().map(|r|r.region_id.as_str())).chain(object_values.iter().map(|o|o["object_id"].as_str().unwrap_or(""))) {
                let prior:Option<String>=t.query_row("SELECT request_hash FROM idempotency WHERE actor_id='native.material_identity' AND command_name='confirm' AND idempotency_key=?1",[identity],|r|r.get(0)).optional().map_err(sql_error)?;
                if let Some(prior)=prior {if prior!=scope_hash{return Err(error(ErrorCode::Conflict,"Material entity identity belongs to another source"));}}else{
                    let collision:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM entities WHERE entity_id=?1 UNION ALL SELECT 1 FROM resources WHERE resource_id=?1)",[identity],|r|r.get(0)).map_err(sql_error)?;
                    if collision{return Err(error(ErrorCode::Conflict,"Material candidate identity needs explicit mapping"));}
                    t.execute("INSERT INTO idempotency VALUES('native.material_identity','confirm',?1,?2,?3)",params![identity,scope_hash,encode(&json!({"entity_id":identity,"material_id":manifest.material_id,"source_version_id":manifest.source_version_id}))?]).map_err(sql_error)?;
                }
            }
            // A previously known ID cannot silently become an unrelated object.
            for object in &object_values {let id=object["object_id"].as_str().unwrap_or("");let exists:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM entities WHERE entity_id=?1)",[id],|r|r.get(0)).map_err(sql_error)?;if exists {let old=head(t,id)?;if old!=*object{return Err(error(ErrorCode::Conflict,"identity_mapping_review_required"));}}}
            let scope=json!({"kind":"entity","entity_id":manifest.material_id});
            for page in &manifest.pages {commit_version(t,&page.page_id,"material_page",&page.revision_id,storage::value(page)?,"material.page_ready",scope.clone(),None,Some(&task.task_id))?;}
            for region in &manifest.regions {commit_entity(t,&region.region_id,"material_region",storage::value(region)?,"material.parse_changed",scope.clone(),None,Some(&task.task_id))?;}
            for object in &object_values {commit_version(t,object["object_id"].as_str().unwrap_or(""),"material_object",object["revision_id"].as_str().unwrap_or(""),object.clone(),"material.page_ready",scope.clone(),None,Some(&task.task_id))?;}
            for (resource,relative) in &resource_records {t.execute("INSERT INTO resources VALUES(?1,?2,?3,?4,?5,'ready') ON CONFLICT(resource_id) DO NOTHING",params![resource.resource_id,relative,resource.sha256,resource.media_type,resource.byte_length]).map_err(sql_error)?;for owner in &resource.owner_refs {t.execute("INSERT INTO resource_owners VALUES(?1,?2) ON CONFLICT DO NOTHING",params![resource.resource_id,owner]).map_err(sql_error)?;}t.execute("INSERT INTO resource_owners VALUES(?1,?2) ON CONFLICT DO NOTHING",params![resource.resource_id,manifest.material_id]).map_err(sql_error)?;}
            t.execute("INSERT INTO parse_versions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![manifest.parse_version_id,manifest.material_id,manifest.source_version_id,manifest.parent_parse_version_id,manifest.format_version,manifest.engine_build,encode(&manifest)?,manifest_hash,storage::value(&manifest.completion)?.as_str().unwrap_or("")]).map_err(sql_error)?;
            commit_version(t,&manifest.material_id,"material",&manifest.parse_version_id,storage::value(&manifest)?,"material.parse_changed",scope,None,Some(&task.task_id))?;
            for prepared in &projections{search::commit_projection(t,prepared)?;}
            emit(t,"search.invalidated",&manifest.material_id,0,&manifest.parse_version_id,json!({"kind":"entity","entity_id":manifest.material_id}),json!({"reason":"head_changed","must_clear":true,"visibility_epoch":crate::storage::epoch(t)?}),None,Some(&task.task_id))?;
            Ok(json!({"material_id":manifest.material_id,"parse_version_id":manifest.parse_version_id,"publication":"committed","completion":manifest.completion,"index_generation":t.query_row("SELECT MAX(index_generation) FROM search_index_state",[],|r|r.get::<_,u64>(0)).map_err(sql_error)?}))
        });
        copied.keep = result.is_ok();
        result
    }
}
fn validate_preview(file: &mut File, media: &str) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    if ![
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
        "image/svg+xml",
        "application/pdf",
    ]
    .contains(&media)
    {
        return Err(error(
            ErrorCode::UnsupportedCapability,
            "Unsupported material resource media type",
        ));
    }
    if media != "image/svg+xml" {
        return Ok(());
    }
    if file.metadata().map_err(io_error)?.len() > 2_000_000 {
        return Err(error(
            ErrorCode::InvalidResult,
            "SVG preview exceeds static validation budget",
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| error(ErrorCode::InvalidResult, "SVG preview is not UTF8"))?;
    for token in text.split('<').skip(1) {
        let token = token.split('>').next().unwrap_or("").to_ascii_lowercase();
        let name = token
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        if token == "?xml version=\"1.0\" encoding=\"utf-8\"?" {
            continue;
        }
        if ![
            "svg",
            "rect",
            "text",
            "tspan",
            "path",
            "line",
            "circle",
            "ellipse",
            "polygon",
            "polyline",
            "g",
            "defs",
            "lineargradient",
            "radialgradient",
            "stop",
            "clippath",
            "title",
            "desc",
        ]
        .contains(&name)
            || token.contains("href")
            || token.contains("url(")
            || token.contains("style=")
            || token
                .split_whitespace()
                .any(|part| part.starts_with("on") || part.starts_with("style"))
        {
            return Err(error(
                ErrorCode::InvalidResult,
                "SVG preview contains unsupported active or external content",
            ));
        }
    }
    Ok(())
}
fn coverage(
    units: u64,
    state: ExtractionCoverageState,
    pages: Vec<String>,
    processed: Vec<String>,
    unprocessed: Vec<CoverageArea>,
    failed: Vec<CoverageArea>,
) -> SearchCoverage {
    SearchCoverage {
        scope_snapshot_id: storage::id(),
        required_units: units,
        indexed_units: 0,
        required_pages: pages,
        indexed_pages: processed,
        unprocessed,
        failed,
        stale: Vec::new(),
        state: match state {
            ExtractionCoverageState::Complete => SearchCoverageState::Complete,
            ExtractionCoverageState::Partial => SearchCoverageState::Partial,
            ExtractionCoverageState::Unavailable => SearchCoverageState::Unavailable,
        },
        source_extraction_revision: None,
    }
}
fn object_id(object: &MaterialObject) -> &String {
    match object {
        MaterialObject::V0(x) => &x.object_id,
        MaterialObject::V1(x) => &x.object_id,
        MaterialObject::V2(x) => &x.object_id,
        MaterialObject::V3(x) => &x.object_id,
        MaterialObject::V4(x) => &x.object_id,
        MaterialObject::V5(x) => &x.object_id,
        MaterialObject::V6(x) => &x.object_id,
        MaterialObject::V7(x) => &x.object_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_publication_has_no_parent_and_republication_requires_current_parent() {
        assert!(validate_publication_parent(None, None, false).is_ok());
        assert!(validate_publication_parent(Some("current"), Some("current"), true).is_ok());
        assert_eq!(
            validate_publication_parent(Some("current"), None, false)
                .unwrap_err()
                .code,
            ErrorCode::StaleInput
        );
        assert_eq!(
            validate_publication_parent(Some("current"), Some("old"), true)
                .unwrap_err()
                .code,
            ErrorCode::StaleInput
        );
        assert_eq!(
            validate_publication_parent(Some("current"), Some("current"), false)
                .unwrap_err()
                .code,
            ErrorCode::StaleInput
        );
    }

    #[test]
    fn duplicate_parse_version_is_idempotent_only_for_the_same_parent_and_package() {
        assert!(validate_existing_parse_identity(
            ("material", "source", Some("parent"), "hash"),
            "material",
            "source",
            Some("parent"),
            "hash",
        )
        .is_ok());
        for (parent, hash) in [(Some("other"), "hash"), (Some("parent"), "changed")] {
            assert_eq!(
                validate_existing_parse_identity(
                    ("material", "source", Some("parent"), "hash"),
                    "material",
                    "source",
                    parent,
                    hash,
                )
                .unwrap_err()
                .code,
                ErrorCode::IdempotencyMismatch
            );
        }
    }
}

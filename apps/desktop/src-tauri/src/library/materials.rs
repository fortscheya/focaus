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
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
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
pub struct MaterialJob {
    pub task: Task,
    pub input: MaterialConvertInput,
    pub material_id: String,
    pub source_version_id: String,
    pub parse_version_id: String,
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
}
impl MaterialService {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            accept_gate: Mutex::new(()),
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
        if self.store.read(|c| lifecycle(c, lesson))? != "active" {
            return Err(error(ErrorCode::SourceDeleted, "Import lesson unavailable"));
        }
        let extension = path
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
        let source = File::open(path).map_err(io_error)?;
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
        let display = path
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("Material")
            .to_owned();
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
        let (source_version,relative,sha)=self.store.read(|c|c.query_row("SELECT m.source_version_id,r.relative_path,r.sha256 FROM materials m JOIN resources r ON r.resource_id=m.resource_id JOIN entities e ON e.entity_id=m.material_id WHERE m.material_id=?1 AND r.state='ready' AND e.lifecycle='active'",[material],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional().map_err(sql_error)?.ok_or_else(||error(ErrorCode::SourceUnavailable,"Managed original unavailable")))?;
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
        Ok(MaterialJob {
            task,
            input,
            material_id: material.into(),
            source_version_id: source_version,
            parse_version_id: parse,
        })
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
        let duplicate = self.store.read(|c| {
            c.query_row(
                "SELECT content_hash FROM parse_versions WHERE parse_version_id=?1",
                [&manifest.parse_version_id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_error)
        })?;
        let manifest_hash = canonical_hash(&storage::value(&manifest)?);
        if let Some(hash) = duplicate {
            if hash != manifest_hash {
                return Err(error(
                    ErrorCode::IdempotencyMismatch,
                    "Parse version ID reused for different package",
                ));
            }
            return Ok(
                json!({"material_id":manifest.material_id,"parse_version_id":manifest.parse_version_id,"publication":"committed","duplicate":true}),
            );
        }
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

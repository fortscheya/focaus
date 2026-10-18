//! Authenticated WebView gateway and gap-free snapshot/outbox delivery.
use crate::{
    capture::RecordingCoordinator,
    storage::{self, sql_error, Result, Store},
    tasks::TaskManager,
    visualizations::Visualizations,
};
use focaus_contracts::*;
use rusqlite::OptionalExtension;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Mutex};

#[derive(Clone)]
struct WindowBinding {
    id: String,
    scopes: Vec<Scope>,
}
struct Subscription {
    window_id: String,
    scopes: Vec<Scope>,
    acked: u64,
    delivered: u64,
    snapshot_pending: bool,
}
pub struct NativeHost {
    pub store: Store,
    pub recording: RecordingCoordinator,
    pub tasks: TaskManager,
    pub visualizations: Visualizations,
    pub search: crate::search::SearchCoordinator,
    pub views: crate::windows::WindowCoordinator,
    pub snapshots: crate::library::snapshots::SnapshotStore,
    pub focaus: crate::library::catchup::FocausCoordinator,
    pub materials: crate::library::materials::MaterialService,
    pub run_id: String,
    windows: Mutex<BTreeMap<String, WindowBinding>>,
    subscriptions: Mutex<BTreeMap<String, Subscription>>,
    dispatch_gate: Mutex<()>,
}
fn parse<T: DeserializeOwned>(v: &Value) -> Result<T> {
    serde_json::from_value(v.clone())
        .map_err(|_| error(ErrorCode::InvalidArgument, "Invalid command payload"))
}
fn scope_allows(allowed: &[Scope], requested: &Scope) -> bool {
    allowed
        .iter()
        .any(|s| matches!(s, Scope::V0(_)) || s == requested)
}
fn scope_match(scopes: &[Scope], event: &Scope) -> bool {
    scopes
        .iter()
        .any(|s| matches!(s, Scope::V0(_)) || s == event)
}
impl NativeHost {
    /// Send+Sync composition object; CPAL streams stay on their owning thread.
    /// Opening a library restores data/tasks but never opens a microphone.
    pub fn open(root: impl AsRef<Path>, migration: &str) -> Result<Self> {
        let store = Store::open(root, migration)?;
        let _ = crate::audio::recover(&store)?;
        let tasks = TaskManager::new(store.clone());
        let _ = tasks.recover()?;
        let views = crate::windows::WindowCoordinator::new(store.clone());
        views.recover()?;
        Ok(Self {
            recording: RecordingCoordinator::new(store.clone()),
            visualizations: Visualizations::new(store.clone()),
            search: crate::search::SearchCoordinator::new(store.clone()),
            snapshots: crate::library::snapshots::SnapshotStore::new(store.clone()),
            focaus: crate::library::catchup::FocausCoordinator::new(store.clone()),
            materials: crate::library::materials::MaterialService::new(store.clone()),
            views,
            tasks,
            store,
            run_id: storage::id(),
            windows: Mutex::new(BTreeMap::new()),
            subscriptions: Mutex::new(BTreeMap::new()),
            dispatch_gate: Mutex::new(()),
        })
    }
    pub fn bind_window(
        &self,
        webview_label: &str,
        window_id: &str,
        allowed_scopes: Vec<Scope>,
    ) -> Result<()> {
        if webview_label.is_empty() || window_id.is_empty() {
            return Err(error(ErrorCode::InvalidArgument, "Window binding required"));
        }
        let mut bindings = self
            .windows
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Window registry unavailable"))?;
        if let Some(old) = bindings.get(webview_label) {
            if old.id != window_id {
                return Err(error(ErrorCode::Conflict, "Window already bound"));
            }
            return Ok(());
        }
        bindings.insert(
            webview_label.into(),
            WindowBinding {
                id: window_id.into(),
                scopes: allowed_scopes,
            },
        );
        Ok(())
    }
    /// Closing a window removes only its subscriptions. Capture/tasks stay alive.
    pub fn unbind_window(&self, label: &str) -> Result<()> {
        let old = self
            .windows
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Window registry unavailable"))?
            .remove(label);
        if let Some(old) = old {
            self.subscriptions
                .lock()
                .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?
                .retain(|_, s| s.window_id != old.id);
        }
        Ok(())
    }
    pub fn dispatch_webview(&self, label: &str, command: Command) -> Result<CommandResult> {
        let _serial = self
            .dispatch_gate
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Command dispatcher unavailable"))?;
        let window = self
            .windows
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Window registry unavailable"))?
            .get(label)
            .cloned()
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "Unknown calling WebView"))?;
        if command.actor != CommandActor::User
            || command.window_id.as_deref() != Some(window.id.as_str())
        {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Actor or window binding mismatch",
            ));
        }
        let name = storage::value(&command.name)?
            .as_str()
            .unwrap_or("")
            .to_owned();
        if schema()["x-host-only-commands"]
            .as_array()
            .is_some_and(|xs| xs.iter().any(|x| x == &json!(name)))
        {
            return Err(error(ErrorCode::PermissionDenied, "Host-only command"));
        }
        validate("Command", &storage::value(&command)?)?;
        let catalogue = command_catalogue();
        let entry = catalogue
            .get(&name)
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "Unknown command"))?;
        validate(entry["input"].as_str().unwrap_or(""), &command.payload)?;
        if name.starts_with("search.") && name != "search.open" {
            if let Some(id) = command.payload["search_id"].as_str() {
                self.search.authorize_session(id, &window.id)?;
            }
        }
        // Narrow bindings are enforced before any content lookup. The initial C
        // bootstrap is a single-user local global scope; it grants no host APIs.
        if !window.scopes.iter().any(|s| matches!(s, Scope::V0(_))) {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Scoped window capability not yet granted for this command",
            ));
        }
        if entry["mutates"] == true && command.expected_revisions.is_none() {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Expected revisions required",
            ));
        }
        let result = match name.as_str() {
            "state.open" => self.state_open(&window, parse(&command.payload)?)?,
            "state.ack" => {
                self.state_ack(&window.id, parse(&command.payload)?)?;
                json!({})
            }
            "state.replay" => self.state_replay(&window.id, parse(&command.payload)?)?,
            "state.close" => {
                let input: StateCloseInput = parse(&command.payload)?;
                let mut subscriptions = self
                    .subscriptions
                    .lock()
                    .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?;
                if subscriptions
                    .get(&input.subscription_id)
                    .is_some_and(|s| s.window_id != window.id)
                {
                    return Err(error(
                        ErrorCode::PermissionDenied,
                        "Subscription belongs to another window",
                    ));
                }
                subscriptions.remove(&input.subscription_id);
                json!({})
            }
            "content.get" => self
                .store
                .read(|c| crate::library::get_content(c, &parse(&command.payload)?))?,
            "resource.read" => {
                crate::library::resources::read_resource(&self.store, parse(&command.payload)?)?
            }
            "visualization.evaluate" => {
                storage::value(&self.visualizations.evaluate(parse(&command.payload)?)?)?
            }
            "search.open" => {
                let input: SearchOpenInput = parse(&command.payload)?;
                self.store
                    .read(|c| crate::windows::authorize(c, &input.view_id, &window.id))?;
                storage::value(&self.search.open(input)?)?
            }
            "search.query" => storage::value(&self.search.query_task(parse(&command.payload)?)?)?,
            "search.page" => storage::value(&self.search.page(parse(&command.payload)?)?)?,
            "search.locate" => storage::value(&self.search.locate(parse(&command.payload)?)?)?,
            "search.close" => {
                let input: SearchCloseInput = parse(&command.payload)?;
                self.search.close(&input.search_id)?;
                json!({})
            }
            "catchup.request" => {
                let input: CatchupRequestInput = parse(&command.payload)?;
                self.store
                    .read(|c| crate::windows::authorize(c, &input.view_id, &window.id))?;
                let task = self.focaus.begin(input)?;
                return Ok(CommandResult {
                    contract_version: CONTRACT_VERSION.into(),
                    command_id: command.command_id,
                    status: CommandResultStatus::Accepted,
                    result: Some(json!({"task_id":task.task_id,"snapshot_id":task.snapshot_id})),
                    committed_revisions: None,
                    event_seq: None,
                    task_id: Some(task.task_id),
                    current_state: Some(task.state),
                    error: None,
                });
            }
            "catchup.display_ack" => {
                let mut response = self.focaus.display_ack(parse(&command.payload)?)?;
                response.command_id = command.command_id;
                return Ok(response);
            }
            "view.open" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::windows::open(
                        t,
                        &parse(&command.payload)?,
                        &window.id,
                        Some(&command.command_id),
                    )
                })
            }
            "view.save_state" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::windows::save(
                        t,
                        &parse(&command.payload)?,
                        &window.id,
                        Some(&command.command_id),
                    )
                })
            }
            "view.move" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::windows::prepare(
                        t,
                        &parse(&command.payload)?,
                        &window.id,
                        Some(&command.command_id),
                    )
                })
            }
            "view.ready" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::windows::ready(
                        t,
                        &parse(&command.payload)?,
                        &window.id,
                        Some(&command.command_id),
                    )
                })
            }
            "view.close" | "view.minimize" => {
                return self.store.command(&window.id, &command, |t| {
                    let id = command.payload["view_id"]
                        .as_str()
                        .ok_or_else(|| error(ErrorCode::InvalidArgument, "View required"))?;
                    crate::windows::set_visibility(
                        t,
                        id,
                        &window.id,
                        if name == "view.close" {
                            "closed"
                        } else {
                            "minimized"
                        },
                        Some(&command.command_id),
                    )
                })
            }
            "course.create" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::library::create_course(
                        t,
                        &parse(&command.payload)?,
                        Some(&command.command_id),
                    )
                })
            }
            "lesson.create" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::library::create_lesson(
                        t,
                        &parse(&command.payload)?,
                        Some(&command.command_id),
                    )
                })
            }
            "recording.start" => {
                let input: RecordingStartInput = parse(&command.payload)?;
                let receipt = self.store.command(&window.id, &command, |t| {
                    RecordingCoordinator::reserve(t, &input, Some(&command.command_id))
                })?;
                let recording = receipt
                    .result
                    .as_ref()
                    .and_then(|v| v["recording_id"].as_str())
                    .ok_or_else(|| {
                        error(
                            ErrorCode::InvalidResult,
                            "Recording reservation unavailable",
                        )
                    })?;
                self.recording
                    .activate(recording, input.device_id.as_deref())?;
                return Ok(receipt);
            }
            "recording.transfer" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::library::transfer(
                        t,
                        &parse(&command.payload)?,
                        Some(&command.command_id),
                    )
                })
            }
            "edit.begin" => {
                return self.store.command(&window.id, &command, |t| {
                    let input: EditBeginInput = parse(&command.payload)?;
                    crate::library::begin_edit(t, &input.lease)
                })
            }
            "edit.end" => {
                return self.store.command(&window.id, &command, |t| {
                    crate::library::end_edit(t, &parse(&command.payload)?)
                })
            }
            "audio.play" => {
                self.recording.start_playback()?;
                return Err(error(
                    ErrorCode::UnsupportedCapability,
                    "Playback output adapter is not connected",
                ));
            }
            "audio.stop" => {
                self.recording.stop_playback();
                json!({})
            }
            "recording.pause" | "recording.resume" | "recording.stop" => {
                if let Some(receipt) = self.store.preflight(&window.id, &command)? {
                    return Ok(receipt);
                }
                let rid = command.payload["recording_id"]
                    .as_str()
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "Recording required"))?;
                if command
                    .expected_revisions
                    .as_ref()
                    .is_none_or(|r| !r.contains_key(rid))
                {
                    return Err(error(
                        ErrorCode::InvalidArgument,
                        "Recording revision required",
                    ));
                }
                let current = self.store.head(rid)?;
                if current["state"] != command.payload["expected_state"] {
                    return Err(error(ErrorCode::Conflict, "Recording state changed"));
                }
                let state = current["state"].as_str().unwrap_or("");
                let next = if name == "recording.resume" {
                    if !["paused", "interrupted"].contains(&state) {
                        return Err(error(
                            ErrorCode::Conflict,
                            "Recording cannot resume from this state",
                        ));
                    }
                    "starting"
                } else if name == "recording.pause" {
                    if !["starting", "recording"].contains(&state) {
                        return Err(error(
                            ErrorCode::Conflict,
                            "Recording cannot pause from this state",
                        ));
                    }
                    "paused"
                } else {
                    if !["starting", "recording", "paused", "interrupted"].contains(&state) {
                        return Err(error(
                            ErrorCode::Conflict,
                            "Recording cannot stop from this state",
                        ));
                    }
                    "stopped"
                };
                if name != "recording.resume" {
                    self.recording.quiesce(rid, name == "recording.stop")?;
                }
                let receipt = self.store.command(&window.id, &command, |t| {
                    crate::capture::state_transaction(t, rid, next, Some(&command.command_id))
                });
                let receipt = match receipt {
                    Ok(receipt) => receipt,
                    Err(e) => {
                        if name != "recording.resume" {
                            let _ = crate::capture::set_state(&self.store, rid, "interrupted");
                        }
                        return Err(e);
                    }
                };
                if name == "recording.resume" {
                    self.recording.reactivate(rid)?;
                }
                return Ok(receipt);
            }
            "app.exit" => {
                self.exit()?;
                json!({"sealed":true})
            }
            _ => {
                return Err(error(
                    ErrorCode::UnsupportedCapability,
                    "Native command is not connected",
                ))
            }
        };
        Ok(CommandResult {
            contract_version: CONTRACT_VERSION.into(),
            command_id: command.command_id,
            status: CommandResultStatus::Ok,
            result: Some(result),
            committed_revisions: Some(BTreeMap::new()),
            event_seq: Some(self.store.read(storage::sequence)?),
            task_id: None,
            current_state: None,
            error: None,
        })
    }
    fn state_open(&self, window: &WindowBinding, input: StateOpenInput) -> Result<Value> {
        let negotiated = negotiate(&input.capabilities)?;
        for scope in &input.scopes {
            if !scope_allows(&window.scopes, scope) {
                return Err(error(ErrorCode::PermissionDenied, "Scope not granted"));
            }
        }
        let mut subscriptions = self
            .subscriptions
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?;
        let subscription_id = storage::id();
        let snapshot=self.store.consistent_snapshot(|c,seq| {
            let mut records:BTreeMap<String,BTreeMap<String,Value>>=BTreeMap::new();
            let mut q=c.prepare("SELECT entity_id,kind,revision_id,entity_seq,lifecycle FROM entities ORDER BY entity_id").map_err(sql_error)?;
            for row in q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,u64>(3)?,r.get::<_,String>(4)?))).map_err(sql_error)? {
                let (entity,kind,revision,entity_seq,lifecycle)=row.map_err(sql_error)?;
                if !input.scopes.iter().any(|s|matches!(s,Scope::V0(_))||storage::value(s).ok().is_some_and(|v|v["entity_id"]==entity||v["course_id"]==entity||v["lesson_id"]==entity)){continue;}
                let collection=match kind.as_str(){"course"=>"courses","lesson"=>"lessons",_=>"content_heads"};
                let body=if lifecycle!="deleted"&&matches!(kind.as_str(),"course"|"lesson"){crate::storage::head(c,&entity)?}else{json!({"entity_id":entity,"kind":kind,"revision_id":revision,"entity_seq":entity_seq,"lifecycle":lifecycle})};
                records.entry(collection.into()).or_default().insert(entity,body);
            }
            for collection in ["courses","lessons","content_heads","task_summaries","progress","view_states","visualizations","search_indexes","catchup_heads"]{records.entry(collection.into()).or_default();}
            let has_global=input.scopes.iter().any(|s|matches!(s,Scope::V0(_)));
            let mut views=c.prepare("SELECT view_id FROM views ORDER BY rowid").map_err(sql_error)?;
            for id in views.query_map([],|r|r.get::<_,String>(0)).map_err(sql_error)? {let id=id.map_err(sql_error)?;let view=crate::windows::get(c,&id)?;if has_global||input.scopes.iter().any(|scope|storage::value(scope).ok().is_some_and(|scope|scope["view_id"]==id||scope["lesson_id"]==view.lesson_id)){records.get_mut("view_states").unwrap().insert(id,storage::value(&view)?);}}
            if has_global {
                let mut tasks=c.prepare("SELECT task_id FROM tasks ORDER BY rowid").map_err(sql_error)?;
                for id in tasks.query_map([],|r|r.get::<_,String>(0)).map_err(sql_error)? {let id=id.map_err(sql_error)?;records.get_mut("task_summaries").unwrap().insert(id.clone(),storage::value(&crate::tasks::task(c,&id)?)?);}
                let mut progress=c.prepare("SELECT lesson_id,body_json FROM progress_states").map_err(sql_error)?;
                for row in progress.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(sql_error)?{let(id,body)=row.map_err(sql_error)?;records.get_mut("progress").unwrap().insert(id,storage::decode(&body)?);}
                let mut indexes=c.prepare("SELECT scope_id,projection_version,index_generation,head_manifest_hash,dirty_ranges_json,coverage_json,visibility_epoch,last_committed_seq FROM search_index_state").map_err(sql_error)?;
                for row in indexes.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?,r.get::<_,u64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,u64>(6)?,r.get::<_,u64>(7)?))).map_err(sql_error)? {let(id,projection,generation,manifest,dirty,coverage,visibility,last)=row.map_err(sql_error)?;records.get_mut("search_indexes").unwrap().insert(id,json!({"projection_version":projection,"index_generation":generation,"head_manifest_hash":manifest,"dirty_ranges":storage::decode::<Value>(&dirty)?,"coverage":storage::decode::<Value>(&coverage)?,"visibility_epoch":visibility,"last_committed_seq":last}));}
            }
            let recording:Option<String>=c.query_row("SELECT recording_id FROM recording_sessions ORDER BY state IN ('starting','recording','paused','stopping') DESC,rowid DESC LIMIT 1",[],|r|r.get(0)).optional().map_err(sql_error)?;
            let mut global=BTreeMap::new();global.insert("recording".into(),recording.map(|id|crate::storage::head(c,&id)).transpose()?.unwrap_or(Value::Null));global.insert("visibility_epoch".into(),json!(crate::storage::epoch(c)?));global.insert("windows".into(),json!([]));
            Ok(AppSnapshot {contract_version:CONTRACT_VERSION.into(),host_run_id:self.run_id.clone(),snapshot_seq:seq,scopes:input.scopes.clone(),global,records,continuations:Vec::new()})
        })?;
        subscriptions.insert(
            subscription_id.clone(),
            Subscription {
                window_id: window.id.clone(),
                scopes: input.scopes,
                acked: snapshot.snapshot_seq,
                delivered: snapshot.snapshot_seq,
                snapshot_pending: true,
            },
        );
        storage::value(&StateOpenResult {
            subscription_id,
            snapshot_seq: snapshot.snapshot_seq,
            snapshot,
            negotiated,
        })
    }
    fn state_ack(&self, window: &str, input: StateAckInput) -> Result<()> {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?;
        let sub = subscriptions
            .get_mut(&input.subscription_id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Subscription unavailable"))?;
        if sub.window_id != window {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Subscription belongs to another window",
            ));
        }
        if input.upto_seq < sub.acked || input.upto_seq > sub.delivered {
            return Err(error(
                ErrorCode::Conflict,
                "Invalid event acknowledgment watermark",
            ));
        }
        sub.acked = input.upto_seq;
        sub.snapshot_pending = false;
        Ok(())
    }
    fn state_replay(&self, window: &str, input: StateReplayInput) -> Result<Value> {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?;
        let sub = subscriptions
            .get_mut(&input.subscription_id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Subscription unavailable"))?;
        if sub.window_id != window {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Subscription belongs to another window",
            ));
        }
        if input.after_seq != sub.acked {
            return Err(error(
                ErrorCode::Conflict,
                "Replay cursor does not match acknowledged watermark",
            ));
        }
        let batch = self.batch(&input.subscription_id, sub)?;
        storage::value(&batch)
    }
    fn batch(&self, id: &str, sub: &mut Subscription) -> Result<EventBatch> {
        let (through, events) = self.store.read(|c| {
            let oldest: Option<u64> = c
                .query_row("SELECT MIN(global_seq) FROM outbox", [], |r| r.get(0))
                .map_err(sql_error)?;
            if oldest.is_some_and(|old| sub.acked + 1 < old) {
                return Err(error(
                    ErrorCode::CursorExpired,
                    "Event cursor expired; reopen snapshot",
                ));
            }
            crate::storage::events(c, sub.acked, 256)
        })?;
        sub.delivered = through;
        Ok(EventBatch {
            subscription_id: id.into(),
            after_seq: sub.acked,
            through_seq: through,
            events: events
                .into_iter()
                .filter(|e| scope_match(&sub.scopes, &e.scope))
                .collect(),
        })
    }
    pub fn take_event_batches(&self, label: &str) -> Result<Vec<EventBatch>> {
        let window = self
            .windows
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Window registry unavailable"))?
            .get(label)
            .cloned()
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "Unknown calling WebView"))?;
        let mut subscriptions = self
            .subscriptions
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Subscriptions unavailable"))?;
        let mut batches = Vec::new();
        for (id, sub) in subscriptions.iter_mut() {
            if sub.window_id == window.id && !sub.snapshot_pending {
                let batch = self.batch(id, sub)?;
                if batch.through_seq > batch.after_seq {
                    batches.push(batch);
                }
            }
        }
        Ok(batches)
    }
    pub fn exit(&self) -> Result<()> {
        self.recording.exit()?;
        self.store.flush()
    }
    pub fn import_material_original(
        &self,
        path: &Path,
        lesson_id: &str,
        purpose: MaterialImportInputPurpose,
        pages: Vec<u64>,
    ) -> Result<crate::library::materials::MaterialJob> {
        self.materials
            .import_original(path, lesson_id, purpose, pages)
    }
    pub fn accept_material_result(&self, input: MaterialPageResultInput) -> Result<Value> {
        self.materials.accept(input)
    }
    pub fn read_resource(&self, input: ResourceReadInput) -> Result<Value> {
        crate::library::resources::read_resource(&self.store, input)
    }
}

/// Trusted, in-process worker boundary. These methods are never Tauri commands;
/// every publication still requires a live local task/attempt and frozen input.
impl HostServices for NativeHost {
    fn invoke<'a>(&'a self, command: Command) -> HostFuture<'a, CommandResult> {
        Box::pin(async move {
            if command.actor == CommandActor::User || command.window_id.is_some() {
                return Err(error(
                    ErrorCode::PermissionDenied,
                    "Worker channel cannot impersonate a user window",
                ));
            }
            validate("Command", &storage::value(&command)?)?;
            let name = storage::value(&command.name)?
                .as_str()
                .unwrap_or("")
                .to_owned();
            let result = match name.as_str() {
                "identity.reserve" => {
                    return self.store.command("native.worker", &command, |t| {
                        crate::library::reserve_identity(t, &parse(&command.payload)?)
                    })
                }
                "snapshot.build" => {
                    storage::value(&self.snapshots.build(parse(&command.payload)?)?)?
                }
                "task.enqueue" => {
                    let input: TaskEnqueueInput = parse(&command.payload)?;
                    storage::value(&self.tasks.enqueue(input.task)?)?
                }
                "task.checkpoint" => {
                    storage::value(&self.tasks.checkpoint(parse(&command.payload)?)?)?
                }
                _ => {
                    return Err(error(
                        ErrorCode::PermissionDenied,
                        "Command is not granted on this worker channel",
                    ))
                }
            };
            Ok(CommandResult {
                contract_version: CONTRACT_VERSION.into(),
                command_id: command.command_id,
                status: CommandResultStatus::Ok,
                result: Some(result),
                committed_revisions: Some(BTreeMap::new()),
                event_seq: Some(self.store.read(storage::sequence)?),
                task_id: None,
                current_state: None,
                error: None,
            })
        })
    }
    fn snapshot_build<'a>(&'a self, input: SnapshotBuildInput) -> HostFuture<'a, InputSnapshot> {
        Box::pin(async move { self.snapshots.build(input) })
    }
    fn result_submit<'a>(&'a self, input: ResultSubmitInput) -> HostFuture<'a, CommandResult> {
        Box::pin(async move {
            if input.candidate["result_semantics"] == "focaus_context_v2" {
                let candidate: CatchupResult = parse(&input.candidate)?;
                self.focaus.submit(&input, candidate)
            } else {
                Err(error(
                    ErrorCode::UnsupportedCapability,
                    "Candidate publication adapter is not connected",
                ))
            }
        })
    }
    fn task_checkpoint<'a>(&'a self, input: TaskCheckpointInput) -> HostFuture<'a, Task> {
        Box::pin(async move { self.tasks.checkpoint(input) })
    }
    fn visualization_evaluate<'a>(
        &'a self,
        input: VisualizationEvaluateInput,
    ) -> HostFuture<'a, EvaluationResult> {
        Box::pin(async move { self.visualizations.evaluate(input) })
    }
    fn visualization_validate<'a>(
        &'a self,
        input: VisualizationValidateInput,
    ) -> HostFuture<'a, ValidationReport> {
        Box::pin(async move { self.visualizations.validate(input) })
    }
    fn search_open<'a>(&'a self, input: SearchOpenInput) -> HostFuture<'a, SearchSession> {
        Box::pin(async move { self.search.open(input) })
    }
    fn search_query<'a>(&'a self, input: SearchQueryInput) -> HostFuture<'a, Task> {
        Box::pin(async move { self.search.query_task(input) })
    }
    fn search_page<'a>(&'a self, input: SearchPageInput) -> HostFuture<'a, SearchPage> {
        Box::pin(async move { self.search.page(input) })
    }
    fn search_locate<'a>(&'a self, input: SearchLocateInput) -> HostFuture<'a, SearchAnchor> {
        Box::pin(async move { self.search.locate(input) })
    }
    fn context_retrieve<'a>(
        &'a self,
        input: ContextRetrieveInput,
    ) -> HostFuture<'a, ContextResult> {
        Box::pin(async move { self.snapshots.context(input) })
    }
    fn catchup_display_ack<'a>(
        &'a self,
        input: CatchupDisplayAckInput,
    ) -> HostFuture<'a, CommandResult> {
        Box::pin(async move { self.focaus.display_ack(input) })
    }
}

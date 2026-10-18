//! Durable work scheduling; R0 capture is deliberately outside this pool.
use crate::storage::{self, decode, emit, encode, sql_error, Result, Store};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::sync::Arc;

/// Presence checks only. Actual secret bytes belong in a subsequent host-owned
/// transport, never in the public CredentialResolver or its return value.
pub trait SecureSlotStore: Send + Sync {
    fn contains(&self, credential_ref: &str) -> Result<bool>;
}
pub struct UnconfiguredSlots;
impl SecureSlotStore for UnconfiguredSlots {
    fn contains(&self, _: &str) -> Result<bool> {
        Ok(false)
    }
}
pub struct NativeCredentialResolver<S: SecureSlotStore> {
    slots: Arc<S>,
}
impl<S: SecureSlotStore> NativeCredentialResolver<S> {
    pub fn new(slots: Arc<S>) -> Self {
        Self { slots }
    }
}
impl<S: SecureSlotStore> CredentialResolver for NativeCredentialResolver<S> {
    fn require_binding(&self, b: &CredentialBinding) -> Result<()> {
        let model = serde_json::to_value(&b.model_id)
            .map_err(|_| error(ErrorCode::InvalidArgument, "Invalid model binding"))?;
        let slot = serde_json::to_value(&b.credential_ref)
            .map_err(|_| error(ErrorCode::InvalidArgument, "Invalid credential binding"))?;
        if b.provider != "bailian" {
            return Err(error(
                ErrorCode::InvalidArgument,
                "credential_binding_mismatch",
            ));
        }
        credential_binding(
            model.as_str().unwrap_or(""),
            &b.region,
            slot.as_str().unwrap_or(""),
        )?;
        if !self.slots.contains(slot.as_str().unwrap_or(""))? {
            return Err(error(
                ErrorCode::AuthRequired,
                "Dedicated model credential is not configured",
            ));
        }
        Ok(())
    }
}
pub fn binding_for(model: &ModelId) -> Result<CredentialBinding> {
    let name = storage::value(model)?;
    let slot = focaus_contracts::schema()["x-model-bindings"][name.as_str().unwrap_or("")].clone();
    if slot.is_null() {
        return Err(error(ErrorCode::InvalidArgument, "Unsupported model"));
    }
    serde_json::from_value(
        json!({"provider":"bailian","region":"cn-beijing","model_id":name,"credential_ref":slot}),
    )
    .map_err(|_| error(ErrorCode::InvalidArgument, "Invalid model binding"))
}
pub fn priority(kind: &TaskKind) -> u64 {
    match kind {
        TaskKind::AsrStream | TaskKind::Catchup | TaskKind::Progress => 1,
        TaskKind::Secondary
        | TaskKind::NoteIncremental
        | TaskKind::Qa
        | TaskKind::VisualizationEvaluate
        | TaskKind::SearchQuery => 2,
        TaskKind::AsrFile | TaskKind::NoteReorganize | TaskKind::Export | TaskKind::Restore => 3,
        _ => 4,
    }
}
fn terminal(state: &TaskState) -> bool {
    matches!(
        state,
        TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled | TaskState::Superseded
    )
}
fn transition_valid(from: &TaskState, to: &TaskState) -> bool {
    use TaskState::*;
    if from == to {
        return true;
    }
    match from {
        Queued => matches!(to, Running | Cancelled | Superseded),
        Running => matches!(
            to,
            Succeeded
                | Failed
                | RetryWait
                | WaitingNetwork
                | WaitingInput
                | Paused
                | CancelRequested
                | Interrupted
                | Superseded
        ),
        WaitingNetwork | WaitingInput | Paused | RetryWait | Interrupted => matches!(
            to,
            Queued | Failed | CancelRequested | Cancelled | Superseded
        ),
        CancelRequested => matches!(to, Cancelled),
        _ => false,
    }
}
pub(crate) fn task(c: &Connection, id: &str) -> Result<Task> {
    let row=c.query_row("SELECT kind,target_id,snapshot_id,state,priority,attempt,deadline_at,idempotency_key,checkpoint_json,safe_error_json,created_at,updated_at,model_id,region FROM tasks WHERE task_id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u64>(4)?,r.get::<_,u64>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,String>(10)?,r.get::<_,String>(11)?,r.get::<_,Option<String>>(12)?,r.get::<_,Option<String>>(13)?))).optional().map_err(sql_error)?.ok_or_else(||error(ErrorCode::NotFound,"Task unavailable"))?;
    let snapshot: String = c
        .query_row(
            "SELECT body_json FROM input_snapshots WHERE snapshot_id=?1",
            [&row.2],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let snapshot: InputSnapshot = decode(&snapshot)?;
    let mut q=c.prepare("SELECT binding_json FROM remote_bindings WHERE task_id=?1 AND attempt=?2 ORDER BY provider_task_id").map_err(sql_error)?;
    let bindings = q
        .query_map(params![id, row.5], |r| r.get::<_, String>(0))
        .map_err(sql_error)?
        .map(|r| decode(&r.map_err(sql_error)?))
        .collect::<Result<Vec<RemoteBinding>>>()?;
    Ok(Task {
        task_id: id.into(),
        kind: decode(&encode(&row.0)?)?,
        target_id: row.1,
        snapshot_id: row.2,
        state: decode(&encode(&row.3)?)?,
        priority: row.4,
        attempt: row.5,
        deadline_at: row.6,
        idempotency_key: row.7,
        checkpoint: decode(&row.8)?,
        error: row.9.map(|v| decode(&v)).transpose()?,
        provider_bindings: bindings,
        created_at: row.10,
        updated_at: row.11,
        model_id: row.12.map(|v| decode(&encode(&v)?)).transpose()?,
        region: row.13,
        prompt_version: snapshot.prompt_version,
        parameters_hash: snapshot.parameters_hash,
    })
}
pub(crate) fn task_event(t: &rusqlite::Transaction<'_>, task: &Task) -> Result<()> {
    emit(
        t,
        "task.changed",
        &task.target_id,
        task.attempt,
        &task.snapshot_id,
        json!({"kind":"entity","entity_id":task.target_id}),
        json!({"task_id":task.task_id,"state":task.state,"attempt":task.attempt}),
        None,
        Some(&task.task_id),
    )?;
    Ok(())
}
#[derive(Clone)]
pub struct TaskManager {
    store: Store,
}
impl TaskManager {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    pub fn enqueue(&self, input: Task) -> Result<Task> {
        if input.priority != priority(&input.kind)
            || input.state != TaskState::Queued
            || input.attempt != 1
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Invalid initial task state or resource priority",
            ));
        }
        validate("Task", &storage::value(&input)?)?;
        self.store.write(|t| {
            let snapshot:String=t.query_row("SELECT body_json FROM input_snapshots WHERE snapshot_id=?1",[&input.snapshot_id],|r|r.get(0)).map_err(sql_error)?;
            let snapshot:InputSnapshot=decode(&snapshot)?;
            if snapshot.purpose!=input.kind||snapshot.target_id!=input.target_id||snapshot.model_id!=input.model_id||snapshot.parameters_hash!=input.parameters_hash{return Err(error(ErrorCode::StaleInput,"Task does not match frozen input"));}
            let (model,slot,region)=if let Some(model)=&input.model_id{let binding=binding_for(model)?;(Some(storage::value(model)?.as_str().unwrap_or("").to_owned()),Some(storage::value(&binding.credential_ref)?.as_str().unwrap_or("").to_owned()),Some(binding.region))}else{(None,None,None)};
            // Latest-wins pending progress/visualization work never builds an old backlog.
            if matches!(input.kind,TaskKind::Progress|TaskKind::VisualizationDiscover){t.execute("UPDATE tasks SET state='superseded',updated_at=?3 WHERE kind=?1 AND target_id=?2 AND state='queued'",params![storage::value(&input.kind)?.as_str().unwrap_or(""),input.target_id,storage::now()]).map_err(sql_error)?;}
            t.execute("INSERT INTO tasks VALUES(?1,?2,?3,?4,'queued',?5,1,?6,?7,?8,NULL,?9,?10,?11,?12,?13)",params![input.task_id,storage::value(&input.kind)?.as_str().unwrap_or(""),input.target_id,input.snapshot_id,input.priority,input.deadline_at,input.idempotency_key,encode(&input.checkpoint)?,model,slot,region,input.created_at,input.updated_at]).map_err(sql_error)?;
            task_event(t,&input)?;Ok(input.clone())
        })
    }
    pub fn get(&self, id: &str) -> Result<Task> {
        self.store.read(|c| task(c, id))
    }
    pub fn transition(&self, id: &str, attempt: u64, next: TaskState) -> Result<Task> {
        self.store.write(|t| {
            let mut current = task(t, id)?;
            if current.attempt != attempt {
                return Err(error(ErrorCode::StaleInput, "Task attempt changed"));
            }
            if !transition_valid(&current.state, &next) {
                return Err(error(ErrorCode::Conflict, "Invalid task transition"));
            }
            current.state = next;
            current.updated_at = storage::now();
            t.execute(
                "UPDATE tasks SET state=?2,updated_at=?3 WHERE task_id=?1",
                params![
                    id,
                    storage::value(&current.state)?.as_str().unwrap_or(""),
                    current.updated_at
                ],
            )
            .map_err(sql_error)?;
            task_event(t, &current)?;
            Ok(current)
        })
    }
    /// A reserved R1 slot is never occupied by R2–R4. R0 uses no task slots.
    pub fn claim(&self, total_slots: u64, reserved_r1: u64) -> Result<Option<Task>> {
        if total_slots == 0 || reserved_r1 > total_slots {
            return Err(error(ErrorCode::InvalidArgument, "Invalid resource budget"));
        }
        self.store.write(|t| {
            let running:u64=t.query_row("SELECT COUNT(*) FROM tasks WHERE state='running'",[],|r|r.get(0)).map_err(sql_error)?;
            if running>=total_slots{return Ok(None);}
            let background:u64=t.query_row("SELECT COUNT(*) FROM tasks WHERE state='running' AND priority>1",[],|r|r.get(0)).map_err(sql_error)?;
            let max_priority=if background>=total_slots-reserved_r1 {1}else{4};
            let next:Option<String>=t.query_row("SELECT task_id FROM tasks WHERE state='queued' AND priority<=?1 AND (deadline_at IS NULL OR deadline_at>?2) AND NOT(kind='progress' AND EXISTS(SELECT 1 FROM tasks r WHERE r.kind='progress' AND r.target_id=tasks.target_id AND r.state='running')) ORDER BY priority,created_at,task_id LIMIT 1",params![max_priority,storage::now()],|r|r.get(0)).optional().map_err(sql_error)?;
            let Some(id)=next else{return Ok(None)};t.execute("UPDATE tasks SET state='running',updated_at=?2 WHERE task_id=?1",params![id,storage::now()]).map_err(sql_error)?;let claimed=task(t,&id)?;task_event(t,&claimed)?;Ok(Some(claimed))
        })
    }
    pub fn checkpoint(&self, input: TaskCheckpointInput) -> Result<Task> {
        reject_sensitive_metadata(&storage::value(&input.safe_state)?)?;
        self.store.write(|t| {
            let current = task(t, &input.task_id)?;
            if current.attempt != input.attempt
                || terminal(&current.state)
                || current.state == TaskState::CancelRequested
            {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Checkpoint attempt no longer active",
                ));
            }
            let mut checkpoint = input.safe_state.clone();
            checkpoint.insert("cursor".into(), json!(input.cursor));
            t.execute(
                "UPDATE tasks SET checkpoint_json=?2,updated_at=?3 WHERE task_id=?1",
                params![input.task_id, encode(&checkpoint)?, storage::now()],
            )
            .map_err(sql_error)?;
            let updated = task(t, &input.task_id)?;
            task_event(t, &updated)?;
            Ok(updated)
        })
    }
    pub fn bind_remote(&self, id: &str, attempt: u64, binding: &RemoteBinding) -> Result<()> {
        self.store.write(|t| {
            let current = task(t, id)?;
            if current.attempt != attempt || terminal(&current.state) {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Remote binding attempt changed",
                ));
            }
            t.execute(
                "INSERT INTO remote_bindings VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",
                params![id, attempt, binding.provider_task_id, encode(binding)?],
            )
            .map_err(sql_error)?;
            Ok(())
        })
    }
    /// Recover in-flight identities/checkpoints. File jobs retain provider IDs;
    /// recovery never re-submits them as an untracked new paid request.
    pub fn recover(&self) -> Result<Vec<Task>> {
        self.store.write(|t| {
            let ids = {
                let mut q = t
                    .prepare(
                        "SELECT task_id FROM tasks WHERE state IN ('running','cancel_requested')",
                    )
                    .map_err(sql_error)?;
                let ids = q
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(sql_error)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(sql_error)?;
                ids
            };
            let mut recovered = Vec::new();
            for id in ids {
                let mut old = task(t, &id)?;
                old.state = if old.state == TaskState::CancelRequested {
                    TaskState::Cancelled
                } else if matches!(
                    old.kind,
                    TaskKind::Progress | TaskKind::AsrStream | TaskKind::Catchup
                ) {
                    TaskState::Superseded
                } else {
                    TaskState::Interrupted
                };
                t.execute(
                    "UPDATE tasks SET state=?2,updated_at=?3 WHERE task_id=?1",
                    params![
                        id,
                        storage::value(&old.state)?.as_str().unwrap_or(""),
                        storage::now()
                    ],
                )
                .map_err(sql_error)?;
                task_event(t, &old)?;
                recovered.push(old);
            }
            Ok(recovered)
        })
    }
}
pub(crate) fn reject_sensitive_metadata(value: &Value) -> Result<()> {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                let k = k.to_ascii_lowercase();
                if [
                    "token",
                    "secret",
                    "password",
                    "authorization",
                    "api_key",
                    "url",
                ]
                .iter()
                .any(|part| k.contains(part))
                {
                    return Err(error(
                        ErrorCode::InvalidArgument,
                        "Sensitive metadata is not persistable",
                    ));
                }
                reject_sensitive_metadata(v)?;
            }
        }
        Value::Array(a) => {
            for x in a {
                reject_sensitive_metadata(x)?;
            }
        }
        _ => (),
    }
    Ok(())
}

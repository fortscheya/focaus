//! focaus request generations, one immutable body and separate display timing.
use super::snapshots::{self, SnapshotStore};
use crate::{
    storage::{self, decode, encode, sql_error, Result, Store},
    tasks::TaskManager,
};
use focaus_contracts::*;
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

struct Request {
    generation: u64,
    task_id: String,
    result_id: String,
    deadline: Instant,
    input_hash: String,
    input: CatchupRequestInput,
    progress: Option<ProgressState>,
}
pub struct FocausCoordinator {
    store: Store,
    requests: Mutex<BTreeMap<String, Request>>,
}
impl FocausCoordinator {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            requests: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn begin(&self, mut input: CatchupRequestInput) -> Result<Task> {
        validate("CatchupRequestInput", &storage::value(&input)?)?;
        if input.remaining_deadline_ms == 0 {
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "focaus display deadline has elapsed",
            ));
        }
        let started = Instant::now();
        let remaining = input.remaining_deadline_ms.min(15_000);
        let deadline = started + Duration::from_millis(remaining);
        let input_hash = canonical_hash(&json!([
            input.view_id,
            input.lesson_id,
            input.recording_id,
            input.assignment_epoch,
            input.request_generation,
            input.triggered_at,
            input.requested_window,
            input.actual_tier
        ]));
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "focaus requests unavailable"))?;
        if let Some(old) = requests.get(&input.view_id) {
            if input.request_generation < old.generation {
                return Err(error(
                    ErrorCode::StaleInput,
                    "Older focaus request generation",
                ));
            }
            if input.request_generation == old.generation {
                if old.input_hash != input_hash {
                    return Err(error(
                        ErrorCode::IdempotencyMismatch,
                        "focaus generation reused with different input",
                    ));
                }
                return TaskManager::new(self.store.clone()).get(&old.task_id);
            }
        }
        let (cutoffs,progress)=self.store.read(|c| {
            let view=crate::windows::get(c,&input.view_id)?;
            if view.lesson_id!=input.lesson_id{return Err(error(ErrorCode::PermissionDenied,"focaus target differs from its view"));}
            let mut cutoffs=Vec::new();
            if let Some(recording)=&input.recording_id {
                let (lesson,epoch,durable):(Option<String>,u64,u64)=c.query_row("SELECT lesson_id,assignment_epoch,durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",[recording],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(sql_error)?;
                if lesson.as_deref()!=Some(input.lesson_id.as_str())||Some(epoch)!=input.assignment_epoch{return Err(error(ErrorCode::StaleInput,"focaus recording ownership changed"));}
                let end=input.requested_window.end_ms.unwrap_or(durable);if end>durable{return Err(error(ErrorCode::InvalidArgument,"focaus cutoff exceeds available audio"));}
                input.requested_window.recording_id=Some(recording.clone());input.requested_window.end_ms=Some(end);
                if input.requested_window.start_ms.is_none(){input.requested_window.start_ms=Some(end.saturating_sub(300_000));}
                if input.requested_window.start_ms.is_some_and(|start|start>end){return Err(error(ErrorCode::InvalidArgument,"Reversed focaus window"));}
                cutoffs.push(EvidenceCutoff {recording_id:recording.clone(),end_ms:Some(end),status:EvidenceCutoffStatus::Known});
            }else{
                let mut q=c.prepare("SELECT recording_id,durable_watermark_ms FROM recording_sessions WHERE lesson_id=?1 ORDER BY rowid").map_err(sql_error)?;
                for row in q.query_map([&input.lesson_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?))).map_err(sql_error)?{let (recording,end)=row.map_err(sql_error)?;cutoffs.push(EvidenceCutoff {recording_id:recording,end_ms:Some(end),status:EvidenceCutoffStatus::Known});}
            }
            let progress:Option<String>=c.query_row("SELECT body_json FROM progress_states WHERE lesson_id=?1",[&input.lesson_id],|r|r.get(0)).optional().map_err(sql_error)?;
            Ok((cutoffs,progress.map(|p|decode::<ProgressState>(&p)).transpose()?))
        })?;
        let snapshot = SnapshotStore::new(self.store.clone()).build_window(
            SnapshotBuildInput {
                purpose: TaskKind::Catchup,
                target_id: input.lesson_id.clone(),
                evidence_cutoff: cutoffs,
                language_tier: Some(input.actual_tier),
            },
            Some(&input.requested_window),
        )?;
        if Instant::now() >= deadline {
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "focaus snapshot exceeded remaining display deadline",
            ));
        }
        let result_id = storage::id();
        let task = Task {
            task_id: storage::id(),
            kind: TaskKind::Catchup,
            target_id: input.lesson_id.clone(),
            snapshot_id: snapshot.snapshot_id,
            state: TaskState::Queued,
            priority: 1,
            attempt: 1,
            deadline_at: Some(storage::wall_after(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64,
            )),
            idempotency_key: format!("{}:{}", input.view_id, input.request_generation),
            checkpoint: BTreeMap::from([
                ("view_id".into(), json!(input.view_id)),
                ("request_generation".into(), json!(input.request_generation)),
                ("result_id".into(), json!(result_id)),
                ("progress_at_trigger".into(), json!(progress)),
                ("trigger_input".into(), json!(input)),
            ]),
            error: None,
            provider_bindings: Vec::new(),
            created_at: storage::now(),
            updated_at: storage::now(),
            model_id: snapshot.model_id,
            region: snapshot.region,
            prompt_version: snapshot.prompt_version,
            parameters_hash: snapshot.parameters_hash,
        };
        let manager = TaskManager::new(self.store.clone());
        if let Some(old) = requests.get(&input.view_id) {
            let state = manager.get(&old.task_id)?;
            if !matches!(
                state.state,
                TaskState::Succeeded
                    | TaskState::Failed
                    | TaskState::Cancelled
                    | TaskState::Superseded
            ) {
                let _ = manager.transition(&old.task_id, state.attempt, TaskState::Superseded);
            }
        }
        manager.enqueue(task.clone())?;
        requests.insert(
            input.view_id.clone(),
            Request {
                generation: input.request_generation,
                task_id: task.task_id.clone(),
                result_id,
                deadline,
                input_hash,
                input,
                progress,
            },
        );
        Ok(task)
    }
    /// Called from host result submission only, never directly from a WebView.
    pub fn submit(
        &self,
        submission: &ResultSubmitInput,
        result: CatchupResult,
    ) -> Result<CommandResult> {
        validate("CatchupResult", &storage::value(&result)?)?;
        let requests = self
            .requests
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "focaus requests unavailable"))?;
        let request = requests
            .values()
            .find(|r| r.task_id == submission.task_id)
            .ok_or_else(|| {
                error(
                    ErrorCode::StaleInput,
                    "focaus request is not current in this host run",
                )
            })?;
        if request.generation != result.request_generation {
            return Err(error(
                ErrorCode::StaleInput,
                "focaus request generation changed",
            ));
        }
        if request.result_id != result.result_id {
            return Err(error(
                ErrorCode::InvalidResult,
                "focaus result identity was not allocated by the host",
            ));
        }
        let late = Instant::now() > request.deadline;
        let output=self.store.write(|t| {
            let task=crate::tasks::task(t,&submission.task_id)?;
            if task.attempt!=submission.attempt||!matches!(task.state,TaskState::Running|TaskState::Queued){return Err(error(ErrorCode::StaleInput,"focaus task attempt is no longer active"));}
            let snapshot=snapshots::load(t,&submission.snapshot_id)?;snapshots::check_frozen(t,&snapshot)?;
            let mut cursor=&snapshot;let mut parents=Vec::new();while cursor.snapshot_id!=task.snapshot_id {let parent=cursor.parent_snapshot_id.as_ref().ok_or_else(||error(ErrorCode::StaleInput,"Result snapshot is outside task input lineage"))?;parents.push(snapshots::load(t,parent)?);if parents.len()>32{return Err(error(ErrorCode::InvalidResult,"Snapshot lineage exceeds budget"));}cursor=parents.last().unwrap();}
            if result.snapshot_id!=snapshot.snapshot_id||result.body_evidence_cutoff!=snapshot.evidence_cutoff||result.presentation_tier!=request.input.actual_tier||result.progress_at_trigger!=request.progress||result.progress_binding.lesson_id!=request.input.lesson_id||result.progress_binding.recording_id!=request.input.recording_id||result.progress_binding.assignment_epoch!=request.input.assignment_epoch{return Err(error(ErrorCode::InvalidResult,"focaus result changed frozen body or pointer evidence"));}
            if result.result_semantics!="focaus_context_v2"||result.displayed_complete_elapsed_ms.is_some(){return Err(error(ErrorCode::InvalidResult,"Invalid focaus publication metadata"));}
            let mut body_sources=submission.candidate.clone();if let Some(o)=body_sources.as_object_mut(){o.remove("progress_at_trigger");}
            let mut refs=super::source_refs_in(&body_sources)?;refs.extend(submission.source_refs.clone());
            for source in &refs{super::check_source(t,source)?;if !super::source_allowed(source,&snapshot){return Err(error(ErrorCode::InvalidResult,"Result cited evidence outside its snapshot"));}}
            for node in &result.path_nodes {if node.evidence_role==PathNodeEvidenceRole::TeacherPath&&node.source_refs.is_empty(){return Err(error(ErrorCode::InvalidResult,"Classroom path node lacks evidence"));}}
            let prior:Option<String>=t.query_row("SELECT result_id FROM catchup_results WHERE view_id=?1 AND request_generation=?2",params![request.input.view_id,result.request_generation],|r|r.get(0)).optional().map_err(sql_error)?;
            if prior.is_some(){return Err(error(ErrorCode::IdempotencyMismatch,"This focaus body is already immutable"));}
            let body=crate::storage::commit_entity(t,&result.result_id,"catchup",storage::value(&result)?,"content.committed",json!({"kind":"view","view_id":request.input.view_id}),None,Some(&task.task_id))?;
            let revision=body["body_revision_id"].as_str().unwrap_or("");
            t.execute("INSERT INTO catchup_results VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![result.result_id,request.input.view_id,result.request_generation,snapshot.snapshot_id,revision,encode(&body)?,encode(&result.body_evidence_cutoff)?,encode(&result.progress_at_trigger)?,encode(&result.progress_binding)?,storage::value(&result.completion_status)?.as_str().unwrap_or(""),result.result_semantics,result.triggered_at,result.completed_at]).map_err(sql_error)?;
            let mut task=task;task.state=if late{TaskState::Failed}else{TaskState::Succeeded};t.execute("UPDATE tasks SET state=?2,updated_at=?3 WHERE task_id=?1",params![task.task_id,storage::value(&task.state)?.as_str().unwrap_or(""),storage::now()]).map_err(sql_error)?;crate::tasks::task_event(t,&task)?;
            t.execute("UPDATE outbox SET payload_json=?3 WHERE entity_id=?1 AND revision_id=?2 AND event_type='content.committed'",params![result.result_id,revision,encode(&json!({"result_id":result.result_id,"revision_id":revision,"request_generation":result.request_generation,"result_semantics":result.result_semantics,"completion_status":result.completion_status,"body_evidence_cutoff":result.body_evidence_cutoff,"progress_binding":result.progress_binding,"publication":if late{"review_required"}else{"committed"}}))?]).map_err(sql_error)?;
            Ok(CommandResult {contract_version:CONTRACT_VERSION.into(),command_id:task.task_id.clone(),status:CommandResultStatus::Ok,result:Some(json!({"publication":if late{"review_required"}else{"committed"},"result_id":result.result_id,"body_revision_id":revision,"request_generation":result.request_generation,"late":late})),committed_revisions:Some(BTreeMap::from([(result.result_id.clone(),revision.into())])),event_seq:Some(crate::storage::sequence(t)?),task_id:Some(task.task_id),current_state:Some(task.state),error:None})
        })?;
        Ok(output)
    }
    pub fn display_ack(&self, input: CatchupDisplayAckInput) -> Result<CommandResult> {
        self.store.write(|t| {
            let (generation,revision,completion):(u64,String,String)=t.query_row("SELECT request_generation,body_revision_id,completion_status FROM catchup_results WHERE result_id=?1",[&input.result_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql_error)?.ok_or_else(||error(ErrorCode::NotFound,"focaus result unavailable"))?;
            let host_completed:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM tasks t JOIN outbox o ON o.task_id=t.task_id WHERE o.entity_id=?1 AND o.revision_id=?2 AND t.state='succeeded')",params![input.result_id,revision],|r|r.get(0)).map_err(sql_error)?;
            if generation!=input.request_generation||revision!=input.body_revision_id{return Err(error(ErrorCode::StaleInput,"Displayed focaus body revision changed"));}
            let prior:Option<(u64,bool)>=t.query_row("SELECT elapsed_ms,displayed_complete FROM catchup_display_acks WHERE result_id=?1 AND request_generation=?2",params![input.result_id,input.request_generation],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql_error)?;
            if let Some((elapsed,complete))=prior {if elapsed!=input.elapsed_ms||complete!=input.displayed_complete{return Err(error(ErrorCode::IdempotencyMismatch,"Display timing acknowledgement is immutable"));}}else{t.execute("INSERT INTO catchup_display_acks VALUES(?1,?2,?3,?4,?5)",params![input.result_id,input.request_generation,input.body_revision_id,input.elapsed_ms,input.displayed_complete]).map_err(sql_error)?;}
            Ok(CommandResult {contract_version:CONTRACT_VERSION.into(),command_id:storage::id(),status:CommandResultStatus::Ok,result:Some(json!({"result_id":input.result_id,"displayed_complete":input.displayed_complete,"elapsed_ms":input.elapsed_ms,"met_deadline":host_completed&&input.displayed_complete&&completion=="complete"&&input.elapsed_ms<=15_000})),committed_revisions:Some(BTreeMap::new()),event_seq:Some(crate::storage::sequence(t)?),task_id:None,current_state:None,error:None})
        })
    }
}

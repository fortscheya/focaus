//! Persistent view identity and two-phase host-window handoff. Creating actual
//! Tauri windows is C's shell adapter responsibility after prepare commits.
use crate::storage::{self, decode, emit, encode, sql_error, Result, Store};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize)]
pub struct WindowRestorePlan {
    pub window_id: String,
    pub placement: String,
    pub view_ids: Vec<String>,
}

pub struct WindowCoordinator {
    store: Store,
}
impl WindowCoordinator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    pub fn recover(&self) -> Result<u64> {
        self.store.write(|t| {
            let transfers = {
                let mut q = t
                    .prepare("SELECT transfer_id FROM view_transfers WHERE state='preparing'")
                    .map_err(sql_error)?;
                let ids = q
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(sql_error)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(sql_error)?;
                ids
            };
            for id in &transfers {
                rollback(t, id)?;
            }
            Ok(transfers.len() as u64)
        })
    }
    /// Stable shell identity, persisted in the existing host intent registry.
    /// Adopt a sole legacy grid host once, so earlier random-run IDs do not
    /// strand an existing workspace on upgrade to the stable binding policy.
    pub fn persistent_window_id(&self, role: &str, owner: Option<&str>) -> Result<String> {
        if !["main", "course"].contains(&role) || (role == "course" && owner.is_none()) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Invalid persistent window role",
            ));
        }
        let key = canonical_hash(&json!([role, owner]));
        self.store.write(|t| {
            let old:Option<String>=t.query_row("SELECT response_json FROM idempotency WHERE actor_id='native.windows' AND command_name='register' AND idempotency_key=?1",[&key],|r|r.get(0)).optional().map_err(sql_error)?;
            if let Some(old)=old{return Ok(decode::<Value>(&old)?["window_id"].as_str().ok_or_else(||error(ErrorCode::CorruptAsset,"Invalid window identity receipt"))?.into());}
            let candidates={let mut q=t.prepare("SELECT DISTINCT v.host_window_id FROM views v JOIN lessons l ON l.lesson_id=v.lesson_id WHERE json_extract(v.state_json,'$.placement')='grid' AND (?1 IS NULL OR l.course_id=?1) ORDER BY v.rowid").map_err(sql_error)?;let ids=q.query_map([owner],|r|r.get::<_,String>(0)).map_err(sql_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(sql_error)?;ids};
            let id=if candidates.len()==1{candidates[0].clone()}else{storage::id()};
            t.execute("INSERT INTO idempotency VALUES('native.windows','register',?1,?1,?2)",params![key,encode(&json!({"window_id":id,"role":role,"owner_id":owner}))?]).map_err(sql_error)?;Ok(id)
        })
    }
    pub fn restore_plan(&self) -> Result<Vec<WindowRestorePlan>> {
        self.store.read(|c|{
            let mut grouped:BTreeMap<String,WindowRestorePlan>=BTreeMap::new();let mut q=c.prepare("SELECT view_id,host_window_id,state_json FROM views WHERE json_extract(state_json,'$.state')<>'closed' ORDER BY rowid").map_err(sql_error)?;
            for row in q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(sql_error)?{let(view,window,state)=row.map_err(sql_error)?;let state:Value=decode(&state)?;let plan=grouped.entry(window.clone()).or_insert(WindowRestorePlan {window_id:window,placement:state["placement"].as_str().unwrap_or("grid").into(),view_ids:Vec::new()});plan.view_ids.push(view);}
            Ok(grouped.into_values().collect())
        })
    }
    /// Target creation/ready timeout cleanup touches only this pending handoff.
    /// Already committed transfers are immutable and are never rolled back.
    pub fn rollback_transfer(&self, transfer_id: &str) -> Result<()> {
        self.store.write(|t| rollback(t, transfer_id))
    }
    pub fn confirm_initial_view(&self, view_id: &str, window: &str) -> Result<()> {
        self.store.write(|t|{authorize(t,view_id,window)?;t.execute("UPDATE views SET state_json=json_set(state_json,'$.initial_host_pending',json('false')) WHERE view_id=?1",[view_id]).map_err(sql_error)?;Ok(())})
    }
    pub fn initial_host_failed(&self, view_id: &str, window: &str) -> Result<()> {
        self.store.write(|t|{authorize(t,view_id,window)?;let state:String=t.query_row("SELECT state_json FROM views WHERE view_id=?1",[view_id],|r|r.get(0)).map_err(sql_error)?;let state:Value=decode(&state)?;
            if state["state"]=="closed"{return Ok(());}
            if state["placement"]!="standalone"||state["initial_host_pending"]!=true{return Err(error(ErrorCode::Conflict,"Initial host failure is stale"));}
            set_visibility(t,view_id,window,"closed",None)?;t.execute("UPDATE views SET state_json=json_set(state_json,'$.initial_host_pending',json('false')) WHERE view_id=?1",[view_id]).map_err(sql_error)?;Ok(())
        })
    }
}
pub(crate) fn get(c: &Connection, id: &str) -> Result<ContentView> {
    let row:Option<(String,String,String,String,String)>=c.query_row("SELECT lesson_id,target_json,host_window_id,view_revision,state_json FROM views WHERE view_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(sql_error)?;
    let (lesson, target, window, revision, state) =
        row.ok_or_else(|| error(ErrorCode::NotFound, "View unavailable"))?;
    let state: Value = decode(&state)?;
    Ok(ContentView {
        view_id: id.into(),
        lesson_id: lesson.clone(),
        target: decode(&target)?,
        host_window_id: window,
        view_revision: revision,
        current_owner_lesson_id: Some(lesson),
        state: serde_json::from_value(state["state"].clone())
            .map_err(|_| error(ErrorCode::CorruptAsset, "Invalid persisted view state"))?,
    })
}
pub(crate) fn authorize(c: &Connection, view: &str, window: &str) -> Result<()> {
    if get(c, view)?.host_window_id != window {
        return Err(error(
            ErrorCode::PermissionDenied,
            "View is hosted by another window",
        ));
    }
    Ok(())
}
fn grid_count(c: &Connection, window: &str) -> Result<u64> {
    let current:u64=c.query_row("SELECT COUNT(*) FROM views WHERE host_window_id=?1 AND json_extract(state_json,'$.state') IN ('open','transferring') AND json_extract(state_json,'$.placement')='grid'",[window],|r|r.get(0)).map_err(sql_error)?;
    let reserved: u64 = c
        .query_row(
            "SELECT COUNT(*) FROM view_transfers WHERE target_window_id=?1 AND state='preparing'",
            [window],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    Ok(current + reserved)
}
pub(crate) fn open(
    t: &Transaction<'_>,
    input: &ViewOpenInput,
    window: &str,
    cause: Option<&str>,
) -> Result<Value> {
    if crate::storage::lifecycle(t, &input.lesson_id)? != "active" {
        return Err(error(ErrorCode::SourceDeleted, "Lesson unavailable"));
    }
    if let ViewTarget::V0(target) = &input.target {
        if target.lesson_id != input.lesson_id {
            return Err(error(
                ErrorCode::InvalidArgument,
                "View collection lesson mismatch",
            ));
        }
    }
    if let ViewTarget::V1(target) = &input.target {
        if crate::storage::lifecycle(t, &target.entity_id)? == "deleted" {
            return Err(error(ErrorCode::SourceDeleted, "View target deleted"));
        }
    }
    let standalone = input.placement == ViewOpenInputPlacement::Standalone;
    if !standalone && grid_count(t, window)? >= 6 {
        return Err(error(
            ErrorCode::GridFull,
            "Six views are open; free a slot or open a standalone window",
        ));
    }
    let host = if standalone {
        storage::id()
    } else {
        window.into()
    };
    let vid = storage::id();
    let revision = storage::id();
    let state = json!({"state":"open","placement":input.placement,"initial_host_pending":standalone,"draft_revision":null,"selection":null,"scroll":0,"camera":null,"search":null});
    t.execute(
        "INSERT INTO views VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            vid,
            input.lesson_id,
            encode(&input.target)?,
            host,
            revision,
            encode(&state)?
        ],
    )
    .map_err(sql_error)?;
    emit(
        t,
        "workspace.view_changed",
        &vid,
        1,
        &revision,
        json!({"kind":"view","view_id":vid}),
        json!({"view_id":vid,"host_window_id":host,"state":"open"}),
        cause,
        None,
    )?;
    storage::value(&get(t, &vid)?)
}
pub(crate) fn save(
    t: &Transaction<'_>,
    input: &ViewSaveStateInput,
    window: &str,
    cause: Option<&str>,
) -> Result<Value> {
    authorize(t, &input.view_id, window)?;
    let view = get(t, &input.view_id)?;
    if view.view_revision != input.revision_id {
        return Err(error(ErrorCode::Conflict, "View revision changed"));
    }
    if view.state == ContentViewState::Transferring {
        return Err(error(ErrorCode::Conflict, "View handoff is preparing"));
    }
    if !input.scroll.is_finite() || input.scroll < 0. {
        return Err(error(ErrorCode::InvalidArgument, "Invalid view scroll"));
    }
    let current: String = t
        .query_row(
            "SELECT state_json FROM views WHERE view_id=?1",
            [&input.view_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let mut current: Value = decode(&current)?;
    let revision = storage::id();
    current["draft_revision"] = json!(input.draft_revision);
    current["selection"] = json!(input.selection);
    current["scroll"] = json!(input.scroll);
    current["camera"] = json!(input.camera);
    // Persist only search intent/anchor; snippets are always revalidated on open.
    current["search"]=input.search.as_ref().map(|s|json!({"query":s.query,"case_sensitive":s.case_sensitive,"scope":s.scope,"selected_hit_id":s.selected_hit_id})).unwrap_or(Value::Null);
    t.execute(
        "UPDATE views SET view_revision=?2,state_json=?3 WHERE view_id=?1",
        params![input.view_id, revision, encode(&current)?],
    )
    .map_err(sql_error)?;
    emit(
        t,
        "workspace.view_changed",
        &input.view_id,
        0,
        &revision,
        json!({"kind":"view","view_id":input.view_id}),
        json!({"view_id":input.view_id,"revision_id":revision}),
        cause,
        None,
    )?;
    storage::value(&get(t, &input.view_id)?)
}
pub(crate) fn prepare(
    t: &Transaction<'_>,
    input: &ViewMoveInput,
    window: &str,
    cause: Option<&str>,
) -> Result<Value> {
    authorize(t, &input.view_id, window)?;
    let view = get(t, &input.view_id)?;
    if view.view_revision != input.base_view_revision || view.state != ContentViewState::Open {
        return Err(error(
            ErrorCode::Conflict,
            "View head changed or is already moving",
        ));
    }
    let state: String = t
        .query_row(
            "SELECT state_json FROM views WHERE view_id=?1",
            [&input.view_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let mut state: Value = decode(&state)?;
    let composing:bool=t.query_row("SELECT EXISTS(SELECT 1 FROM edit_leases WHERE view_id=?1 AND (composition_active=1 OR draft_revision IS NOT NULL))",[&input.view_id],|r|r.get(0)).map_err(sql_error)?;
    if composing || !state["draft_revision"].is_null() {
        return Err(error(
            ErrorCode::EditProtected,
            "Commit or safely preserve composition and draft before handoff",
        ));
    }
    let target = input.target_window_id.clone().unwrap_or_else(storage::id);
    if target == window {
        return Err(error(
            ErrorCode::InvalidArgument,
            "View is already in that host",
        ));
    }
    if input.target_window_id.is_some() && grid_count(t, &target)? >= 6 {
        return Err(error(ErrorCode::GridFull, "Target grid is full"));
    }
    let epoch: u64 = t
        .query_row(
            "SELECT COALESCE(MAX(lease_epoch),0)+1 FROM edit_leases WHERE view_id=?1",
            [&input.view_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let transfer = storage::id();
    t.execute(
        "INSERT INTO view_transfers VALUES(?1,?2,?3,?4,?5,?6,'preparing')",
        params![
            transfer,
            input.view_id,
            window,
            target,
            input.base_view_revision,
            epoch
        ],
    )
    .map_err(sql_error)?;
    state["state"] = json!("transferring");
    state["target_placement"] = json!(if input.target_window_id.is_some() {
        "grid"
    } else {
        "standalone"
    });
    t.execute(
        "UPDATE views SET state_json=?2 WHERE view_id=?1",
        params![input.view_id, encode(&state)?],
    )
    .map_err(sql_error)?;
    emit(
        t,
        "workspace.view_changed",
        &input.view_id,
        0,
        &view.view_revision,
        json!({"kind":"view","view_id":input.view_id}),
        json!({"view_id":input.view_id,"transfer_id":transfer,"state":"preparing","target_window_id":target}),
        cause,
        None,
    )?;
    storage::value(&ViewTransfer {
        transfer_id: transfer,
        view_id: input.view_id.clone(),
        source_window_id: window.into(),
        target_window_id: target,
        base_view_revision: input.base_view_revision.clone(),
        lease_epoch: epoch,
        state: ViewTransferState::Preparing,
        draft_revision: None,
    })
}
pub(crate) fn ready(
    t: &Transaction<'_>,
    input: &ViewReadyInput,
    window: &str,
    cause: Option<&str>,
) -> Result<Value> {
    let (view_id,target,base,epoch,state):(String,String,String,u64,String)=t.query_row("SELECT view_id,target_window_id,base_view_revision,lease_epoch,state FROM view_transfers WHERE transfer_id=?1",[&input.transfer_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(sql_error)?;
    if target != window {
        return Err(error(
            ErrorCode::PermissionDenied,
            "Only the target WebView can acknowledge handoff",
        ));
    }
    if base != input.view_revision {
        return Err(error(ErrorCode::Conflict, "View handoff base changed"));
    }
    if state == "committed" {
        return storage::value(&get_transfer(t, &input.transfer_id)?);
    }
    if state != "preparing"
        || base != input.view_revision
        || get(t, &view_id)?.view_revision != base
    {
        return Err(error(ErrorCode::Conflict, "View handoff snapshot changed"));
    }
    let saved: String = t
        .query_row(
            "SELECT state_json FROM views WHERE view_id=?1",
            [&view_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    let mut saved: Value = decode(&saved)?;
    saved["state"] = json!("open");
    saved["placement"] = saved["target_placement"].take();
    let revision = storage::id();
    t.execute(
        "UPDATE views SET host_window_id=?2,view_revision=?3,state_json=?4 WHERE view_id=?1",
        params![view_id, target, revision, encode(&saved)?],
    )
    .map_err(sql_error)?;
    t.execute(
        "UPDATE edit_leases SET lease_epoch=?2 WHERE view_id=?1",
        params![view_id, epoch],
    )
    .map_err(sql_error)?;
    t.execute(
        "UPDATE view_transfers SET state='committed' WHERE transfer_id=?1",
        [&input.transfer_id],
    )
    .map_err(sql_error)?;
    emit(
        t,
        "workspace.view_changed",
        &view_id,
        0,
        &revision,
        json!({"kind":"view","view_id":view_id}),
        json!({"view_id":view_id,"transfer_id":input.transfer_id,"state":"committed","host_window_id":target}),
        cause,
        None,
    )?;
    storage::value(&get_transfer(t, &input.transfer_id)?)
}
pub(crate) fn get_transfer(c: &Connection, id: &str) -> Result<ViewTransfer> {
    let (view_id,source_window_id,target_window_id,base_view_revision,lease_epoch,state):(String,String,String,String,u64,String)=c.query_row("SELECT view_id,source_window_id,target_window_id,base_view_revision,lease_epoch,state FROM view_transfers WHERE transfer_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(sql_error)?;
    Ok(ViewTransfer {
        transfer_id: id.into(),
        view_id,
        source_window_id,
        target_window_id,
        base_view_revision,
        lease_epoch,
        state: decode(&encode(&state)?)?,
        draft_revision: None,
    })
}
pub(crate) fn rollback(t: &Transaction<'_>, transfer: &str) -> Result<()> {
    let view: Option<String> = t
        .query_row(
            "SELECT view_id FROM view_transfers WHERE transfer_id=?1 AND state='preparing'",
            [transfer],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some(view) = view {
        t.execute(
            "UPDATE view_transfers SET state='rolled_back' WHERE transfer_id=?1",
            [transfer],
        )
        .map_err(sql_error)?;
        t.execute(
            "UPDATE views SET state_json=json_set(state_json,'$.state','open') WHERE view_id=?1",
            [&view],
        )
        .map_err(sql_error)?;
        let rev = get(t, &view)?.view_revision;
        emit(
            t,
            "workspace.view_changed",
            &view,
            0,
            &rev,
            json!({"kind":"view","view_id":view}),
            json!({"view_id":view,"transfer_id":transfer,"state":"rolled_back"}),
            None,
            None,
        )?;
    }
    Ok(())
}
pub(crate) fn set_visibility(
    t: &Transaction<'_>,
    view: &str,
    window: &str,
    state: &str,
    cause: Option<&str>,
) -> Result<Value> {
    authorize(t, view, window)?;
    let current = get(t, view)?;
    if current.state == ContentViewState::Transferring {
        return Err(error(ErrorCode::Conflict, "View is moving"));
    }
    let composing: bool = t
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM edit_leases WHERE view_id=?1 AND composition_active=1)",
            [view],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if composing {
        return Err(error(
            ErrorCode::EditProtected,
            "Preserve composition before closing this view",
        ));
    }
    let rev = storage::id();
    t.execute("UPDATE views SET view_revision=?2,state_json=json_set(state_json,'$.state',?3) WHERE view_id=?1",params![view,rev,state]).map_err(sql_error)?;
    emit(
        t,
        "workspace.view_changed",
        view,
        0,
        &rev,
        json!({"kind":"view","view_id":view}),
        json!({"view_id":view,"state":state}),
        cause,
        None,
    )?;
    storage::value(&get(t, view)?)
}

//! Library transactions, immutable revisions and evidence publication gates.
pub mod catchup;
pub mod snapshots;
use crate::storage::{
    self, commit_entity, decode, emit, encode, head, lifecycle, sql_error, Result,
};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};

pub(crate) fn check_source(c: &Connection, source: &SourceRef) -> Result<()> {
    if lifecycle(c, &source.entity_id)? == "deleted" {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Evidence permanently deleted",
        ));
    }
    // Material SourceVersion refers to immutable binary bytes. Its hash is not
    // the canonical JSON hash of the material descriptor or parse manifest.
    let binary:Option<(String,String)>=c.query_row("SELECT r.sha256,r.state FROM materials m JOIN resources r ON r.resource_id=m.resource_id WHERE m.material_id=?1 AND m.source_version_id=?2",params![source.entity_id,source.revision_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql_error)?;
    if let Some((hash, state)) = binary {
        if state != "ready" {
            return Err(error(
                ErrorCode::SourceUnavailable,
                "Original material bytes unavailable",
            ));
        }
        if hash != source.content_hash {
            return Err(error(
                ErrorCode::StaleInput,
                "Original material hash changed",
            ));
        }
        if let Selector::V2(selector) = &source.selector {
            if selector.material_id != source.entity_id
                || selector.source_version_id != source.revision_id
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Material source selector mismatch",
                ));
            }
        }
        return Ok(());
    }
    let hash:Option<String>=c.query_row("SELECT content_hash FROM revisions WHERE revision_id=?1 AND entity_id=?2 AND body_json IS NOT NULL",params![source.revision_id,source.entity_id],|r|r.get(0)).optional().map_err(sql_error)?;
    if hash.as_deref() != Some(source.content_hash.as_str()) {
        return Err(error(
            ErrorCode::StaleInput,
            "Evidence revision or hash unavailable",
        ));
    }
    Ok(())
}
pub(crate) fn source_refs_in(value: &Value) -> Result<Vec<SourceRef>> {
    let mut refs = Vec::new();
    fn walk(v: &Value, refs: &mut Vec<SourceRef>) -> Result<()> {
        match v {
            Value::Object(o)
                if ["entity_id", "revision_id", "content_hash", "selector"]
                    .iter()
                    .all(|key| o.contains_key(*key)) =>
            {
                refs.push(serde_json::from_value(v.clone()).map_err(|_| {
                    error(
                        ErrorCode::InvalidResult,
                        "Malformed candidate source reference",
                    )
                })?);
            }
            Value::Object(o) => {
                for child in o.values() {
                    walk(child, refs)?;
                }
            }
            Value::Array(a) => {
                for child in a {
                    walk(child, refs)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    walk(value, &mut refs)?;
    Ok(refs)
}
pub(crate) fn source_allowed(source: &SourceRef, snapshot: &InputSnapshot) -> bool {
    snapshot.sources.iter().any(|allowed| {
        allowed.entity_id == source.entity_id
            && allowed.revision_id == source.revision_id
            && allowed.content_hash == source.content_hash
            && (matches!(allowed.selector, Selector::V4(_)) || allowed.selector == source.selector)
    })
}
pub(crate) fn reserve_identity(t: &Transaction<'_>, input: &IdentityReserveInput) -> Result<Value> {
    if input.candidate_keys.len() > 4096 || input.entity_type.len() > 64 {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Identity allocation budget exceeded",
        ));
    }
    let scope = crate::storage::lifecycle(t, &input.scope_id);
    if scope.is_err() {
        let task: bool = t
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1)",
                [&input.scope_id],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        if !task {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Identity scope unavailable",
            ));
        }
    }
    let mut mapping = std::collections::BTreeMap::new();
    for key in &input.candidate_keys {
        if key.len() > 512 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Identity matching key exceeds budget",
            ));
        }
        let hash = canonical_hash(&json!([input.scope_id, input.entity_type, key]));
        let old:Option<String>=t.query_row("SELECT response_json FROM idempotency WHERE actor_id='native.identity' AND command_name=?1 AND idempotency_key=?2",params![input.entity_type,hash],|r|r.get(0)).optional().map_err(sql_error)?;
        let id = if let Some(old) = old {
            decode::<Value>(&old)?["entity_id"]
                .as_str()
                .ok_or_else(|| error(ErrorCode::CorruptAsset, "Invalid identity receipt"))?
                .to_owned()
        } else {
            let id = storage::id();
            t.execute("INSERT INTO idempotency VALUES('native.identity',?1,?2,?2,?3)",params![input.entity_type,hash,encode(&json!({"entity_id":id,"scope_id":input.scope_id,"entity_type":input.entity_type}))?]).map_err(sql_error)?;
            id
        };
        mapping.insert(key.clone(), id);
    }
    Ok(json!({"mapping":mapping}))
}
pub(crate) fn create_course(
    t: &Transaction<'_>,
    input: &CourseCreateInput,
    cause: Option<&str>,
) -> Result<Value> {
    if input.name.trim().is_empty() {
        return Err(error(ErrorCode::InvalidArgument, "Course name required"));
    }
    let cid = storage::id();
    let body = commit_entity(
        t,
        &cid,
        "course",
        json!({"course_id":cid,"name":input.name,"revision_id":storage::id(),"entity_seq":0}),
        "content.committed",
        json!({"kind":"global"}),
        cause,
        None,
    )?;
    t.execute(
        "INSERT INTO courses VALUES(?1,?2)",
        params![cid, input.name],
    )
    .map_err(sql_error)?;
    Ok(body)
}
pub(crate) fn create_lesson(
    t: &Transaction<'_>,
    input: &LessonCreateInput,
    cause: Option<&str>,
) -> Result<Value> {
    if lifecycle(t, &input.course_id)? != "active" || input.title.trim().is_empty() {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Active course and lesson title required",
        ));
    }
    let lid = storage::id();
    let attachments = storage::id();
    let body = commit_entity(
        t,
        &lid,
        "lesson",
        json!({"lesson_id":lid,"course_id":input.course_id,"title":input.title,"actual_date":input.actual_date,"timezone":input.timezone,"requested_language_tier":3,"last_recording_tier":null,"material_scope":[],"revision_id":storage::id(),"entity_seq":0}),
        "content.committed",
        json!({"kind":"course","course_id":input.course_id}),
        cause,
        None,
    )?;
    t.execute("INSERT INTO lessons(lesson_id,course_id,title,actual_date,timezone,attachment_set_revision) VALUES(?1,?2,?3,?4,?5,?6)",params![lid,input.course_id,input.title,input.actual_date,input.timezone,attachments]).map_err(sql_error)?;
    Ok(body)
}
pub(crate) fn get_content(c: &Connection, input: &ContentGetInput) -> Result<Value> {
    if lifecycle(c, &input.entity_id)? == "deleted" {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Content permanently deleted",
        ));
    }
    if let Some(revision) = &input.revision_id {
        let body: Option<String> = c
            .query_row(
                "SELECT body_json FROM revisions WHERE entity_id=?1 AND revision_id=?2",
                params![input.entity_id, revision],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql_error)?
            .flatten();
        body.map(|s| decode(&s))
            .unwrap_or_else(|| Err(error(ErrorCode::NotFound, "Content revision unavailable")))
    } else {
        head(c, &input.entity_id)
    }
}
pub(crate) fn transfer(
    t: &Transaction<'_>,
    input: &RecordingTransferInput,
    cause: Option<&str>,
) -> Result<Value> {
    if lifecycle(t, &input.target_lesson_id)? != "active" {
        return Err(error(ErrorCode::SourceDeleted, "Target lesson unavailable"));
    }
    let (epoch,state,tier):(u64,String,u64)=t.query_row("SELECT assignment_epoch,state,capture_language_tier FROM recording_sessions WHERE recording_id=?1",[&input.recording_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(sql_error)?;
    if epoch != input.expected_epoch {
        return Err(error(ErrorCode::Conflict, "Recording assignment changed"));
    }
    let mut body = head(t, &input.recording_id)?;
    let old = body["lesson_id"].clone();
    body["lesson_id"] = json!(input.target_lesson_id);
    body["assignment_epoch"] = json!(epoch + 1);
    t.execute("UPDATE recording_sessions SET lesson_id=?2,assignment_epoch=assignment_epoch+1 WHERE recording_id=?1",params![input.recording_id,input.target_lesson_id]).map_err(sql_error)?;
    if ["starting", "recording", "paused", "stopping"].contains(&state.as_str()) {
        t.execute(
            "UPDATE lessons SET last_recording_tier=?2 WHERE lesson_id=?1",
            params![input.target_lesson_id, tier],
        )
        .map_err(sql_error)?;
    }
    // Both transcript tables resolve ownership through recording_id. No transcript
    // IDs, stable text, note rows or material attachments are moved or rewritten.
    let changed = commit_entity(
        t,
        &input.recording_id,
        "recording",
        body,
        "recording.state_changed",
        json!({"kind":"global"}),
        cause,
        None,
    )?;
    let snapshots: Vec<String> = {
        let mut q=t.prepare("SELECT snapshot_id FROM input_snapshots WHERE json_extract(body_json,'$.recording_id')=?1 AND json_extract(body_json,'$.assignment_epoch')<>?2").map_err(sql_error)?;
        let rows = q
            .query_map(params![input.recording_id, epoch + 1], |r| r.get(0))
            .map_err(sql_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows
    };
    for snapshot in snapshots {
        t.execute("UPDATE tasks SET state='superseded',updated_at=?2 WHERE snapshot_id=?1 AND kind NOT IN ('asr_stream','asr_file') AND state NOT IN ('succeeded','failed','cancelled','superseded')",params![snapshot,storage::now()]).map_err(sql_error)?;
    }
    t.execute("UPDATE source_edges SET freshness='update_available' WHERE to_entity_id=?1 OR to_entity_id IN (SELECT utterance_id FROM raw_utterances WHERE recording_id=?1) OR to_entity_id IN (SELECT secondary_id FROM secondary_segments WHERE recording_id=?1)",[&input.recording_id]).map_err(sql_error)?;
    t.execute(
        "UPDATE search_sessions SET state_json=json_set(state_json,'$.status','invalidated')",
        [],
    )
    .map_err(sql_error)?;
    emit(
        t,
        "search.invalidated",
        &input.recording_id,
        changed["entity_seq"].as_u64().unwrap_or(0),
        changed["revision_id"].as_str().unwrap_or(""),
        json!({"kind":"global"}),
        json!({"reason":"transferred","visibility_epoch":crate::storage::epoch(t)?,"must_clear":true,"old_lesson_id":old,"new_lesson_id":input.target_lesson_id}),
        cause,
        None,
    )?;
    Ok(changed)
}

pub(crate) fn selector_ids(selector: &Selector) -> Vec<String> {
    match selector {
        Selector::V0(x) => vec![x.audio_span_id.clone()],
        Selector::V1(x) => {
            let mut ids = vec![x.block_id.clone()];
            ids.extend(x.anchor.element_ids.clone());
            ids
        }
        Selector::V2(x) => vec![x.page_id.clone(), x.region_id.clone()],
        Selector::V3(x) => {
            let mut ids = vec![x.object_id.clone()];
            ids.extend(x.element_ids.clone());
            ids
        }
        Selector::V4(_) => vec![],
    }
}
pub(crate) fn intersects(a: &Selector, b: &Selector) -> bool {
    if matches!(a, Selector::V4(_)) || matches!(b, Selector::V4(_)) {
        return true;
    }
    match (a, b) {
        (Selector::V1(a), Selector::V1(b)) if a.block_id == b.block_id => {
            if a.anchor.element_ids.is_empty() || b.anchor.element_ids.is_empty() {
                true
            } else {
                a.anchor
                    .element_ids
                    .iter()
                    .any(|id| b.anchor.element_ids.contains(id))
            }
        }
        (Selector::V3(a), Selector::V3(b)) if a.object_id == b.object_id => {
            a.element_ids.is_empty()
                || b.element_ids.is_empty()
                || a.element_ids.iter().any(|id| b.element_ids.contains(id))
        }
        _ => {
            let ids = selector_ids(a);
            selector_ids(b).iter().any(|x| ids.contains(x))
        }
    }
}
pub(crate) fn begin_edit(t: &Transaction<'_>, lease: &EditLease) -> Result<Value> {
    let moving: bool = t
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM view_transfers WHERE view_id=?1 AND state='preparing')",
            [&lease.view_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if moving {
        return Err(error(
            ErrorCode::EditProtected,
            "View handoff freezes new edits",
        ));
    }
    let _ = head(t, &lease.target_id)?;
    let mut q = t
        .prepare("SELECT view_id,selectors_json,lease_epoch FROM edit_leases WHERE target_id=?1")
        .map_err(sql_error)?;
    for row in q
        .query_map([&lease.target_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u64>(2)?,
            ))
        })
        .map_err(sql_error)?
    {
        let (view, selectors, epoch) = row.map_err(sql_error)?;
        if view == lease.view_id {
            if lease.lease_epoch <= epoch {
                return Err(error(ErrorCode::Conflict, "Edit lease epoch must increase"));
            }
        } else {
            let selectors: Vec<Selector> = decode(&selectors)?;
            if selectors
                .iter()
                .any(|a| lease.selectors.iter().any(|b| intersects(a, b)))
            {
                return Err(error(
                    ErrorCode::EditProtected,
                    "Another view is editing this range",
                ));
            }
        }
    }
    t.execute("INSERT INTO edit_leases VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(view_id,target_id) DO UPDATE SET lease_epoch=excluded.lease_epoch,selectors_json=excluded.selectors_json,composition_active=excluded.composition_active,draft_revision=excluded.draft_revision",params![lease.view_id,lease.target_id,lease.lease_epoch,encode(&lease.selectors)?,lease.composition_active,lease.draft_revision]).map_err(sql_error)?;
    storage::value(lease)
}
pub(crate) fn end_edit(t: &Transaction<'_>, input: &EditEndInput) -> Result<Value> {
    let current: Option<(u64, Option<String>)> = t
        .query_row(
            "SELECT lease_epoch,draft_revision FROM edit_leases WHERE view_id=?1 AND target_id=?2",
            params![input.view_id, input.target_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some((epoch, draft)) = current {
        if epoch != input.lease_epoch {
            return Err(error(ErrorCode::Conflict, "Edit lease changed"));
        }
        // Releasing an active lease does not authorize overwriting an uncommitted draft.
        if input.draft_revision.is_some() || draft.is_some() {
            t.execute("UPDATE edit_leases SET composition_active=0,draft_revision=?3 WHERE view_id=?1 AND target_id=?2",params![input.view_id,input.target_id,input.draft_revision.clone().or(draft)]).map_err(sql_error)?;
        } else {
            t.execute(
                "DELETE FROM edit_leases WHERE view_id=?1 AND target_id=?2",
                params![input.view_id, input.target_id],
            )
            .map_err(sql_error)?;
        }
    }
    Ok(json!({"released":input.draft_revision.is_none()}))
}

//! Frozen, versioned host input and bounded prerequisite retrieval.
use super::check_source;
use crate::storage::{self, decode, encode, head, lifecycle, sql_error, Result, Store};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct SnapshotStore {
    store: Store,
}
impl SnapshotStore {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    pub fn build(&self, input: SnapshotBuildInput) -> Result<InputSnapshot> {
        self.build_window(input, None)
    }
    pub fn build_window(
        &self,
        input: SnapshotBuildInput,
        window: Option<&CatchupWindow>,
    ) -> Result<InputSnapshot> {
        let snapshot = self.store.read(|c| assemble(c, &input, window))?;
        validate("InputSnapshot", &storage::value(&snapshot)?)?;
        self.store.write(|t| {
            check_frozen(t, &snapshot)?;
            persist(t, &snapshot)?;
            Ok(snapshot.clone())
        })
    }
    pub fn get(&self, id: &str) -> Result<InputSnapshot> {
        self.store.read(|c| load(c, id))
    }
    pub fn context(&self, input: ContextRetrieveInput) -> Result<ContextResult> {
        if input.remaining_budget_ms == 0 {
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "focaus retrieval deadline exceeded",
            ));
        }
        if input.definition_ids.len() + input.binding_ids.len() > 128 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Prerequisite retrieval limit exceeded",
            ));
        }
        let started = Instant::now();
        let budget = Duration::from_millis(input.remaining_budget_ms.min(200));
        let mut result=self.store.read(|c| {
            let original=load(c,&input.snapshot.snapshot_id)?;
            if original!=input.snapshot{return Err(error(ErrorCode::StaleInput,"Input snapshot was modified"));}
            check_frozen(c,&original)?;
            let lesson=original.lesson_id.as_deref().ok_or_else(||error(ErrorCode::InvalidArgument,"A lesson is required for prerequisite retrieval"))?;
            let course:String=c.query_row("SELECT course_id FROM lessons WHERE lesson_id=?1",[lesson],|r|r.get(0)).map_err(sql_error)?;
            if course!=input.course_id{return Err(error(ErrorCode::PermissionDenied,"Prerequisite course is outside frozen scope"));}
            let receipt:String=c.query_row("SELECT response_json FROM idempotency WHERE actor_id='native.snapshots' AND command_name='snapshot.build' AND idempotency_key=?1",[&original.snapshot_id],|r|r.get(0)).map_err(sql_error)?;let watermark:Value=decode(&receipt)?;let seq=watermark["snapshot_seq"].as_u64().unwrap_or(0);
            let mut definitions=Vec::new();let mut bindings=Vec::new();let mut refs=Vec::new();let mut limitations=Vec::new();
            for id in &input.definition_ids {
                if started.elapsed()>budget{return Err(error(ErrorCode::DeadlineExceeded,"focaus prerequisite retrieval budget exceeded"));}
                let row:Option<(String,String)>=c.query_row("SELECT d.lesson_id,d.body_json FROM definitions d JOIN lessons l ON d.lesson_id=l.lesson_id JOIN entities e ON e.entity_id=d.definition_id WHERE d.definition_id=?1 AND l.course_id=?2 AND e.lifecycle='active'",params![id,course],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql_error)?;
                let Some((owner,data))=row else{limitations.push("definition_unavailable".into());continue;};
                if owner!=lesson {
                    let prior:bool=c.query_row("SELECT a.actual_date IS NOT NULL AND b.actual_date IS NOT NULL AND a.actual_date<b.actual_date FROM lessons a JOIN lessons b ON b.lesson_id=?2 WHERE a.lesson_id=?1",params![owner,lesson],|r|r.get(0)).map_err(sql_error)?;
                    if !prior{limitations.push("prior_lesson_order_unconfirmed".into());continue;}
                }
                let definition:Definition=decode(&data)?;
                if !evidence_eligible(c,&source(c,id,Selector::V4(WholeSelector {kind:"whole".into()}))?,&original,seq)?{limitations.push("definition_not_available_at_trigger".into());continue;}
                if !evidence_eligible(c,&definition.valid_from_anchor,&original,seq)?{limitations.push("definition_beyond_frozen_evidence".into());continue;}
                for source in &definition.source_refs {if evidence_eligible(c,source,&original,seq)? {refs.push(source.clone());}else{return Err(error(ErrorCode::StaleInput,"Prerequisite evidence exceeds frozen cutoff"));}}
                definitions.push(definition);
            }
            for id in &input.binding_ids {
                if started.elapsed()>budget{return Err(error(ErrorCode::DeadlineExceeded,"focaus prerequisite retrieval budget exceeded"));}
                let row:Option<String>=c.query_row("SELECT b.body_json FROM symbol_bindings b JOIN definitions d ON b.definition_id=d.definition_id JOIN lessons l ON d.lesson_id=l.lesson_id JOIN entities e ON e.entity_id=b.binding_id WHERE b.binding_id=?1 AND l.course_id=?2 AND d.lesson_id=?3 AND e.lifecycle='active'",params![id,course,lesson],|r|r.get(0)).optional().map_err(sql_error)?;
                if let Some(data)=row {let binding:SymbolBinding=decode(&data)?;if evidence_eligible(c,&source(c,id,Selector::V4(WholeSelector {kind:"whole".into()}))?,&original,seq)?&&evidence_eligible(c,&binding.valid_from_anchor,&original,seq)?{refs.push(binding.valid_from_anchor.clone());bindings.push(binding);}else{limitations.push("binding_beyond_frozen_evidence".into());}}
            }
            let mut snapshot=original.clone();snapshot.snapshot_id=storage::id();snapshot.parent_snapshot_id=Some(original.snapshot_id.clone());
            for source in &refs {if !snapshot.sources.contains(source){snapshot.sources.push(source.clone());}}
            Ok(ContextResult {snapshot,definitions,symbol_bindings:bindings,source_refs:refs,limitations})
        })?;
        if started.elapsed() > budget {
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "focaus retrieval deadline exceeded",
            ));
        }
        validate("InputSnapshot", &storage::value(&result.snapshot)?)?;
        self.store.write(|t| {
            check_frozen(t, &result.snapshot)?;
            persist(t, &result.snapshot)
        })?;
        result.snapshot.contract_version = CONTRACT_VERSION.into();
        Ok(result)
    }
}
pub(crate) fn load(c: &Connection, id: &str) -> Result<InputSnapshot> {
    let body: Option<String> = c
        .query_row(
            "SELECT body_json FROM input_snapshots WHERE snapshot_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .flatten();
    decode(&body.ok_or_else(|| error(ErrorCode::SourceUnavailable, "Frozen input unavailable"))?)
}
pub(crate) fn persist(t: &Transaction<'_>, snapshot: &InputSnapshot) -> Result<()> {
    t.execute(
        "INSERT INTO input_snapshots VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            snapshot.snapshot_id,
            snapshot.parent_snapshot_id,
            snapshot.contract_version,
            encode(snapshot)?,
            canonical_hash(&json!(snapshot)),
            snapshot.available_at
        ],
    )
    .map_err(sql_error)?;
    // Internal audit receipt pins availability to an outbox watermark without
    // inventing fields in the public InputSnapshot or another metadata database.
    let watermark = if let Some(parent) = &snapshot.parent_snapshot_id {
        let receipt:String=t.query_row("SELECT response_json FROM idempotency WHERE actor_id='native.snapshots' AND command_name='snapshot.build' AND idempotency_key=?1",[parent],|r|r.get(0)).map_err(sql_error)?;
        decode::<Value>(&receipt)?
    } else {
        json!({"snapshot_seq":crate::storage::sequence(t)?,"visibility_epoch":crate::storage::epoch(t)?})
    };
    t.execute(
        "INSERT INTO idempotency VALUES('native.snapshots','snapshot.build',?1,?2,?3)",
        params![
            snapshot.snapshot_id,
            canonical_hash(&json!(snapshot)),
            encode(&watermark)?
        ],
    )
    .map_err(sql_error)?;
    Ok(())
}
pub(crate) fn check_frozen(c: &Connection, s: &InputSnapshot) -> Result<()> {
    if s.contract_version != CONTRACT_VERSION {
        return Err(error(
            ErrorCode::UnsupportedVersion,
            "Frozen input contract changed",
        ));
    }
    if lifecycle(c, &s.target_id)? == "deleted" {
        return Err(error(ErrorCode::SourceDeleted, "Task target deleted"));
    }
    if let (Some(recording), Some(epoch)) = (&s.recording_id, s.assignment_epoch) {
        let current: u64 = c
            .query_row(
                "SELECT assignment_epoch FROM recording_sessions WHERE recording_id=?1",
                [recording],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        if current != epoch {
            return Err(error(
                ErrorCode::StaleInput,
                "Recording ownership epoch changed",
            ));
        }
    }
    for source in &s.sources {
        check_source(c, source)?;
    }
    Ok(())
}
fn source(c: &Connection, id: &str, selector: Selector) -> Result<SourceRef> {
    let (revision,hash):(String,String)=c.query_row("SELECT r.revision_id,r.content_hash FROM entities e JOIN revisions r ON e.revision_id=r.revision_id WHERE e.entity_id=?1 AND e.lifecycle<>'deleted'",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(sql_error)?;
    Ok(SourceRef {
        entity_id: id.into(),
        revision_id: revision,
        content_hash: hash,
        selector,
    })
}
fn assemble(
    c: &Connection,
    input: &SnapshotBuildInput,
    window: Option<&CatchupWindow>,
) -> Result<InputSnapshot> {
    let target = head(c, &input.target_id)?;
    let mut recording = target["recording_id"].as_str().map(str::to_owned);
    if recording.is_none() && input.evidence_cutoff.len() == 1 {
        recording = Some(input.evidence_cutoff[0].recording_id.clone());
    }
    let lesson = if let Some(recording) = &recording {
        c.query_row(
            "SELECT lesson_id FROM recording_sessions WHERE recording_id=?1",
            [recording],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(sql_error)?
        .flatten()
    } else {
        target["lesson_id"].as_str().map(str::to_owned)
    };
    if let Some(expected) = target["lesson_id"].as_str() {
        if lesson.as_deref() != Some(expected) {
            return Err(error(
                ErrorCode::PermissionDenied,
                "Evidence recording is outside target lesson",
            ));
        }
    }
    let mut epoch = None;
    let mut available = BTreeMap::new();
    for cutoff in &input.evidence_cutoff {
        let durable: u64 = c
            .query_row(
                "SELECT durable_watermark_ms FROM recording_sessions WHERE recording_id=?1",
                [&cutoff.recording_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| error(ErrorCode::NotFound, "Evidence recording unavailable"))?;
        if cutoff.end_ms.is_some_and(|end| end > durable) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Evidence cutoff exceeds durable audio",
            ));
        }
        available.insert(cutoff.recording_id.clone(), cutoff.end_ms);
    }
    if let Some(rid) = &recording {
        epoch = Some(
            c.query_row(
                "SELECT assignment_epoch FROM recording_sessions WHERE recording_id=?1",
                [rid],
                |r| r.get::<_, u64>(0),
            )
            .map_err(sql_error)?,
        );
    }
    let (attachments, facts) = if let Some(lid) = &lesson {
        c.query_row(
            "SELECT attachment_set_revision,fact_set_revision FROM lessons WHERE lesson_id=?1",
            [lid],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .map_err(sql_error)?
    } else {
        (
            target["revision_id"]
                .as_str()
                .unwrap_or(&input.target_id)
                .into(),
            None,
        )
    };
    let mut sources = Vec::new();
    let asr = matches!(input.purpose, TaskKind::AsrStream | TaskKind::AsrFile);
    if asr {
        for (rid, end) in &available {
            let Some(end) = end else {
                continue;
            };
            let mut q=c.prepare("SELECT audio_span_id FROM audio_spans WHERE recording_id=?1 AND end_ms<=?2 ORDER BY start_ms,rowid").map_err(sql_error)?;
            for id in q
                .query_map(params![rid, end], |r| r.get::<_, String>(0))
                .map_err(sql_error)?
            {
                let id = id.map_err(sql_error)?;
                let span: AudioSpan = decode(&encode(&head(c, &id)?)?)?;
                let selector = if let (Some(track), Some(range), Some(mapping)) =
                    (span.track_id, span.frame_range, span.mapping_revision_id)
                {
                    Selector::V0(AudioSelector {
                        kind: "audio".into(),
                        recording_id: rid.clone(),
                        audio_span_id: id.clone(),
                        track_id: track,
                        start_frame: range.start_frame,
                        end_frame: range.end_frame,
                        start_ms: span.timeline_range.start_ms,
                        end_ms: span.timeline_range.end_ms,
                        mapping_revision_id: mapping,
                    })
                } else {
                    Selector::V4(WholeSelector {
                        kind: "whole".into(),
                    })
                };
                sources.push(source(c, &id, selector)?);
            }
        }
    } else if let Some(lid) = &lesson {
        let start = window.and_then(|w| w.start_ms).unwrap_or(0);
        for (table, key) in [
            ("raw_utterances", "utterance_id"),
            ("secondary_segments", "secondary_id"),
        ] {
            let sql=format!("SELECT u.{key},u.recording_id FROM {table} u JOIN recording_sessions r ON u.recording_id=r.recording_id JOIN entities e ON u.{key}=e.entity_id WHERE r.lesson_id=?1 AND e.lifecycle='active' ORDER BY u.rowid");
            let mut q = c.prepare(&sql).map_err(sql_error)?;
            for row in q
                .query_map([lid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(sql_error)?
            {
                let (id, rid) = row.map_err(sql_error)?;
                let Some(Some(cutoff)) = available.get(&rid) else {
                    continue;
                };
                let audio_key = if table == "raw_utterances" {
                    "audio_spans_json"
                } else {
                    "ordered_spans_json"
                };
                let range_sql=format!("SELECT MIN(a.start_ms),MAX(a.end_ms) FROM {table} u,json_each(u.{audio_key}) j JOIN audio_spans a ON a.audio_span_id=j.value WHERE u.{key}=?1");
                let (begin, end): (Option<u64>, Option<u64>) = c
                    .query_row(&range_sql, [&id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(sql_error)?;
                if end.is_none()
                    || end.is_some_and(|e| e > *cutoff)
                    || begin.is_some_and(|b| b < start)
                {
                    continue;
                }
                sources.push(source(
                    c,
                    &id,
                    Selector::V4(WholeSelector {
                        kind: "whole".into(),
                    }),
                )?);
            }
        }
        // Attachments are frozen by exact material versions and current scope.
        let mut q=c.prepare("SELECT DISTINCT a.material_id FROM material_attachments a JOIN entities e ON e.entity_id=a.material_id WHERE a.lesson_id=?1 AND a.lifecycle='active' AND e.lifecycle='active' ORDER BY a.rowid").map_err(sql_error)?;
        for id in q
            .query_map([lid], |r| r.get::<_, String>(0))
            .map_err(sql_error)?
        {
            sources.push(source(
                c,
                &id.map_err(sql_error)?,
                Selector::V4(WholeSelector {
                    kind: "whole".into(),
                }),
            )?);
        }
    }
    if sources.len() > 4096 {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Frozen evidence exceeds bounded snapshot budget",
        ));
    }
    let model = match input.purpose {
        TaskKind::AsrStream => Some("qwen-audio-3.1-asr-flash-streaming"),
        TaskKind::AsrFile => Some("qwen-audio-3.1-asr-flash-filetrans"),
        TaskKind::Progress => Some("decision-model-preview"),
        TaskKind::Secondary
        | TaskKind::NoteIncremental
        | TaskKind::NoteReorganize
        | TaskKind::LanguageRender
        | TaskKind::Catchup
        | TaskKind::Qa
        | TaskKind::MaterialUnderstand
        | TaskKind::VisualizationDiscover
        | TaskKind::VisualizationBuild => Some("qwen3.8-flash"),
        _ => None,
    };
    Ok(InputSnapshot {
        snapshot_id: storage::id(),
        parent_snapshot_id: None,
        contract_version: CONTRACT_VERSION.into(),
        purpose: input.purpose.clone(),
        target_id: input.target_id.clone(),
        lesson_id: lesson,
        recording_id: recording,
        assignment_epoch: epoch,
        sources,
        attachment_set_revision: attachments,
        fact_set_revision: facts,
        target_base_revision: target["revision_id"].as_str().map(str::to_owned),
        edit_revision: None,
        language_tier: input.language_tier,
        evidence_cutoff: input.evidence_cutoff.clone(),
        available_at: storage::now(),
        executor_id: if model.is_some() {
            "focaus.ai".into()
        } else {
            "focaus.native".into()
        },
        executor_version: "1.0.0".into(),
        model_id: model.map(|m| serde_json::from_value(json!(m)).unwrap()),
        region: model.map(|_| "cn-beijing".into()),
        prompt_version: model.map(|_| "1.0.0".into()),
        parameters_hash: canonical_hash(
            &json!({"purpose":input.purpose,"tier":input.language_tier,"window":window}),
        ),
        calibration_seq: None,
        decision_seq: None,
    })
}
fn evidence_eligible(
    c: &Connection,
    source: &SourceRef,
    s: &InputSnapshot,
    seq: u64,
) -> Result<bool> {
    check_source(c, source)?;
    let committed: Option<u64> = c
        .query_row(
            "SELECT MAX(global_seq) FROM outbox WHERE revision_id=?1",
            [&source.revision_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if committed.is_none_or(|commit| commit > seq) {
        return Ok(false);
    }
    let mut q=c.prepare("SELECT recording_id,MAX(end_ms) FROM audio_spans WHERE audio_span_id=?1 OR audio_span_id IN (SELECT value FROM raw_utterances,json_each(audio_spans_json) WHERE utterance_id=?1) OR audio_span_id IN (SELECT value FROM secondary_segments,json_each(ordered_spans_json) WHERE secondary_id=?1) GROUP BY recording_id").map_err(sql_error)?;
    for row in q
        .query_map([&source.entity_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
        })
        .map_err(sql_error)?
    {
        let (recording, end) = row.map_err(sql_error)?;
        if let Some(cutoff) = s
            .evidence_cutoff
            .iter()
            .find(|x| x.recording_id == recording)
        {
            if cutoff.end_ms.is_none_or(|cutoff| end > cutoff) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

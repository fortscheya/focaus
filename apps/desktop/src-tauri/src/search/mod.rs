//! Rebuildable projection in the authoritative library. All reads recheck the
//! visibility barrier, owners and pinned revisions before returning any snippet.
mod unicode15;
use crate::storage::{
    self, check_revision, decode, emit, encode, epoch, head, lifecycle, sql_error, Result, Store,
};
use focaus_contracts::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
use unicode_normalization::char::{canonical_combining_class, compose, decompose_canonical};

pub const PROFILE: &str = "nfc_casefold_ws_v1";
#[derive(Clone)]
struct Token {
    c: char,
    start: u64,
    end: u64,
}
fn nfc(tokens: Vec<Token>) -> Vec<Token> {
    let mut decomposed = Vec::new();
    for t in tokens {
        decompose_canonical(t.c, |c| {
            decomposed.push(Token {
                c,
                start: t.start,
                end: t.end,
            })
        });
    }
    // Stable canonical ordering, bounded O(n log n) even for a long combining run.
    let mut begin = 0;
    for i in 0..=decomposed.len() {
        if i == decomposed.len() || canonical_combining_class(decomposed[i].c) == 0 {
            decomposed[begin..i].sort_by_key(|t| canonical_combining_class(t.c));
            begin = i + 1;
        }
    }
    let mut out: Vec<Token> = Vec::new();
    let mut starter: Option<usize> = None;
    let mut last_class = 0;
    for t in decomposed {
        let class = canonical_combining_class(t.c);
        if let Some(i) = starter {
            if last_class == 0 || last_class < class {
                if let Some(c) = compose(out[i].c, t.c) {
                    out[i].c = c;
                    out[i].start = out[i].start.min(t.start);
                    out[i].end = out[i].end.max(t.end);
                    continue;
                }
            }
        }
        if class == 0 {
            starter = Some(out.len());
        }
        last_class = class;
        out.push(t);
    }
    out
}
fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{0085}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}')
}
pub fn normalize(text: &str, case_sensitive: bool) -> Result<(String, Vec<OffsetEntry>)> {
    if text.len() > 4_000_000 {
        return Err(error(
            ErrorCode::InvalidArgument,
            "Search unit exceeds input budget",
        ));
    }
    let mut tokens = nfc(text
        .chars()
        .enumerate()
        .map(|(i, c)| Token {
            c,
            start: i as u64,
            end: i as u64 + 1,
        })
        .collect());
    if !case_sensitive {
        tokens = nfc(tokens
            .into_iter()
            .flat_map(|t| {
                let folded = unicode15::fold(t.c)
                    .map(str::to_owned)
                    .unwrap_or_else(|| t.c.to_string());
                folded
                    .chars()
                    .map(|c| Token {
                        c,
                        start: t.start,
                        end: t.end,
                    })
                    .collect::<Vec<_>>()
            })
            .collect());
    }
    let mut collapsed: Vec<Token> = Vec::new();
    for mut t in tokens {
        if whitespace(t.c) {
            t.c = ' ';
            if let Some(last) = collapsed.last_mut() {
                if last.c == ' ' {
                    last.start = last.start.min(t.start);
                    last.end = last.end.max(t.end);
                    continue;
                }
            }
        }
        collapsed.push(t);
    }
    let mut output = String::new();
    let mut map = Vec::with_capacity(collapsed.len());
    for (i, t) in collapsed.into_iter().enumerate() {
        output.push(t.c);
        map.push(OffsetEntry {
            normalized_range: ScalarRange {
                start: i as u64,
                end: i as u64 + 1,
            },
            original_range: ScalarRange {
                start: t.start,
                end: t.end,
            },
            element_ids: Vec::new(),
        });
    }
    Ok((output, map))
}
pub fn match_ranges(text: &str, map: &[OffsetEntry], query: &str) -> Vec<ScalarRange> {
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = query.chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut prefix = vec![0; needle.len()];
    let mut j = 0;
    for i in 1..needle.len() {
        while j > 0 && needle[i] != needle[j] {
            j = prefix[j - 1];
        }
        if needle[i] == needle[j] {
            j += 1;
        }
        prefix[i] = j;
    }
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    j = 0;
    for (i, c) in hay.into_iter().enumerate() {
        while j > 0 && c != needle[j] {
            j = prefix[j - 1];
        }
        if c == needle[j] {
            j += 1;
        }
        if j == needle.len() {
            let entries = &map[i + 1 - j..i + 1];
            let start = entries
                .iter()
                .map(|e| e.original_range.start)
                .min()
                .unwrap_or(0);
            let end = entries
                .iter()
                .map(|e| e.original_range.end)
                .max()
                .unwrap_or(0);
            if seen.insert((start, end)) {
                found.push(ScalarRange { start, end });
            }
            j = prefix[j - 1];
        }
    }
    found
}

/// Native content extraction covers canonical objects, not the mounted DOM.
pub fn extract(
    owner: &str,
    revision: &str,
    _content_hash: &str,
    kind: SearchUnitKind,
    tier: Option<u64>,
    body: &Value,
) -> Result<Vec<SearchUnit>> {
    let mut units = Vec::new();
    let mut fields: Vec<(String, String, Value)> = Vec::new();
    fn field(
        fields: &mut Vec<(String, String, Value)>,
        path: String,
        text: &Value,
        selector: &Value,
    ) {
        if let Some(text) = text.as_str() {
            if !text.is_empty() {
                fields.push((path, text.into(), selector.clone()));
            }
        }
    }
    fn walk(
        v: &Value,
        path: &str,
        selector: Value,
        tier: Option<u64>,
        fields: &mut Vec<(String, String, Value)>,
    ) {
        match v {
            Value::Object(o) => {
                let selector = if let Some(id) = o.get("object_id") {
                    json!({"kind":"object","object_id":id,"element_ids":[]})
                } else {
                    selector
                };
                if let Some(runs) = o.get("runs").and_then(Value::as_array) {
                    let text = runs
                        .iter()
                        .filter_map(|r| r["text"].as_str())
                        .collect::<String>();
                    let ids = runs
                        .iter()
                        .filter_map(|r| r.get("element_id").cloned())
                        .collect::<Vec<_>>();
                    let sel = json!({"kind":"text","block_id":o["block_id"],"anchor":{"left_element_id":null,"right_element_id":null,"element_ids":ids,"scalar_range":{"start":0,"end":text.chars().count()}}});
                    fields.push((format!("{path}/runs"), text, sel));
                }
                for (k, x) in o {
                    if k == "runs" {
                        continue;
                    }
                    let child = format!("{path}/{}", k.replace('~', "~0").replace('/', "~1"));
                    if k == "presentation_by_tier" {
                        if let Some(rows) = x.as_array() {
                            for (i, p) in rows.iter().enumerate() {
                                if p["tier"].as_u64() == tier {
                                    walk(
                                        p,
                                        &format!("{child}/{i}"),
                                        selector.clone(),
                                        tier,
                                        fields,
                                    );
                                }
                            }
                        }
                        continue;
                    }
                    let mut sel = selector.clone();
                    for id_key in ["cell_id", "node_id", "edge_id", "series_id", "axis_id"] {
                        if let Some(id) = o.get(id_key) {
                            if sel["kind"] == "object" {
                                sel["element_ids"] = json!([id]);
                            }
                        }
                    }
                    if [
                        "text",
                        "title",
                        "label",
                        "description",
                        "latex",
                        "alt",
                        "value",
                        "unit",
                        "formula",
                        "name",
                        "purpose",
                        "meaning",
                        "expression",
                    ]
                    .contains(&k.as_str())
                    {
                        field(fields, child.clone(), x, &sel);
                    }
                    if matches!(k.as_str(), "legend" | "labels") {
                        if let Some(rows) = x.as_array() {
                            for (i, s) in rows.iter().enumerate() {
                                field(fields, format!("{child}/{i}"), s, &sel);
                            }
                        }
                    }
                    if !matches!(
                        k.as_str(),
                        "source_refs"
                            | "validation"
                            | "provenance"
                            | "value_bindings"
                            | "parameters"
                            | "fallback"
                    ) {
                        walk(x, &child, sel, tier, fields);
                    }
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{path}/{i}"), selector.clone(), tier, fields);
                }
            }
            _ => (),
        }
    }
    let default = json!({"kind":"text","block_id":owner,"anchor":{"left_element_id":null,"right_element_id":null,"element_ids":[],"scalar_range":{"start":0,"end":0}}});
    walk(body, "", default, tier, &mut fields);
    for (field_path, display_text, mut selector) in fields {
        let count = display_text.chars().count() as u64;
        if selector["kind"] == "text" {
            selector["anchor"]["scalar_range"] = json!({"start":0,"end":count});
        }
        let selector: Selector = serde_json::from_value(selector)
            .map_err(|_| error(ErrorCode::InvalidResult, "Invalid native text anchor"))?;
        let (normalized_text, offset_map) = normalize(&display_text, false)?;
        let anchor = SearchAnchor {
            owner_entity_id: owner.into(),
            revision_id: revision.into(),
            selector,
            field_path: field_path.clone(),
            scalar_range: ScalarRange {
                start: 0,
                end: count,
            },
            audio_spans: Vec::new(),
            fallback: None,
        };
        let field_hash = search_text_hash(&display_text);
        units.push(SearchUnit {
            search_unit_id: storage::id(),
            owner_entity_id: owner.into(),
            revision_id: revision.into(),
            content_hash: field_hash,
            kind: kind.clone(),
            visible_tier: tier,
            field_path,
            display_text,
            normalized_text,
            normalization_profile: PROFILE.into(),
            offset_map,
            anchor,
            equivalence_group_id: None,
            source_role: if kind == SearchUnitKind::Material {
                "material_supplement".into()
            } else {
                "classroom".into()
            },
        });
    }
    Ok(units)
}

#[derive(Serialize, Deserialize)]
struct StoredSession {
    session: SearchSession,
    coverage: SearchCoverage,
}
#[derive(Clone)]
pub struct SearchCoordinator {
    store: Store,
}
impl SearchCoordinator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
    /// Projection calculation is done before the short generation-swap transaction.
    pub fn project(
        &self,
        owner: &str,
        revision: &str,
        units: Vec<SearchUnit>,
        mut coverage: SearchCoverage,
    ) -> Result<u64> {
        let mut normalized = Vec::with_capacity(units.len());
        let tier = units.first().and_then(|u| u.visible_tier);
        if units.iter().any(|u| u.visible_tier != tier) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Projection tiers must be committed separately",
            ));
        }
        let state_key = projection_key(owner, revision, tier);
        for mut unit in units {
            if unit.owner_entity_id != owner
                || unit.revision_id != revision
                || unit.anchor.owner_entity_id != owner
                || unit.anchor.revision_id != revision
            {
                return Err(error(ErrorCode::StaleInput, "Projection owner mismatch"));
            }
            if unit.content_hash != search_text_hash(&unit.display_text) {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Search field byte hash mismatch",
                ));
            }
            let (text, map) = normalize(&unit.display_text, false)?;
            unit.normalized_text = text;
            unit.offset_map = map;
            unit.normalization_profile = PROFILE.into();
            validate("SearchUnit", &storage::value(&unit)?)?;
            normalized.push(unit);
        }
        self.store.write(|t| {
            check_revision(t,owner,revision)?;
            let generation:u64=t.query_row("SELECT COALESCE(MAX(index_generation),0)+1 FROM search_index_state",[],|r|r.get(0)).map_err(sql_error)?;
            t.execute("DELETE FROM search_units WHERE owner_entity_id=?1 AND revision_id=?2 AND visible_tier IS ?3",params![owner,revision,tier]).map_err(sql_error)?;
            for u in &normalized {t.execute("INSERT INTO search_units VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![u.search_unit_id,u.owner_entity_id,u.revision_id,u.content_hash,storage::value(&u.kind)?.as_str().unwrap_or(""),u.visible_tier,u.field_path,u.display_text,u.normalized_text,PROFILE,encode(&u.offset_map)?,encode(&u.anchor)?,u.equivalence_group_id,u.source_role,generation]).map_err(sql_error)?;}
            coverage.indexed_units=normalized.len() as u64;coverage.scope_snapshot_id=storage::id();
            let visibility=epoch(t)?;let manifest_hash=canonical_hash(&json!({"owner":owner,"revision":revision,"profile":PROFILE}));let seq=crate::storage::sequence(t)?;
            t.execute("INSERT INTO search_index_state VALUES(?1,1,?2,?3,'[]',?4,?5,?6) ON CONFLICT(scope_id) DO UPDATE SET projection_version=1,index_generation=excluded.index_generation,head_manifest_hash=excluded.head_manifest_hash,dirty_ranges_json='[]',coverage_json=excluded.coverage_json,visibility_epoch=excluded.visibility_epoch,last_committed_seq=excluded.last_committed_seq",params![state_key,generation,manifest_hash,encode(&coverage)?,visibility,seq]).map_err(sql_error)?;
            emit(t,"search.index_changed",owner,0,revision,json!({"kind":"entity","entity_id":owner}),json!({"index_generation":generation,"coverage":coverage,"dirty_ranges":[]}),None,None)?;Ok(generation)
        })
    }
    pub fn project_native(
        &self,
        owner: &str,
        kind: SearchUnitKind,
        tier: Option<u64>,
    ) -> Result<u64> {
        let (body,revision,hash)=self.store.read(|c|{let body=head(c,owner)?;let (r,h):(String,String)=c.query_row("SELECT r.revision_id,r.content_hash FROM entities e JOIN revisions r ON e.revision_id=r.revision_id WHERE e.entity_id=?1",[owner],|r|Ok((r.get(0)?,r.get(1)?))).map_err(sql_error)?;Ok((body,r,h))})?;
        let units = extract(owner, &revision, &hash, kind, tier, &body)?;
        let count = units.len() as u64;
        self.project(
            owner,
            &revision,
            units,
            SearchCoverage {
                scope_snapshot_id: storage::id(),
                required_units: count,
                indexed_units: 0,
                required_pages: Vec::new(),
                indexed_pages: Vec::new(),
                unprocessed: Vec::new(),
                failed: Vec::new(),
                stale: Vec::new(),
                state: SearchCoverageState::Complete,
                source_extraction_revision: Some(revision.clone()),
            },
        )
    }
    pub fn open(&self, input: SearchOpenInput) -> Result<SearchSession> {
        self.store.write(|t| {
            let (lesson, target): (String, String) = t
                .query_row(
                    "SELECT lesson_id,target_json FROM views WHERE view_id=?1",
                    [&input.view_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(sql_error)?
                .ok_or_else(|| error(ErrorCode::NotFound, "Focused content view unavailable"))?;
            if decode::<ViewTarget>(&target)? != input.target {
                return Err(error(
                    ErrorCode::PermissionDenied,
                    "Search target is not bound to this view",
                ));
            }
            let (kind, ids) = resolve_targets(t, &lesson, &input.target)?;
            if input.version_mode == SearchOpenInputVersionMode::Historical
                && input.revision_ids.len() != ids.len()
            {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "Historical search needs explicit revisions",
                ));
            }
            let revisions = if input.version_mode == SearchOpenInputVersionMode::Historical {
                input.revision_ids.clone()
            } else {
                ids.iter()
                    .map(|id| {
                        t.query_row(
                            "SELECT revision_id FROM entities WHERE entity_id=?1",
                            [id],
                            |r| r.get::<_, String>(0),
                        )
                        .map_err(sql_error)
                    })
                    .collect::<Result<Vec<_>>>()?
            };
            let scope = SearchScope {
                kind,
                view_id: input.view_id.clone(),
                lesson_id: lesson,
                target_ids: ids,
                revision_ids: revisions,
                actual_visible_tier: input.actual_visible_tier,
                version_mode: if input.version_mode == SearchOpenInputVersionMode::Historical {
                    SearchScopeVersionMode::Historical
                } else {
                    SearchScopeVersionMode::Current
                },
            };
            let (generation, mut coverage) = coverage_for(t, &scope)?;
            let snapshot = storage::id();
            coverage.scope_snapshot_id = snapshot.clone();
            let session = SearchSession {
                search_id: storage::id(),
                view_id: input.view_id.clone(),
                query_seq: 0,
                query: String::new(),
                case_sensitive: false,
                scope,
                scope_snapshot_id: snapshot,
                index_generation: generation,
                visibility_epoch: epoch(t)?,
                selected_hit_id: None,
                cursor: None,
                status: if coverage.state == SearchCoverageState::Indexing {
                    SearchSessionStatus::Updating
                } else {
                    SearchSessionStatus::Ready
                },
            };
            t.execute(
                "INSERT INTO search_sessions VALUES(?1,?2,0,?3,?4,?5,?6)",
                params![
                    session.search_id,
                    session.view_id,
                    session.scope_snapshot_id,
                    generation,
                    session.visibility_epoch,
                    encode(&StoredSession {
                        session: session.clone(),
                        coverage
                    })?
                ],
            )
            .map_err(sql_error)?;
            Ok(session)
        })
    }
    /// Query seq and receipt are persisted; query work is bounded local R2 work.
    pub fn query(&self, input: SearchQueryInput) -> Result<SearchSession> {
        if input.query.chars().count() > 512 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "Search query exceeds input budget",
            ));
        }
        self.store.write(|t| {
            let mut stored = load_session(t, &input.search_id)?;
            check_session(t, &stored)?;
            if input.query_seq < stored.session.query_seq {
                return Err(error(ErrorCode::StaleInput, "Older search query"));
            }
            if input.query_seq == stored.session.query_seq {
                if input.query == stored.session.query
                    && input.case_sensitive == stored.session.case_sensitive
                {
                    return Ok(stored.session);
                }
                return Err(error(
                    ErrorCode::IdempotencyMismatch,
                    "Query sequence reused with different input",
                ));
            }
            stored.session.query_seq = input.query_seq;
            stored.session.query = input.query;
            stored.session.case_sensitive = input.case_sensitive;
            stored.session.status = SearchSessionStatus::Ready;
            t.execute(
                "UPDATE search_sessions SET query_seq=?2,state_json=?3 WHERE search_id=?1",
                params![input.search_id, input.query_seq, encode(&stored)?],
            )
            .map_err(sql_error)?;
            emit(
                t,
                "search.results_changed",
                &input.search_id,
                input.query_seq,
                &stored.session.scope_snapshot_id,
                json!({"kind":"view","view_id":stored.session.view_id}),
                json!({"search_id":input.search_id,"query_seq":input.query_seq,"status":"ready"}),
                None,
                None,
            )?;
            Ok(stored.session)
        })
    }
    pub fn query_task(&self, input: SearchQueryInput) -> Result<Task> {
        let session = self.query(input)?;
        let intent = format!("{}:{}", session.search_id, session.query_seq);
        let existing=self.store.read(|c|c.query_row("SELECT task_id FROM tasks WHERE kind='search_query' AND idempotency_key=?1 ORDER BY rowid LIMIT 1",[&intent],|r|r.get::<_,String>(0)).optional().map_err(sql_error))?;
        if let Some(existing) = existing {
            return crate::tasks::TaskManager::new(self.store.clone()).get(&existing);
        }
        let snapshot = InputSnapshot {
            snapshot_id: storage::id(),
            parent_snapshot_id: None,
            contract_version: CONTRACT_VERSION.into(),
            purpose: TaskKind::SearchQuery,
            target_id: session
                .scope
                .target_ids
                .first()
                .cloned()
                .unwrap_or(session.view_id.clone()),
            lesson_id: Some(session.scope.lesson_id.clone()),
            recording_id: None,
            assignment_epoch: None,
            sources: Vec::new(),
            attachment_set_revision: storage::id(),
            fact_set_revision: None,
            target_base_revision: None,
            edit_revision: None,
            language_tier: session.scope.actual_visible_tier,
            evidence_cutoff: Vec::new(),
            available_at: storage::now(),
            executor_id: "focaus.search".into(),
            executor_version: "1.0.0".into(),
            model_id: None,
            region: None,
            prompt_version: None,
            parameters_hash: canonical_hash(&json!([session.query, session.case_sensitive])),
            calibration_seq: None,
            decision_seq: None,
        };
        self.store.write(|t| {
            t.execute(
                "INSERT INTO input_snapshots VALUES(?1,NULL,?2,?3,?4,?5)",
                params![
                    snapshot.snapshot_id,
                    CONTRACT_VERSION,
                    encode(&snapshot)?,
                    canonical_hash(&json!(snapshot)),
                    snapshot.available_at
                ],
            )
            .map_err(sql_error)?;
            Ok(())
        })?;
        let manager = crate::tasks::TaskManager::new(self.store.clone());
        let task = Task {
            task_id: storage::id(),
            kind: TaskKind::SearchQuery,
            target_id: snapshot.target_id,
            snapshot_id: snapshot.snapshot_id,
            state: TaskState::Queued,
            priority: 2,
            attempt: 1,
            deadline_at: None,
            idempotency_key: intent,
            checkpoint: std::collections::BTreeMap::from([
                ("search_id".into(), json!(session.search_id)),
                ("query_seq".into(), json!(session.query_seq)),
            ]),
            error: None,
            provider_bindings: Vec::new(),
            created_at: storage::now(),
            updated_at: storage::now(),
            model_id: None,
            region: None,
            prompt_version: None,
            parameters_hash: snapshot.parameters_hash,
        };
        manager.enqueue(task.clone())?;
        manager.transition(&task.task_id, 1, TaskState::Running)?;
        match self.page(SearchPageInput {
            search_id: session.search_id,
            query_seq: session.query_seq,
            scope_snapshot_id: session.scope_snapshot_id,
            cursor: None,
        }) {
            Ok(_) => manager.transition(&task.task_id, 1, TaskState::Succeeded),
            Err(e) => {
                manager.transition(&task.task_id, 1, TaskState::Failed)?;
                Err(e)
            }
        }
    }
    pub fn authorize_session(&self, id: &str, window: &str) -> Result<()> {
        self.store.read(|c| {
            let stored = load_session(c, id)?;
            crate::windows::authorize(c, &stored.session.view_id, window)
        })
    }
    pub fn page(&self, input: SearchPageInput) -> Result<SearchPage> {
        self.store.read(|c| {
            let stored = load_session(c, &input.search_id)?;
            check_session(c, &stored)?;
            if stored.session.query_seq != input.query_seq
                || stored.session.scope_snapshot_id != input.scope_snapshot_id
            {
                return Err(error(
                    ErrorCode::SearchInvalidated,
                    "Search query snapshot changed",
                ));
            }
            if stored.coverage.state == SearchCoverageState::Indexing
                || stored.coverage.state == SearchCoverageState::Unavailable
            {
                return Err(error(
                    ErrorCode::IndexNotReady,
                    "Search projection not ready",
                ));
            }
            let all = find_hits(c, &stored)?;
            let offset = if let Some(cursor) = input.cursor {
                let (offset, hash) = cursor
                    .split_once(':')
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "Invalid search cursor"))?;
                if hash != cursor_hash(&stored) {
                    return Err(error(ErrorCode::SearchInvalidated, "Search cursor changed"));
                }
                offset
                    .parse::<usize>()
                    .map_err(|_| error(ErrorCode::InvalidArgument, "Invalid search cursor"))?
            } else {
                0
            };
            if offset > all.len() {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "Search cursor outside result",
                ));
            }
            let end = (offset + 50).min(all.len());
            Ok(SearchPage {
                search_id: input.search_id,
                query_seq: input.query_seq,
                scope_snapshot_id: input.scope_snapshot_id,
                total_count: all.len() as u64,
                count_status: if stored.coverage.state == SearchCoverageState::Complete {
                    SearchPageCountStatus::Exact
                } else {
                    SearchPageCountStatus::Provisional
                },
                hits: all[offset..end].to_vec(),
                coverage: stored.coverage,
                index_generation: stored.session.index_generation,
                visibility_epoch: stored.session.visibility_epoch,
                next_cursor: if end < all.len() {
                    Some(format!("{end}:{}", cursor_hash_session(&stored.session)))
                } else {
                    None
                },
            })
        })
    }
    pub fn locate(&self, input: SearchLocateInput) -> Result<SearchAnchor> {
        self.store.read(|c| {
            let stored = load_session(c, &input.search_id)?;
            if stored.session.view_id != input.view_id {
                return Err(error(
                    ErrorCode::PermissionDenied,
                    "Search belongs to another view",
                ));
            }
            check_session(c, &stored)?;
            let hit = find_hits(c, &stored)?
                .into_iter()
                .find(|h| h.hit_id == input.hit_id)
                .ok_or_else(|| error(ErrorCode::SearchInvalidated, "Search hit unavailable"))?;
            if hit.revision_id != input.expected_revision {
                return Err(error(ErrorCode::SearchInvalidated, "Hit revision changed"));
            }
            Ok(hit.matched_anchor)
        })
    }
    pub fn close(&self, id: &str) -> Result<()> {
        self.store.write(|t| {
            t.execute("DELETE FROM search_sessions WHERE search_id=?1", [id])
                .map_err(sql_error)?;
            Ok(())
        })
    }
}
fn cursor_hash(stored: &StoredSession) -> String {
    cursor_hash_session(&stored.session)
}
fn cursor_hash_session(s: &SearchSession) -> String {
    canonical_hash(&json!([
        s.search_id,
        s.query_seq,
        s.scope_snapshot_id,
        s.index_generation,
        s.visibility_epoch
    ]))
}
fn load_session(c: &Connection, id: &str) -> Result<StoredSession> {
    let data: Option<String> = c
        .query_row(
            "SELECT state_json FROM search_sessions WHERE search_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    decode(&data.ok_or_else(|| error(ErrorCode::NotFound, "Search session unavailable"))?)
}
fn check_session(c: &Connection, stored: &StoredSession) -> Result<()> {
    let s = &stored.session;
    if epoch(c)? != s.visibility_epoch {
        return Err(error(
            ErrorCode::SearchInvalidated,
            "Search visibility barrier changed",
        ));
    }
    for (owner, revision) in s.scope.target_ids.iter().zip(&s.scope.revision_ids) {
        if lifecycle(c, owner)? == "deleted" {
            return Err(error(
                ErrorCode::SourceDeleted,
                "Search source permanently deleted",
            ));
        }
        if s.scope.version_mode == SearchScopeVersionMode::Current
            && check_revision(c, owner, revision).is_err()
        {
            return Err(error(ErrorCode::SearchInvalidated, "Content head changed"));
        }
    }
    let (generation, _) = coverage_for(c, &s.scope)?;
    if generation != s.index_generation {
        return Err(error(
            ErrorCode::SearchInvalidated,
            "Search index generation changed",
        ));
    }
    Ok(())
}
fn resolve_targets(
    c: &Connection,
    lesson: &str,
    target: &ViewTarget,
) -> Result<(SearchScopeKind, Vec<String>)> {
    match target {
        ViewTarget::V1(entity) => {
            let kind: String = c
                .query_row(
                    "SELECT kind FROM entities WHERE entity_id=?1 AND lifecycle<>'deleted'",
                    [&entity.entity_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql_error)?
                .ok_or_else(|| error(ErrorCode::NotFound, "Search target unavailable"))?;
            let kind = match kind.as_str() {
                "note" => SearchScopeKind::Note,
                "material" => SearchScopeKind::Material,
                "raw" | "raw_utterance" => SearchScopeKind::Raw,
                "secondary" | "secondary_segment" => SearchScopeKind::Secondary,
                _ => {
                    return Err(error(
                        ErrorCode::UnsupportedCapability,
                        "Content type has no search projection",
                    ))
                }
            };
            Ok((kind, vec![entity.entity_id.clone()]))
        }
        ViewTarget::V0(collection) => {
            if collection.lesson_id != lesson {
                return Err(error(
                    ErrorCode::PermissionDenied,
                    "Collection lesson mismatch",
                ));
            }
            let name = storage::value(&collection.content_kind)?
                .as_str()
                .unwrap_or("")
                .to_owned();
            let (kind,sql)=match name.as_str(){"raw"=>(SearchScopeKind::Raw,"SELECT u.utterance_id FROM raw_utterances u JOIN recording_sessions r ON u.recording_id=r.recording_id JOIN entities e ON e.entity_id=u.utterance_id WHERE r.lesson_id=?1 AND e.lifecycle<>'deleted' ORDER BY r.rowid,u.ordinal,u.rowid"),"secondary"=>(SearchScopeKind::Secondary,"SELECT u.secondary_id FROM secondary_segments u JOIN recording_sessions r ON u.recording_id=r.recording_id JOIN entities e ON e.entity_id=u.secondary_id WHERE r.lesson_id=?1 AND e.lifecycle<>'deleted' ORDER BY r.rowid,u.rowid"),"note"=>(SearchScopeKind::Note,"SELECT n.note_id FROM notes n JOIN entities e ON e.entity_id=n.note_id WHERE lesson_id=?1 AND e.lifecycle<>'deleted' ORDER BY n.rowid"),"material"=>(SearchScopeKind::Material,"SELECT DISTINCT m.material_id FROM material_attachments m JOIN entities e ON e.entity_id=m.material_id WHERE lesson_id=?1 AND m.lifecycle='active' AND e.lifecycle<>'deleted' ORDER BY m.rowid"),_=>return Err(error(ErrorCode::UnsupportedCapability,"Collection type has no search projection"))};
            let mut q = c.prepare(sql).map_err(sql_error)?;
            let ids = q
                .query_map([lesson], |r| r.get::<_, String>(0))
                .map_err(sql_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(sql_error)?;
            Ok((kind, ids))
        }
    }
}
fn projection_key(owner: &str, revision: &str, tier: Option<u64>) -> String {
    format!(
        "{owner}:{revision}:{}",
        tier.map(|v| v.to_string()).unwrap_or_else(|| "all".into())
    )
}
fn coverage_for(c: &Connection, scope: &SearchScope) -> Result<(u64, SearchCoverage)> {
    let mut coverage = SearchCoverage {
        scope_snapshot_id: storage::id(),
        required_units: 0,
        indexed_units: 0,
        required_pages: Vec::new(),
        indexed_pages: Vec::new(),
        unprocessed: Vec::new(),
        failed: Vec::new(),
        stale: Vec::new(),
        state: SearchCoverageState::Complete,
        source_extraction_revision: None,
    };
    let mut generation = 0;
    for (id, revision) in scope.target_ids.iter().zip(&scope.revision_ids) {
        let row:Option<(u64,String,String,String)>=c.query_row("SELECT index_generation,coverage_json,dirty_ranges_json,head_manifest_hash FROM search_index_state WHERE scope_id=?1",[projection_key(id,revision,if scope.kind==SearchScopeKind::Note {scope.actual_visible_tier}else{None})],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(sql_error)?;
        let Some((g, data, dirty, manifest)) = row else {
            coverage.state = SearchCoverageState::Indexing;
            coverage.stale.push(CoverageArea {
                page_id: None,
                region_id: None,
                reason: "projection_not_built".into(),
            });
            continue;
        };
        generation = generation.max(g);
        if dirty != "[]"
            || manifest
                != canonical_hash(&json!({"owner":id,"revision":revision,"profile":PROFILE}))
        {
            coverage.state = SearchCoverageState::Indexing;
            coverage.stale.push(CoverageArea {
                page_id: None,
                region_id: None,
                reason: "projection_stale".into(),
            });
        }
        let part: SearchCoverage = decode(&data)?;
        coverage.required_units += part.required_units;
        coverage.indexed_units += part.indexed_units;
        coverage.required_pages.extend(part.required_pages);
        coverage.indexed_pages.extend(part.indexed_pages);
        coverage.unprocessed.extend(part.unprocessed);
        coverage.failed.extend(part.failed);
        if part.state != SearchCoverageState::Complete
            && coverage.state != SearchCoverageState::Indexing
        {
            coverage.state = SearchCoverageState::Partial;
        }
    }
    Ok((generation, coverage))
}
fn find_hits(c: &Connection, stored: &StoredSession) -> Result<Vec<SearchHit>> {
    let start = Instant::now();
    let session = &stored.session;
    let (needle, _) = normalize(&session.query, session.case_sensitive)?;
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for (owner, revision) in session
        .scope
        .target_ids
        .iter()
        .zip(&session.scope.revision_ids)
    {
        let mut q=c.prepare("SELECT search_unit_id,display_text,normalized_text,offset_map_json,anchor_json,equivalence_group_id FROM search_units WHERE owner_entity_id=?1 AND revision_id=?2 AND (visible_tier IS NULL OR visible_tier=?3) ORDER BY rowid").map_err(sql_error)?;
        for row in q
            .query_map(
                params![owner, revision, session.scope.actual_visible_tier],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .map_err(sql_error)?
        {
            if start.elapsed() > Duration::from_millis(250) {
                return Err(error(
                    ErrorCode::DeadlineExceeded,
                    "Search query CPU budget exceeded",
                ));
            }
            let (unit, text, normalized, map, anchor, equivalence) = row.map_err(sql_error)?;
            let anchor: SearchAnchor = decode(&anchor)?;
            let (normalized, map) = if session.case_sensitive {
                normalize(&text, true)?
            } else {
                (normalized, decode::<Vec<OffsetEntry>>(&map)?)
            };
            for range in match_ranges(&normalized, &map, &needle) {
                if !seen.insert((
                    owner.clone(),
                    equivalence.clone().unwrap_or_else(|| unit.clone()),
                    range.start,
                    range.end,
                )) {
                    continue;
                }
                if output.len() >= 100_000 {
                    return Err(error(
                        ErrorCode::DeadlineExceeded,
                        "Search result budget exceeded",
                    ));
                }
                let mut matched_anchor = anchor.clone();
                matched_anchor.scalar_range = range.clone();
                let snippet = text
                    .chars()
                    .skip(range.start.saturating_sub(32) as usize)
                    .take((range.end - range.start + 64) as usize)
                    .collect();
                output.push(SearchHit {
                    hit_id: canonical_hash(&json!([
                        session.search_id,
                        session.query_seq,
                        session.index_generation,
                        unit,
                        range.start,
                        range.end
                    ])),
                    search_unit_id: unit.clone(),
                    revision_id: revision.clone(),
                    matched_anchor,
                    ranges: vec![range],
                    display_snippet: snippet,
                    equivalence_group_id: equivalence.clone(),
                    nav_kind: if session.scope.kind == SearchScopeKind::Material {
                        SearchHitNavKind::PageRegion
                    } else if matches!(anchor.selector, Selector::V3(_)) {
                        SearchHitNavKind::ObjectFocus
                    } else {
                        SearchHitNavKind::Text
                    },
                });
            }
        }
    }
    Ok(output)
}

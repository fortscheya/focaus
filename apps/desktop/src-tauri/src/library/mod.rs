//! Library transactions, immutable revisions and evidence publication gates.
pub mod catchup;
pub mod materials;
pub mod resources;
pub mod snapshots;
use crate::storage::{
    self, check_revision, commit_entity, decode, emit, encode, head, lifecycle, sql_error, Result,
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
            let parsed:Option<String>=c.query_row("SELECT manifest_json FROM parse_versions WHERE parse_version_id=?1 AND material_id=?2 AND source_version_id=?3",params![selector.parse_version_id,source.entity_id,source.revision_id],|r|r.get(0)).optional().map_err(sql_error)?.flatten();
            let manifest: MaterialManifest = decode(&parsed.ok_or_else(|| {
                error(
                    ErrorCode::SourceUnavailable,
                    "Referenced material parse version unavailable",
                )
            })?)?;
            if !manifest.pages.iter().any(|p| p.page_id == selector.page_id)
                || !manifest
                    .regions
                    .iter()
                    .any(|r| r.region_id == selector.region_id && r.page_id == selector.page_id)
            {
                return Err(error(
                    ErrorCode::InvalidResult,
                    "Referenced material page or region unavailable",
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
    // A frozen canonical parse head also authorizes its exact binary-source
    // region references. The latter retain the SourceVersion byte checksum;
    // check_source separately proves the material/parse/region closure.
    if let Selector::V2(selector) = &source.selector {
        if snapshot.sources.iter().any(|allowed| {
            allowed.entity_id == source.entity_id
                && allowed.revision_id == selector.parse_version_id
                && matches!(allowed.selector, Selector::V4(_))
        }) {
            return true;
        }
    }
    snapshot.sources.iter().any(|allowed| {
        allowed.entity_id == source.entity_id
            && allowed.revision_id == source.revision_id
            && allowed.content_hash == source.content_hash
            && (matches!(allowed.selector, Selector::V4(_)) || allowed.selector == source.selector)
    })
}

fn source_edge_from_revision(
    c: &Connection,
    edge_id: &str,
    from_entity_id: &str,
    from_revision_id: &str,
) -> Result<SourceEdge> {
    let body: Option<String> = c
        .query_row(
            "SELECT body_json FROM revisions WHERE entity_id=?1 AND revision_id=?2",
            params![from_entity_id, from_revision_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .flatten();
    let body = body.ok_or_else(|| {
        error(
            ErrorCode::UnsupportedCapability,
            "Source edge provenance is unavailable in the stored revision",
        )
    })?;
    let body: Value = decode(&body)?;
    let stored = body
        .get("source_edges")
        .and_then(Value::as_array)
        .and_then(|edges| {
            edges
                .iter()
                .find(|edge| edge.get("edge_id").and_then(Value::as_str) == Some(edge_id))
        })
        .ok_or_else(|| {
            error(
                ErrorCode::UnsupportedCapability,
                "Source edge provenance is not stored with the source revision",
            )
        })?;
    let edge: SourceEdge = serde_json::from_value(stored.clone()).map_err(|_| {
        error(
            ErrorCode::CorruptAsset,
            "Stored source edge does not match the contract",
        )
    })?;
    validate("SourceEdge", &storage::value(&edge)?)?;
    if edge.edge_id != edge_id
        || edge.from_entity_id != from_entity_id
        || edge.from_revision_id != from_revision_id
    {
        return Err(error(
            ErrorCode::CorruptAsset,
            "Stored source edge identity does not match its revision",
        ));
    }
    Ok(edge)
}

fn return_anchor_allowed(granted: &Selector, requested: &Selector) -> bool {
    // The return anchor belongs to the source side of the edge. A whole source
    // authorizes a concrete anchor in that source entity; narrower selectors
    // must match exactly because the input carries no entity identity.
    matches!(granted, Selector::V4(_)) || granted == requested
}

pub(crate) fn resolve_source(c: &Connection, input: &SourceResolveInput) -> Result<Value> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = c
        .query_row(
            "SELECT from_entity_id,from_revision_id,from_selector_json,to_entity_id,to_revision_id,to_ref_json,role,availability,freshness,snapshot_id FROM source_edges WHERE edge_id=?1",
            [&input.edge_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error)?;
    let Some((
        from_entity_id,
        from_revision_id,
        from_selector_json,
        to_entity_id,
        to_revision_id,
        to_ref_json,
        role,
        availability,
        freshness,
        snapshot_id,
    )) = row
    else {
        return Err(error(ErrorCode::NotFound, "Source edge unavailable"));
    };
    let from_selector: Selector = decode(&from_selector_json)?;
    let to: SourceRef = decode(&to_ref_json)?;
    if to.entity_id != to_entity_id || to.revision_id != to_revision_id {
        return Err(error(
            ErrorCode::CorruptAsset,
            "Source edge target does not match its source reference",
        ));
    }
    let mut edge =
        source_edge_from_revision(c, &input.edge_id, &from_entity_id, &from_revision_id)?;
    let projected_availability: SourceEdgeAvailability =
        serde_json::from_value(Value::String(availability)).map_err(|_| {
            error(
                ErrorCode::CorruptAsset,
                "Stored source edge availability is invalid",
            )
        })?;
    let projected_freshness: SourceEdgeFreshness =
        serde_json::from_value(Value::String(freshness)).map_err(|_| {
            error(
                ErrorCode::CorruptAsset,
                "Stored source edge freshness is invalid",
            )
        })?;
    if edge.from_selector != from_selector
        || edge.to != to
        || storage::value(&edge.role)?.as_str() != Some(role.as_str())
        || edge.snapshot_id != snapshot_id
    {
        return Err(error(
            ErrorCode::CorruptAsset,
            "Stored source edge projection is inconsistent",
        ));
    }
    // Availability and freshness are mutable projection state. Keep the
    // immutable provenance from the revision, but return the current index
    // status to the caller.
    edge.availability = projected_availability;
    edge.freshness = projected_freshness;
    match edge.availability {
        SourceEdgeAvailability::Available => check_source(c, &edge.to)?,
        SourceEdgeAvailability::SourceDeleted => {
            return Err(error(
                ErrorCode::SourceDeleted,
                "Resolved source was permanently deleted",
            ));
        }
        SourceEdgeAvailability::SourceRemoved
        | SourceEdgeAvailability::IntermediateUnavailable
        | SourceEdgeAvailability::Unresolved => {
            return Err(error(
                ErrorCode::SourceUnavailable,
                "Resolved source is unavailable",
            ));
        }
    }
    if !return_anchor_allowed(&edge.from_selector, &input.return_anchor) {
        return Err(error(
            ErrorCode::PermissionDenied,
            "Return anchor is outside the resolved source",
        ));
    }
    Ok(json!({
        "edge": storage::value(&edge)?,
        "resolved_source": storage::value(&edge.to)?,
        "return_anchor": storage::value(&input.return_anchor)?,
        "traversed_edges": [edge.edge_id],
        "skipped_layers": [],
    }))
}

pub(crate) fn resolve_definition(
    c: &Connection,
    input: &DefinitionResolveInput,
) -> Result<Value> {
    let row: Option<(String, String, String)> = c
        .query_row(
            "SELECT d.scope_id,d.body_json,e.lifecycle FROM definitions d JOIN entities e ON e.entity_id=d.definition_id WHERE d.definition_id=?1",
            [&input.definition_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let Some((stored_scope, body, lifecycle)) = row else {
        return Err(error(ErrorCode::NotFound, "Definition unavailable"));
    };
    if lifecycle == "deleted" {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Definition permanently deleted",
        ));
    }
    if lifecycle != "active" {
        return Err(error(ErrorCode::SourceUnavailable, "Definition unavailable"));
    }
    if stored_scope != input.scope_id {
        return Err(error(
            ErrorCode::PermissionDenied,
            "Definition is outside the requested scope",
        ));
    }
    let definition: Definition = decode(&body)?;
    if definition.definition_id != input.definition_id || definition.scope_id != stored_scope {
        return Err(error(
            ErrorCode::CorruptAsset,
            "Stored definition identity does not match its index",
        ));
    }
    validate("Definition", &storage::value(&definition)?)?;
    check_source(c, &definition.valid_from_anchor)?;
    if let Some(anchor) = &definition.valid_to_anchor {
        check_source(c, anchor)?;
    }
    for source in &definition.source_refs {
        check_source(c, source)?;
    }
    storage::value(&definition)
}

fn entity_body(t: &Transaction<'_>, entity_id: &str) -> Result<(String, Value)> {
    let kind: String = t
        .query_row(
            "SELECT kind FROM entities WHERE entity_id=?1 AND lifecycle='active'",
            [entity_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| error(ErrorCode::NotFound, "Editable content unavailable"))?;
    Ok((kind, head(t, entity_id)?))
}

fn content_scope(body: &Value) -> Value {
    body["lesson_id"]
        .as_str()
        .map(|lesson| json!({"kind":"lesson","lesson_id":lesson}))
        .unwrap_or_else(|| json!({"kind":"global"}))
}

fn find_object<'a>(value: &'a Value, object_id: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => {
            if object.get("object_id").and_then(Value::as_str) == Some(object_id) {
                return Some(value);
            }
            object.values().find_map(|child| find_object(child, object_id))
        }
        Value::Array(values) => values.iter().find_map(|child| find_object(child, object_id)),
        _ => None,
    }
}

fn find_object_mut<'a>(value: &'a mut Value, object_id: &str) -> Option<&'a mut Value> {
    if value.get("object_id").and_then(Value::as_str) == Some(object_id) {
        return Some(value);
    }
    match value {
        Value::Object(object) => object
            .values_mut()
            .find_map(|child| find_object_mut(child, object_id)),
        Value::Array(values) => values
            .iter_mut()
            .find_map(|child| find_object_mut(child, object_id)),
        _ => None,
    }
}

fn find_text_content<'a>(value: &'a Value, block_id: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => {
            if object
                .get("content")
                .and_then(|content| content.get("block_id"))
                .and_then(Value::as_str)
                == Some(block_id)
            {
                return object.get("content");
            }
            object
                .values()
                .find_map(|child| find_text_content(child, block_id))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_text_content(child, block_id)),
        _ => None,
    }
}

fn find_text_content_mut<'a>(value: &'a mut Value, block_id: &str) -> Option<&'a mut Value> {
    let matches = value
        .get("content")
        .and_then(|content| content.get("block_id"))
        .and_then(Value::as_str)
        == Some(block_id);
    if matches {
        return value.get_mut("content");
    }
    match value {
        Value::Object(object) => object
            .values_mut()
            .find_map(|child| find_text_content_mut(child, block_id)),
        Value::Array(values) => values
            .iter_mut()
            .find_map(|child| find_text_content_mut(child, block_id)),
        _ => None,
    }
}

fn text_selection(body: &Value, selector: &Selector) -> Result<Value> {
    let Selector::V1(selector) = selector else {
        return Err(error(
            ErrorCode::UnsupportedCapability,
            "Text mutation requires a text selector",
        ));
    };
    let content = find_text_content(body, &selector.block_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Text block unavailable"))?;
    let runs = content["runs"]
        .as_array()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Text runs unavailable"))?;
    let text: String = runs
        .iter()
        .map(|run| run["text"].as_str().unwrap_or(""))
        .collect();
    if !selector.anchor.element_ids.is_empty()
        && !runs.iter().any(|run| {
            run["element_id"]
                .as_str()
                .is_some_and(|id| selector.anchor.element_ids.iter().any(|wanted| wanted == id))
        })
    {
        return Err(error(ErrorCode::StaleInput, "Text selector elements changed"));
    }
    let chars: Vec<char> = text.chars().collect();
    let start = selector.anchor.scalar_range.start as usize;
    let end = selector.anchor.scalar_range.end as usize;
    if start > end || end > chars.len() {
        return Err(error(ErrorCode::StaleInput, "Text selector range changed"));
    }
    Ok(Value::String(chars[start..end].iter().collect()))
}

fn object_item(body: &Value, selector: &Selector, collection: &str, id_key: &str) -> Result<Value> {
    let Selector::V3(selector) = selector else {
        return Err(error(
            ErrorCode::UnsupportedCapability,
            "Structured mutation requires an object selector",
        ));
    };
    let item_id = selector
        .element_ids
        .first()
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "Structured item selector required"))?;
    let object = find_object(body, &selector.object_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Structured object unavailable"))?;
    object["content"][collection]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item[id_key].as_str() == Some(item_id.as_str()))
        })
        .cloned()
        .ok_or_else(|| error(ErrorCode::NotFound, "Structured item unavailable"))
}

fn visualization_parameters(body: &Value, selector: &Selector) -> Result<Value> {
    let Selector::V3(selector) = selector else {
        return Err(error(
            ErrorCode::UnsupportedCapability,
            "Parameter mutation requires an object selector",
        ));
    };
    let object = if body["object_id"].as_str() == Some(selector.object_id.as_str()) {
        body
    } else {
        find_object(body, &selector.object_id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Visualization object unavailable"))?
    };
    let parameters = object["parameters"]
        .as_object()
        .or_else(|| object["content"]["parameters"].as_object())
        .ok_or_else(|| error(ErrorCode::UnsupportedCapability, "Visualization parameters unavailable"))?;
    Ok(Value::Object(parameters.clone()))
}

fn patch_before_value(body: &Value, patch: &UserPatch) -> Result<Value> {
    match patch.operation {
        UserPatchOperation::TextInsert
        | UserPatchOperation::TextDelete
        | UserPatchOperation::TextReplace => text_selection(body, &patch.selector),
        UserPatchOperation::CellSet => object_item(body, &patch.selector, "cells", "cell_id"),
        UserPatchOperation::NodeSet => object_item(body, &patch.selector, "nodes", "node_id"),
        UserPatchOperation::EdgeSet => object_item(body, &patch.selector, "edges", "edge_id"),
        UserPatchOperation::ParameterSet => visualization_parameters(body, &patch.selector),
        _ => Err(error(
            ErrorCode::UnsupportedCapability,
            "This content patch operation is not connected",
        )),
    }
}

fn apply_text_patch(body: &mut Value, patch: &UserPatch) -> Result<()> {
    let Selector::V1(selector) = &patch.selector else {
        return Err(error(ErrorCode::UnsupportedCapability, "Text selector required"));
    };
    let content = find_text_content_mut(body, &selector.block_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Text block unavailable"))?;
    let runs = content["runs"]
        .as_array_mut()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Text runs unavailable"))?;
    if runs.len() != 1 {
        return Err(error(
            ErrorCode::UnsupportedCapability,
            "Multi-run text mutation is not connected",
        ));
    }
    let old = runs[0]["text"]
        .as_str()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Text run unavailable"))?;
    if !selector.anchor.element_ids.is_empty()
        && runs[0]["element_id"].as_str().is_none_or(|id| {
            !selector.anchor.element_ids.iter().any(|wanted| wanted == id)
        })
    {
        return Err(error(ErrorCode::StaleInput, "Text selector elements changed"));
    }
    let chars: Vec<char> = old.chars().collect();
    let start = selector.anchor.scalar_range.start as usize;
    let end = selector.anchor.scalar_range.end as usize;
    if start > end || end > chars.len() {
        return Err(error(ErrorCode::StaleInput, "Text selector range changed"));
    }
    let replacement = match patch.operation {
        UserPatchOperation::TextInsert => {
            if start != end {
                return Err(error(ErrorCode::InvalidArgument, "Text insert range must be empty"));
            }
            patch
                .after_value
                .as_str()
                .ok_or_else(|| error(ErrorCode::InvalidArgument, "Text insert value required"))?
        }
        UserPatchOperation::TextDelete => "",
        UserPatchOperation::TextReplace => patch
            .after_value
            .as_str()
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "Text replacement value required"))?,
        _ => unreachable!(),
    };
    let mut next = String::new();
    next.extend(chars[..start].iter());
    next.push_str(replacement);
    next.extend(chars[end..].iter());
    runs[0]["text"] = json!(next);
    Ok(())
}

fn apply_structured_patch(body: &mut Value, patch: &UserPatch, collection: &str, id_key: &str, allowed: &[&str]) -> Result<()> {
    let Selector::V3(selector) = &patch.selector else {
        return Err(error(ErrorCode::UnsupportedCapability, "Object selector required"));
    };
    let item_id = selector
        .element_ids
        .first()
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "Structured item selector required"))?;
    let object = find_object_mut(body, &selector.object_id)
        .ok_or_else(|| error(ErrorCode::NotFound, "Structured object unavailable"))?;
    let item = object["content"][collection]
        .as_array_mut()
        .and_then(|items| items.iter_mut().find(|item| item[id_key].as_str() == Some(item_id)))
        .ok_or_else(|| error(ErrorCode::NotFound, "Structured item unavailable"))?;
    let updates = patch
        .after_value
        .as_object()
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "Structured patch object required"))?;
    for key in updates.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(error(ErrorCode::InvalidArgument, "Structured patch field is not editable"));
        }
    }
    for (key, value) in updates {
        item[key] = value.clone();
    }
    Ok(())
}

fn apply_parameter_patch(body: &mut Value, patch: &UserPatch) -> Result<()> {
    let Selector::V3(selector) = &patch.selector else {
        return Err(error(ErrorCode::UnsupportedCapability, "Object selector required"));
    };
    let object = if body["object_id"].as_str() == Some(selector.object_id.as_str()) {
        body
    } else {
        find_object_mut(body, &selector.object_id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Visualization object unavailable"))?
    };
    let parameters = if object.get("parameters").is_some() {
        object
            .get_mut("parameters")
            .ok_or_else(|| error(ErrorCode::UnsupportedCapability, "Visualization parameters unavailable"))?
    } else {
        object["content"].get_mut("parameters").ok_or_else(|| {
            error(ErrorCode::UnsupportedCapability, "Visualization parameters unavailable")
        })?
    };
    let parameters = parameters
        .as_object_mut()
        .ok_or_else(|| error(ErrorCode::InvalidResult, "Visualization parameters are not an object"))?;
    let update = patch
        .after_value
        .as_object()
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "Parameter patch object required"))?;
    if let (Some(path), Some(value)) = (update.get("path").and_then(Value::as_str), update.get("value")) {
        if path.is_empty() || path.contains('.') {
            return Err(error(ErrorCode::InvalidArgument, "Only direct parameter paths are editable"));
        }
        parameters.insert(path.into(), value.clone());
    } else {
        for (key, value) in update {
            if key == "path" || key == "value" {
                continue;
            }
            parameters.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

fn apply_user_patch(body: &mut Value, patch: &UserPatch) -> Result<()> {
    match patch.operation {
        UserPatchOperation::TextInsert
        | UserPatchOperation::TextDelete
        | UserPatchOperation::TextReplace => apply_text_patch(body, patch),
        UserPatchOperation::CellSet => apply_structured_patch(
            body,
            patch,
            "cells",
            "cell_id",
            &["value", "unit", "formula", "status"],
        ),
        UserPatchOperation::NodeSet => {
            apply_structured_patch(body, patch, "nodes", "node_id", &["label", "x", "y"])
        }
        UserPatchOperation::EdgeSet => apply_structured_patch(
            body,
            patch,
            "edges",
            "edge_id",
            &["from_node_id", "to_node_id", "directed", "label", "status"],
        ),
        UserPatchOperation::ParameterSet => apply_parameter_patch(body, patch),
        _ => Err(error(
            ErrorCode::UnsupportedCapability,
            "This content patch operation is not connected",
        )),
    }
}

fn require_edit_lease(t: &Transaction<'_>, patch: &UserPatch, lease_epoch: u64) -> Result<()> {
    let mut q = t
        .prepare("SELECT selectors_json,lease_epoch FROM edit_leases WHERE target_id=?1")
        .map_err(sql_error)?;
    for row in q
        .query_map([&patch.target_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))
        .map_err(sql_error)?
    {
        let (selectors, epoch) = row.map_err(sql_error)?;
        if epoch == lease_epoch {
            let selectors: Vec<Selector> = decode(&selectors)?;
            if selectors.iter().any(|selector| intersects(selector, &patch.selector)) {
                return Ok(());
            }
        }
    }
    Err(error(
        ErrorCode::EditProtected,
        "Matching edit lease is required",
    ))
}

fn sync_projection(t: &Transaction<'_>, kind: &str, entity_id: &str, body: &Value) -> Result<()> {
    match kind {
        "note" => {
            t.execute(
                "UPDATE notes SET fact_set_revision=?2,object_graph_json=?3 WHERE note_id=?1",
                params![
                    entity_id,
                    body["fact_set_revision"].as_str(),
                    encode(&body["objects"])?
                ],
            )
            .map_err(sql_error)?;
        }
        "secondary_segment" => {
            t.execute(
                "UPDATE secondary_segments SET revision_id=?2,raw_refs_json=?3,ordered_spans_json=?4,text=?5,generation_revision=?6 WHERE secondary_id=?1",
                params![
                    entity_id,
                    body["revision_id"].as_str(),
                    encode(&body["raw_refs"])? ,
                    encode(&body["ordered_spans"])? ,
                    body["text"].as_str(),
                    body["generation_revision"].as_str()
                ],
            )
            .map_err(sql_error)?;
        }
        _ => (),
    }
    Ok(())
}

pub(crate) fn content_patch(
    t: &Transaction<'_>,
    input: &ContentPatchInput,
    cause: Option<&str>,
) -> Result<Value> {
    let patch = &input.patch;
    if patch.author != "user" {
        return Err(error(ErrorCode::PermissionDenied, "Only user patches are accepted"));
    }
    require_edit_lease(t, patch, input.lease_epoch)?;
    let (kind, mut body) = entity_body(t, &patch.target_id)?;
    if ["raw_utterance", "audio_span", "material", "material_region"].contains(&kind.as_str()) {
        return Err(error(
            ErrorCode::EditProtected,
            "Original evidence layers are immutable",
        ));
    }
    check_revision(t, &patch.target_id, &patch.base_revision)?;
    let before = patch_before_value(&body, patch)?;
    if canonical_hash(&before) != patch.before_hash {
        return Err(error(ErrorCode::StaleInput, "Patch before value changed"));
    }
    apply_user_patch(&mut body, patch)?;
    let scope = content_scope(&body);
    let changed = commit_entity(
        t,
        &patch.target_id,
        &kind,
        body,
        "content.committed",
        scope,
        cause,
        None,
    )?;
    sync_projection(t, &kind, &patch.target_id, &changed)?;
    let operation = storage::value(&patch.operation)?
        .as_str()
        .unwrap_or("")
        .to_owned();
    t.execute(
        "INSERT INTO user_patches VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            patch.patch_id,
            patch.target_id,
            patch.base_revision,
            encode(&patch.selector)?,
            patch.before_hash,
            operation,
            encode(&patch.after_value)?,
            patch.intent,
            patch.undo_group_id,
            patch.created_at
        ],
    )
    .map_err(sql_error)?;
    Ok(json!({
        "target_id": patch.target_id,
        "revision_id": changed["revision_id"],
        "undo_group_id": patch.undo_group_id
    }))
}

pub(crate) fn content_undo(
    t: &Transaction<'_>,
    input: &ContentUndoInput,
    cause: Option<&str>,
) -> Result<Value> {
    let patch: (String, String) = t
        .query_row(
            "SELECT target_id,base_revision FROM user_patches WHERE undo_group_id=?1 ORDER BY rowid DESC LIMIT 1",
            [&input.undo_group_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| error(ErrorCode::NotFound, "Undo group unavailable"))?;
    check_revision(t, &patch.0, &input.current_revision)?;
    let parent: String = t
        .query_row(
            "SELECT parent_revision_id FROM revision_parents WHERE revision_id=?1 AND parent_revision_id=?2",
            params![input.current_revision, patch.1],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| error(ErrorCode::Conflict, "Undo base is no longer the current parent"))?;
    let (kind, body): (String, String) = t
        .query_row(
            "SELECT e.kind,r.body_json FROM entities e JOIN revisions r ON r.revision_id=?2 WHERE e.entity_id=?1 AND r.entity_id=e.entity_id",
            params![patch.0, parent],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql_error)?;
    let body: Value = decode(&body)?;
    let scope = content_scope(&body);
    let changed = commit_entity(
        t,
        &patch.0,
        &kind,
        body,
        "content.committed",
        scope,
        cause,
        None,
    )?;
    sync_projection(t, &kind, &patch.0, &changed)?;
    Ok(json!({
        "target_id": patch.0,
        "revision_id": changed["revision_id"],
        "undo_group_id": input.undo_group_id
    }))
}

fn clone_note_object(value: &Value) -> Value {
    let mut cloned = value.clone();
    fn rewrite(value: &mut Value) {
        match value {
            Value::Object(object) => {
                for key in ["object_id", "element_id", "block_id", "revision_id"] {
                    if object.get(key).and_then(Value::as_str).is_some() {
                        object.insert(key.into(), json!(storage::id()));
                    }
                }
                for (key, child) in object.iter_mut() {
                    if key != "source_refs" {
                        rewrite(child);
                    }
                }
            }
            Value::Array(values) => values.iter_mut().for_each(rewrite),
            _ => (),
        }
    }
    rewrite(&mut cloned);
    cloned
}

pub(crate) fn note_add_selection(
    t: &Transaction<'_>,
    input: &NoteAddSelectionInput,
    cause: Option<&str>,
) -> Result<Value> {
    if input.object_ids.is_empty() || input.sources.is_empty() {
        return Err(error(ErrorCode::InvalidArgument, "Selection sources and objects are required"));
    }
    let mut note = head(t, &input.note_id)?;
    if note["note_id"].as_str() != Some(input.note_id.as_str()) {
        return Err(error(ErrorCode::CorruptAsset, "Note identity mismatch"));
    }
    validate("Note", &note)?;
    for source in &input.sources {
        check_source(t, source)?;
    }
    let objects = note["objects"]
        .as_array_mut()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Note object graph unavailable"))?;
    let mut additions = Vec::new();
    for object_id in &input.object_ids {
        if objects
            .iter()
            .any(|object| object["object_id"].as_str() == Some(object_id.as_str()))
        {
            return Err(error(ErrorCode::Conflict, "Selected object is already in the note"));
        }
        let mut source_object = None;
        for source in &input.sources {
            if let Ok(body) = head(t, &source.entity_id) {
                if let Some(object) = find_object(&body, object_id) {
                    source_object = Some(object.clone());
                    break;
                }
            }
        }
        let mut object = clone_note_object(&source_object.ok_or_else(|| {
            error(ErrorCode::NotFound, "Selected source object unavailable")
        })?);
        let refs = object["source_refs"]
            .as_array_mut()
            .ok_or_else(|| error(ErrorCode::CorruptAsset, "Selected object lacks source refs"))?;
        for source in &input.sources {
            let value = storage::value(source)?;
            if !refs.contains(&value) {
                refs.push(value);
            }
        }
        for source in source_refs_in(&object)? {
            check_source(t, &source)?;
        }
        let _: MaterialObject = serde_json::from_value(object.clone()).map_err(|_| {
            error(ErrorCode::InvalidResult, "Selected object does not match the object contract")
        })?;
        additions.push(object.clone());
        objects.push(object);
    }
    validate("Note", &note)?;
    let scope = content_scope(&note);
    let changed = commit_entity(
        t,
        &input.note_id,
        "note",
        note,
        "content.committed",
        scope,
        cause,
        None,
    )?;
    sync_projection(t, "note", &input.note_id, &changed)?;
    Ok(json!({
        "note_id": input.note_id,
        "revision_id": changed["revision_id"],
        "object_ids": additions.iter().filter_map(|object| object["object_id"].as_str()).collect::<Vec<_>>()
    }))
}

fn secondary_scope(t: &Transaction<'_>, secondary_id: &str) -> Result<Value> {
    let lesson: Option<String> = t
        .query_row(
            "SELECT l.lesson_id FROM secondary_segments s JOIN recording_sessions r ON r.recording_id=s.recording_id LEFT JOIN lessons l ON l.lesson_id=r.lesson_id WHERE s.secondary_id=?1",
            [secondary_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .flatten();
    Ok(lesson
        .map(|lesson| json!({"kind":"lesson","lesson_id":lesson}))
        .unwrap_or_else(|| json!({"kind":"global"})))
}

fn correction_data(
    t: &Transaction<'_>,
    suggestion_id: &str,
) -> Result<(String, CorrectionSuggestion, Value)> {
    let (secondary_id, body, state): (String, String, String) = t
        .query_row(
            "SELECT secondary_id,body_json,state FROM correction_suggestions WHERE suggestion_id=?1",
            [suggestion_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| error(ErrorCode::NotFound, "Correction suggestion unavailable"))?;
    let suggestion: CorrectionSuggestion = decode(&body)?;
    if suggestion.suggestion_id != suggestion_id || storage::value(&suggestion.state)?.as_str() != Some(state.as_str()) {
        return Err(error(ErrorCode::CorruptAsset, "Correction suggestion index mismatch"));
    }
    let secondary = head(t, &secondary_id)?;
    if suggestion.secondary_id != secondary_id {
        return Err(error(ErrorCode::CorruptAsset, "Correction secondary identity mismatch"));
    }
    validate("SecondarySegment", &secondary)?;
    Ok((secondary_id, suggestion, secondary))
}

fn update_suggestion_body(body: &mut Value, suggestion: &CorrectionSuggestion) -> Result<()> {
    let suggestions = body["suggestions"]
        .as_array_mut()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Secondary suggestions unavailable"))?;
    let target = suggestions
        .iter_mut()
        .find(|candidate| candidate["suggestion_id"].as_str() == Some(suggestion.suggestion_id.as_str()))
        .ok_or_else(|| error(ErrorCode::NotFound, "Suggestion is not present in secondary revision"))?;
    *target = storage::value(suggestion)?;
    Ok(())
}

fn check_secondary_sources(t: &Transaction<'_>, secondary: &Value) -> Result<()> {
    let raw_refs: Vec<SourceRef> = serde_json::from_value(secondary["raw_refs"].clone())
        .map_err(|_| error(ErrorCode::CorruptAsset, "Secondary source refs are malformed"))?;
    for source in &raw_refs {
        check_source(t, source)?;
    }
    Ok(())
}

fn replace_secondary_text(
    body: &mut Value,
    suggestion: &CorrectionSuggestion,
    final_text: &str,
) -> Result<()> {
    let text = body["text"]
        .as_str()
        .ok_or_else(|| error(ErrorCode::CorruptAsset, "Secondary text unavailable"))?;
    let chars: Vec<char> = text.chars().collect();
    let start = suggestion.anchor.scalar_range.start as usize;
    let end = suggestion.anchor.scalar_range.end as usize;
    if start > end || end > chars.len() {
        return Err(error(ErrorCode::StaleInput, "Correction anchor changed"));
    }
    let original: String = chars[start..end].iter().collect();
    if original != suggestion.original_text {
        return Err(error(ErrorCode::StaleInput, "Correction original text changed"));
    }
    let mut next = String::new();
    next.extend(chars[..start].iter());
    next.push_str(final_text);
    next.extend(chars[end..].iter());
    body["text"] = json!(next);
    Ok(())
}

fn commit_correction(
    t: &Transaction<'_>,
    secondary_id: &str,
    body: Value,
    suggestion: &CorrectionSuggestion,
    cause: Option<&str>,
) -> Result<Value> {
    validate("SecondarySegment", &body)?;
    let changed = commit_entity(
        t,
        secondary_id,
        "secondary_segment",
        body,
        "content.committed",
        secondary_scope(t, secondary_id)?,
        cause,
        None,
    )?;
    sync_projection(t, "secondary_segment", secondary_id, &changed)?;
    t.execute(
        "UPDATE correction_suggestions SET body_json=?2,state=?3,source_revision=?4 WHERE suggestion_id=?1",
        params![
            suggestion.suggestion_id,
            encode(suggestion)?,
            storage::value(&suggestion.state)?.as_str().unwrap_or("pending"),
            suggestion.source_revision
        ],
    )
    .map_err(sql_error)?;
    Ok(json!({
        "secondary_id": secondary_id,
        "revision_id": changed["revision_id"],
        "suggestion_id": suggestion.suggestion_id,
        "state": storage::value(&suggestion.state)?
    }))
}

pub(crate) fn correction_draft(
    t: &Transaction<'_>,
    input: &CorrectionDraftInput,
    _cause: Option<&str>,
) -> Result<Value> {
    let (secondary_id, mut suggestion, secondary) = correction_data(t, &input.suggestion_id)?;
    if suggestion.state != CorrectionSuggestionState::Pending {
        return Err(error(ErrorCode::Conflict, "Correction suggestion is no longer pending"));
    }
    if secondary["revision_id"].as_str() != Some(suggestion.source_revision.as_str()) {
        return Err(error(ErrorCode::StaleInput, "Correction source revision changed"));
    }
    check_secondary_sources(t, &secondary)?;
    if input.composition_active {
        return Ok(json!({"saved":false,"composition_active":true}));
    }
    suggestion.suggested_text = input.draft.clone();
    t.execute(
        "UPDATE correction_suggestions SET body_json=?2 WHERE suggestion_id=?1",
        params![suggestion.suggestion_id, encode(&suggestion)?],
    )
    .map_err(sql_error)?;
    Ok(json!({
        "secondary_id": secondary_id,
        "revision_id": secondary["revision_id"],
        "suggestion_id": suggestion.suggestion_id,
        "state": storage::value(&suggestion.state)?,
        "saved": true
    }))
}

fn correction_commit(
    t: &Transaction<'_>,
    suggestion_id: &str,
    base_revision: &str,
    final_text: &str,
    accept: bool,
    cause: Option<&str>,
) -> Result<Value> {
    let (secondary_id, mut suggestion, mut secondary) = correction_data(t, suggestion_id)?;
    if suggestion.state != CorrectionSuggestionState::Pending {
        return Err(error(ErrorCode::Conflict, "Correction suggestion is no longer pending"));
    }
    if secondary["revision_id"].as_str() != Some(base_revision)
        || suggestion.source_revision != base_revision
    {
        return Err(error(ErrorCode::StaleInput, "Correction base revision changed"));
    }
    check_secondary_sources(t, &secondary)?;
    if accept {
        if suggestion.semantic_risk == CorrectionSuggestionSemanticRisk::Critical {
            return Err(error(
                ErrorCode::UnsupportedCapability,
                "Critical correction confirmation is not represented by the current contract",
            ));
        }
        replace_secondary_text(&mut secondary, &suggestion, final_text)?;
        suggestion.suggested_text = final_text.into();
        suggestion.state = CorrectionSuggestionState::Accepted;
    } else {
        suggestion.state = CorrectionSuggestionState::Rejected;
    }
    update_suggestion_body(&mut secondary, &suggestion)?;
    commit_correction(t, &secondary_id, secondary, &suggestion, cause)
}

pub(crate) fn correction_accept(
    t: &Transaction<'_>,
    input: &CorrectionAcceptInput,
    cause: Option<&str>,
) -> Result<Value> {
    correction_commit(
        t,
        &input.suggestion_id,
        &input.base_revision,
        &input.final_text,
        true,
        cause,
    )
}

pub(crate) fn correction_reject(
    t: &Transaction<'_>,
    input: &CorrectionRejectInput,
    cause: Option<&str>,
) -> Result<Value> {
    correction_commit(
        t,
        &input.suggestion_id,
        &input.base_revision,
        &input.final_text,
        false,
        cause,
    )
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
    let entity_state: Option<String> = c
        .query_row(
            "SELECT lifecycle FROM entities WHERE entity_id=?1",
            [&input.entity_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if entity_state.as_deref() == Some("deleted") {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Content permanently deleted",
        ));
    }
    let material:Option<(String,String,String,u64)>=c.query_row("SELECT m.source_version_id,m.resource_id,r.sha256,r.byte_length FROM materials m JOIN resources r ON r.resource_id=m.resource_id WHERE m.material_id=?1 AND r.state='ready'",[&input.entity_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(sql_error)?;
    if let Some((source_version, resource, sha, length)) = material {
        if input.revision_id.as_deref() == Some(source_version.as_str()) {
            return Ok(
                json!({"kind":"material_source_version","material_id":input.entity_id,"source_version_id":source_version,"resource_id":resource,"sha256":sha,"byte_length":length}),
            );
        }
        let parse = if let Some(revision) = &input.revision_id {
            c.query_row("SELECT manifest_json FROM parse_versions WHERE material_id=?1 AND parse_version_id=?2",params![input.entity_id,revision],|r|r.get::<_,Option<String>>(0)).optional().map_err(sql_error)?.flatten()
        } else {
            let body = head(c, &input.entity_id)?;
            if let Some(parse) = body["parse_version_id"].as_str() {
                c.query_row(
                    "SELECT manifest_json FROM parse_versions WHERE parse_version_id=?1",
                    [parse],
                    |r| r.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(sql_error)?
                .flatten()
            } else {
                None
            }
        };
        if let Some(parse) = parse {
            return decode(&parse);
        }
    }
    // Host projections are not content revisions and have dedicated stores.
    let view: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM views WHERE view_id=?1)",
            [&input.entity_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if view {
        let view = crate::windows::get(c, &input.entity_id)?;
        if input
            .revision_id
            .as_ref()
            .is_some_and(|r| r != &view.view_revision)
        {
            return Err(error(ErrorCode::Conflict, "View state revision changed"));
        }
        return storage::value(&view);
    }
    let task: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1)",
            [&input.entity_id],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if task {
        let task = crate::tasks::task(c, &input.entity_id)?;
        if input
            .revision_id
            .as_ref()
            .is_some_and(|r| r != &task.snapshot_id)
        {
            return Err(error(ErrorCode::Conflict, "Task input revision changed"));
        }
        return storage::value(&task);
    }
    let search: Option<String> = c
        .query_row(
            "SELECT state_json FROM search_sessions WHERE search_id=?1",
            [&input.entity_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some(search) = search {
        let stored: Value = decode(&search)?;
        if input
            .revision_id
            .as_ref()
            .is_some_and(|r| stored["session"]["scope_snapshot_id"].as_str() != Some(r))
        {
            return Err(error(
                ErrorCode::SearchInvalidated,
                "Search snapshot changed",
            ));
        }
        return Ok(stored["session"].clone());
    }
    if let Some(revision) = &input.revision_id {
        let progress:Option<String>=c.query_row("SELECT body_json FROM progress_states WHERE lesson_id=?1 AND json_extract(body_json,'$.snapshot_id')=?2",params![input.entity_id,revision],|r|r.get(0)).optional().map_err(sql_error)?;
        if let Some(progress) = progress {
            return decode(&progress);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{commit_version, Store};
    use std::path::PathBuf;

    struct TestStore {
        path: PathBuf,
        store: Store,
    }

    impl TestStore {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "focaus-library-{}",
                storage::id()
            ));
            let store = Store::open(&path, crate::INITIAL_MIGRATION).expect("test database");
            Self { path, store }
        }
    }

    impl Drop for TestStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn lesson(store: &Store) -> String {
        store
            .write(|t| {
                let course = create_course(
                    t,
                    &CourseCreateInput {
                        name: "library resolver test".into(),
                    },
                    None,
                )?;
                let lesson = create_lesson(
                    t,
                    &LessonCreateInput {
                        course_id: course["course_id"].as_str().unwrap().into(),
                        title: "resolver".into(),
                        actual_date: None,
                        timezone: "Asia/Shanghai".into(),
                    },
                    None,
                )?;
                Ok(lesson["lesson_id"].as_str().unwrap().to_owned())
            })
            .unwrap()
    }

    fn source_entity(t: &Transaction<'_>) -> Result<SourceRef> {
        let entity_id = storage::id();
        let body = commit_entity(
            t,
            &entity_id,
            "note",
            json!({"entity_id":entity_id}),
            "content.committed",
            json!({"kind":"global"}),
            None,
            None,
        )?;
        let revision_id = body
            .get("revision_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                t.query_row(
                    "SELECT revision_id FROM entities WHERE entity_id=?1",
                    [&entity_id],
                    |r| r.get(0),
                )
                .ok()
            })
            .ok_or_else(|| {
                error(
                    ErrorCode::InvalidResult,
                    "Test source revision unavailable",
                )
            })?;
        let content_hash: String = t
            .query_row(
                "SELECT content_hash FROM revisions WHERE revision_id=?1",
                [&revision_id],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        Ok(SourceRef {
            entity_id,
            revision_id,
            content_hash,
            selector: Selector::V4(WholeSelector {
                kind: "whole".into(),
            }),
        })
    }

    fn note_fixture(store: &Store, lesson_id: &str, text: &str) -> (String, String, String) {
        let note_id = storage::id();
        let object_id = storage::id();
        let block_id = storage::id();
        let run_id = storage::id();
        store
            .write(|t| {
                let fact_set_revision = storage::id();
                let body = json!({
                    "note_id":note_id,
                    "lesson_id":lesson_id,
                    "fact_set_revision":fact_set_revision,
                    "fact_ids":[],
                    "objects":[{
                        "object_id":object_id,
                        "revision_id":storage::id(),
                        "type":"text",
                        "source_refs":[],
                        "region_ids":[],
                        "layout":{"bounds":[0,0,1,1],"z_order":0,"group_id":null,"constraints":[]},
                        "extraction":"native",
                        "editability":"native",
                        "status":"complete",
                        "children":[],
                        "content":{"block_id":block_id,"runs":[{"element_id":run_id,"text":text}],"level":1}
                    }],
                    "revision_id":storage::id()
                });
                let committed = commit_entity(
                    t,
                    &note_id,
                    "note",
                    body,
                    "content.committed",
                    json!({"kind":"lesson","lesson_id":lesson_id}),
                    None,
                    None,
                )?;
                t.execute(
                    "INSERT INTO notes VALUES(?1,?2,?3,?4)",
                    params![
                        note_id,
                        lesson_id,
                        fact_set_revision,
                        encode(&committed["objects"])?
                    ],
                )
                .map_err(sql_error)?;
                Ok(())
            })
            .unwrap();
        (note_id, object_id, block_id)
    }

    fn source_ref(store: &Store, entity_id: &str) -> SourceRef {
        store
            .read(|c| {
                c.query_row(
                    "SELECT e.revision_id,r.content_hash FROM entities e JOIN revisions r ON r.revision_id=e.revision_id WHERE e.entity_id=?1",
                    [entity_id],
                    |r| {
                        Ok(SourceRef {
                            entity_id: entity_id.into(),
                            revision_id: r.get(0)?,
                            content_hash: r.get(1)?,
                            selector: Selector::V4(WholeSelector {
                                kind: "whole".into(),
                            }),
                        })
                    },
                )
                .map_err(sql_error)
            })
            .unwrap()
    }

    fn begin_note_text_lease(store: &Store, note_id: &str, block_id: &str, epoch: u64) {
        store
            .write(|t| {
                begin_edit(
                    t,
                    &EditLease {
                        view_id: storage::id(),
                        target_id: note_id.into(),
                        selectors: vec![Selector::V1(TextSelector {
                            kind: "text".into(),
                            block_id: block_id.into(),
                            anchor: TextAnchor {
                                left_element_id: None,
                                right_element_id: None,
                                element_ids: Vec::new(),
                                scalar_range: ScalarRange { start: 0, end: 5 },
                            },
                        })],
                        lease_epoch: epoch,
                        composition_active: false,
                        draft_revision: None,
                    },
                )?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn definition_resolve_checks_scope_and_source_revision() {
        let db = TestStore::new();
        let lesson_id = lesson(&db.store);
        let (definition_id, scope_id, definition) = db
            .store
            .write(|t| {
                let source = source_entity(t)?;
                let definition_id = storage::id();
                let scope_id = storage::id();
                let definition = Definition {
                    definition_id: definition_id.clone(),
                    concept_id: storage::id(),
                    lesson_id: lesson_id.clone(),
                    scope_id: scope_id.clone(),
                    valid_from_anchor: source.clone(),
                    valid_to_anchor: None,
                    source_refs: vec![source],
                    definition_fact_ids: Vec::new(),
                };
                commit_entity(
                    t,
                    &definition_id,
                    "definition",
                    storage::value(&definition)?,
                    "content.committed",
                    json!({"kind":"lesson","lesson_id":lesson_id}),
                    None,
                    None,
                )?;
                t.execute(
                    "INSERT INTO definitions VALUES(?1,?2,?3,?4,?5)",
                    params![
                        definition_id,
                        definition.concept_id,
                        definition.lesson_id,
                        definition.scope_id,
                        storage::encode(&definition)?
                    ],
                )
                .map_err(sql_error)?;
                Ok((definition.definition_id.clone(), scope_id, definition))
            })
            .unwrap();

        let resolved = db
            .store
            .read(|c| {
                resolve_definition(
                    c,
                    &DefinitionResolveInput {
                        definition_id: definition_id.clone(),
                        visible_tier: 3,
                        scope_id: scope_id.clone(),
                    },
                )
            })
            .unwrap();
        assert_eq!(
            serde_json::from_value::<Definition>(resolved).unwrap(),
            definition
        );

        let wrong_scope = db
            .store
            .read(|c| {
                resolve_definition(
                    c,
                    &DefinitionResolveInput {
                        definition_id: definition_id.clone(),
                        visible_tier: 3,
                        scope_id: storage::id(),
                    },
                )
            })
            .unwrap_err();
        assert_eq!(wrong_scope.code, ErrorCode::PermissionDenied);

        let mut stale = definition.clone();
        stale.valid_from_anchor.content_hash = "0".repeat(64);
        stale.source_refs[0].content_hash = "0".repeat(64);
        db.store
            .write(|t| {
                t.execute(
                    "UPDATE definitions SET body_json=?2 WHERE definition_id=?1",
                    params![stale.definition_id, storage::encode(&stale)?],
                )
                .map_err(sql_error)?;
                Ok(())
            })
            .unwrap();
        let stale_error = db
            .store
            .read(|c| {
                resolve_definition(
                    c,
                    &DefinitionResolveInput {
                        definition_id: definition_id.clone(),
                        visible_tier: 3,
                        scope_id: scope_id.clone(),
                    },
                )
            })
            .unwrap_err();
        assert_eq!(stale_error.code, ErrorCode::StaleInput);

        let missing = db
            .store
            .read(|c| {
                resolve_definition(
                    c,
                    &DefinitionResolveInput {
                        definition_id: storage::id(),
                        visible_tier: 3,
                        scope_id,
                    },
                )
            })
            .unwrap_err();
        assert_eq!(missing.code, ErrorCode::NotFound);
    }

    #[test]
    fn source_resolve_uses_revision_provenance_and_projection_status() {
        let db = TestStore::new();
        let missing = db
            .store
            .read(|c| {
                resolve_source(
                    c,
                    &SourceResolveInput {
                        edge_id: storage::id(),
                        return_anchor: Selector::V4(WholeSelector {
                            kind: "whole".into(),
                        }),
                    },
                )
            })
            .unwrap_err();
        assert_eq!(missing.code, ErrorCode::NotFound);

        let (edge_id, target, from_selector) = db
            .store
            .write(|t| {
                let from_entity_id = storage::id();
                let from_revision_id = storage::id();
                let target = source_entity(t)?;
                let edge_id = storage::id();
                let from_selector = Selector::V4(WholeSelector {
                    kind: "whole".into(),
                });
                let edge = SourceEdge {
                    edge_id: edge_id.clone(),
                    from_entity_id: from_entity_id.clone(),
                    from_revision_id: from_revision_id.clone(),
                    from_selector: from_selector.clone(),
                    to: target.clone(),
                    role: SourceEdgeRole::TeacherSaid,
                    created_by: SourceEdgeCreatedBy::Engine,
                    snapshot_id: storage::id(),
                    availability: SourceEdgeAvailability::Available,
                    freshness: SourceEdgeFreshness::Current,
                };
                commit_version(
                    t,
                    &from_entity_id,
                    "fact",
                    &from_revision_id,
                    json!({
                        "entity_id":from_entity_id,
                        "revision_id":from_revision_id,
                        "source_edges":[edge]
                    }),
                    "content.committed",
                    json!({"kind":"global"}),
                    None,
                    None,
                )?;
                t.execute(
                    "INSERT INTO source_edges VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    params![
                        edge.edge_id,
                        edge.from_entity_id,
                        edge.from_revision_id,
                        storage::encode(&edge.from_selector)?,
                        edge.to.entity_id,
                        edge.to.revision_id,
                        storage::encode(&edge.to)?,
                        storage::value(&edge.role)?.as_str().unwrap(),
                        storage::value(&edge.availability)?.as_str().unwrap(),
                        storage::value(&edge.freshness)?.as_str().unwrap(),
                        edge.snapshot_id
                    ],
                )
                .map_err(sql_error)?;
                Ok((edge_id, target, from_selector))
            })
            .unwrap();
        let resolved = db
            .store
            .read(|c| {
                resolve_source(
                    c,
                    &SourceResolveInput {
                        edge_id: edge_id.clone(),
                        return_anchor: from_selector.clone(),
                    },
                )
            })
            .unwrap();
        assert_eq!(resolved["resolved_source"]["entity_id"], target.entity_id);
        assert_eq!(resolved["edge"]["freshness"], "current");

        db.store
            .write(|t| {
                t.execute(
                    "UPDATE source_edges SET freshness='update_available' WHERE edge_id=?1",
                    [&edge_id],
                )
                .map_err(sql_error)?;
                Ok(())
            })
            .unwrap();
        let projected = db
            .store
            .read(|c| {
                resolve_source(
                    c,
                    &SourceResolveInput {
                        edge_id,
                        return_anchor: from_selector,
                    },
                )
            })
            .unwrap();
        assert_eq!(projected["edge"]["freshness"], "update_available");
    }

    #[test]
    fn content_patch_and_undo_commit_real_revisions_with_lease_and_cas() {
        let db = TestStore::new();
        let lesson_id = lesson(&db.store);
        let (note_id, _object_id, block_id) = note_fixture(&db.store, &lesson_id, "hello");
        let base_revision = db.store.head(&note_id).unwrap()["revision_id"]
            .as_str()
            .unwrap()
            .to_owned();
        begin_note_text_lease(&db.store, &note_id, &block_id, 1);
        let patch = UserPatch {
            patch_id: storage::id(),
            target_id: note_id.clone(),
            base_revision: base_revision.clone(),
            operation: UserPatchOperation::TextReplace,
            selector: Selector::V1(TextSelector {
                kind: "text".into(),
                block_id,
                anchor: TextAnchor {
                    left_element_id: None,
                    right_element_id: None,
                    element_ids: Vec::new(),
                    scalar_range: ScalarRange { start: 0, end: 5 },
                },
            }),
            before_hash: canonical_hash(&json!("hello")),
            after_value: json!("world"),
            author: "user".into(),
            intent: "replace text".into(),
            undo_group_id: storage::id(),
            created_at: storage::now(),
        };
        let result = db
            .store
            .write(|t| {
                content_patch(
                    t,
                    &ContentPatchInput {
                        patch: patch.clone(),
                        lease_epoch: 1,
                    },
                    None,
                )
            })
            .unwrap();
        let current = result["revision_id"].as_str().unwrap().to_owned();
        assert_eq!(
            db.store.head(&note_id).unwrap()["objects"][0]["content"]["runs"][0]["text"],
            "world"
        );
        let stale = db
            .store
            .write(|t| {
                content_patch(
                    t,
                    &ContentPatchInput {
                        patch: UserPatch {
                            base_revision: base_revision.clone(),
                            before_hash: canonical_hash(&json!("wrong")),
                            ..patch.clone()
                        },
                        lease_epoch: 1,
                    },
                    None,
                )
            })
            .unwrap_err();
        assert_eq!(stale.code, ErrorCode::Conflict);
        let undone = db
            .store
            .write(|t| {
                content_undo(
                    t,
                    &ContentUndoInput {
                        undo_group_id: patch.undo_group_id.clone(),
                        current_revision: current,
                    },
                    None,
                )
            })
            .unwrap();
        assert_ne!(undone["revision_id"], patch.base_revision);
        assert_eq!(
            db.store.head(&note_id).unwrap()["objects"][0]["content"]["runs"][0]["text"],
            "hello"
        );
    }

    #[test]
    fn note_add_selection_requires_source_provenance_and_copies_object() {
        let db = TestStore::new();
        let lesson_id = lesson(&db.store);
        let (source_note, source_object, _) = note_fixture(&db.store, &lesson_id, "source");
        let (target_note, _, _) = note_fixture(&db.store, &lesson_id, "target");
        let mut stale_source = source_ref(&db.store, &source_note);
        stale_source.content_hash = "0".repeat(64);
        let stale = db
            .store
            .write(|t| {
                note_add_selection(
                    t,
                    &NoteAddSelectionInput {
                        note_id: target_note.clone(),
                        sources: vec![stale_source],
                        object_ids: vec![source_object.clone()],
                    },
                    None,
                )
            })
            .unwrap_err();
        assert_eq!(stale.code, ErrorCode::StaleInput);

        let source = source_ref(&db.store, &source_note);
        db.store
            .write(|t| {
                note_add_selection(
                    t,
                    &NoteAddSelectionInput {
                        note_id: target_note.clone(),
                        sources: vec![source],
                        object_ids: vec![source_object],
                    },
                    None,
                )
            })
            .unwrap();
        assert_eq!(
            db.store.head(&target_note).unwrap()["objects"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn correction_accept_updates_secondary_revision_but_keeps_raw_source_immutable() {
        let db = TestStore::new();
        let lesson_id = lesson(&db.store);
        let (source_note, _, _) = note_fixture(&db.store, &lesson_id, "raw source");
        let source = source_ref(&db.store, &source_note);
        let (secondary_id, suggestion_id, base_revision) = db
            .store
            .write(|t| {
                let recording = crate::capture::RecordingCoordinator::reserve(
                    t,
                    &RecordingStartInput {
                        lesson_id: Some(lesson_id.clone()),
                        device_id: None,
                    },
                    None,
                )?;
                let recording_id = recording["recording_id"].as_str().unwrap().to_owned();
                let secondary_id = storage::id();
                let suggestion_id = storage::id();
                let base_revision = storage::id();
                let generation_revision = storage::id();
                let suggestion = CorrectionSuggestion {
                    suggestion_id: suggestion_id.clone(),
                    secondary_id: secondary_id.clone(),
                    source_revision: base_revision.clone(),
                    anchor: TextAnchor {
                        left_element_id: None,
                        right_element_id: None,
                        element_ids: Vec::new(),
                        scalar_range: ScalarRange { start: 0, end: 5 },
                    },
                    original_text: "hello".into(),
                    suggested_text: "hallo".into(),
                    reason: "ordinary typo".into(),
                    semantic_risk: CorrectionSuggestionSemanticRisk::Ordinary,
                    state: CorrectionSuggestionState::Pending,
                };
                let body = json!({
                    "secondary_id":secondary_id,
                    "recording_id":recording_id,
                    "raw_refs":[source],
                    "ordered_spans":[],
                    "text":"hello",
                    "suggestions":[suggestion],
                    "generation_revision":generation_revision,
                    "revision_id":base_revision
                });
                commit_version(
                    t,
                    &secondary_id,
                    "secondary_segment",
                    &base_revision,
                    body,
                    "content.committed",
                    json!({"kind":"lesson","lesson_id":lesson_id}),
                    None,
                    None,
                )?;
                let committed = head(t, &secondary_id)?;
                t.execute(
                    "INSERT INTO secondary_segments VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        secondary_id,
                        recording_id,
                        base_revision,
                        encode(&committed["raw_refs"])?,
                        encode(&committed["ordered_spans"])?,
                        committed["text"].as_str(),
                        committed["generation_revision"].as_str()
                    ],
                )
                .map_err(sql_error)?;
                t.execute(
                    "INSERT INTO correction_suggestions VALUES(?1,?2,?3,?4,?5)",
                    params![
                        suggestion_id,
                        secondary_id,
                        base_revision,
                        encode(&committed["suggestions"][0])?,
                        "pending"
                    ],
                )
                .map_err(sql_error)?;
                Ok((secondary_id, suggestion_id, base_revision))
            })
            .unwrap();
        let stale = db
            .store
            .write(|t| {
                correction_accept(
                    t,
                    &CorrectionAcceptInput {
                        suggestion_id: suggestion_id.clone(),
                        base_revision: storage::id(),
                        final_text: "hallo".into(),
                    },
                    None,
                )
            })
            .unwrap_err();
        assert_eq!(stale.code, ErrorCode::StaleInput);
        let accepted = db
            .store
            .write(|t| {
                correction_accept(
                    t,
                    &CorrectionAcceptInput {
                        suggestion_id,
                        base_revision,
                        final_text: "hallo".into(),
                    },
                    None,
                )
            })
            .unwrap();
        assert_eq!(db.store.head(&secondary_id).unwrap()["text"], "hallo");
        assert_ne!(accepted["revision_id"], Value::Null);
        assert!(db.store.head(&source_note).unwrap()["objects"].is_array());
    }
}

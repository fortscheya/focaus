//! C07 semantic discovery and controlled visualization proposals.
//!
//! This module can recognize a candidate and check its provenance/template
//! semantics, but it cannot calculate a matrix, mutate the registry, or mark
//! an object `ready`.  N remains the sole owner of deterministic evaluation
//! and publication.

use crate::{binding_matches, invalid, invalid_argument, models, PROMPT_VISUAL_DISCOVER_V1};
use focaus_contracts::{
    ModelId, SafeError, SourceRef, TaskKind, ValueBinding,
    VisualizationDiscoverInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const VISUAL_PROTOCOL: &str = "focaus.visualization/1.0.0";
pub const REGISTRY_REVISION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualizationCandidate {
    pub semantic_key: String,
    pub template_id: String,
    pub template_version: String,
    pub purpose: String,
    pub scope_id: String,
    pub concept_fact_ids: Vec<String>,
    pub definition_ids: Vec<String>,
    pub symbol_binding_ids: Vec<String>,
    pub parameters: Value,
    pub value_bindings: Vec<ValueBinding>,
    pub source_refs: Vec<SourceRef>,
    pub placement_anchor: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryDisposition {
    Candidate(ValidatedCandidate),
    DuplicateExisting,
    DeletedByUser,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedCandidate {
    pub candidate: VisualizationCandidate,
    pub semantic_check: SemanticCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCheck {
    pub schema: CheckStatus,
    pub source: CheckStatus,
    pub semantic: CheckStatus,
    pub math: CheckStatus,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Pending,
    Fail,
}

pub fn validate_discover_binding(
    input: &VisualizationDiscoverInput,
    binding: &focaus_contracts::CredentialBinding,
) -> Result<(), SafeError> {
    if input.snapshot.purpose != TaskKind::VisualizationDiscover
        || input.snapshot.model_id != Some(ModelId::Qwen38Flash)
        || input.snapshot.region.as_deref() != Some(models::REGION)
        || input.snapshot.prompt_version.as_deref() != Some(PROMPT_VISUAL_DISCOVER_V1)
    {
        return Err(invalid_argument("visual_discover_snapshot_binding_mismatch"));
    }
    binding_matches(binding, ModelId::Qwen38Flash)
}

pub fn discover_candidate(
    input: &VisualizationDiscoverInput,
    value: Value,
) -> Result<DiscoveryDisposition, SafeError> {
    let semantic_key = value
        .get("semantic_key")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("visual_candidate_semantic_key_missing"))?;
    if input.deleted_keys.iter().any(|key| key == semantic_key) {
        return Ok(DiscoveryDisposition::DeletedByUser);
    }
    if input.existing_keys.iter().any(|key| key == semantic_key) {
        return Ok(DiscoveryDisposition::DuplicateExisting);
    }
    let candidate: VisualizationCandidate = serde_json::from_value(value)
        .map_err(|_| invalid("visual_candidate_schema_mismatch"))?;
    let validated = semantic_check(&candidate, input)?;
    Ok(DiscoveryDisposition::Candidate(validated))
}

pub fn semantic_check(
    candidate: &VisualizationCandidate,
    input: &VisualizationDiscoverInput,
) -> Result<ValidatedCandidate, SafeError> {
    if !input.supported_templates.iter().any(|id| id == &candidate.template_id) {
        return Err(invalid("template_not_negotiated"));
    }
    if candidate.semantic_key.is_empty() || candidate.scope_id.is_empty() || candidate.purpose.is_empty() {
        return Err(invalid("visual_candidate_identity_missing"));
    }
    if candidate.source_refs.is_empty() {
        return Err(invalid("visual_candidate_source_missing"));
    }
    if candidate.template_version != template_version(&candidate.template_id).unwrap_or_default() {
        return Err(invalid("visual_candidate_template_version_mismatch"));
    }
    reject_arbitrary_code(&candidate.parameters)?;
    let mut diagnostics = Vec::new();
    match candidate.template_id.as_str() {
        "function_curve_2d" => {
            let params: focaus_contracts::CurveParameters = serde_json::from_value(candidate.parameters.clone())
                .map_err(|_| invalid("curve_parameters_schema_mismatch"))?;
            if params.samples == 0 || params.samples > 4096 || params.x_min >= params.x_max {
                return Err(invalid("curve_parameters_out_of_range"));
            }
            diagnostics.push("deterministic_math_pending_host_validator".into());
        }
        "conv2d_spatial" => {
            let params: focaus_contracts::Conv2dParameters = serde_json::from_value(candidate.parameters.clone())
                .map_err(|_| invalid("conv2d_parameters_schema_mismatch"))?;
            if params.operator_dimension != 2 || params.scene_dimension != 3 {
                return Err(invalid("conv2d_dimension_semantics_mismatch"));
            }
            if params.dilation != [1, 1] || params.stride.len() != 2 || params.padding.len() != 4 {
                return Err(invalid("conv2d_unsupported_shape_parameters"));
            }
            diagnostics.push("shape_and_values_pending_host_validator".into());
        }
        _ => return Err(invalid("template_not_in_controlled_registry")),
    }
    Ok(ValidatedCandidate {
        candidate: candidate.clone(),
        semantic_check: SemanticCheck { schema: CheckStatus::Pass, source: CheckStatus::Pass, semantic: CheckStatus::Pass, math: CheckStatus::Pending, diagnostics },
    })
}

fn template_version(template_id: &str) -> Option<String> {
    let registry: Value = serde_json::from_str(include_str!("../../../schemas/registry.json")).ok()?;
    registry.get("templates")?.get(template_id)?.get("version")?.as_str().map(str::to_owned)
}

fn reject_arbitrary_code(value: &Value) -> Result<(), SafeError> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if matches!(key.as_str(), "code" | "script" | "javascript" | "wasm" | "html" | "url" | "remote_resource") {
                    return Err(invalid("arbitrary_visualization_code_or_remote_resource"));
                }
                reject_arbitrary_code(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_arbitrary_code(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Proposal JSON is a candidate for `result.submit`; it contains no authority
/// ID and is intentionally marked proposed until N validates it.
pub fn build_proposal(
    validated: &ValidatedCandidate,
    input: &VisualizationDiscoverInput,
) -> Result<Value, SafeError> {
    if validated.semantic_check.semantic != CheckStatus::Pass
        || validated.semantic_check.math != CheckStatus::Pending
    {
        return Err(invalid("visual_candidate_not_semantically_ready"));
    }
    let candidate = &validated.candidate;
    let mut object = Map::new();
    object.insert("protocol".into(), json!("focaus.visualization"));
    object.insert("protocol_version".into(), json!(VISUAL_PROTOCOL));
    object.insert("registry_revision".into(), json!(REGISTRY_REVISION));
    object.insert("template_id".into(), json!(candidate.template_id));
    object.insert("template_version".into(), json!(candidate.template_version));
    object.insert("semantic_key".into(), json!(candidate.semantic_key));
    object.insert("purpose".into(), json!(candidate.purpose));
    object.insert("scope_id".into(), json!(candidate.scope_id));
    object.insert("parameters".into(), candidate.parameters.clone());
    object.insert("value_bindings".into(), json!(candidate.value_bindings));
    object.insert("source_refs".into(), json!(candidate.source_refs));
    object.insert("placement_anchor".into(), candidate.placement_anchor.clone());
    object.insert("snapshot_id".into(), json!(input.snapshot.snapshot_id));
    object.insert("generation_status".into(), json!("proposed"));
    object.insert("math_validation".into(), json!("pending_host_validator"));
    Ok(Value::Object(object))
}

pub fn build_generation_request(
    input: &VisualizationDiscoverInput,
    binding: &focaus_contracts::CredentialBinding,
    excerpts: &[Value],
) -> Result<Value, SafeError> {
    validate_discover_binding(input, binding)?;
    for excerpt in excerpts {
        reject_arbitrary_code(excerpt)?;
    }
    Ok(crate::transport::build_chat_completion_body(
        models::MULTIMODAL,
        PROMPT_VISUAL_DISCOVER_V1,
        "Return only controlled visualization candidates. Never emit code, URLs, or arbitrary templates.",
        json!({
        "snapshot_id": input.snapshot.snapshot_id,
        "supported_templates": input.supported_templates,
        "existing_semantic_keys": input.existing_keys,
        "deleted_semantic_keys": input.deleted_keys,
        "excerpts": excerpts,
        "output_contract": "controlled_visualization_candidate_v1"
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{EvidenceCutoff, EvidenceCutoffStatus, InputSnapshot, Selector, TaskKind, WholeSelector};

    fn source() -> SourceRef {
        SourceRef { entity_id: "fact".into(), revision_id: "rev".into(), content_hash: "hash".into(), selector: Selector::V4(WholeSelector { kind: "whole".into() }) }
    }

    fn input() -> VisualizationDiscoverInput {
        VisualizationDiscoverInput {
            snapshot: InputSnapshot { snapshot_id: "snap".into(), parent_snapshot_id: None, contract_version: "2.0.0".into(), purpose: TaskKind::VisualizationDiscover, target_id: "note".into(), lesson_id: Some("lesson".into()), recording_id: None, assignment_epoch: None, sources: vec![source()], attachment_set_revision: "a".into(), fact_set_revision: Some("facts".into()), target_base_revision: Some("base".into()), edit_revision: None, language_tier: Some(3), evidence_cutoff: vec![EvidenceCutoff { recording_id: "rec".into(), end_ms: None, status: EvidenceCutoffStatus::Unknown }], available_at: "now".into(), executor_id: "ai".into(), executor_version: "1".into(), model_id: Some(ModelId::Qwen38Flash), region: Some("cn-beijing".into()), prompt_version: Some(PROMPT_VISUAL_DISCOVER_V1.into()), parameters_hash: "p".into(), calibration_seq: None, decision_seq: None },
            existing_keys: vec![], deleted_keys: vec![], supported_templates: vec!["function_curve_2d".into(), "conv2d_spatial".into()],
        }
    }

    fn curve() -> Value {
        json!({
            "semantic_key":"f(scope,concept)", "template_id":"function_curve_2d", "template_version":"1.0.0", "purpose":"show relation", "scope_id":"scope", "concept_fact_ids":["fact"], "definition_ids":[], "symbol_binding_ids":[],
            "parameters":{"formula_id":"affine_v1","slope":1.0,"intercept":0.0,"x_min":0.0,"x_max":1.0,"samples":10}, "value_bindings":[], "source_refs":[source()], "placement_anchor":{"kind":"whole"}
        })
    }

    #[test]
    fn candidate_is_controlled_and_math_stays_pending() {
        let input = input();
        let found = discover_candidate(&input, curve()).unwrap();
        let DiscoveryDisposition::Candidate(validated) = found else { panic!("candidate expected") };
        assert_eq!(validated.semantic_check.math, CheckStatus::Pending);
        let proposal = build_proposal(&validated, &input).unwrap();
        assert_eq!(proposal["generation_status"], "proposed");
        assert_eq!(proposal["protocol_version"], VISUAL_PROTOCOL);
    }

    #[test]
    fn duplicate_and_deleted_semantic_keys_are_noops() {
        let mut input = input();
        input.existing_keys.push("f(scope,concept)".into());
        assert_eq!(discover_candidate(&input, curve()).unwrap(), DiscoveryDisposition::DuplicateExisting);
        input.existing_keys.clear();
        input.deleted_keys.push("f(scope,concept)".into());
        assert_eq!(discover_candidate(&input, curve()).unwrap(), DiscoveryDisposition::DeletedByUser);
    }

    #[test]
    fn arbitrary_code_and_wrong_template_are_rejected() {
        let input = input();
        let mut bad = curve();
        bad["parameters"]["script"] = json!("x");
        assert_eq!(discover_candidate(&input, bad).unwrap_err().code, focaus_contracts::ErrorCode::InvalidResult);
        let mut unsupported = curve();
        unsupported["template_id"] = json!("arbitrary_webgl");
        unsupported["template_version"] = json!("1.0.0");
        assert_eq!(discover_candidate(&input, unsupported).unwrap_err().code, focaus_contracts::ErrorCode::InvalidResult);
    }
}

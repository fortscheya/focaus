//! Shared contract boundary. Generated types are not permission checks.
//! The native host validates actor/channel, scope, CAS and source availability before commit.
mod generated;
pub use generated::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::OnceLock};
pub const CONTRACT_VERSION: &str = "2.0.0";
pub const MATERIAL_FORMAT: &str = "focaus.material/2.0.0";
pub const VISUALIZATION_PROTOCOL: &str = "focaus.visualization/1.0.0";
pub const SCHEMA_TEXT: &str = include_str!("../../../schemas/contracts.schema.json");
pub fn schema() -> &'static Value { static SCHEMA: OnceLock<Value> = OnceLock::new(); SCHEMA.get_or_init(|| serde_json::from_str(SCHEMA_TEXT).expect("bundled contract schema")) }
pub fn error(code: ErrorCode, message: &str) -> SafeError { SafeError { code, retryable:false, scope:SafeErrorScope::Command, message:message.into(), details:None } }
pub fn validate(kind: &str, value: &Value) -> Result<(), SafeError> {
    if schema()["$defs"].get(kind).is_none() { return Err(error(ErrorCode::InvalidResult,"Unknown contract type")); }
    let mut root=schema().clone(); root["$ref"]=json!(format!("#/$defs/{kind}"));
    let validator=jsonschema::options().should_validate_formats(true).build(&root).map_err(|_| error(ErrorCode::InvalidResult,"Invalid bundled schema"))?;
    if !validator.is_valid(value) { return Err(error(ErrorCode::InvalidResult,"Contract schema mismatch")); }
    Ok(())
}
/// RFC 8785 canonical JSON; failures reject non-canonicalizable input.
pub fn canonical_hash(value: &Value) -> String {
    format!("{:x}",Sha256::digest(serde_json_canonicalizer::to_vec(value).expect("finite JSON")))
}
pub fn credential_binding(model: &str, region: &str, slot: &str) -> Result<(),SafeError> {
    if region!="cn-beijing" || schema()["x-model-bindings"].get(model).and_then(Value::as_str)!=Some(slot) { return Err(error(ErrorCode::InvalidArgument,"credential_binding_mismatch")); }
    Ok(())
}
pub fn negotiate(c: &Capabilities) -> Result<NegotiatedCapabilities,SafeError> {
    if !c.supported_contract_majors.contains(&2) || !c.supported_material_majors.contains(&2) { return Err(error(ErrorCode::UnsupportedVersion,"Unsupported contract or material major")); }
    let mut unsupported=Vec::new();
    if !c.visualization_protocol_majors.contains(&1) { unsupported.push("visualization".into()); }
    for (k,v) in schema()["x-registry"]["templates"].as_object().expect("registry") {
      if !c.templates.get(k).is_some_and(|versions| versions.iter().any(|x| x==v["version"].as_str().unwrap_or(""))) { unsupported.push(k.clone()); }
    }
    if !c.search_projection_versions.contains(&1) {unsupported.push("search".into());}
    if !c.focaus_result_semantics.iter().any(|x|x=="focaus_context_v2") {unsupported.push("focaus".into());}
    Ok(NegotiatedCapabilities {contract_version:CONTRACT_VERSION.into(), material_version:"2.0.0".into(), visualization_version:"1.0.0".into(), write_allowed:unsupported.is_empty(), unsupported})
}
pub fn read_visualization(value: &Value) -> Result<Value,SafeError> {
    if value["protocol_version"].as_str().and_then(|s|s.split('.').next())!=Some("1") {return Err(error(ErrorCode::UnsupportedVersion,"Unknown visualization major"));}
    let supported=value["template_id"].as_str().and_then(|id|schema()["x-registry"]["templates"].get(id));
    let editable=supported.is_some_and(|s|s["version"]==value["template_version"]);
    Ok(json!({"mode":if editable {"supported"} else {"unsupported"},"write_allowed":editable,"original":value,"fallback":value["fallback"]}))
}
pub fn adapt_legacy_catchup(value: Value) -> LegacySummary { LegacySummary{result_semantics:"legacy_summary".into(),original:value,completion_status:"partial".into(),new_acceptance_eligible:false} }
/// Owned secret handle boundary: no secret bytes are part of public data or Debug output.
/// N implements this with host secure storage; A receives a bounded transport capability.
pub trait CredentialResolver: Send + Sync { fn require_binding(&self, binding:&CredentialBinding) -> Result<(),SafeError>; }
pub type HostFuture<'a,T> = Pin<Box<dyn Future<Output=Result<T,SafeError>> + Send + 'a>>;
pub trait HostServices: Send + Sync {
 fn invoke<'a>(&'a self, command:Command) -> HostFuture<'a,CommandResult>;
 fn snapshot_build<'a>(&'a self,input:SnapshotBuildInput)->HostFuture<'a,InputSnapshot>;
 fn result_submit<'a>(&'a self,input:ResultSubmitInput)->HostFuture<'a,CommandResult>;
 fn task_checkpoint<'a>(&'a self,input:TaskCheckpointInput)->HostFuture<'a,Task>;
 fn visualization_evaluate<'a>(&'a self,input:VisualizationEvaluateInput)->HostFuture<'a,EvaluationResult>;
 fn visualization_validate<'a>(&'a self,input:VisualizationValidateInput)->HostFuture<'a,ValidationReport>;
 fn search_open<'a>(&'a self,input:SearchOpenInput)->HostFuture<'a,SearchSession>;
 fn search_query<'a>(&'a self,input:SearchQueryInput)->HostFuture<'a,Task>;
 fn search_page<'a>(&'a self,input:SearchPageInput)->HostFuture<'a,SearchPage>;
 fn search_locate<'a>(&'a self,input:SearchLocateInput)->HostFuture<'a,SearchAnchor>;
 fn context_retrieve<'a>(&'a self,input:ContextRetrieveInput)->HostFuture<'a,ContextResult>;
 fn catchup_display_ack<'a>(&'a self,input:CatchupDisplayAckInput)->HostFuture<'a,CommandResult>;
}
/// G1 composition stub always fails closed. It never pretends a fixture is native success.
pub struct StubHost;
macro_rules! unavailable { ($name:ident,$input:ty,$output:ty) => {fn $name<'a>(&'a self,_input:$input)->HostFuture<'a,$output>{Box::pin(async {Err(error(ErrorCode::UnsupportedCapability,"G1 composition stub; native adapter not connected"))})}}; }
impl HostServices for StubHost {
 unavailable!(invoke,Command,CommandResult);
 unavailable!(snapshot_build,SnapshotBuildInput,InputSnapshot);
 unavailable!(result_submit,ResultSubmitInput,CommandResult);
 unavailable!(task_checkpoint,TaskCheckpointInput,Task);
 unavailable!(visualization_evaluate,VisualizationEvaluateInput,EvaluationResult);
 unavailable!(visualization_validate,VisualizationValidateInput,ValidationReport);
 unavailable!(search_open,SearchOpenInput,SearchSession);
 unavailable!(search_query,SearchQueryInput,Task);
 unavailable!(search_page,SearchPageInput,SearchPage);
 unavailable!(search_locate,SearchLocateInput,SearchAnchor);
 unavailable!(context_retrieve,ContextRetrieveInput,ContextResult);
 unavailable!(catchup_display_ack,CatchupDisplayAckInput,CommandResult);
}
pub fn command_catalogue() -> BTreeMap<String,Value> {serde_json::from_value(schema()["x-command-catalogue"].clone()).expect("catalogue")}

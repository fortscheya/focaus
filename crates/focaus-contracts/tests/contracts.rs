use focaus_contracts::*;
use serde_json::Value;
#[test]
fn schema_fixtures() {
 let cases:Value=serde_json::from_str(include_str!("../../../fixtures/contracts/cases.json")).unwrap();
 for c in cases.as_array().unwrap() {
  if c["level"]!="schema" {continue;}
  let got=validate(c["type"].as_str().unwrap(),&c["value"]);
  assert_eq!(got.is_ok(),c["valid"].as_bool().unwrap(),"{}: {:?}",c["name"],got);
 }
}
#[test]
fn generated_types_are_consumable() {
 let cases:Value=serde_json::from_str(include_str!("../../../fixtures/contracts/cases.json")).unwrap();
 for c in cases.as_array().unwrap().iter().filter(|c|c["valid"]==true) {
  let v=c["value"].clone();
  match c["type"].as_str().unwrap() {
   "InputSnapshot"=>{serde_json::from_value::<InputSnapshot>(v).unwrap();},
   "VisualizationContent"=>{serde_json::from_value::<VisualizationContent>(v).unwrap();},
   "MaterialManifest"=>{serde_json::from_value::<MaterialManifest>(v).unwrap();},
   "CatchupResult"=>{serde_json::from_value::<CatchupResult>(v).unwrap();},
   "Command"=>{serde_json::from_value::<Command>(v).unwrap();},
   _=>{}
  }
 }
}
#[test]
fn credential_binding_never_crosses_slots() {
 for (m,s) in schema()["x-model-bindings"].as_object().unwrap() {
  for other in schema()["x-model-bindings"].as_object().unwrap().values() {
   assert_eq!(credential_binding(m,"cn-beijing",other.as_str().unwrap()).is_ok(),s==other);
  }
  assert!(credential_binding(m,"wrong-region",s.as_str().unwrap()).is_err());
 }
}
#[test]
fn canonical_hash_is_cross_language() {
 let vectors:Value=serde_json::from_str(include_str!("../../../fixtures/contracts/hash-golden.json")).unwrap();
 for c in vectors.as_array().unwrap() {assert_eq!(canonical_hash(&c["value"]),c["sha256"].as_str().unwrap());}
}
#[test]
fn unknown_component_preserves_original_but_refuses_writes() {
 let v=serde_json::json!({"protocol_version":"1.0.0","template_id":"unknown","template_version":"1.0.0","opaque":{"future":true},"fallback":{"description":"unsupported"}});
 let result=read_visualization(&v).unwrap();assert_eq!(result["write_allowed"],false);assert_eq!(result["original"],v);
 let mut future=v.clone();future["protocol_version"]=serde_json::json!("9.0.0");assert!(read_visualization(&future).is_err());
}

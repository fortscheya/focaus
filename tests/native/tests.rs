use super::*;
use focaus_contracts::*;
use rusqlite::params;
use serde_json::{json,Value};
use std::{collections::BTreeMap,sync::{Arc,Mutex}};

struct TestLibrary {path:std::path::PathBuf,store:storage::Store}
impl TestLibrary {fn new()->Self {let path=std::env::temp_dir().join(format!("focaus-native-{}",storage::id()));let store=storage::Store::open(&path,INITIAL_MIGRATION).unwrap();Self {path,store}}}
impl Drop for TestLibrary {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.path);}}
fn command(name:&str,window:&str,payload:Value)->Command {serde_json::from_value(json!({"contract_version":"2.0.0","command_id":storage::id(),"name":name,"actor":"user","window_id":window,"payload":payload,"idempotency_key":storage::id(),"expected_revisions":{}})).unwrap()}
fn reserve(store:&storage::Store)->Value {store.write(|t|capture::RecordingCoordinator::reserve(t,&RecordingStartInput {lesson_id:None,device_id:None},None)).unwrap()}
fn capabilities()->Capabilities {Capabilities {supported_contract_majors:vec![2],supported_material_majors:vec![2],visualization_protocol_majors:vec![1],templates:[("function_curve_2d".into(),vec!["1.0.0".into()]),("conv2d_spatial".into(),vec!["1.0.0".into()])].into(),search_projection_versions:vec![1],focaus_result_semantics:vec!["focaus_context_v2".into()]}}

#[test] fn migration_and_rational_time() {
    let db=TestLibrary::new();assert_eq!(db.store.visibility_epoch().unwrap(),0);
    assert_eq!(audio::frame_ms(48001,48000).unwrap(),1000);
    assert_eq!(audio::frame_ms(4_800_000_001,48000).unwrap(),100_000_000);
    assert!(audio::frame_ms(1,0).is_err());
    assert_eq!(storage::Store::open(&db.path,INITIAL_MIGRATION).unwrap().visibility_epoch().unwrap(),0);
}
#[test] fn original_segment_recovery_keeps_decodable_prefix_and_never_opens_device() {
    let db=TestLibrary::new();let recording=reserve(&db.store);let rid=recording["recording_id"].as_str().unwrap();
    let meta=audio::SegmentMeta::new(rid,48000,1,0,0).unwrap();let asset=meta.asset_id.clone();
    let mut writer=audio::SegmentWriter::create(&db.path,meta).unwrap();writer.append(&[0.25;4800]).unwrap();drop(writer);
    use std::io::Write;
    let part=db.path.join("capture").join(rid).join(format!("{asset}.wav.part"));std::fs::OpenOptions::new().append(true).open(part).unwrap().write_all(&[255]).unwrap();
    let recovered=audio::recover(&db.store).unwrap();assert_eq!(recovered.recovered_segments,1);assert_eq!(recovered.recovered_frames,4800);assert_eq!(recovered.failed_segments,0);
    assert_eq!(db.store.head(rid).unwrap()["state"],"interrupted");assert_eq!(db.store.head(rid).unwrap()["durable_watermark_ms"],100);
    assert_eq!(audio::recover(&db.store).unwrap().recovered_segments,0);
}
#[test] fn original_segments_and_gap_do_not_invent_silence() {
    let db=TestLibrary::new();let r=reserve(&db.store);let rid=r["recording_id"].as_str().unwrap();
    let mut writer=audio::SegmentWriter::create(&db.path,audio::SegmentMeta::new(rid,48000,1,0,0).unwrap()).unwrap();writer.append(&[0.;480]).unwrap();writer.seal(&db.store,false).unwrap();audio::record_gap(&db.store,rid,10,1000,false).unwrap();
    let rows=db.store.read(|c|Ok(c.query_row("SELECT COUNT(*) FROM audio_spans WHERE coverage_status='captured'",[],|r|r.get::<_,u64>(0)).map_err(storage::sql_error)?)).unwrap();assert_eq!(rows,1);
}
#[test] fn concurrent_start_is_enforced_by_authoritative_unique_session() {
    let db=TestLibrary::new();reserve(&db.store);
    let e=db.store.write(|t|capture::RecordingCoordinator::reserve(t,&RecordingStartInput {lesson_id:None,device_id:None},None)).unwrap_err();assert_eq!(e.code,ErrorCode::RecordingBusy);
    let coordinator=capture::RecordingCoordinator::new(db.store.clone());assert_eq!(coordinator.start_playback().unwrap_err().code,ErrorCode::PlaybackBlocked);
}
#[test] fn business_revision_outbox_and_receipt_commit_atomically() {
    let db=TestLibrary::new();let win=storage::id();let mut c=command("course.create",&win,json!({"name":"课"}));
    let first=db.store.command(&win,&c,|t|library::create_course(t,&CourseCreateInput {name:"课".into()},None)).unwrap();
    c.command_id=storage::id();let again=db.store.command(&win,&c,|_|panic!("duplicate intent executed")).unwrap();assert_eq!(first.result,again.result);assert_eq!(again.command_id,c.command_id);
    c.payload=json!({"name":"changed"});assert_eq!(db.store.command(&win,&c,|_|Ok(Value::Null)).unwrap_err().code,ErrorCode::IdempotencyMismatch);
    let before=db.store.read(storage::sequence).unwrap();let eid=storage::id();
    let aborted=db.store.write(|t|->storage::Result<()> {storage::commit_entity(t,&eid,"note",json!({"x":1}),"content.committed",json!({"kind":"global"}),None,None)?;Err(error(ErrorCode::InvalidResult,"injected"))});assert!(aborted.is_err());assert_eq!(db.store.read(storage::sequence).unwrap(),before);assert!(db.store.head(&eid).is_err());
}
#[test] fn authenticated_window_snapshot_ack_and_filtered_watermark() {
    fn send_sync<T:Send+Sync>(){}send_sync::<ipc::NativeHost>();
    let db=TestLibrary::new();let host=ipc::NativeHost::open(&db.path,INITIAL_MIGRATION).unwrap();let window=storage::id();host.bind_window("main",&window,vec![serde_json::from_value(json!({"kind":"global"})).unwrap()]).unwrap();
    let mut forged=command("course.create",&window,json!({"name":"x"}));forged.actor=CommandActor::Host;assert_eq!(host.dispatch_webview("main",forged).unwrap_err().code,ErrorCode::PermissionDenied);
    let internal=command("result.submit",&window,json!({}));assert_eq!(host.dispatch_webview("main",internal).unwrap_err().code,ErrorCode::PermissionDenied);
    let response=host.dispatch_webview("main",command("state.open",&window,json!({"scopes":[{"kind":"global"}],"last_seq":null,"capabilities":capabilities()}))).unwrap();validate("CommandResult",&json!(response)).unwrap();let opened=response.result.unwrap();let sid=opened["subscription_id"].as_str().unwrap();let seq=opened["snapshot_seq"].as_u64().unwrap();
    let response=host.dispatch_webview("main",command("course.create",&window,json!({"name":"before ack"}))).unwrap();validate("CommandResult",&json!(response)).unwrap();assert!(host.take_event_batches("main").unwrap().is_empty());
    host.dispatch_webview("main",command("state.ack",&window,json!({"subscription_id":sid,"upto_seq":seq}))).unwrap();let batches=host.take_event_batches("main").unwrap();assert_eq!(batches.len(),1);assert_eq!(batches[0].events.len(),1);assert_eq!(batches[0].after_seq,seq);validate("EventBatch",&json!(batches[0])).unwrap();
    let bad=host.dispatch_webview("main",command("state.ack",&window,json!({"subscription_id":sid,"upto_seq":batches[0].through_seq+1}))).unwrap_err();assert_eq!(bad.code,ErrorCode::Conflict);
    host.unbind_window("main").unwrap();assert!(host.take_event_batches("main").is_err());
}
struct Slots {calls:Mutex<Vec<String>>,available:bool}
impl tasks::SecureSlotStore for Slots {fn contains(&self,slot:&str)->storage::Result<bool>{self.calls.lock().unwrap().push(slot.into());Ok(self.available)}}
#[test] fn dedicated_credential_bindings_fail_before_any_other_slot_lookup() {
    let slots=Arc::new(Slots {calls:Mutex::new(Vec::new()),available:false});let resolver=tasks::NativeCredentialResolver::new(slots.clone());
    for model in ["qwen-audio-3.1-asr-flash-streaming","qwen-audio-3.1-asr-flash-filetrans","decision-model-preview","qwen3.8-flash"] {
        let model:ModelId=serde_json::from_value(json!(model)).unwrap();let binding=tasks::binding_for(&model).unwrap();assert_eq!(resolver.require_binding(&binding).unwrap_err().code,ErrorCode::AuthRequired);
    }
    assert_eq!(slots.calls.lock().unwrap().len(),4);
    let mut wrong=tasks::binding_for(&serde_json::from_value(json!("qwen-audio-3.1-asr-flash-filetrans")).unwrap()).unwrap();wrong.credential_ref=serde_json::from_value(json!("bailian_beijing_asr_streaming")).unwrap();assert_eq!(resolver.require_binding(&wrong).unwrap_err().code,ErrorCode::InvalidArgument);assert_eq!(slots.calls.lock().unwrap().len(),4);
}
#[test] fn independent_math_golden_and_rectangular_multichannel_case() {
    let gold:Value=serde_json::from_str(include_str!("../../fixtures/contracts/math-golden.json")).unwrap();
    for case in gold.as_array().unwrap(){let template=if case["type"]=="CurveParameters"{"function_curve_2d"}else{"conv2d_spatial"};let r=visualizations::evaluate_template(template,&VisualizationEvaluateInput {object_id:storage::id(),revision_id:storage::id(),parameters:case["parameters"].clone(),interaction_seq:1,algorithm_version:"1.0.0".into()}).unwrap();assert_eq!(json!(r.shape),case["shape"],"{}",case["name"]);for (actual,expected) in r.values.iter().zip(case["values"].as_array().unwrap()){assert!((actual-expected.as_f64().unwrap()).abs()<1e-9);}}
    let mut input=VisualizationEvaluateInput {object_id:storage::id(),revision_id:storage::id(),parameters:json!({"operator_dimension":2,"scene_dimension":3,"operator_semantics":"cross_correlation","axis_order":"CHW","input":[[[1,2,3],[4,5,6]],[[10,20,30],[40,50,60]]],"kernel":[[[1,2]],[[1,-1]]],"stride":[1,1],"padding":[0,0,0,0],"dilation":[1,1],"bias":3}),interaction_seq:1,algorithm_version:"1.0.0".into()};
    let r=visualizations::evaluate_template("conv2d_spatial",&input).unwrap();assert_eq!(r.shape,[2,2]);assert_eq!(r.values,[-2.,1.,7.,10.]);input.parameters["stride"]=json!([0,1]);assert!(visualizations::evaluate_template("conv2d_spatial",&input).is_err());
}
#[test] fn task_resource_reservation_cancellation_and_restart_keep_checkpoint() {
    let db=TestLibrary::new();let target=storage::id();
    let snapshot=InputSnapshot {snapshot_id:storage::id(),parent_snapshot_id:None,contract_version:CONTRACT_VERSION.into(),purpose:TaskKind::MaterialConvert,target_id:target.clone(),lesson_id:None,recording_id:None,assignment_epoch:None,sources:Vec::new(),attachment_set_revision:storage::id(),fact_set_revision:None,target_base_revision:None,edit_revision:None,language_tier:None,evidence_cutoff:Vec::new(),available_at:storage::now(),executor_id:"native-test".into(),executor_version:"1.0.0".into(),model_id:None,region:None,prompt_version:None,parameters_hash:canonical_hash(&Value::Null),calibration_seq:None,decision_seq:None};
    db.store.write(|t|{t.execute("INSERT INTO input_snapshots VALUES(?1,NULL,?2,?3,?4,?5)",params![snapshot.snapshot_id,CONTRACT_VERSION,storage::encode(&snapshot)?,canonical_hash(&json!(snapshot)),snapshot.available_at]).map_err(storage::sql_error)?;Ok(())}).unwrap();
    let manager=tasks::TaskManager::new(db.store.clone());let tid=storage::id();manager.enqueue(Task {task_id:tid.clone(),kind:TaskKind::MaterialConvert,target_id:target,snapshot_id:snapshot.snapshot_id,state:TaskState::Queued,priority:4,attempt:1,deadline_at:None,idempotency_key:storage::id(),checkpoint:BTreeMap::new(),error:None,provider_bindings:Vec::new(),created_at:storage::now(),updated_at:storage::now(),model_id:None,region:None,prompt_version:None,parameters_hash:snapshot.parameters_hash}).unwrap();
    assert!(manager.claim(1,1).unwrap().is_none());assert_eq!(manager.claim(2,1).unwrap().unwrap().task_id,tid);
    manager.checkpoint(TaskCheckpointInput {task_id:tid.clone(),attempt:1,cursor:"page-2".into(),safe_state:BTreeMap::new()}).unwrap();let recovered=manager.recover().unwrap();assert_eq!(recovered[0].state,TaskState::Interrupted);assert_eq!(recovered[0].checkpoint["cursor"],"page-2");
    manager.transition(&tid,1,TaskState::CancelRequested).unwrap();assert!(manager.checkpoint(TaskCheckpointInput {task_id:tid.clone(),attempt:1,cursor:"late".into(),safe_state:BTreeMap::new()}).is_err());manager.transition(&tid,1,TaskState::Cancelled).unwrap();assert!(manager.transition(&tid,1,TaskState::Queued).is_err());
}

//! Real single-host SQLite integration through the same wire boundary as Tauri.
//! No fixture host, microphone, network, or model is used.
use focaus_contracts::{CommandResult, ErrorCode};
use focaus_desktop::{ipc::NativeHost, storage, wire, INITIAL_MIGRATION};
use serde_json::{json, Value};

#[test]
fn host_math_matches_independent_goldens_including_asymmetric_padding() {
    let cases: Value = serde_json::from_str(include_str!("../../fixtures/contracts/math-golden.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let template = if case["type"] == "CurveParameters" {"function_curve_2d"} else {"conv2d_spatial"};
        let input = focaus_contracts::VisualizationEvaluateInput {
            object_id:storage::id(), revision_id:storage::id(), interaction_seq:1,
            parameters:case["parameters"].clone(), algorithm_version:"1.0.0".into(),
        };
        let result = focaus_desktop::visualizations::evaluate_template(template, &input).unwrap();
        assert_eq!(serde_json::to_value(result.shape).unwrap(), case["shape"], "{}", case["name"]);
        let expected:Vec<f64> = serde_json::from_value(case["values"].clone()).unwrap();
        assert_eq!(result.values, expected, "{}", case["name"]);
    }
}

fn capabilities() -> Value {
    json!({"supported_contract_majors":[2],"supported_material_majors":[2],
        "visualization_protocol_majors":[1],"templates":{"function_curve_2d":["1.0.0"],"conv2d_spatial":["1.0.0"]},
        "search_projection_versions":[1],"focaus_result_semantics":["focaus_context_v2"]})
}
fn send(host:&NativeHost, label:&str, window:&str, name:&str, payload:Value, intent:&str) -> Result<CommandResult,focaus_contracts::SafeError> {
    let command = wire::decode_command(json!({"contract_version":"2.0.0","command_id":storage::id(),
        "actor":"user","window_id":window,"name":name,"payload":payload,
        "idempotency_key":intent,"expected_revisions":{}}))?;
    let result=host.dispatch_webview(label,command)?;
    wire::validate_result(&result)?;
    let output=focaus_contracts::command_catalogue()[name]["output"].as_str().unwrap().to_owned();
    if let Some(body)=&result.result {focaus_contracts::validate(&output,body)?;}
    Ok(result)
}
fn bind(host:&NativeHost,label:&str)->String {
    let id=storage::id();
    host.bind_window(label,&id,vec![serde_json::from_value(json!({"kind":"global"})).unwrap()]).unwrap(); id
}
fn open(host:&NativeHost,label:&str,window:&str)->Value {
    send(host,label,window,"state.open",json!({"scopes":[{"kind":"global"}],"last_seq":null,"capabilities":capabilities()}),"open").unwrap().result.unwrap()
}

#[test]
fn two_windows_share_durable_state_and_restart_without_capture() {
    let dir=std::env::temp_dir().join(format!("focaus-integration-{}",storage::id()));
    let host=NativeHost::open(&dir,INITIAL_MIGRATION).unwrap();
    let first_run=host.run_id.clone();
    let first=bind(&host,"main"); let second=bind(&host,"course-1");
    let opened=open(&host,"main",&first);
    assert_eq!(opened["negotiated"]["write_allowed"],true);
    assert_eq!(opened["snapshot"]["global"]["recording"],Value::Null);
    let course=send(&host,"main",&first,"course.create",json!({"name":"本地联调课程"}),"create-course").unwrap().result.unwrap();
    let duplicate=send(&host,"main",&first,"course.create",json!({"name":"本地联调课程"}),"create-course").unwrap().result.unwrap();
    assert_eq!(course,duplicate);
    let lesson=send(&host,"main",&first,"lesson.create",json!({"course_id":course["course_id"],"title":"第一堂","actual_date":null,"timezone":"Asia/Shanghai"}),"create-lesson").unwrap().result.unwrap();
    assert!(host.take_event_batches("main").unwrap().is_empty());
    send(&host,"main",&first,"state.ack",json!({"subscription_id":opened["subscription_id"],"upto_seq":opened["snapshot_seq"]}),"ack").unwrap();
    let batches=host.take_event_batches("main").unwrap();
    assert_eq!(batches.len(),1); assert_eq!(batches[0].events.len(),2);
    for event in &batches[0].events {
        let content=send(&host,"main",&first,"content.get",json!({"entity_id":event.entity_id,"revision_id":event.revision_id,"selector":null}),"read").unwrap().result.unwrap();
        assert!(content.is_object());
    }
    send(&host,"main",&first,"state.ack",json!({"subscription_id":opened["subscription_id"],"upto_seq":batches[0].through_seq}),"ack-next").unwrap();
    assert!(host.take_event_batches("main").unwrap().is_empty());
    let other=open(&host,"course-1",&second);
    assert_eq!(other["snapshot"]["records"]["courses"].as_object().unwrap().len(),1);
    assert_eq!(other["snapshot"]["records"]["lessons"].as_object().unwrap().len(),1);
    assert_eq!(send(&host,"course-1",&first,"content.get",json!({"entity_id":lesson["lesson_id"],"revision_id":null,"selector":null}),"forged").unwrap_err().code,ErrorCode::PermissionDenied);
    host.unbind_window("main").unwrap();
    host.exit().unwrap(); drop(host);
    let reopened=NativeHost::open(&dir,INITIAL_MIGRATION).unwrap();
    assert_ne!(reopened.run_id,first_run);
    let window=bind(&reopened,"main");let snapshot=open(&reopened,"main",&window);
    assert_eq!(snapshot["snapshot"]["global"]["recording"],Value::Null);
    assert_eq!(snapshot["snapshot"]["records"]["lessons"].as_object().unwrap().len(),1);
    reopened.exit().unwrap();drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}

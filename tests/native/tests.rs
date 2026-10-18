use super::*;
use focaus_contracts::*;
use rusqlite::params;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct TestLibrary {
    path: std::path::PathBuf,
    store: storage::Store,
}
impl TestLibrary {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("focaus-native-{}", storage::id()));
        let store = storage::Store::open(&path, INITIAL_MIGRATION).unwrap();
        Self { path, store }
    }
}
impl Drop for TestLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn command(name: &str, window: &str, payload: Value) -> Command {
    serde_json::from_value(json!({"contract_version":"2.0.0","command_id":storage::id(),"name":name,"actor":"user","window_id":window,"payload":payload,"idempotency_key":storage::id(),"expected_revisions":{}})).unwrap()
}
fn reserve(store: &storage::Store) -> Value {
    store
        .write(|t| {
            capture::RecordingCoordinator::reserve(
                t,
                &RecordingStartInput {
                    lesson_id: None,
                    device_id: None,
                },
                None,
            )
        })
        .unwrap()
}
fn capabilities() -> Capabilities {
    Capabilities {
        supported_contract_majors: vec![2],
        supported_material_majors: vec![2],
        visualization_protocol_majors: vec![1],
        templates: [
            ("function_curve_2d".into(), vec!["1.0.0".into()]),
            ("conv2d_spatial".into(), vec!["1.0.0".into()]),
        ]
        .into(),
        search_projection_versions: vec![1],
        focaus_result_semantics: vec!["focaus_context_v2".into()],
    }
}
fn lesson(store: &storage::Store) -> String {
    store
        .write(|t| {
            let course = library::create_course(
                t,
                &CourseCreateInput {
                    name: "native fixture".into(),
                },
                None,
            )?;
            let lesson = library::create_lesson(
                t,
                &LessonCreateInput {
                    course_id: course["course_id"].as_str().unwrap().into(),
                    title: "local test".into(),
                    actual_date: None,
                    timezone: "Asia/Shanghai".into(),
                },
                None,
            )?;
            Ok(lesson["lesson_id"].as_str().unwrap().to_owned())
        })
        .unwrap()
}
fn note(store: &storage::Store, lesson: &str, text: &str) -> String {
    let nid = storage::id();
    let object = storage::id();
    let block = storage::id();
    let run = storage::id();
    store.write(|t|{let fact_revision=storage::id();let body=json!({"note_id":nid,"lesson_id":lesson,"fact_set_revision":fact_revision,"fact_ids":[],"objects":[{"object_id":object,"revision_id":storage::id(),"type":"text","source_refs":[],"region_ids":[],"layout":{"bounds":[0,0,1,1],"z_order":0,"group_id":null,"constraints":[]},"extraction":"native","editability":"native","status":"complete","children":[],"content":{"block_id":block,"runs":[{"element_id":run,"text":text}],"level":1}}],"revision_id":storage::id()});
        let body=storage::commit_entity(t,&nid,"note",body,"content.committed",json!({"kind":"lesson","lesson_id":lesson}),None,None)?;t.execute("INSERT INTO notes VALUES(?1,?2,?3,?4)",params![nid,lesson,fact_revision,storage::encode(&body["objects"])?]).map_err(storage::sql_error)?;Ok(())}).unwrap();
    nid
}
fn view(store: &storage::Store, lesson: &str, owner: &str, window: &str) -> ContentView {
    let input:ViewOpenInput=serde_json::from_value(json!({"lesson_id":lesson,"target":{"kind":"entity","entity_id":owner,"revision_id":null,"selector":null},"placement":"grid"})).unwrap();
    let value = store
        .write(|t| windows::open(t, &input, window, None))
        .unwrap();
    serde_json::from_value(value).unwrap()
}

#[test]
fn migration_and_rational_time() {
    let db = TestLibrary::new();
    assert_eq!(db.store.visibility_epoch().unwrap(), 0);
    assert_eq!(audio::frame_ms(48001, 48000).unwrap(), 1000);
    assert_eq!(audio::frame_ms(4_800_000_001, 48000).unwrap(), 100_000_000);
    assert!(audio::frame_ms(1, 0).is_err());
    assert_eq!(
        storage::Store::open(&db.path, INITIAL_MIGRATION)
            .unwrap()
            .visibility_epoch()
            .unwrap(),
        0
    );
}
#[test]
fn original_segment_recovery_keeps_decodable_prefix_and_never_opens_device() {
    let db = TestLibrary::new();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap();
    let meta = audio::SegmentMeta::new(rid, 48000, 1, 0, 0).unwrap();
    let asset = meta.asset_id.clone();
    let mut writer = audio::SegmentWriter::create(&db.path, meta).unwrap();
    writer.append(&[0.25; 4800]).unwrap();
    drop(writer);
    use std::io::Write;
    let part = db
        .path
        .join("capture")
        .join(rid)
        .join(format!("{asset}.wav.part"));
    std::fs::OpenOptions::new()
        .append(true)
        .open(part)
        .unwrap()
        .write_all(&[255])
        .unwrap();
    let recovered = audio::recover(&db.store).unwrap();
    assert_eq!(recovered.recovered_segments, 1);
    assert_eq!(recovered.recovered_frames, 4800);
    assert_eq!(recovered.failed_segments, 0);
    assert_eq!(db.store.head(rid).unwrap()["state"], "interrupted");
    assert_eq!(db.store.head(rid).unwrap()["durable_watermark_ms"], 100);
    assert_eq!(audio::recover(&db.store).unwrap().recovered_segments, 0);
}
#[test]
fn original_segments_and_gap_do_not_invent_silence() {
    let db = TestLibrary::new();
    let r = reserve(&db.store);
    let rid = r["recording_id"].as_str().unwrap();
    let mut writer = audio::SegmentWriter::create(
        &db.path,
        audio::SegmentMeta::new(rid, 48000, 1, 0, 0).unwrap(),
    )
    .unwrap();
    writer.append(&[0.; 480]).unwrap();
    writer.seal(&db.store, false).unwrap();
    audio::record_gap(&db.store, rid, 10, 1000, false).unwrap();
    let rows = db
        .store
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audio_spans WHERE coverage_status='captured'",
                [],
                |r| r.get::<_, u64>(0),
            )
            .map_err(storage::sql_error)?)
        })
        .unwrap();
    assert_eq!(rows, 1);
}
#[test]
fn concurrent_start_is_enforced_by_authoritative_unique_session() {
    let db = TestLibrary::new();
    reserve(&db.store);
    let e = db
        .store
        .write(|t| {
            capture::RecordingCoordinator::reserve(
                t,
                &RecordingStartInput {
                    lesson_id: None,
                    device_id: None,
                },
                None,
            )
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::RecordingBusy);
    let coordinator = capture::RecordingCoordinator::new(db.store.clone());
    assert_eq!(
        coordinator.start_playback().unwrap_err().code,
        ErrorCode::PlaybackBlocked
    );
}
#[test]
fn business_revision_outbox_and_receipt_commit_atomically() {
    let db = TestLibrary::new();
    let win = storage::id();
    let mut c = command("course.create", &win, json!({"name":"课"}));
    let first = db
        .store
        .command(&win, &c, |t| {
            library::create_course(t, &CourseCreateInput { name: "课".into() }, None)
        })
        .unwrap();
    c.command_id = storage::id();
    let again = db
        .store
        .command(&win, &c, |_| panic!("duplicate intent executed"))
        .unwrap();
    assert_eq!(first.result, again.result);
    assert_eq!(again.command_id, c.command_id);
    c.payload = json!({"name":"changed"});
    assert_eq!(
        db.store
            .command(&win, &c, |_| Ok(Value::Null))
            .unwrap_err()
            .code,
        ErrorCode::IdempotencyMismatch
    );
    let before = db.store.read(storage::sequence).unwrap();
    let eid = storage::id();
    let aborted = db.store.write(|t| -> storage::Result<()> {
        storage::commit_entity(
            t,
            &eid,
            "note",
            json!({"x":1}),
            "content.committed",
            json!({"kind":"global"}),
            None,
            None,
        )?;
        Err(error(ErrorCode::InvalidResult, "injected"))
    });
    assert!(aborted.is_err());
    assert_eq!(db.store.read(storage::sequence).unwrap(), before);
    assert!(db.store.head(&eid).is_err());
}
#[test]
fn authenticated_window_snapshot_ack_and_filtered_watermark() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ipc::NativeHost>();
    let db = TestLibrary::new();
    let host = ipc::NativeHost::open(&db.path, INITIAL_MIGRATION).unwrap();
    let window = storage::id();
    host.bind_window(
        "main",
        &window,
        vec![serde_json::from_value(json!({"kind":"global"})).unwrap()],
    )
    .unwrap();
    let mut forged = command("course.create", &window, json!({"name":"x"}));
    forged.actor = CommandActor::Host;
    assert_eq!(
        host.dispatch_webview("main", forged).unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    let internal = command("result.submit", &window, json!({}));
    assert_eq!(
        host.dispatch_webview("main", internal).unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    let response = host
        .dispatch_webview(
            "main",
            command(
                "state.open",
                &window,
                json!({"scopes":[{"kind":"global"}],"last_seq":null,"capabilities":capabilities()}),
            ),
        )
        .unwrap();
    validate("CommandResult", &json!(response)).unwrap();
    let opened = response.result.unwrap();
    let sid = opened["subscription_id"].as_str().unwrap();
    let seq = opened["snapshot_seq"].as_u64().unwrap();
    let response = host
        .dispatch_webview(
            "main",
            command("course.create", &window, json!({"name":"before ack"})),
        )
        .unwrap();
    validate("CommandResult", &json!(response)).unwrap();
    assert!(host.take_event_batches("main").unwrap().is_empty());
    host.dispatch_webview(
        "main",
        command(
            "state.ack",
            &window,
            json!({"subscription_id":sid,"upto_seq":seq}),
        ),
    )
    .unwrap();
    let batches = host.take_event_batches("main").unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].events.len(), 1);
    assert_eq!(batches[0].after_seq, seq);
    validate("EventBatch", &json!(batches[0])).unwrap();
    let bad = host
        .dispatch_webview(
            "main",
            command(
                "state.ack",
                &window,
                json!({"subscription_id":sid,"upto_seq":batches[0].through_seq+1}),
            ),
        )
        .unwrap_err();
    assert_eq!(bad.code, ErrorCode::Conflict);
    host.unbind_window("main").unwrap();
    assert!(host.take_event_batches("main").is_err());
}
struct Slots {
    calls: Mutex<Vec<String>>,
    available: bool,
}
impl tasks::SecureSlotStore for Slots {
    fn contains(&self, slot: &str) -> storage::Result<bool> {
        self.calls.lock().unwrap().push(slot.into());
        Ok(self.available)
    }
}
#[test]
fn dedicated_credential_bindings_fail_before_any_other_slot_lookup() {
    let slots = Arc::new(Slots {
        calls: Mutex::new(Vec::new()),
        available: false,
    });
    let resolver = tasks::NativeCredentialResolver::new(slots.clone());
    for model in [
        "qwen-audio-3.1-asr-flash-streaming",
        "qwen-audio-3.1-asr-flash-filetrans",
        "decision-model-preview",
        "qwen3.8-flash",
    ] {
        let model: ModelId = serde_json::from_value(json!(model)).unwrap();
        let binding = tasks::binding_for(&model).unwrap();
        assert_eq!(
            resolver.require_binding(&binding).unwrap_err().code,
            ErrorCode::AuthRequired
        );
    }
    assert_eq!(slots.calls.lock().unwrap().len(), 4);
    let mut wrong = tasks::binding_for(
        &serde_json::from_value(json!("qwen-audio-3.1-asr-flash-filetrans")).unwrap(),
    )
    .unwrap();
    wrong.credential_ref = serde_json::from_value(json!("bailian_beijing_asr_streaming")).unwrap();
    assert_eq!(
        resolver.require_binding(&wrong).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(slots.calls.lock().unwrap().len(), 4);
}
#[test]
fn independent_math_golden_and_rectangular_multichannel_case() {
    let gold: Value =
        serde_json::from_str(include_str!("../../fixtures/contracts/math-golden.json")).unwrap();
    for case in gold.as_array().unwrap() {
        let template = if case["type"] == "CurveParameters" {
            "function_curve_2d"
        } else {
            "conv2d_spatial"
        };
        let r = visualizations::evaluate_template(
            template,
            &VisualizationEvaluateInput {
                object_id: storage::id(),
                revision_id: storage::id(),
                parameters: case["parameters"].clone(),
                interaction_seq: 1,
                algorithm_version: "1.0.0".into(),
            },
        )
        .unwrap();
        assert_eq!(json!(r.shape), case["shape"], "{}", case["name"]);
        for (actual, expected) in r.values.iter().zip(case["values"].as_array().unwrap()) {
            assert!((actual - expected.as_f64().unwrap()).abs() < 1e-9);
        }
    }
    let mut input = VisualizationEvaluateInput {
        object_id: storage::id(),
        revision_id: storage::id(),
        parameters: json!({"operator_dimension":2,"scene_dimension":3,"operator_semantics":"cross_correlation","axis_order":"CHW","input":[[[1,2,3],[4,5,6]],[[10,20,30],[40,50,60]]],"kernel":[[[1,2]],[[1,-1]]],"stride":[1,1],"padding":[0,0,0,0],"dilation":[1,1],"bias":3}),
        interaction_seq: 1,
        algorithm_version: "1.0.0".into(),
    };
    let r = visualizations::evaluate_template("conv2d_spatial", &input).unwrap();
    assert_eq!(r.shape, [2, 2]);
    assert_eq!(r.values, [-2., 1., 7., 10.]);
    input.parameters["stride"] = json!([0, 1]);
    assert!(visualizations::evaluate_template("conv2d_spatial", &input).is_err());
}

#[test]
fn visualization_evaluate_uses_persisted_row_revision_and_bounded_determinism() {
    let db = TestLibrary::new();
    let lesson_id = lesson(&db.store);
    let note_id = note(&db.store, &lesson_id, "visualization owner");
    let object_id = storage::id();
    let revision_id = storage::id();
    let parameters = json!({
        "operator_dimension":2,
        "scene_dimension":3,
        "operator_semantics":"cross_correlation",
        "axis_order":"CHW",
        "input":[[[1,2],[3,4]]],
        "kernel":[[[1]]],
        "stride":[1,1],
        "padding":[0,0,0,0],
        "dilation":[1,1],
        "bias":0
    });
    let content: VisualizationContent = serde_json::from_value(json!({
        "object_id":object_id,
        "revision_id":revision_id,
        "parent_revision_ids":[],
        "protocol":"focaus.visualization",
        "protocol_version":"1.0.0",
        "template_id":"conv2d_spatial",
        "template_version":"1.0.0",
        "registry_revision":"1.0.0",
        "concept_fact_ids":[],
        "definition_ids":[],
        "symbol_binding_ids":[],
        "scope_id":storage::id(),
        "semantic_key":"native-test",
        "purpose":"test",
        "placement_anchor":{"kind":"whole"},
        "parameters":parameters,
        "parameter_schema_version":"1.0.0",
        "value_bindings":[],
        "computation":{
            "algorithm_id":"conv2d_v1",
            "algorithm_version":"1.0.0",
            "operator_semantics":"cross_correlation",
            "formula_id":"",
            "input_shape":[1,2,2],
            "output_shape":[2,2],
            "precision_policy_id":"f64_abs_1e-9_rel_1e-9",
            "dependency_binding_ids":[]
        },
        "resources":[],
        "source_refs":[],
        "snapshot_id":storage::id(),
        "fact_set_revision":storage::id(),
        "interaction_state":{
            "selected_input_ids":[],
            "selected_output_ids":[],
            "step_index":0,
            "camera":null,
            "user_parameter_overrides":[]
        },
        "presentation_by_tier":[],
        "generation_status":"ready",
        "validation":{
            "checks":{"schema":"pass","source":"unknown","semantic":"pass","math":"pass"},
            "validator_version":"1.0.0",
            "parameters_hash":canonical_hash(&parameters),
            "validated_revision":revision_id,
            "diagnostics":[]
        },
        "fallback":{"preview_resource_id":null,"description":"native test","table":[]}
    })).unwrap();
    db.store.write(|t| {
        let committed = storage::commit_version(
            t,
            &object_id,
            "visualization",
            &revision_id,
            storage::value(&content)?,
            "visualization.changed",
            json!({"kind":"lesson","lesson_id":lesson_id}),
            None,
            None,
        )?;
        t.execute(
            "INSERT INTO visualizations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                object_id,
                note_id,
                revision_id,
                "conv2d_spatial",
                "1.0.0",
                "1.0.0",
                "native-test",
                0,
                canonical_hash(&parameters),
                storage::encode(&committed)?,
                "ready",
                storage::encode(&committed["validation"])?
            ],
        ).map_err(storage::sql_error)?;
        Ok(())
    }).unwrap();
    let visualizations = visualizations::Visualizations::new(db.store.clone());
    let evaluated = visualizations.evaluate(VisualizationEvaluateInput {
        object_id: object_id.clone(),
        revision_id: revision_id.clone(),
        parameters: parameters.clone(),
        interaction_seq: 1,
        algorithm_version: "1.0.0".into(),
    }).unwrap();
    assert_eq!(evaluated.shape, [2, 2]);
    assert_eq!(evaluated.values, [1., 2., 3., 4.]);

    assert_eq!(
        visualizations.evaluate(VisualizationEvaluateInput {
            object_id: object_id.clone(),
            revision_id: storage::id(),
            parameters: parameters.clone(),
            interaction_seq: 2,
            algorithm_version: "1.0.0".into(),
        }).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(
        visualizations.evaluate(VisualizationEvaluateInput {
            object_id: object_id.clone(),
            revision_id: revision_id.clone(),
            parameters: parameters.clone(),
            interaction_seq: 0,
            algorithm_version: "1.0.0".into(),
        }).unwrap_err().code,
        ErrorCode::StaleInput
    );
    let mut invalid = parameters.clone();
    invalid["stride"] = json!([0, 1]);
    let invalid_parameters = visualizations.evaluate(VisualizationEvaluateInput {
            object_id: object_id.clone(),
            revision_id: revision_id.clone(),
            parameters: invalid,
            interaction_seq: 3,
            algorithm_version: "1.0.0".into(),
        }).unwrap_err();
    assert!(matches!(invalid_parameters.code, ErrorCode::InvalidArgument | ErrorCode::InvalidResult));
    let big = vec![vec![vec![0.0; 257]; 257]];
    assert_eq!(
        visualizations.evaluate(VisualizationEvaluateInput {
            object_id: object_id.clone(),
            revision_id: revision_id.clone(),
            parameters: json!({
                "operator_dimension":2,
                "scene_dimension":3,
                "operator_semantics":"cross_correlation",
                "axis_order":"CHW",
                "input":big,
                "kernel":[[[1]]],
                "stride":[1,1],
                "padding":[0,0,0,0],
                "dilation":[1,1],
                "bias":0
            }),
            interaction_seq: 3,
            algorithm_version: "1.0.0".into(),
        }).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let mut wrong_template = content;
    wrong_template.template_id = "unregistered_template".into();
    db.store.write(|t| {
        t.execute(
            "UPDATE visualizations SET description_json=?2 WHERE object_id=?1",
            params![object_id, storage::encode(&wrong_template)?],
        ).map_err(storage::sql_error)?;
        Ok(())
    }).unwrap();
    assert_eq!(
        visualizations.evaluate(VisualizationEvaluateInput {
            object_id: object_id,
            revision_id: revision_id,
            parameters,
            interaction_seq: 4,
            algorithm_version: "1.0.0".into(),
        }).unwrap_err().code,
        ErrorCode::UnsupportedCapability
    );
}

#[test]
fn task_resource_reservation_cancellation_and_restart_keep_checkpoint() {
    let db = TestLibrary::new();
    let target = storage::id();
    let snapshot = InputSnapshot {
        snapshot_id: storage::id(),
        parent_snapshot_id: None,
        contract_version: CONTRACT_VERSION.into(),
        purpose: TaskKind::MaterialConvert,
        target_id: target.clone(),
        lesson_id: None,
        recording_id: None,
        assignment_epoch: None,
        sources: Vec::new(),
        attachment_set_revision: storage::id(),
        fact_set_revision: None,
        target_base_revision: None,
        edit_revision: None,
        language_tier: None,
        evidence_cutoff: Vec::new(),
        available_at: storage::now(),
        executor_id: "native-test".into(),
        executor_version: "1.0.0".into(),
        model_id: None,
        region: None,
        prompt_version: None,
        parameters_hash: canonical_hash(&Value::Null),
        calibration_seq: None,
        decision_seq: None,
    };
    db.store
        .write(|t| {
            t.execute(
                "INSERT INTO input_snapshots VALUES(?1,NULL,?2,?3,?4,?5)",
                params![
                    snapshot.snapshot_id,
                    CONTRACT_VERSION,
                    storage::encode(&snapshot)?,
                    canonical_hash(&json!(snapshot)),
                    snapshot.available_at
                ],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let manager = tasks::TaskManager::new(db.store.clone());
    let tid = storage::id();
    manager
        .enqueue(Task {
            task_id: tid.clone(),
            kind: TaskKind::MaterialConvert,
            target_id: target,
            snapshot_id: snapshot.snapshot_id,
            state: TaskState::Queued,
            priority: 4,
            attempt: 1,
            deadline_at: None,
            idempotency_key: storage::id(),
            checkpoint: BTreeMap::new(),
            error: None,
            provider_bindings: Vec::new(),
            created_at: storage::now(),
            updated_at: storage::now(),
            model_id: None,
            region: None,
            prompt_version: None,
            parameters_hash: snapshot.parameters_hash,
        })
        .unwrap();
    assert!(manager.claim(1, 1).unwrap().is_none());
    assert_eq!(manager.claim(2, 1).unwrap().unwrap().task_id, tid);
    manager
        .checkpoint(TaskCheckpointInput {
            task_id: tid.clone(),
            attempt: 1,
            cursor: "page-2".into(),
            safe_state: BTreeMap::new(),
        })
        .unwrap();
    let recovered = manager.recover().unwrap();
    assert_eq!(recovered[0].state, TaskState::Interrupted);
    assert_eq!(recovered[0].checkpoint["cursor"], "page-2");
    manager
        .transition(&tid, 1, TaskState::CancelRequested)
        .unwrap();
    assert!(manager
        .checkpoint(TaskCheckpointInput {
            task_id: tid.clone(),
            attempt: 1,
            cursor: "late".into(),
            safe_state: BTreeMap::new()
        })
        .is_err());
    manager.transition(&tid, 1, TaskState::Cancelled).unwrap();
    assert!(manager.transition(&tid, 1, TaskState::Queued).is_err());
    let watermark = manager.get(&tid).unwrap().snapshot_id;
    let projected = db
        .store
        .read(|c| {
            library::get_content(
                c,
                &ContentGetInput {
                    entity_id: tid.clone(),
                    revision_id: Some(watermark),
                    selector: None,
                },
            )
        })
        .unwrap();
    validate("Task", &projected).unwrap();
    assert_eq!(projected["state"], "cancelled");
}

#[test]
fn unicode_fifteen_casefold_nfc_whitespace_and_scalar_mapping() {
    let golden: Value =
        serde_json::from_str(include_str!("../../fixtures/contracts/search-golden.json")).unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let (text, map) = search::normalize(case["text"].as_str().unwrap(), false).unwrap();
        assert_eq!(text, case["normalized"].as_str().unwrap());
        let (query, _) = search::normalize(case["query"].as_str().unwrap(), false).unwrap();
        let hits = search::match_ranges(&text, &map, &query);
        assert_eq!(json!([hits[0].start, hits[0].end]), case["original_range"]);
    }
    let (hangul, map) = search::normalize("\u{1100}\u{1161}", false).unwrap();
    assert_eq!(hangul, "가");
    assert_eq!(
        (map[0].original_range.start, map[0].original_range.end),
        (0, 2)
    );
    let (folded, map) = search::normalize("ﬃ", false).unwrap();
    assert_eq!(folded, "ffi");
    assert_eq!(search::match_ranges(&folded, &map, "f").len(), 1);
    let (reordered, _) = search::normalize("a\u{301}\u{323}", false).unwrap();
    assert_eq!(reordered, "ạ\u{301}");
}
#[test]
fn full_document_search_pagination_heads_and_deletion_barrier() {
    let db = TestLibrary::new();
    let lesson = lesson(&db.store);
    let owner = note(&db.store, &lesson, &"学习 Straße 😀 ".repeat(60));
    let window = storage::id();
    let view = view(&db.store, &lesson, &owner, &window);
    let search = search::SearchCoordinator::new(db.store.clone());
    search
        .project_native(&owner, SearchUnitKind::Note, Some(3))
        .unwrap();
    let session = search
        .open(SearchOpenInput {
            view_id: view.view_id.clone(),
            target: view.target.clone(),
            version_mode: SearchOpenInputVersionMode::Current,
            actual_visible_tier: Some(3),
            revision_ids: Vec::new(),
        })
        .unwrap();
    validate("SearchSession", &json!(session)).unwrap();
    let task = search
        .query_task(SearchQueryInput {
            search_id: session.search_id.clone(),
            query_seq: 1,
            query: "学习".into(),
            case_sensitive: false,
        })
        .unwrap();
    assert_eq!(task.state, TaskState::Succeeded);
    let first = search
        .page(SearchPageInput {
            search_id: session.search_id.clone(),
            query_seq: 1,
            scope_snapshot_id: session.scope_snapshot_id.clone(),
            cursor: None,
        })
        .unwrap();
    validate("SearchPage", &json!(first)).unwrap();
    assert_eq!(first.total_count, 60);
    assert_eq!(first.hits.len(), 50);
    let second = search
        .page(SearchPageInput {
            search_id: session.search_id.clone(),
            query_seq: 1,
            scope_snapshot_id: session.scope_snapshot_id.clone(),
            cursor: first.next_cursor.clone(),
        })
        .unwrap();
    assert_eq!(second.hits.len(), 10);
    let input = SearchLocateInput {
        search_id: session.search_id.clone(),
        view_id: view.view_id,
        hit_id: first.hits[0].hit_id.clone(),
        expected_revision: first.hits[0].revision_id.clone(),
    };
    assert_eq!(search.locate(input.clone()).unwrap().scalar_range.start, 0);
    db.store
        .write(|t| {
            t.execute(
                "UPDATE entities SET lifecycle='deleted' WHERE entity_id=?1",
                [&owner],
            )
            .map_err(storage::sql_error)?;
            t.execute(
                "UPDATE revisions SET body_json=NULL WHERE entity_id=?1",
                [&owner],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        search.locate(input).unwrap_err().code,
        ErrorCode::SearchInvalidated | ErrorCode::SourceDeleted
    ));
    assert!(search
        .page(SearchPageInput {
            search_id: session.search_id,
            query_seq: 1,
            scope_snapshot_id: session.scope_snapshot_id,
            cursor: None
        })
        .is_err());
}
#[test]
fn view_handoff_rolls_back_or_atomically_changes_single_owner() {
    let db = TestLibrary::new();
    let lesson = lesson(&db.store);
    let owner = note(&db.store, &lesson, "草稿");
    let source = storage::id();
    let target = storage::id();
    let view = view(&db.store, &lesson, &owner, &source);
    let transfer: ViewTransfer = serde_json::from_value(
        db.store
            .write(|t| {
                windows::prepare(
                    t,
                    &ViewMoveInput {
                        view_id: view.view_id.clone(),
                        target_window_id: Some(target.clone()),
                        base_view_revision: view.view_revision.clone(),
                    },
                    &source,
                    None,
                )
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        db.store
            .read(|c| windows::get(c, &view.view_id))
            .unwrap()
            .host_window_id,
        source
    );
    assert_eq!(
        windows::WindowCoordinator::new(db.store.clone())
            .recover()
            .unwrap(),
        1
    );
    assert_eq!(
        db.store
            .read(|c| windows::get(c, &view.view_id))
            .unwrap()
            .state,
        ContentViewState::Open
    );
    assert!(db
        .store
        .write(|t| windows::ready(
            t,
            &ViewReadyInput {
                transfer_id: transfer.transfer_id,
                view_revision: view.view_revision.clone()
            },
            &target,
            None
        ))
        .is_err());
    let prepared: ViewTransfer = serde_json::from_value(
        db.store
            .write(|t| {
                windows::prepare(
                    t,
                    &ViewMoveInput {
                        view_id: view.view_id.clone(),
                        target_window_id: Some(target.clone()),
                        base_view_revision: view.view_revision.clone(),
                    },
                    &source,
                    None,
                )
            })
            .unwrap(),
    )
    .unwrap();
    let moved = db
        .store
        .write(|t| {
            windows::ready(
                t,
                &ViewReadyInput {
                    transfer_id: prepared.transfer_id,
                    view_revision: view.view_revision,
                },
                &target,
                None,
            )
        })
        .unwrap();
    validate("ViewTransfer", &moved).unwrap();
    assert_eq!(moved["target_window_id"], target);
    assert_eq!(moved["state"], "committed");
    assert!(db
        .store
        .read(|c| windows::authorize(c, &view.view_id, &source))
        .is_err());
    let current = db.store.read(|c| windows::get(c, &view.view_id)).unwrap();
    let host = ipc::NativeHost::open(&db.path, INITIAL_MIGRATION).unwrap();
    host.bind_window(
        "target",
        &target,
        vec![serde_json::from_value(json!({"kind":"global"})).unwrap()],
    )
    .unwrap();
    let snapshot = host
        .dispatch_webview(
            "target",
            command(
                "state.open",
                &target,
                json!({"scopes":[{"kind":"global"}],"last_seq":null,"capabilities":capabilities()}),
            ),
        )
        .unwrap();
    assert_eq!(
        snapshot.result.unwrap()["snapshot"]["records"]["view_states"][&view.view_id]
            ["host_window_id"],
        target
    );
    let content=host.dispatch_webview("target",command("content.get",&target,json!({"entity_id":view.view_id,"revision_id":current.view_revision,"selector":null}))).unwrap();
    validate("ContentView", &content.result.unwrap()).unwrap();
}

#[test]
fn derived_context_snapshot_inherits_cutoff_and_rejects_new_evidence() {
    let db = TestLibrary::new();
    let lesson = lesson(&db.store);
    let owner = note(&db.store, &lesson, "定义来源");
    fn definition(store: &storage::Store, lesson: &str, owner: &str) -> String {
        let id = storage::id();
        store.write(|t|{let (revision,hash):(String,String)=t.query_row("SELECT r.revision_id,r.content_hash FROM revisions r JOIN entities e ON e.revision_id=r.revision_id WHERE e.entity_id=?1",[owner],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage::sql_error)?;let source=SourceRef {entity_id:owner.into(),revision_id:revision,content_hash:hash,selector:Selector::V4(WholeSelector {kind:"whole".into()})};let def=Definition {definition_id:id.clone(),concept_id:storage::id(),lesson_id:lesson.into(),scope_id:storage::id(),valid_from_anchor:source.clone(),valid_to_anchor:None,source_refs:vec![source],definition_fact_ids:Vec::new()};storage::commit_entity(t,&id,"definition",json!(def),"content.committed",json!({"kind":"lesson","lesson_id":lesson}),None,None)?;t.execute("INSERT INTO definitions VALUES(?1,?2,?3,?4,?5)",params![id,def.concept_id,lesson,def.scope_id,storage::encode(&def)?]).map_err(storage::sql_error)?;Ok(())}).unwrap();
        id
    }
    let before = definition(&db.store, &lesson, &owner);
    let snapshots = library::snapshots::SnapshotStore::new(db.store.clone());
    let snapshot = snapshots
        .build(SnapshotBuildInput {
            purpose: TaskKind::Catchup,
            target_id: lesson.clone(),
            evidence_cutoff: Vec::new(),
            language_tier: Some(3),
        })
        .unwrap();
    let future_owner = note(&db.store, &lesson, "稍后才出现的资料");
    let after = definition(&db.store, &lesson, &future_owner);
    let course = db.store.head(&lesson).unwrap()["course_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = snapshots
        .context(ContextRetrieveInput {
            snapshot: snapshot.clone(),
            definition_ids: vec![before, after.clone()],
            binding_ids: Vec::new(),
            course_id: course.clone(),
            remaining_budget_ms: 1000,
        })
        .unwrap();
    assert_eq!(result.definitions.len(), 1);
    assert_eq!(
        result.snapshot.parent_snapshot_id,
        Some(snapshot.snapshot_id)
    );
    assert_eq!(result.snapshot.evidence_cutoff, snapshot.evidence_cutoff);
    let nested = snapshots
        .context(ContextRetrieveInput {
            snapshot: result.snapshot,
            definition_ids: vec![after],
            binding_ids: Vec::new(),
            course_id: course,
            remaining_budget_ms: 1000,
        })
        .unwrap();
    assert!(nested.definitions.is_empty());
}

#[test]
fn focaus_generations_body_revision_and_ui_deadline_are_independent() {
    let db = TestLibrary::new();
    let lid = lesson(&db.store);
    let nid = note(&db.store, &lid, "focaus fixture");
    let win = storage::id();
    let view = view(&db.store, &lid, &nid, &win);
    let recording = db
        .store
        .write(|t| {
            capture::RecordingCoordinator::reserve(
                t,
                &RecordingStartInput {
                    lesson_id: Some(lid.clone()),
                    device_id: None,
                },
                None,
            )
        })
        .unwrap();
    let rid = recording["recording_id"].as_str().unwrap().to_owned();
    let mut writer = audio::SegmentWriter::create(
        &db.path,
        audio::SegmentMeta::new(&rid, 48000, 1, 0, 0).unwrap(),
    )
    .unwrap();
    writer.append(&[0.25; 4800]).unwrap();
    let span = writer.seal(&db.store, false).unwrap();
    let raw = storage::id();
    db.store.write(|t|{let body=storage::commit_entity(t,&raw,"raw_utterance",json!({"utterance_id":raw,"recording_id":rid,"audio_spans":[span["audio_span_id"]],"ordinal":0,"speaker_ref":null,"state":"final","text":"先定义斜率，再代入直线方程。","revision_id":storage::id(),"entity_seq":0,"remote_binding":{"provider":"fixture","provider_task_id":"local-fixture","connection_epoch":1,"provider_sentence_id":"1","local_request_segment_id":storage::id()}}),"transcript.raw_changed",json!({"kind":"lesson","lesson_id":lid}),None,None)?;t.execute("INSERT INTO raw_utterances VALUES(?1,?2,0,'final',?3,?4,NULL,?5,?6)",params![raw,rid,body["text"].as_str(),body["revision_id"].as_str(),storage::encode(&body["remote_binding"])?,storage::encode(&body["audio_spans"])?]).map_err(storage::sql_error)?;Ok(())}).unwrap();
    let focaus = library::catchup::FocausCoordinator::new(db.store.clone());
    let trigger = CatchupRequestInput {
        view_id: view.view_id,
        lesson_id: lid.clone(),
        recording_id: Some(rid.clone()),
        assignment_epoch: Some(0),
        request_generation: 1,
        triggered_at: storage::now(),
        remaining_deadline_ms: 15000,
        requested_window: CatchupWindow {
            recording_id: Some(rid.clone()),
            start_ms: Some(0),
            end_ms: Some(100),
            start_source: None,
        },
        estimate_basis: "explicit".into(),
        actual_tier: 3,
    };
    let task = focaus.begin(trigger.clone()).unwrap();
    let same = focaus.begin(trigger.clone()).unwrap();
    assert_eq!(task.task_id, same.task_id);
    tasks::TaskManager::new(db.store.clone())
        .transition(&task.task_id, 1, TaskState::Running)
        .unwrap();
    let snapshot = library::snapshots::SnapshotStore::new(db.store.clone())
        .get(&task.snapshot_id)
        .unwrap();
    assert!(!snapshot.sources.is_empty());
    let result:CatchupResult=serde_json::from_value(json!({"result_semantics":"focaus_context_v2","result_id":task.checkpoint["result_id"],"request_generation":1,"snapshot_id":snapshot.snapshot_id,"body_revision_id":storage::id(),"requested_window":trigger.requested_window,"effective_window":trigger.requested_window,"estimate_basis":"explicit","current_focus":{"topic":"直线","problem":"代入方程","activity":"derivation","step":"计算","source_refs":[snapshot.sources[0]],"status":"supported"},"path_nodes":[{"node_id":"n1","role":"bridge","fact_ids":[],"definition_ids":[],"binding_ids":[],"body_block_ids":[nid],"evidence_role":"teacher_path","source_refs":[snapshot.sources[0]]}],"path_edges":[],"prerequisites":[],"prerequisites_reason":"none_needed","body":[{"block_id":nid,"text":"已定义斜率，接下来代入方程。"}],"presentation_tier":3,"body_evidence_cutoff":snapshot.evidence_cutoff,"body_generated_at":storage::now(),"progress_at_trigger":null,"progress_unavailable_reason":"not_available","progress_binding":{"lesson_id":lid,"recording_id":rid,"assignment_epoch":0,"no_pointer_reason":"not_available"},"coverage":{"audio_gaps":[],"asr_gaps":[],"material_supplement_used":false,"range_status":"complete"},"limitations":[],"completion_status":"complete","triggered_at":trigger.triggered_at,"completed_at":storage::now(),"displayed_complete_elapsed_ms":null})).unwrap();
    let submit = ResultSubmitInput {
        task_id: task.task_id.clone(),
        attempt: 1,
        snapshot_id: snapshot.snapshot_id,
        candidate: json!(result),
        source_refs: result.current_focus.source_refs.clone(),
        resources: Vec::new(),
    };
    let receipt = focaus.submit(&submit, result.clone()).unwrap();
    validate("CommandResult", &json!(receipt)).unwrap();
    let revision = receipt.result.unwrap()["body_revision_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = db.store.head(&result.result_id).unwrap();
    let ack = focaus
        .display_ack(CatchupDisplayAckInput {
            result_id: result.result_id.clone(),
            request_generation: 1,
            body_revision_id: revision,
            elapsed_ms: 15001,
            displayed_complete: true,
        })
        .unwrap();
    assert_eq!(ack.result.unwrap()["met_deadline"], false);
    assert_eq!(db.store.head(&result.result_id).unwrap(), before);
    let mut next = trigger;
    next.request_generation = 2;
    focaus.begin(next).unwrap();
    assert_eq!(
        focaus.submit(&submit, result).unwrap_err().code,
        ErrorCode::StaleInput
    );
}

#[test]
fn managed_resource_capability_rejects_links_escape_and_overwrite() {
    use std::io::Read;
    let db = TestLibrary::new();
    let stage = db.path.join("stage");
    std::fs::create_dir(&stage).unwrap();
    std::fs::write(stage.join("a.txt"), "学习").unwrap();
    std::fs::write(db.path.join("private.txt"), "private fixture").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(db.path.join("private.txt"), stage.join("escape.txt")).unwrap();
    assert!(
        library::resources::open_relative(&stage, std::path::Path::new("../private.txt")).is_err()
    );
    assert!(library::resources::open_relative(&stage, std::path::Path::new("escape.txt")).is_err());
    let source = library::resources::open_relative(&stage, std::path::Path::new("a.txt")).unwrap();
    let (hash, length) =
        library::resources::persist_bytes(source, &db.path, "resources/fixture.txt", 100).unwrap();
    assert_eq!(hash, search_text_hash("学习"));
    assert_eq!(length, 6);
    let source = library::resources::open_relative(&stage, std::path::Path::new("a.txt")).unwrap();
    assert!(
        library::resources::persist_bytes(source, &db.path, "resources/fixture.txt", 100).is_err()
    );
    let mut content = String::new();
    std::fs::File::open(db.path.join("resources/fixture.txt"))
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert_eq!(content, "学习");
}

#[test]
fn stable_window_binding_and_initial_standalone_failure_preserve_workspace() {
    let db = TestLibrary::new();
    let lesson = lesson(&db.store);
    let owner = note(&db.store, &lesson, "恢复窗口");
    let original = storage::id();
    let grid = view(&db.store, &lesson, &owner, &original);
    let windows = windows::WindowCoordinator::new(db.store.clone());
    assert_eq!(
        windows.persistent_window_id("main", None).unwrap(),
        original
    );
    let reopened = storage::Store::open(&db.path, INITIAL_MIGRATION).unwrap();
    assert_eq!(
        windows::WindowCoordinator::new(reopened)
            .persistent_window_id("main", None)
            .unwrap(),
        original
    );
    let standalone: ContentView = serde_json::from_value(
        db.store
            .write(|t| {
                windows::open(
                    t,
                    &ViewOpenInput {
                        lesson_id: lesson.clone(),
                        target: grid.target.clone(),
                        placement: ViewOpenInputPlacement::Standalone,
                    },
                    &original,
                    None,
                )
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(windows.restore_plan().unwrap().len(), 2);
    windows
        .initial_host_failed(&standalone.view_id, &standalone.host_window_id)
        .unwrap();
    assert_eq!(windows.restore_plan().unwrap().len(), 1);
    assert_eq!(
        db.store
            .read(|c| windows::get(c, &grid.view_id))
            .unwrap()
            .state,
        ContentViewState::Open
    );
}

#[test]
fn persisted_capture_watermark_is_typed_and_stop_rebases_only_watermark_growth() {
    let db = TestLibrary::new();
    let win = storage::id();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap();
    let mut stop = command(
        "recording.stop",
        &win,
        json!({"recording_id":rid,"expected_state":"starting"}),
    );
    stop.expected_revisions = Some(BTreeMap::from([(
        rid.into(),
        recording["revision_id"].as_str().unwrap().into(),
    )]));
    let mut writer = audio::SegmentWriter::create(
        &db.path,
        audio::SegmentMeta::new(rid, 48000, 1, 0, 0).unwrap(),
    )
    .unwrap();
    writer.append(&[0.; 4800]).unwrap();
    writer.seal(&db.store, false).unwrap();
    assert_eq!(db.store.head(rid).unwrap()["durable_watermark_ms"], 100);
    assert!(db.store.preflight(&win, &stop).unwrap().is_none());
    let (_, events) = db.store.read(|c| storage::events(c, 0, 100)).unwrap();
    let coverage = events
        .iter()
        .find(|e| e.event_type == "recording.coverage_changed")
        .unwrap();
    assert_eq!(coverage.payload["recording_id"], rid);
    assert_ne!(coverage.entity_id, rid);
    let watermark = events
        .iter()
        .find(|e| {
            e.event_type == "recording.state_changed"
                && e.payload.get("change_kind") == Some(&json!("durable_watermark"))
        })
        .unwrap();
    assert_eq!(watermark.entity_id, rid);
    validate("RecordingSession", &db.store.head(rid).unwrap()).unwrap();
    let receipt = db
        .store
        .command(&win, &stop, |t| {
            capture::state_transaction(t, rid, "stopped", Some(&stop.command_id))
        })
        .unwrap();
    assert_eq!(receipt.result.unwrap()["state"], "stopped");
    let mut newer = stop;
    newer.idempotency_key = Some(storage::id());
    assert_eq!(
        db.store.preflight(&win, &newer).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[test]
fn old_stop_request_cannot_cross_pause_or_assignment_epoch() {
    for changed in ["paused", "assignment"] {
        let db = TestLibrary::new();
        let win = storage::id();
        let recording = reserve(&db.store);
        let rid = recording["recording_id"].as_str().unwrap();
        let mut stop = command(
            "recording.stop",
            &win,
            json!({"recording_id":rid,"expected_state":"starting"}),
        );
        stop.expected_revisions = Some(BTreeMap::from([(
            rid.into(),
            recording["revision_id"].as_str().unwrap().into(),
        )]));
        if changed == "paused" {
            capture::set_state(&db.store, rid, "paused").unwrap();
        } else {
            db.store.write(|t|{t.execute("UPDATE recording_sessions SET assignment_epoch=assignment_epoch+1 WHERE recording_id=?1",[rid]).map_err(storage::sql_error)?;let mut head=storage::head(t,rid)?;head["assignment_epoch"]=json!(1);storage::commit_entity(t,rid,"recording",head,"recording.state_changed",json!({"kind":"global"}),None,None)?;Ok(())}).unwrap();
        }
        assert_eq!(
            db.store.preflight(&win, &stop).unwrap_err().code,
            ErrorCode::Conflict
        );
    }
}

#[test]
#[ignore = "requires an integrated material_engine module; set FOCAUS_TEST_MATERIAL_ENGINE"]
fn material_sidecar_roundtrip_uses_managed_original_and_host_search() {
    use std::{
        io::Write,
        process::{Command as Process, Stdio},
    };
    let engine = std::env::var_os("FOCAUS_TEST_MATERIAL_ENGINE")
        .expect("explicit integrated engine path required");
    let db = TestLibrary::new();
    let host = ipc::NativeHost::open(&db.path, INITIAL_MIGRATION).unwrap();
    let window = storage::id();
    host.bind_window(
        "main",
        &window,
        vec![serde_json::from_value(json!({"kind":"global"})).unwrap()],
    )
    .unwrap();
    let created = host
        .dispatch_webview(
            "main",
            command("course.create", &window, json!({"name":"合成资料纵向验证"})),
        )
        .unwrap();
    let course = created.result.unwrap()["course_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created=host.dispatch_webview("main",command("lesson.create",&window,json!({"course_id":course,"title":"Material native integration","actual_date":null,"timezone":"Asia/Shanghai"}))).unwrap();
    let lesson = created.result.unwrap()["lesson_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let source = db.path.join("original.txt");
    let text = "未挂载课件 Straße 学习 😀";
    std::fs::write(&source, text).unwrap();
    let mut job = host
        .import_material_original(
            &source,
            &lesson,
            MaterialImportInputPurpose::Textbook,
            vec![0],
        )
        .unwrap();
    job.input.budget_ms = 3000;
    std::fs::remove_file(&source).unwrap();
    assert!(std::path::Path::new(&job.input.source_handle).is_file());
    host.tasks
        .transition(&job.task.task_id, 1, TaskState::Running)
        .unwrap();
    let request = job.engine_request().unwrap();
    validate("EngineRequest", &json!(request)).unwrap();
    let mut child = Process::new("python3")
        .args(["-m", "material_engine"])
        .env("PYTHONPATH", engine)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "engine process failed");
    let frames = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(
        !frames.iter().any(|frame| frame["kind"] == "error"),
        "{frames:?}"
    );
    let completed = frames.iter().find(|f| f["kind"] == "completed").unwrap();
    let manifest: MaterialManifest =
        serde_json::from_value(completed["payload"]["manifest"].clone()).unwrap();
    validate("MaterialManifest", &json!(manifest)).unwrap();
    let mut wrong = manifest.clone();
    wrong.source_hash = "0".repeat(64);
    assert!(host
        .accept_material_result(MaterialPageResultInput {
            task_id: job.task.task_id.clone(),
            snapshot_id: job.task.snapshot_id.clone(),
            manifest: wrong
        })
        .is_err());
    assert!(host.store.head(&job.material_id).unwrap()["parse_version_id"].is_null());
    let preview = manifest
        .resources
        .iter()
        .find(|r| r.media_type == "image/svg+xml")
        .unwrap();
    let preview_path = std::path::Path::new(&job.input.staging_handle).join(&preview.relative_path);
    let safe_bytes = std::fs::read(&preview_path).unwrap();
    let unsafe_svg = "<svg><script>bad()</script></svg>";
    std::fs::write(&preview_path, unsafe_svg).unwrap();
    let mut unsafe_manifest = manifest.clone();
    let unsafe_preview = unsafe_manifest
        .resources
        .iter_mut()
        .find(|r| r.resource_id == preview.resource_id)
        .unwrap();
    unsafe_preview.sha256 = search_text_hash(unsafe_svg);
    unsafe_preview.byte_length = unsafe_svg.len() as u64;
    assert_eq!(
        host.accept_material_result(MaterialPageResultInput {
            task_id: job.task.task_id.clone(),
            snapshot_id: job.task.snapshot_id.clone(),
            manifest: unsafe_manifest
        })
        .unwrap_err()
        .code,
        ErrorCode::InvalidResult
    );
    std::fs::write(&preview_path, safe_bytes).unwrap();
    let input = MaterialPageResultInput {
        task_id: job.task.task_id.clone(),
        snapshot_id: job.task.snapshot_id.clone(),
        manifest: manifest.clone(),
    };
    let accepted = host.accept_material_result(input.clone()).unwrap();
    assert_eq!(accepted["publication"], "committed");
    let content=host.dispatch_webview("main",command("content.get",&window,json!({"entity_id":job.material_id,"revision_id":job.parse_version_id,"selector":null}))).unwrap();
    validate("MaterialManifest", &content.result.unwrap()).unwrap();
    let bytes = host
        .read_resource(ResourceReadInput {
            resource_id: preview.resource_id.clone(),
            offset: 0,
            length: 100,
        })
        .unwrap();
    assert!(!bytes["bytes"].as_array().unwrap().is_empty());
    assert_eq!(
        host.accept_material_result(input).unwrap()["duplicate"],
        true
    );
    let opened=host.dispatch_webview("main",command("view.open",&window,json!({"lesson_id":lesson,"target":{"kind":"entity","entity_id":job.material_id,"revision_id":null,"selector":null},"placement":"grid"}))).unwrap();
    let view: ContentView = serde_json::from_value(opened.result.unwrap()).unwrap();
    let search=host.dispatch_webview("main",command("search.open",&window,json!({"view_id":view.view_id,"target":view.target,"version_mode":"current","actual_visible_tier":null,"revision_ids":[]}))).unwrap();
    let session: SearchSession = serde_json::from_value(search.result.unwrap()).unwrap();
    host.dispatch_webview("main",command("search.query",&window,json!({"search_id":session.search_id,"query_seq":1,"query":"SS","case_sensitive":false}))).unwrap();
    let page = host
        .search
        .page(SearchPageInput {
            search_id: session.search_id,
            query_seq: 1,
            scope_snapshot_id: session.scope_snapshot_id,
            cursor: None,
        })
        .unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(
        (page.hits[0].ranges[0].start, page.hits[0].ranges[0].end),
        (10, 11)
    );
    assert_ne!(page.hits[0].matched_anchor.owner_entity_id, job.material_id);
    assert_eq!(page.hits[0].nav_kind, SearchHitNavKind::PageRegion);
    host.store
        .read(|c| library::check_source(c, &job.input.snapshot.sources[0]))
        .unwrap();
}

#[test]
fn native_material_acceptance_publishes_local_manifest_and_rejects_stale_inputs() {
    fn manifest(
        material_id: &str,
        source_version_id: &str,
        parse_version_id: &str,
        source_hash: &str,
    ) -> MaterialManifest {
        let page_id = storage::id();
        let region_id = storage::id();
        let object_id = storage::id();
        let object_revision = storage::id();
        let block_id = storage::id();
        let element_id = storage::id();
        let source = SourceRef {
            entity_id: material_id.into(),
            revision_id: source_version_id.into(),
            content_hash: source_hash.into(),
            selector: Selector::V2(MaterialSelector {
                kind: "material_region".into(),
                material_id: material_id.into(),
                source_version_id: source_version_id.into(),
                parse_version_id: parse_version_id.into(),
                page_id: page_id.clone(),
                region_id: region_id.clone(),
                bbox: vec![0., 0., 1., 1.],
                coordinate_space: "rotated_page_normalized".into(),
            }),
        };
        serde_json::from_value(json!({
            "format":"focaus.material",
            "format_version":"2.0.0",
            "material_id":material_id,
            "source_version_id":source_version_id,
            "parse_version_id":parse_version_id,
            "parent_parse_version_id":null,
            "source_hash":source_hash,
            "engine_build":"native-test",
            "parse_profile":"light",
            "requested_pages":[0],
            "pages":[{
                "page_id":page_id,
                "revision_id":storage::id(),
                "source_page_index":0,
                "display_label":"1",
                "width":100.0,
                "height":100.0,
                "unit":"pt",
                "rotation":0,
                "source_transform":[],
                "preview_resource_id":null,
                "reading_order":[object_id],
                "region_ids":[region_id],
                "status":"complete",
                "errors":[]
            }],
            "regions":[{
                "region_id":region_id,
                "page_id":page_id,
                "bbox":[0.,0.,1.,1.],
                "object_ids":[object_id],
                "semantic_role":"body",
                "parent_region_id":null
            }],
            "objects":[{
                "object_id":object_id,
                "revision_id":object_revision,
                "source_refs":[source],
                "region_ids":[region_id],
                "layout":{"bounds":[0.,0.,1.,1.],"z_order":0,"group_id":null,"constraints":[]},
                "extraction":"native",
                "editability":"native",
                "status":"complete",
                "children":[],
                "type":"text",
                "content":{"block_id":block_id,"runs":[{"element_id":element_id,"text":"local material"}],"level":1}
            }],
            "resources":[],
            "corrections_applied":[],
            "unresolved_conflicts":[],
            "provenance":{},
            "completion":"complete",
            "search_units":[{
                "unit_key":"local-material-text",
                "owner_entity_id":object_id,
                "revision_id":object_revision,
                "content_hash":search_text_hash("local material"),
                "display_text":"local material",
                "field_path":"/content/runs",
                "anchor":{
                    "owner_entity_id":object_id,
                    "revision_id":object_revision,
                    "selector":{
                        "kind":"material_region",
                        "material_id":material_id,
                        "source_version_id":source_version_id,
                        "parse_version_id":parse_version_id,
                        "page_id":page_id,
                        "region_id":region_id,
                        "bbox":[0.,0.,1.,1.],
                        "coordinate_space":"rotated_page_normalized"
                    },
                    "field_path":"/content/runs",
                    "scalar_range":{"start":0,"end":14},
                    "audio_spans":[],
                    "fallback":null
                },
                "equivalence_group_id":null,
                "alternate_anchors":[],
                "source_role":"material_supplement",
                "extraction_version":"1.0.0"
            }],
            "search_coverage":{
                "requested_pages":[page_id],
                "processed_pages":[page_id],
                "unprocessed":[],
                "failed":[],
                "state":"complete"
            }
        }))
        .unwrap()
    }

    let db = TestLibrary::new();
    let host = ipc::NativeHost::open(&db.path, INITIAL_MIGRATION).unwrap();
    let lesson_id = lesson(&db.store);
    let source_file = db.path.join("minimal.txt");
    std::fs::write(&source_file, "local source").unwrap();
    let job = host
        .import_material_original(
            &source_file,
            &lesson_id,
            MaterialImportInputPurpose::Textbook,
            vec![0],
        )
        .unwrap();
    let source = job.input.snapshot.sources[0].clone();
    host.tasks
        .transition(&job.task.task_id, 1, TaskState::Running)
        .unwrap();
    let input = MaterialPageResultInput {
        task_id: job.task.task_id.clone(),
        snapshot_id: job.task.snapshot_id.clone(),
        manifest: manifest(
            &job.material_id,
            &job.source_version_id,
            &job.parse_version_id,
            &source.content_hash,
        ),
    };
    validate("MaterialManifest", &json!(input.manifest)).unwrap();
    let committed = host.accept_material_result(input.clone()).unwrap();
    assert_eq!(committed["publication"], "committed");
    let events: Vec<String> = db
        .store
        .read(|c| {
            let mut q = c
                .prepare("SELECT event_type FROM outbox WHERE task_id=?1")
                .map_err(storage::sql_error)?;
            let events = q.query_map([&job.task.task_id], |r| r.get(0))
                .map_err(storage::sql_error)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(storage::sql_error)?;
            Ok(events)
        })
        .unwrap();
    assert!(events.iter().any(|event| event == "material.page_ready"));
    assert!(events.iter().any(|event| event == "material.parse_changed"));
    assert!(events.iter().any(|event| event == "search.invalidated"));

    let content = db
        .store
        .read(|c| library::get_content(c, &ContentGetInput {
            entity_id: job.material_id.clone(),
            revision_id: Some(job.parse_version_id.clone()),
            selector: None,
        }))
        .unwrap();
    assert_eq!(content["format"], "focaus.material");
    assert_eq!(content["format_version"], "2.0.0");
    assert_eq!(host.accept_material_result(input.clone()).unwrap()["duplicate"], true);

    let window = storage::id();
    host.bind_window(
        "main",
        &window,
        vec![serde_json::from_value(json!({"kind":"global"})).unwrap()],
    )
    .unwrap();
    let content = host
        .dispatch_webview(
            "main",
            command(
                "content.get",
                &window,
                json!({
                    "entity_id":job.material_id,
                    "revision_id":job.parse_version_id,
                    "selector":null
                }),
            ),
        )
        .unwrap();
    validate("MaterialManifest", &json!(content.result.as_ref().unwrap())).unwrap();

    let opened = host
        .dispatch_webview(
            "main",
            command(
                "view.open",
                &window,
                json!({
                    "lesson_id":lesson_id,
                    "target":{
                        "kind":"entity",
                        "entity_id":job.material_id,
                        "revision_id":null,
                        "selector":null
                    },
                    "placement":"grid"
                }),
            ),
        )
        .unwrap();
    let view: ContentView = serde_json::from_value(opened.result.unwrap()).unwrap();
    let opened_search = host
        .dispatch_webview(
            "main",
            command(
                "search.open",
                &window,
                json!({
                    "view_id":view.view_id,
                    "target":view.target,
                    "version_mode":"current",
                    "actual_visible_tier":null,
                    "revision_ids":[]
                }),
            ),
        )
        .unwrap();
    let session: SearchSession = serde_json::from_value(opened_search.result.unwrap()).unwrap();
    assert!(session.scope.target_ids.contains(&job.material_id));
    let queried = host
        .dispatch_webview(
            "main",
            command(
                "search.query",
                &window,
                json!({
                    "search_id":session.search_id,
                    "query_seq":1,
                    "query":"material",
                    "case_sensitive":false
                }),
            ),
        )
        .unwrap();
    let query_task: Task = serde_json::from_value(queried.result.unwrap()).unwrap();
    assert_eq!(query_task.state, TaskState::Succeeded);
    let page_result = host
        .dispatch_webview(
            "main",
            command(
                "search.page",
                &window,
                json!({
                    "search_id":session.search_id,
                    "query_seq":1,
                    "scope_snapshot_id":session.scope_snapshot_id,
                    "cursor":null
                }),
            ),
        )
        .unwrap();
    let page: SearchPage = serde_json::from_value(page_result.result.unwrap()).unwrap();
    validate("SearchPage", &json!(page)).unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.count_status, SearchPageCountStatus::Exact);
    assert_eq!(page.coverage.state, SearchCoverageState::Complete);
    assert_eq!(page.coverage.indexed_units, 1);
    assert!(page
        .coverage
        .required_pages
        .contains(&input.manifest.pages[0].page_id));
    assert!(page.index_generation > 0);
    assert_eq!(page.visibility_epoch, session.visibility_epoch);
    let hit = page.hits[0].clone();
    let object_id = hit.matched_anchor.owner_entity_id.clone();
    assert_ne!(object_id, job.material_id);
    assert_eq!(hit.nav_kind, SearchHitNavKind::PageRegion);
    assert_eq!(hit.matched_anchor.scalar_range, ScalarRange { start: 6, end: 14 });
    let located_result = host
        .dispatch_webview(
            "main",
            command(
                "search.locate",
                &window,
                json!({
                    "search_id":session.search_id,
                    "view_id":view.view_id,
                    "hit_id":hit.hit_id,
                    "expected_revision":hit.revision_id
                }),
            ),
        )
        .unwrap();
    let located: SearchAnchor = serde_json::from_value(located_result.result.unwrap()).unwrap();
    assert_eq!(located, hit.matched_anchor);

    let from_revision = storage::id();
    let from_selector = Selector::V3(ObjectSelector {
        kind: "object".into(),
        object_id: object_id.clone(),
        element_ids: Vec::new(),
    });
    let edge = SourceEdge {
        edge_id: storage::id(),
        from_entity_id: object_id.clone(),
        from_revision_id: from_revision.clone(),
        from_selector: from_selector.clone(),
        to: source.clone(),
        role: SourceEdgeRole::MaterialSupplement,
        created_by: SourceEdgeCreatedBy::Engine,
        snapshot_id: storage::id(),
        availability: SourceEdgeAvailability::Available,
        freshness: SourceEdgeFreshness::Current,
    };
    db.store
        .write(|t| {
            let mut object = storage::head(t, &object_id)?;
            object["source_edges"] = json!([edge.clone()]);
            storage::commit_version(
                t,
                &object_id,
                "material_object",
                &from_revision,
                object,
                "material.page_ready",
                json!({"kind":"entity","entity_id":job.material_id}),
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
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let source_input = json!({
        "edge_id":edge.edge_id,
        "return_anchor":from_selector
    });
    let resolved = host
        .dispatch_webview("main", command("source.resolve", &window, source_input.clone()))
        .unwrap();
    let resolved = resolved.result.unwrap();
    assert_eq!(resolved["resolved_source"]["entity_id"], job.material_id);
    assert_eq!(resolved["edge"]["availability"], "available");
    assert_eq!(resolved["edge"]["freshness"], "current");
    assert_eq!(resolved["traversed_edges"][0], edge.edge_id);
    assert!(resolved["skipped_layers"].as_array().unwrap().is_empty());

    db.store
        .write(|t| {
            t.execute(
                "UPDATE source_edges SET freshness='update_available' WHERE edge_id=?1",
                [&edge.edge_id],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let projected = host
        .dispatch_webview("main", command("source.resolve", &window, source_input.clone()))
        .unwrap();
    assert_eq!(
        projected.result.unwrap()["edge"]["freshness"],
        "update_available"
    );
    let denied = host
        .dispatch_webview(
            "main",
            command(
                "source.resolve",
                &window,
                json!({
                    "edge_id":edge.edge_id,
                    "return_anchor":{"kind":"whole"}
                }),
            ),
        )
        .unwrap_err();
    assert_eq!(denied.code, ErrorCode::PermissionDenied);

    let stale_hash = "0".repeat(64);
    db.store
        .write(|t| {
            t.execute(
                "UPDATE resources SET sha256=?1 WHERE resource_id=(SELECT resource_id FROM materials WHERE material_id=?2)",
                params![stale_hash, job.material_id],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let stale_source = host
        .dispatch_webview("main", command("source.resolve", &window, source_input.clone()))
        .unwrap_err();
    assert_eq!(stale_source.code, ErrorCode::StaleInput);
    db.store
        .write(|t| {
            t.execute(
                "UPDATE resources SET sha256=?1 WHERE resource_id=(SELECT resource_id FROM materials WHERE material_id=?2)",
                params![source.content_hash, job.material_id],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();

    let mut wrong_source = input.manifest.clone();
    wrong_source.parse_version_id = storage::id();
    wrong_source.source_hash = "0".repeat(64);
    assert_eq!(
        host.accept_material_result(MaterialPageResultInput {
            task_id: job.task.task_id.clone(),
            snapshot_id: job.task.snapshot_id.clone(),
            manifest: wrong_source,
        })
        .unwrap_err()
        .code,
        ErrorCode::InvalidResult
    );
    let mut stale_parent = input.manifest;
    stale_parent.parse_version_id = storage::id();
    assert_eq!(
        host.accept_material_result(MaterialPageResultInput {
            task_id: job.task.task_id,
            snapshot_id: job.task.snapshot_id,
            manifest: stale_parent,
        })
        .unwrap_err()
        .code,
        ErrorCode::StaleInput
    );

    db.store
        .write(|t| {
            t.execute(
                "UPDATE entities SET lifecycle='deleted' WHERE entity_id=?1",
                [&job.material_id],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let deleted_source = host
        .dispatch_webview("main", command("source.resolve", &window, source_input))
        .unwrap_err();
    assert_eq!(deleted_source.code, ErrorCode::SourceDeleted);
    let deleted_content = host
        .dispatch_webview(
            "main",
            command(
                "content.get",
                &window,
                json!({
                    "entity_id":job.material_id,
                    "revision_id":job.parse_version_id,
                    "selector":null
                }),
            ),
        )
        .unwrap_err();
    assert_eq!(deleted_content.code, ErrorCode::SourceDeleted);
    let deleted_page = host
        .dispatch_webview(
            "main",
            command(
                "search.page",
                &window,
                json!({
                    "search_id":session.search_id,
                    "query_seq":1,
                    "scope_snapshot_id":session.scope_snapshot_id,
                    "cursor":null
                }),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        deleted_page.code,
        ErrorCode::SearchInvalidated | ErrorCode::SourceDeleted
    ));
    let deleted_locate = host
        .dispatch_webview(
            "main",
            command(
                "search.locate",
                &window,
                json!({
                    "search_id":session.search_id,
                    "view_id":view.view_id,
                    "hit_id":hit.hit_id,
                    "expected_revision":hit.revision_id
                }),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        deleted_locate.code,
        ErrorCode::SearchInvalidated | ErrorCode::SourceDeleted
    ));
}

#[test]
fn original_file_sealing_continues_under_sqlite_lock_and_metadata_queue_overflow() {
    use std::{sync::mpsc, thread, time::Duration};
    let db = TestLibrary::new();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap().to_owned();
    let (locked_tx, locked_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let lock_store = db.store.clone();
    let lock = thread::spawn(move || {
        lock_store.write(|_| {
            locked_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(3));
            Ok(())
        })
    });
    locked_rx.recv().unwrap();
    let (sealed_tx, sealed_rx) = mpsc::sync_channel(1);
    let writer_store = db.store.clone();
    let writer_id = rid.clone();
    let writer = thread::spawn(move || -> storage::Result<()> {
        let registrar =
            audio::registrar::MetadataRegistrar::new(writer_store.clone(), writer_id.clone(), 1)?;
        registrar.started()?;
        for i in 0..5 {
            let meta = audio::SegmentMeta::new(&writer_id, 48000, 1, i * 100, i * 4800)?;
            let relative = meta.relative();
            let mut file = audio::SegmentWriter::create(writer_store.root(), meta)?;
            file.append(&[0.25; 4800])?;
            registrar.segment(file.seal_bytes(writer_store.root())?)?;
            assert!(writer_store.root().join(relative).is_file());
        }
        registrar.gap(audio::PendingGap::create(
            writer_store.root(),
            &writer_id,
            500,
            600,
            false,
        )?)?;
        sealed_tx.send(()).unwrap();
        registrar.finish()
    });
    // This must arrive while the SQLite writer is still deliberately held.
    let sealed = sealed_rx.recv_timeout(Duration::from_secs(2));
    release_tx.send(()).unwrap();
    lock.join().unwrap().unwrap();
    assert!(sealed.is_ok(), "original writer waited for SQLite");
    writer.join().unwrap().unwrap();
    let counts=db.store.read(|c|c.query_row("SELECT COUNT(*),SUM(coverage_status='capture_gap') FROM audio_spans WHERE recording_id=?1",[&rid],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,u64>(1)?))).map_err(storage::sql_error)).unwrap();
    assert_eq!(counts, (6, 1));
    assert_eq!(db.store.head(&rid).unwrap()["durable_watermark_ms"], 500);
    assert_eq!(db.store.head(&rid).unwrap()["state"], "recording");
    assert_eq!(
        std::fs::read_dir(db.path.join("capture").join(&rid))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn segment_end_uses_global_rational_boundary_without_accumulated_rounding() {
    let db = TestLibrary::new();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap();
    let mut writer = audio::SegmentWriter::create(
        &db.path,
        audio::SegmentMeta::new(rid, 48000, 1, 1000, 48001).unwrap(),
    )
    .unwrap();
    writer.append(&vec![0.; 47999]).unwrap();
    let span = writer.seal(&db.store, false).unwrap();
    assert_eq!(span["timeline_range"]["end_ms"], 2000);
}

#[test]
fn sealed_original_and_gap_journals_survive_unregistered_restart_and_replay_once() {
    let db = TestLibrary::new();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap();
    let meta = audio::SegmentMeta::new(rid, 48000, 1, 0, 0).unwrap();
    let relative = meta.relative();
    let mut writer = audio::SegmentWriter::create(&db.path, meta).unwrap();
    writer.append(&[0.25; 4800]).unwrap();
    drop(writer.seal_bytes(&db.path).unwrap());
    drop(audio::PendingGap::create(&db.path, rid, 100, 200, false).unwrap());
    let original_hash = audio::raw_hash(&db.path.join(&relative)).unwrap();
    let recovered = audio::recover(&db.store).unwrap();
    assert_eq!(recovered.failed_segments, 0);
    assert_eq!(recovered.recovered_segments, 1);
    assert_eq!(recovered.recovered_frames, 4800);
    assert_eq!(
        audio::raw_hash(&db.path.join(&relative)).unwrap(),
        original_hash
    );
    let counts = db.store.read(|c| {
        c.query_row("SELECT COUNT(*),SUM(coverage_status='capture_gap') FROM audio_spans WHERE recording_id=?1", [rid], |r| Ok((r.get::<_,u64>(0)?,r.get::<_,u64>(1)?)))
            .map_err(storage::sql_error)
    }).unwrap();
    assert_eq!(counts, (2, 1));
    assert_eq!(db.store.head(rid).unwrap()["state"], "interrupted");
    assert_eq!(db.store.head(rid).unwrap()["durable_watermark_ms"], 100);
    let seq = db.store.read(storage::sequence).unwrap();
    let replayed = audio::recover(&db.store).unwrap();
    assert_eq!(replayed.recovered_segments, 0);
    assert_eq!(replayed.failed_segments, 0);
    assert_eq!(db.store.read(storage::sequence).unwrap(), seq);
    assert_eq!(
        std::fs::read_dir(db.path.join("capture").join(rid))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn failed_registrar_preserves_later_original_journals_and_rejects_deleted_recording() {
    use std::time::{Duration, Instant};
    let db = TestLibrary::new();
    let recording = reserve(&db.store);
    let rid = recording["recording_id"].as_str().unwrap();
    db.store
        .write(|t| {
            t.execute(
                "UPDATE entities SET lifecycle='deleted' WHERE entity_id=?1",
                [rid],
            )
            .map_err(storage::sql_error)?;
            Ok(())
        })
        .unwrap();
    let seq = db.store.read(storage::sequence).unwrap();
    let registrar =
        audio::registrar::MetadataRegistrar::new(db.store.clone(), rid.into(), 1).unwrap();
    registrar.started().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !registrar.failed() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(registrar.failed(), "metadata failure was not observed");
    let mut writer = audio::SegmentWriter::create(
        &db.path,
        audio::SegmentMeta::new(rid, 48000, 1, 0, 0).unwrap(),
    )
    .unwrap();
    writer.append(&[0.25; 4800]).unwrap();
    registrar
        .segment(writer.seal_bytes(&db.path).unwrap())
        .unwrap();
    registrar
        .gap(audio::PendingGap::create(&db.path, rid, 100, 200, false).unwrap())
        .unwrap();
    assert_eq!(
        registrar.finish().unwrap_err().code,
        ErrorCode::SourceDeleted
    );
    assert_eq!(
        std::fs::read_dir(db.path.join("capture").join(rid))
            .unwrap()
            .count(),
        2
    );
    assert_eq!(
        std::fs::read_dir(db.path.join("resources"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(db.store.read(storage::sequence).unwrap(), seq);
    assert_eq!(audio::recover(&db.store).unwrap().failed_segments, 2);
    assert_eq!(db.store.read(storage::sequence).unwrap(), seq);
}

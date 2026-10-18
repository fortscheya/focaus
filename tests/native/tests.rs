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

//! Sanitized protocol record/replay fixtures.
//!
//! These values mirror the public provider event shapes and contain no API
//! key, workspace ID, audio, or provider response from a live request. They
//! are suitable for A/N local adapter tests only; passing them is not a cloud
//! acceptance result.

use serde_json::{json, Value};

pub fn streaming_task_started() -> Value {
    json!({
        "header": { "task_id": "fixture-stream-task", "event": "task-started", "attributes": {} },
        "payload": {}
    })
}

pub fn streaming_sentence_start() -> Value {
    json!({
        "header": { "task_id": "fixture-stream-task", "event": "result-generated", "attributes": {} },
        "payload": { "output": { "sentence": {
            "begin_time": 0,
            "end_time": null,
            "text": "",
            "sentence_begin": true,
            "sentence_end": false,
            "sentence_id": 1,
            "words": []
        }}}
    })
}

pub fn streaming_sentence_final() -> Value {
    json!({
        "header": { "task_id": "fixture-stream-task", "event": "result-generated", "attributes": {} },
        "payload": { "output": { "sentence": {
            "sentence_id": 1,
            "begin_time": 160,
            "end_time": 1640,
            "text": "欢迎使用阿里云。",
            "speaker_id": null,
            "sentence_end": true,
            "words": []
        }}}
    })
}

pub fn streaming_task_finished() -> Value {
    json!({
        "header": { "task_id": "fixture-stream-task", "event": "task-finished", "attributes": {} },
        "payload": { "output": {}, "usage": { "input_tokens": 10, "output_tokens": 4, "total_tokens": 14 } }
    })
}

pub fn filetrans_submitted() -> Value {
    json!({
        "request_id": "fixture-request",
        "output": { "task_status": "PENDING", "task_id": "fixture-file-task" }
    })
}

pub fn filetrans_succeeded() -> Value {
    json!({
        "request_id": "fixture-request",
        "output": {
            "task_id": "fixture-file-task",
            "task_status": "SUCCEEDED",
            "results": [{ "subtask_status": "SUCCEEDED", "transcription_url": "https://fixture.invalid/transcription.json" }],
            "task_metrics": { "TOTAL": 1, "SUCCEEDED": 1, "FAILED": 0 }
        }
    })
}

pub fn progress_response() -> Value {
    json!({
        "model": "decision-model-preview",
        "request_id": "fixture-progress-request",
        "answers": { "current_position": {
            "type": "choice",
            "choice": "page-1",
            "confidence": 0.9,
            "probabilities": { "page-1": 0.9, "__unlocated__": 0.1 }
        }},
        "latency_ms": 12.0
    })
}

pub fn qwen38_chat_completion() -> Value {
    json!({
        "id": "fixture-chat-completion",
        "object": "chat.completion",
        "model": "qwen3.8-flash",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "{\"ok\":true}" }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12 }
    })
}

pub fn asr_result_candidate_provisional() -> Value {
    json!({
        "result_semantics": "asr_raw_candidate_v1",
        "submission_key": "asr:fixture-stream-task:1:fixture-stream-snapshot",
        "task_id": "fixture-stream-task",
        "attempt": 1,
        "snapshot_id": "fixture-stream-snapshot",
        "source_refs": [{
            "entity_id": "fixture-audio-span",
            "revision_id": "fixture-audio-revision",
            "content_hash": "fixture-hash",
            "selector": { "kind": "whole" }
        }],
        "candidate": {
            "task_id": "fixture-stream-task",
            "attempt": 1,
            "snapshot_id": "fixture-stream-snapshot",
            "remote_binding": {
                "provider": "bailian",
                "provider_task_id": "fixture-stream-task",
                "connection_epoch": 1,
                "provider_sentence_id": "1",
                "local_request_segment_id": "fixture-stream-task"
            },
            "state": "provisional",
            "text": "hello",
            "relative_start_ms": 0,
            "relative_end_ms": 100,
            "speaker_ref": null,
            "mapping_revision_id": "fixture-mapping-revision"
        }
    })
}

pub fn asr_result_candidate_final() -> Value {
    let mut value = asr_result_candidate_provisional();
    value["candidate"]["state"] = json!("final");
    value["candidate"]["text"] = json!("hello world");
    value["candidate"]["relative_end_ms"] = json!(120);
    value
}

pub fn secondary_result_candidate() -> Value {
    json!({
        "result_semantics": "secondary_segment_v1",
        "submission_key": "secondary:fixture-secondary-task:1:fixture-secondary-snapshot:fixture-secondary-revision",
        "task_id": "fixture-secondary-task",
        "attempt": 1,
        "snapshot_id": "fixture-secondary-snapshot",
        "target_revision": "fixture-secondary-revision",
        "source_refs": [{
            "entity_id": "fixture-utterance",
            "revision_id": "fixture-raw-revision",
            "content_hash": "fixture-hash",
            "selector": { "kind": "whole" }
        }],
        "candidate": {
            "secondary_id": "fixture-secondary",
            "recording_id": "fixture-recording",
            "raw_refs": [{
                "entity_id": "fixture-utterance",
                "revision_id": "fixture-raw-revision",
                "content_hash": "fixture-hash",
                "selector": { "kind": "whole" }
            }],
            "ordered_spans": ["fixture-span"],
            "text": "hello world",
            "suggestions": [],
            "generation_revision": "fixture-generation-revision",
            "revision_id": "fixture-secondary-revision"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::{decode_asr_result_candidate, decode_stream_event, filetrans_cancel_allowed, parse_filetrans_status, DecodedStreamEvent, FileTransStatus};
    use crate::progress::parse_system_one_choice;
    use crate::secondary::decode_secondary_result_candidate;
    use focaus_contracts::AsrRawCandidateState;

    #[test]
    fn public_shaped_fixtures_replay_without_network() {
        assert!(matches!(decode_stream_event(&streaming_task_started()), Ok(DecodedStreamEvent::TaskStarted { .. })));
        assert!(matches!(decode_stream_event(&streaming_sentence_start()).unwrap(), DecodedStreamEvent::SentenceStart));
        let DecodedStreamEvent::Sentence(sentence) = decode_stream_event(&streaming_sentence_final()).unwrap() else { panic!("sentence fixture expected") };
        assert_eq!(sentence.sentence_id, "1");
        assert_eq!(sentence.text, "欢迎使用阿里云。");
        assert!(matches!(decode_stream_event(&streaming_task_finished()), Ok(DecodedStreamEvent::TaskFinished { .. })));
        let (task_id, status) = parse_filetrans_status(&filetrans_submitted()).unwrap();
        assert_eq!(task_id, "fixture-file-task");
        assert_eq!(status, FileTransStatus::Pending);
        assert!(filetrans_cancel_allowed(&status));
        let (_, status) = parse_filetrans_status(&filetrans_succeeded()).unwrap();
        assert_eq!(status, FileTransStatus::Succeeded);
        assert!(!filetrans_cancel_allowed(&status));
        assert_eq!(parse_system_one_choice(&progress_response(), "current_position").unwrap(), "page-1");
        assert_eq!(qwen38_chat_completion()["model"], "qwen3.8-flash");
        let provisional = decode_asr_result_candidate(&asr_result_candidate_provisional()).unwrap();
        let final_result = decode_asr_result_candidate(&asr_result_candidate_final()).unwrap();
        assert_eq!(provisional.candidate.state, AsrRawCandidateState::Provisional);
        assert_eq!(final_result.candidate.state, AsrRawCandidateState::Final);
        assert_eq!(final_result.candidate.text, "hello world");
        assert_eq!(decode_secondary_result_candidate(&secondary_result_candidate()).unwrap().candidate.secondary_id, "fixture-secondary");
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::{decode_stream_event, filetrans_cancel_allowed, parse_filetrans_status, DecodedStreamEvent, FileTransStatus};
    use crate::progress::parse_system_one_choice;

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
    }
}

//! Focaus AI workflow adapters.
//!
//! This crate owns protocol and proposal logic only. It deliberately has no
//! database, filesystem, credential-value, or network implementation. The
//! native host supplies snapshots, checkpoints, context retrieval and the
//! single result submission boundary through [`HostServices`].

pub use focaus_contracts::HostServices;

pub mod asr;
pub mod catchup;
pub mod fixtures;
pub mod progress;
pub mod runtime;
pub mod secondary;
pub mod transport;
pub mod visualization;

/// Exact Beijing model IDs used by the product.
pub mod models {
    pub const REGION: &str = "cn-beijing";
    pub const STREAMING_ASR: &str = "qwen-audio-3.1-asr-flash-streaming";
    pub const FILETRANS_ASR: &str = "qwen-audio-3.1-asr-flash-filetrans";
    pub const PROGRESS: &str = "decision-model-preview";
    pub const MULTIMODAL: &str = "qwen3.8-flash";

    pub const CREDENTIAL_STREAMING_ASR: &str = "bailian_beijing_asr_streaming";
    pub const CREDENTIAL_FILETRANS_ASR: &str = "bailian_beijing_asr_filetrans";
    pub const CREDENTIAL_PROGRESS: &str = "bailian_beijing_progress";
    pub const CREDENTIAL_MULTIMODAL: &str = "bailian_beijing_multimodal";
}

pub const PROMPT_ASR_STREAM_V1: &str = "asr_stream_v1";
pub const PROMPT_ASR_FILE_V1: &str = "asr_file_v1";
pub const PROMPT_PROGRESS_V1: &str = "progress_decision_v1";
pub const PROMPT_FOCAUS_V2: &str = "focaus_context_v2";
pub const PROMPT_SECONDARY_V1: &str = "secondary_direct_speech_v1";
pub const PROMPT_VISUAL_DISCOVER_V1: &str = "visual_discover_v1";

/// Construct a safe task-scoped error without including provider responses or
/// credential values.
pub(crate) fn task_error(
    code: focaus_contracts::ErrorCode,
    message: impl Into<String>,
    retryable: bool,
) -> focaus_contracts::SafeError {
    focaus_contracts::SafeError {
        code,
        retryable,
        scope: focaus_contracts::SafeErrorScope::Task,
        message: message.into(),
        details: None,
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> focaus_contracts::SafeError {
    task_error(focaus_contracts::ErrorCode::InvalidResult, message, false)
}

pub(crate) fn invalid_argument(message: impl Into<String>) -> focaus_contracts::SafeError {
    task_error(focaus_contracts::ErrorCode::InvalidArgument, message, false)
}

pub(crate) fn stale(message: impl Into<String>) -> focaus_contracts::SafeError {
    task_error(focaus_contracts::ErrorCode::StaleInput, message, false)
}

pub(crate) fn cancelled(message: impl Into<String>) -> focaus_contracts::SafeError {
    task_error(focaus_contracts::ErrorCode::Cancelled, message, false)
}

pub(crate) fn binding_for_model(
    model: focaus_contracts::ModelId,
) -> focaus_contracts::CredentialBinding {
    use focaus_contracts::{CredentialBinding, CredentialRef, ModelId};
    let credential_ref = match model {
        ModelId::QwenAudio31AsrFlashStreaming => CredentialRef::BailianBeijingAsrStreaming,
        ModelId::QwenAudio31AsrFlashFiletrans => CredentialRef::BailianBeijingAsrFiletrans,
        ModelId::DecisionModelPreview => CredentialRef::BailianBeijingProgress,
        ModelId::Qwen38Flash => CredentialRef::BailianBeijingMultimodal,
    };
    CredentialBinding {
        provider: "bailian".to_string(),
        region: models::REGION.to_string(),
        model_id: model,
        credential_ref,
    }
}

pub(crate) fn binding_matches(
    binding: &focaus_contracts::CredentialBinding,
    expected: focaus_contracts::ModelId,
) -> Result<(), focaus_contracts::SafeError> {
    let expected_binding = binding_for_model(expected.clone());
    if binding != &expected_binding {
        return Err(invalid_argument("credential_binding_mismatch"));
    }
    let model_name = serde_json::to_value(&binding.model_id)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let slot_name = serde_json::to_value(&binding.credential_ref)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    focaus_contracts::credential_binding(&model_name, &binding.region, &slot_name)
}

pub fn model_from_json(value: &serde_json::Value) -> Option<focaus_contracts::ModelId> {
    serde_json::from_value(value.clone()).ok()
}

/// Collapse whitespace for deterministic ASR boundary comparisons.
pub(crate) fn compact_text(text: &str) -> String {
    text.split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use focaus_contracts::{CredentialRef, ModelId};

    #[test]
    fn model_binding_is_one_to_one_and_beijing_only() {
        let binding = binding_for_model(ModelId::QwenAudio31AsrFlashFiletrans);
        assert_eq!(binding.region, "cn-beijing");
        assert_eq!(binding.credential_ref, CredentialRef::BailianBeijingAsrFiletrans);
        assert!(binding_matches(&binding, ModelId::QwenAudio31AsrFlashFiletrans).is_ok());
        let wrong = binding_for_model(ModelId::QwenAudio31AsrFlashStreaming);
        assert!(binding_matches(&wrong, ModelId::QwenAudio31AsrFlashFiletrans).is_err());
    }

    #[test]
    fn provider_model_nickname_is_not_accepted_as_model_id() {
        assert!(model_from_json(&serde_json::json!("3.8Flash")).is_none());
        assert_eq!(compact_text("  A\n B  "), "a b");
    }
}

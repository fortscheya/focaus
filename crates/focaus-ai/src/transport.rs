//! Optional host-injected Beijing HTTP/WebSocket transport.
//!
//! This is an adapter, not an automatic network client.  Callers must supply
//! an endpoint and a host credential capability explicitly.  No constructor
//! reads environment variables or local configuration, and no error includes
//! the response body or authorization value.

use crate::{binding_matches, invalid_argument, task_error};
use focaus_contracts::{CredentialBinding, ErrorCode, ModelId, SafeError};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::http::Request, MaybeTlsStream, WebSocketStream};
use tokio::net::TcpStream;

pub type ProviderWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A host-owned capability.  The returned value is consumed immediately by a
/// transport request and is never placed in a task, snapshot, diagnostic or
/// debug string.  The host remains responsible for secure storage and policy.
pub trait CredentialCapability: Send + Sync {
    fn bearer_token(&self, binding: &CredentialBinding) -> Result<String, SafeError>;
}

#[derive(Debug, Clone)]
pub struct JsonResponse {
    pub status: u16,
    pub body: Value,
}

#[derive(Clone)]
pub struct ReqwestTransport<C> {
    client: Client,
    credentials: Arc<C>,
}

impl<C> ReqwestTransport<C>
where
    C: CredentialCapability + 'static,
{
    pub fn new(client: Client, credentials: Arc<C>) -> Self {
        Self { client, credentials }
    }

    pub async fn post_json(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
        body: Value,
    ) -> Result<JsonResponse, SafeError> {
        validate_endpoint(endpoint)?;
        validate_binding(binding)?;
        let token = self.credentials.bearer_token(binding)?;
        let response = self.client.post(endpoint).bearer_auth(token).json(&body).send().await
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_network_unavailable", true))?;
        decode_response(response).await
    }

    pub async fn get_json(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
    ) -> Result<JsonResponse, SafeError> {
        validate_endpoint(endpoint)?;
        validate_binding(binding)?;
        let token = self.credentials.bearer_token(binding)?;
        let response = self.client.get(endpoint).bearer_auth(token).send().await
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_network_unavailable", true))?;
        decode_response(response).await
    }

    /// Open a streaming connection only when the caller explicitly invokes
    /// this method.  The returned socket is owned by the caller and no task
    /// state is persisted here.
    pub async fn connect_stream(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
    ) -> Result<ProviderWebSocket, SafeError> {
        validate_ws_endpoint(endpoint)?;
        validate_binding(binding)?;
        let token = self.credentials.bearer_token(binding)?;
        let request = Request::builder()
            .uri(endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "focaus-ai/contract-2.0")
            .body(())
            .map_err(|_| invalid_argument("invalid_stream_endpoint"))?;
        let (socket, _) = connect_async(request)
            .await
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_stream_unavailable", true))?;
        Ok(socket)
    }
}

fn validate_binding(binding: &CredentialBinding) -> Result<(), SafeError> {
    let expected = match binding.model_id {
        ModelId::QwenAudio31AsrFlashStreaming => ModelId::QwenAudio31AsrFlashStreaming,
        ModelId::QwenAudio31AsrFlashFiletrans => ModelId::QwenAudio31AsrFlashFiletrans,
        ModelId::DecisionModelPreview => ModelId::DecisionModelPreview,
        ModelId::Qwen38Flash => ModelId::Qwen38Flash,
    };
    binding_matches(binding, expected)
}

fn validate_endpoint(endpoint: &str) -> Result<(), SafeError> {
    if !endpoint.starts_with("https://") || endpoint.contains('@') {
        return Err(invalid_argument("provider_endpoint_must_be_https_without_userinfo"));
    }
    Ok(())
}

fn validate_ws_endpoint(endpoint: &str) -> Result<(), SafeError> {
    if !(endpoint.starts_with("wss://") || endpoint.starts_with("https://")) || endpoint.contains('@') {
        return Err(invalid_argument("provider_stream_endpoint_must_be_tls_without_userinfo"));
    }
    Ok(())
}

async fn decode_response(response: reqwest::Response) -> Result<JsonResponse, SafeError> {
    let status = response.status();
    let body = response.json::<Value>().await.map_err(|_| task_error(ErrorCode::ProviderError, "provider_invalid_json_response", status.is_server_error()))?;
    if status.is_success() {
        return Ok(JsonResponse { status: status.as_u16(), body });
    }
    let (code, retryable, message) = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (ErrorCode::AuthRequired, false, "provider_auth_required"),
        StatusCode::TOO_MANY_REQUESTS => (ErrorCode::RateLimited, true, "provider_rate_limited"),
        value if value.is_server_error() => (ErrorCode::ProviderError, true, "provider_server_error"),
        _ => (ErrorCode::ProviderError, false, "provider_request_failed"),
    };
    Err(task_error(code, message, retryable))
}

/// Parse only the stable remote task ID.  The local task ID remains supplied
/// by the caller and is never replaced by this provider value.
pub fn parse_remote_task_id(response: &JsonResponse) -> Result<String, SafeError> {
    response.body.get("output").and_then(|value| value.get("task_id")).and_then(Value::as_str)
        .or_else(|| response.body.get("task_id").and_then(Value::as_str))
        .filter(|task_id| !task_id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| task_error(ErrorCode::InvalidResult, "provider_task_id_missing", false))
}

pub fn ensure_model_endpoint(model: ModelId, endpoint: &str) -> Result<(), SafeError> {
    validate_endpoint(endpoint)?;
    if model == ModelId::QwenAudio31AsrFlashStreaming && endpoint.contains("filetrans") {
        return Err(invalid_argument("streaming_model_filetrans_endpoint_mismatch"));
    }
    if model == ModelId::QwenAudio31AsrFlashFiletrans && endpoint.contains("stream") {
        return Err(invalid_argument("filetrans_model_streaming_endpoint_mismatch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_for_model;
    use focaus_contracts::ModelId;

    #[test]
    fn transport_requires_exact_binding_and_tls_endpoint() {
        let binding = binding_for_model(ModelId::QwenAudio31AsrFlashFiletrans);
        assert!(validate_binding(&binding).is_ok());
        let mut wrong = binding.clone();
        wrong.region = "cn-shanghai".into();
        assert_eq!(validate_binding(&wrong).unwrap_err().message, "credential_binding_mismatch");
        assert!(validate_endpoint("http://example.invalid").is_err());
        assert!(ensure_model_endpoint(ModelId::QwenAudio31AsrFlashFiletrans, "https://api.example/filetrans").is_ok());
        assert!(ensure_model_endpoint(ModelId::QwenAudio31AsrFlashFiletrans, "https://api.example/stream").is_err());
    }

    #[test]
    fn task_id_parser_never_substitutes_local_identity() {
        let response = JsonResponse { status: 200, body: serde_json::json!({"output":{"task_id":"provider-123"}}) };
        assert_eq!(parse_remote_task_id(&response).unwrap(), "provider-123");
    }
}

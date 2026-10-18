//! Optional host-injected Beijing HTTP/WebSocket transport.
//!
//! This is an adapter, not an automatic network client.  Callers must supply
//! an endpoint and a host credential capability explicitly.  No constructor
//! reads environment variables or local configuration, and no error includes
//! the response body or authorization value.

use crate::{binding_matches, invalid_argument, task_error};
use focaus_contracts::{CredentialBinding, ErrorCode, ModelId, SafeError};
use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client, StatusCode, Url};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{header, HeaderValue, Request},
    },
    MaybeTlsStream, WebSocketStream,
};
use tokio::net::TcpStream;
use std::time::Duration;
use tokio::time::timeout;

pub type ProviderWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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

/// A host-approved endpoint set for one exact model/credential binding.  The
/// path prefixes are explicit so a redirect or arbitrary URL cannot silently
/// move audio or credentials to another service.
#[derive(Debug, Clone, PartialEq)]
pub struct EndpointPolicy {
    pub binding: CredentialBinding,
    pub http_origin: String,
    pub http_path_prefix: String,
    pub websocket_origin: String,
    pub websocket_path_prefix: String,
    pub workspace_id: Option<String>,
}

impl EndpointPolicy {
    pub fn new(
        binding: CredentialBinding,
        http_origin: impl Into<String>,
        http_path_prefix: impl Into<String>,
        websocket_origin: impl Into<String>,
        websocket_path_prefix: impl Into<String>,
    ) -> Result<Self, SafeError> {
        validate_binding(&binding)?;
        let policy = Self {
            binding,
            http_origin: http_origin.into(),
            http_path_prefix: http_path_prefix.into(),
            websocket_origin: websocket_origin.into(),
            websocket_path_prefix: websocket_path_prefix.into(),
            workspace_id: None,
        };
        validate_base_origin(&policy.http_origin, "https")?;
        validate_base_origin(&policy.websocket_origin, "wss")?;
        validate_path_prefix(&policy.http_path_prefix)?;
        validate_path_prefix(&policy.websocket_path_prefix)?;
        Ok(policy)
    }

    pub fn for_beijing(binding: CredentialBinding, workspace_id: impl Into<String>) -> Result<Self, SafeError> {
        let workspace_id = workspace_id.into();
        if workspace_id.trim().is_empty() || workspace_id.contains('.') || workspace_id.contains('/') {
            return Err(invalid_argument("workspace_id_invalid"));
        }
        let http_path = match binding.model_id.clone() {
            ModelId::QwenAudio31AsrFlashFiletrans => "/api/v1/",
            ModelId::DecisionModelPreview | ModelId::Qwen38Flash => "/compatible-mode/v1/",
            ModelId::QwenAudio31AsrFlashStreaming => "/api/v1/",
        };
        let mut policy = Self::new(
            binding,
            format!("https://{workspace_id}.cn-beijing.maas.aliyuncs.com"),
            http_path,
            format!("wss://{workspace_id}.cn-beijing.maas.aliyuncs.com"),
            "/api-ws/v1/inference",
        )?;
        policy.workspace_id = Some(workspace_id);
        Ok(policy)
    }
}

#[derive(Clone)]
pub struct ReqwestTransport<C> {
    client: Client,
    credentials: Arc<C>,
    policy: EndpointPolicy,
}

impl<C> ReqwestTransport<C>
where
    C: CredentialCapability + 'static,
{
    /// Build the production transport with bounded connection/response
    /// behavior and redirects disabled.  Tests can still use the pure request
    /// builders without constructing this client.
    pub fn new(credentials: Arc<C>, policy: EndpointPolicy) -> Result<Self, SafeError> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .map_err(|_| task_error(ErrorCode::ProviderError, "provider_client_init_failed", false))?;
        Ok(Self { client, credentials, policy })
    }

    pub async fn post_json(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
        body: Value,
    ) -> Result<JsonResponse, SafeError> {
        self.post_json_with_headers(endpoint, binding, body, &[]).await
    }

    pub async fn post_json_with_headers(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
        body: Value,
        headers: &[(&str, &str)],
    ) -> Result<JsonResponse, SafeError> {
        validate_http_endpoint(endpoint, binding, &self.policy)?;
        let token = self.credentials.bearer_token(binding)?;
        let mut request = self.client.post(endpoint).bearer_auth(token).json(&body);
        if let Some(workspace_id) = &self.policy.workspace_id {
            request = request.header("X-DashScope-WorkSpace", workspace_id);
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().await
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_network_unavailable", true))?;
        decode_response(response).await
    }

    pub async fn get_json(
        &self,
        endpoint: &str,
        binding: &CredentialBinding,
    ) -> Result<JsonResponse, SafeError> {
        validate_http_endpoint(endpoint, binding, &self.policy)?;
        let token = self.credentials.bearer_token(binding)?;
        let mut request = self.client.get(endpoint).bearer_auth(token);
        if let Some(workspace_id) = &self.policy.workspace_id {
            request = request.header("X-DashScope-WorkSpace", workspace_id);
        }
        let response = request.send().await
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
        validate_ws_endpoint(endpoint, binding, &self.policy)?;
        let token = self.credentials.bearer_token(binding)?;
        let request = build_ws_request(endpoint, binding, &self.policy, &token)?;
        let (socket, _) = timeout(CONNECT_TIMEOUT, connect_async(request))
            .await
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_stream_connect_timeout", true))?
            .map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_stream_unavailable", true))?;
        Ok(socket)
    }
}

fn validate_binding(binding: &CredentialBinding) -> Result<(), SafeError> {
    let expected = match binding.model_id.clone() {
        ModelId::QwenAudio31AsrFlashStreaming => ModelId::QwenAudio31AsrFlashStreaming,
        ModelId::QwenAudio31AsrFlashFiletrans => ModelId::QwenAudio31AsrFlashFiletrans,
        ModelId::DecisionModelPreview => ModelId::DecisionModelPreview,
        ModelId::Qwen38Flash => ModelId::Qwen38Flash,
    };
    binding_matches(binding, expected)
}

fn validate_endpoint(endpoint: &str) -> Result<(), SafeError> {
    let parsed = Url::parse(endpoint).map_err(|_| invalid_argument("provider_endpoint_invalid"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(invalid_argument("provider_endpoint_must_be_https_without_userinfo"));
    }
    Ok(())
}

fn validate_ws_endpoint(endpoint: &str, binding: &CredentialBinding, policy: &EndpointPolicy) -> Result<(), SafeError> {
    validate_binding(binding)?;
    if &policy.binding != binding {
        return Err(invalid_argument("endpoint_binding_mismatch"));
    }
    let parsed = Url::parse(endpoint).map_err(|_| invalid_argument("provider_stream_endpoint_invalid"))?;
    if parsed.scheme() != "wss" || parsed.host_str().is_none() || parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() || !origin_matches(&parsed, &policy.websocket_origin) || !path_matches(&parsed.path(), &policy.websocket_path_prefix) {
        return Err(invalid_argument("provider_stream_endpoint_must_be_tls_without_userinfo"));
    }
    Ok(())
}

fn validate_base_origin(origin: &str, expected_scheme: &str) -> Result<(), SafeError> {
    let parsed = Url::parse(origin).map_err(|_| invalid_argument("endpoint_policy_origin_invalid"))?;
    if parsed.scheme() != expected_scheme || parsed.host_str().is_none() || parsed.username() != "" || parsed.password().is_some() || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(invalid_argument("endpoint_policy_origin_invalid"));
    }
    Ok(())
}

fn validate_path_prefix(prefix: &str) -> Result<(), SafeError> {
    if !prefix.starts_with('/') || prefix.contains("..") {
        return Err(invalid_argument("endpoint_policy_path_invalid"));
    }
    Ok(())
}

fn origin_matches(url: &Url, expected_origin: &str) -> bool {
    let Ok(expected) = Url::parse(expected_origin) else { return false };
    url.scheme() == expected.scheme()
        && url.host_str() == expected.host_str()
        && url.port_or_known_default() == expected.port_or_known_default()
}

fn validate_http_endpoint(endpoint: &str, binding: &CredentialBinding, policy: &EndpointPolicy) -> Result<(), SafeError> {
    validate_binding(binding)?;
    if &policy.binding != binding {
        return Err(invalid_argument("endpoint_binding_mismatch"));
    }
    let parsed = Url::parse(endpoint).map_err(|_| invalid_argument("provider_endpoint_invalid"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() || !origin_matches(&parsed, &policy.http_origin) || !path_matches(&parsed.path(), &policy.http_path_prefix) {
        return Err(invalid_argument("provider_endpoint_not_in_allowlist"));
    }
    Ok(())
}

fn path_matches(path: &str, prefix: &str) -> bool {
    let normalized = prefix.trim_end_matches('/');
    path == normalized || path.strip_prefix(normalized).is_some_and(|rest| rest.starts_with('/'))
}

/// Build a standards-compliant WebSocket client request.  `IntoClientRequest`
/// supplies the GET/Upgrade/Connection/Sec-WebSocket headers; only the
/// host-injected Authorization and a product user agent are added here.
pub fn build_ws_request(
    endpoint: &str,
    binding: &CredentialBinding,
    policy: &EndpointPolicy,
    token: &str,
) -> Result<Request<()>, SafeError> {
    validate_ws_endpoint(endpoint, binding, policy)?;
    if token.is_empty() {
        return Err(task_error(ErrorCode::AuthRequired, "provider_auth_required", false));
    }
    let mut request = endpoint.into_client_request().map_err(|_| invalid_argument("invalid_stream_endpoint"))?;
    let value = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|_| invalid_argument("provider_auth_header_invalid"))?;
    let mut value = value;
    value.set_sensitive(true);
    request.headers_mut().insert(header::AUTHORIZATION, value);
    request.headers_mut().insert(header::USER_AGENT, HeaderValue::from_static("focaus-ai/contract-2.0"));
    if let Some(workspace_id) = &policy.workspace_id {
        let workspace = HeaderValue::from_str(workspace_id).map_err(|_| invalid_argument("workspace_id_invalid"))?;
        request.headers_mut().insert("X-DashScope-WorkSpace", workspace);
    }
    Ok(request.map(|_| ()))
}

async fn decode_response(response: reqwest::Response) -> Result<JsonResponse, SafeError> {
    let status = response.status();
    if !status.is_success() {
        let (code, retryable, message) = status_error(status);
        return Err(task_error(code, message, retryable));
    }
    if response.content_length().is_some_and(|size| size > MAX_RESPONSE_BYTES as u64) {
        return Err(task_error(ErrorCode::ProviderError, "provider_response_too_large", false));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| task_error(ErrorCode::NetworkUnavailable, "provider_response_read_failed", true))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(task_error(ErrorCode::ProviderError, "provider_response_too_large", false));
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = serde_json::from_slice::<Value>(&bytes)
        .map_err(|_| task_error(ErrorCode::ProviderError, "provider_invalid_json_response", false))?;
    Ok(JsonResponse { status: status.as_u16(), body })
}

fn status_error(status: StatusCode) -> (ErrorCode, bool, &'static str) {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (ErrorCode::AuthRequired, false, "provider_auth_required"),
        StatusCode::TOO_MANY_REQUESTS => (ErrorCode::RateLimited, true, "provider_rate_limited"),
        value if value.is_server_error() => (ErrorCode::ProviderError, true, "provider_server_error"),
        _ => (ErrorCode::ProviderError, false, "provider_request_failed"),
    }
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

/// Official OpenAI-compatible Chat Completions body used by qwen3.8-flash.
/// The caller supplies an already bounded, source-reviewed input object; this
/// helper only handles the wire envelope and requests JSON output.
pub fn build_chat_completion_body(
    model: &str,
    _prompt_version: &str,
    system_instruction: &str,
    input: Value,
) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_instruction },
            { "role": "user", "content": [{ "type": "text", "text": serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string()) }] }
        ],
        "response_format": { "type": "json_object" }
    })
}

pub fn parse_chat_completion_json(value: &Value) -> Result<Value, SafeError> {
    let choice = value.get("choices").and_then(Value::as_array).and_then(|choices| choices.first()).ok_or_else(|| task_error(ErrorCode::InvalidResult, "chat_completion_choices_missing", false))?;
    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        return Err(task_error(ErrorCode::InvalidResult, "chat_completion_truncated", false));
    }
    let content = choice.get("message").and_then(|message| message.get("content")).and_then(Value::as_str).filter(|content| !content.trim().is_empty()).ok_or_else(|| task_error(ErrorCode::InvalidResult, "chat_completion_content_missing", false))?;
    let content = content.trim().strip_prefix("```json").and_then(|text| text.strip_suffix("```" )).map(str::trim).unwrap_or(content);
    serde_json::from_str(content).map_err(|_| task_error(ErrorCode::InvalidResult, "chat_completion_json_invalid", false))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncTaskAction {
    Fetch,
    Cancel,
}

pub fn build_async_task_endpoint(policy: &EndpointPolicy, provider_task_id: &str, action: AsyncTaskAction) -> Result<String, SafeError> {
    if policy.binding.model_id != ModelId::QwenAudio31AsrFlashFiletrans || provider_task_id.is_empty() || provider_task_id.contains('/') || provider_task_id.contains('?') || provider_task_id.contains('#') {
        return Err(invalid_argument("filetrans_task_endpoint_invalid"));
    }
    let base = policy.http_origin.trim_end_matches('/');
    let prefix = policy.http_path_prefix.trim_end_matches('/');
    let suffix = match action {
        AsyncTaskAction::Fetch => "",
        AsyncTaskAction::Cancel => "/cancel",
    };
    Ok(format!("{base}{prefix}/tasks/{provider_task_id}{suffix}"))
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
    fn endpoint_policy_binds_host_path_and_ws_upgrade_headers() {
        let binding = binding_for_model(ModelId::QwenAudio31AsrFlashStreaming);
        let policy = EndpointPolicy::new(binding.clone(), "https://beijing.example", "/v1/", "wss://beijing.example", "/v1/").unwrap();
        let request = build_ws_request("wss://beijing.example/v1/asr", &binding, &policy, "fixture-token").unwrap();
        assert_eq!(request.method(), "GET");
        assert!(request.headers().contains_key("upgrade"));
        assert_eq!(request.headers().get("authorization").unwrap(), "Bearer fixture-token");
        assert!(build_ws_request("wss://other.example/v1/asr", &binding, &policy, "fixture-token").is_err());
        assert!(build_ws_request("wss://beijing.example/other", &binding, &policy, "fixture-token").is_err());
        assert!(EndpointPolicy::new(binding.clone(), "https://api.example", "/v1/foo", "wss://api.example", "/v1/foo").unwrap().http_path_prefix == "/v1/foo");
        let segment_policy = EndpointPolicy::new(binding.clone(), "https://api.example", "/v1/foo", "wss://api.example", "/v1/foo").unwrap();
        assert!(validate_http_endpoint("https://api.example/v1/foo/bar", &binding, &segment_policy).is_ok());
        assert!(validate_http_endpoint("https://api.example/v1/foobar", &binding, &segment_policy).is_err());
        let official = EndpointPolicy::for_beijing(binding.clone(), "workspace123").unwrap();
        let official_request = build_ws_request("wss://workspace123.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference", &binding, &official, "fixture-token").unwrap();
        assert_eq!(official_request.headers().get("x-dashscope-workspace").unwrap(), "workspace123");
        assert_eq!(status_error(StatusCode::UNAUTHORIZED).0, ErrorCode::AuthRequired);
        assert_eq!(status_error(StatusCode::TOO_MANY_REQUESTS).0, ErrorCode::RateLimited);
    }

    #[test]
    fn task_id_parser_never_substitutes_local_identity() {
        let response = JsonResponse { status: 200, body: serde_json::json!({"output":{"task_id":"provider-123"}}) };
        assert_eq!(parse_remote_task_id(&response).unwrap(), "provider-123");
        let policy = EndpointPolicy::for_beijing(binding_for_model(ModelId::QwenAudio31AsrFlashFiletrans), "workspace123").unwrap();
        assert_eq!(build_async_task_endpoint(&policy, "provider-123", AsyncTaskAction::Fetch).unwrap(), "https://workspace123.cn-beijing.maas.aliyuncs.com/api/v1/tasks/provider-123");
        assert_eq!(build_async_task_endpoint(&policy, "provider-123", AsyncTaskAction::Cancel).unwrap(), "https://workspace123.cn-beijing.maas.aliyuncs.com/api/v1/tasks/provider-123/cancel");
    }

    #[test]
    fn chat_fixture_uses_exact_model_and_json_output_envelope() {
        let body = build_chat_completion_body("qwen3.8-flash", "fixture_v1", "Return JSON.", serde_json::json!({"x":1}));
        assert_eq!(body["model"], "qwen3.8-flash");
        assert_eq!(body["messages"][1]["content"][0]["type"], "text");
        assert_eq!(body["response_format"]["type"], "json_object");
        let parsed = parse_chat_completion_json(&crate::fixtures::qwen38_chat_completion()).unwrap();
        assert_eq!(parsed["ok"], true);
        assert!(parse_chat_completion_json(&serde_json::json!({"choices":[{"finish_reason":"length","message":{"content":"{}"}}]})).is_err());
    }
}

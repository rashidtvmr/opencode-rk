//! Bounded OpenAI Responses API client for native turns.

use std::time::Duration;

use opencode_rk_contracts::{
    MAX_ASSISTANT_ACTIVITY_FIELD_BYTES, MAX_ASSISTANT_REFERENCES, MAX_INLINE_PAYLOAD_BYTES,
    MAX_ASSISTANT_TOOL_FIELD_BYTES, MAX_REASONING_SUMMARY_BYTES,
};
use reqwest::redirect::Policy;
use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;

use crate::config::ProviderConfig;

/// Maximum transcript entries admitted to one provider request.
pub const MAX_RESPONSES_INPUT_MESSAGES: usize = 100;
/// Maximum UTF-8 transcript bytes admitted before JSON framing.
pub const MAX_RESPONSES_INPUT_BYTES: usize = 512 * 1024;
/// Hard cap on one upstream HTTP response body.
pub const MAX_RESPONSES_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsesRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResponsesInput {
    pub role: ResponsesRole,
    pub content: String,
}

impl ResponsesInput {
    #[must_use]
    pub fn new(role: ResponsesRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// One transcript item in a Responses request.
///
/// Text items serialize exactly like the legacy [`ResponsesInput`]
/// (`{role, content}`) so existing turns keep an identical wire shape; tool
/// items use the Responses function-call format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResponsesItem {
    Text {
        role: ResponsesRole,
        content: String,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    FunctionCallOutput {
        call_id: String,
        output: String,
    },
}

impl From<ResponsesInput> for ResponsesItem {
    fn from(value: ResponsesInput) -> Self {
        ResponsesItem::Text {
            role: value.role,
            content: value.content,
        }
    }
}

impl Serialize for ResponsesItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            ResponsesItem::Text { role, content } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("role", role)?;
                map.serialize_entry("content", content)?;
                map.end()
            }
            ResponsesItem::FunctionCall {
                call_id,
                name,
                arguments,
            } => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "function_call")?;
                map.serialize_entry("call_id", call_id)?;
                map.serialize_entry("name", name)?;
                map.serialize_entry("arguments", arguments)?;
                map.end()
            }
            ResponsesItem::FunctionCallOutput { call_id, output } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "function_call_output")?;
                map.serialize_entry("call_id", call_id)?;
                map.serialize_entry("output", output)?;
                map.end()
            }
        }
    }
}

/// One `function` tool advertised to the provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponsesTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ResponsesTool {
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

impl Serialize for ResponsesTool {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("type", "function")?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("description", &self.description)?;
        map.serialize_entry("parameters", &self.parameters)?;
        map.end()
    }
}

/// Build a Responses request payload from typed transcript items.
///
/// Shared by the legacy text clients and the agentic tool loop so request
/// shapes cannot drift. `tools` is omitted entirely when empty; `stream` is
/// omitted when false (matching the historical payloads). Callers add
/// client-specific extras (e.g. `max_output_tokens`) afterwards.
pub fn responses_request_payload<I: Serialize>(
    model: &str,
    reasoning_effort: &str,
    input: &[I],
    tools: &[ResponsesTool],
    stream: bool,
) -> Result<serde_json::Value, ResponsesError> {
    if model.trim().is_empty() {
        return Err(ResponsesError::EmptyModel);
    }
    if !matches!(
        reasoning_effort,
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(ResponsesError::UnsupportedReasoningEffort(
            reasoning_effort.to_owned(),
        ));
    }
    if input.len() > MAX_RESPONSES_INPUT_MESSAGES {
        return Err(ResponsesError::TooManyMessages {
            max: MAX_RESPONSES_INPUT_MESSAGES,
            actual: input.len(),
        });
    }
    let bytes = input
        .iter()
        .try_fold(0_usize, |total, item| {
            let len = serde_json::to_vec(item)
                .map(|value| value.len())
                .unwrap_or(usize::MAX);
            total.checked_add(len)
        })
        .unwrap_or(usize::MAX);
    if bytes > MAX_RESPONSES_INPUT_BYTES {
        return Err(ResponsesError::InputTooLarge {
            max: MAX_RESPONSES_INPUT_BYTES,
            actual: bytes,
        });
    }

    let mut payload = serde_json::json!({
        "model": model,
        "input": input,
        "reasoning": { "effort": reasoning_effort },
    });
    if !tools.is_empty() {
        payload["tools"] = serde_json::to_value(tools)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
    }
    if stream {
        payload["stream"] = serde_json::Value::Bool(true);
    }
    Ok(payload)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResponsesError {
    #[error("provider configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("provider credential is unavailable: {0}")]
    MissingCredential(String),
    #[error("model id must not be empty")]
    EmptyModel,
    #[error("reasoning effort is unsupported: {0}")]
    UnsupportedReasoningEffort(String),
    #[error("too many transcript messages: max {max}, actual {actual}")]
    TooManyMessages { max: usize, actual: usize },
    #[error("transcript exceeds {max} bytes: actual {actual}")]
    InputTooLarge { max: usize, actual: usize },
    #[error("provider request failed: {0}")]
    Request(String),
    #[error("provider returned HTTP {status}: {message}")]
    Upstream { status: u16, message: String },
    #[error("provider response exceeded {max} bytes")]
    ResponseTooLarge { max: usize },
    #[error("provider returned invalid JSON: {0}")]
    InvalidJson(String),
    #[error("provider response contained no assistant output text")]
    EmptyOutput,
    #[error("assistant output exceeds {max} bytes")]
    OutputTooLarge { max: usize },
    #[error("reasoning summary exceeds {max} bytes")]
    ReasoningSummaryTooLarge { max: usize },
    #[error("unsupported provider stream event: {0}")]
    UnsupportedStreamEvent(String),
    #[error("provider stream failed: {0}")]
    StreamFailed(String),
    #[error("provider stream ended before a completion event")]
    UnexpectedStreamEnd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResponsesStopReason {
    /// Model finished normally (`response.completed`).
    Completed,
    /// Model stopped early (`response.incomplete`), e.g. max_output_tokens
    /// or content filter. Standard OpenAI Responses termination cause.
    Incomplete { reason: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResponsesStreamEvent {
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    UrlCitation {
        label: String,
        url: String,
    },
    /// One provider-requested tool invocation (Responses `function_call` item).
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    /// Terminal event. Every well-formed stream ends here exactly once.
    Completed {
        stop_reason: ResponsesStopReason,
    },
}

/// Pure SSE event parser shared by the live stream and tests.
///
/// Extracted from `OpenAiResponsesStream` so terminal-event semantics
/// (stop reasons, function-call items) are testable without a TCP peer.
/// `event_name` is the SSE `event:` field when the peer sends one.
#[derive(Debug, Default)]
pub struct ResponsesStreamParser {
    output_bytes: usize,
    reasoning_summary_bytes: usize,
    references: usize,
    completed: bool,
}

impl ResponsesStreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_exhausted(&self) -> bool {
        self.completed
    }

    pub fn parse_event(
        &mut self,
        event_name: Option<&str>,
        event: &str,
    ) -> Result<Option<ResponsesStreamEvent>, ResponsesError> {
        self.parse_event_impl(event_name, event)
    }

    fn parse_event_impl(
        &mut self,
        event_name: Option<&str>,
        event: &str,
    ) -> Result<Option<ResponsesStreamEvent>, ResponsesError> {
        if self.completed {
            return Ok(None);
        }
        // Accept either raw SSE `data:` lines or a bare JSON payload.
        let mut data = String::new();
        let mut saw_data_prefix = false;
        for line in event.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with(':') {
                continue;
            }
            if let Some(value) = line.strip_prefix("data:") {
                saw_data_prefix = true;
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
            }
        }
        if !saw_data_prefix {
            data.push_str(event);
        }
        if data.is_empty() || data == "[DONE]" {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(&data)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let event_type = value.get("type").and_then(Value::as_str).or(event_name);
        match event_type {
            Some("response.output_text.delta") => {
                let delta = value.get("delta").and_then(Value::as_str).ok_or_else(|| {
                    ResponsesError::InvalidJson("stream delta is missing text".into())
                })?;
                self.output_bytes = self.output_bytes.saturating_add(delta.len());
                if self.output_bytes > MAX_INLINE_PAYLOAD_BYTES {
                    return Err(ResponsesError::OutputTooLarge {
                        max: MAX_INLINE_PAYLOAD_BYTES,
                    });
                }
                Ok(Some(ResponsesStreamEvent::OutputTextDelta(
                    delta.to_owned(),
                )))
            }
            Some("response.reasoning_summary_text.delta") => {
                let delta = value.get("delta").and_then(Value::as_str).ok_or_else(|| {
                    ResponsesError::InvalidJson("reasoning summary delta is missing text".into())
                })?;
                self.reasoning_summary_bytes =
                    self.reasoning_summary_bytes.saturating_add(delta.len());
                if self.reasoning_summary_bytes > MAX_REASONING_SUMMARY_BYTES {
                    return Err(ResponsesError::ReasoningSummaryTooLarge {
                        max: MAX_REASONING_SUMMARY_BYTES,
                    });
                }
                Ok(Some(ResponsesStreamEvent::ReasoningSummaryDelta(
                    delta.to_owned(),
                )))
            }
            Some("response.output_text.annotation.added") => {
                let annotation = value.get("annotation").ok_or_else(|| {
                    ResponsesError::InvalidJson("citation event is missing annotation".into())
                })?;
                if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
                    return Err(ResponsesError::InvalidJson(
                        "unsupported output-text annotation".into(),
                    ));
                }
                let url = annotation
                    .get("url")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ResponsesError::InvalidJson("URL citation is missing url".into())
                    })?;
                let label = annotation
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ResponsesError::InvalidJson("URL citation is missing title".into())
                    })?;
                let valid = |text: &str| {
                    !text.is_empty()
                        && text.as_bytes().len() <= MAX_ASSISTANT_ACTIVITY_FIELD_BYTES
                        && !text.chars().any(char::is_control)
                };
                if !valid(label) || !valid(url) || !url.starts_with("https://") {
                    return Err(ResponsesError::InvalidJson(
                        "URL citation contains invalid or over-bound fields".into(),
                    ));
                }
                self.references = self.references.saturating_add(1);
                if self.references > MAX_ASSISTANT_REFERENCES {
                    return Err(ResponsesError::InvalidJson(format!(
                        "too many URL citations: max {MAX_ASSISTANT_REFERENCES}"
                    )));
                }
                Ok(Some(ResponsesStreamEvent::UrlCitation {
                    label: label.to_owned(),
                    url: url.to_owned(),
                }))
            }
            Some("response.output_item.done") => {
                let item_type = value
                    .pointer("/item/type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if item_type != "function_call" {
                    return Ok(None);
                }
                let call_id = value
                    .pointer("/item/call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let name = value
                    .pointer("/item/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let arguments = value
                    .pointer("/item/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if call_id.is_empty() || name.is_empty() {
                    return Err(ResponsesError::InvalidJson(
                        "function_call item is missing call_id or name".into(),
                    ));
                }
                if call_id.as_bytes().len() > MAX_ASSISTANT_TOOL_FIELD_BYTES
                    || name.as_bytes().len() > MAX_ASSISTANT_TOOL_FIELD_BYTES
                    || call_id.chars().any(char::is_control)
                    || name.chars().any(char::is_control)
                {
                    return Err(ResponsesError::InvalidJson(
                        "function_call identity exceeds the durable activity bound".into(),
                    ));
                }
                Ok(Some(ResponsesStreamEvent::FunctionCall {
                    call_id,
                    name,
                    arguments,
                }))
            }
            Some("response.completed") => {
                self.completed = true;
                Ok(Some(ResponsesStreamEvent::Completed {
                    stop_reason: ResponsesStopReason::Completed,
                }))
            }
            Some("response.incomplete") => {
                self.completed = true;
                let reason = value
                    .pointer("/response/incomplete_details/reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Ok(Some(ResponsesStreamEvent::Completed {
                    stop_reason: ResponsesStopReason::Incomplete { reason },
                }))
            }
            Some("response.failed") | Some("error") => {
                let message = value
                    .pointer("/response/error/message")
                    .or_else(|| value.pointer("/error/message"))
                    .or_else(|| value.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("provider stream failed")
                    .to_owned();
                Err(ResponsesError::StreamFailed(message))
            }
            Some(
                "response.created"
                | "response.queued"
                | "response.in_progress"
                | "response.output_item.added"
                | "response.content_part.added"
                | "response.output_text.done"
                | "response.content_part.done"
                | "response.function_call_arguments.delta"
                | "response.function_call_arguments.done"
                | "response.reasoning_summary_part.added"
                | "response.reasoning_summary_part.done"
                | "response.reasoning_summary_text.done",
            ) => Ok(None),
            Some(other) => Err(ResponsesError::UnsupportedStreamEvent(other.to_owned())),
            None => Err(ResponsesError::InvalidJson(
                "provider stream event is missing type".to_owned(),
            )),
        }
    }
}

pub struct OpenAiResponsesStream {
    response: reqwest::Response,
    pending: Vec<u8>,
    received_bytes: usize,
    parser: ResponsesStreamParser,
}

/// One reusable, redirect-disabled OpenAI Responses client.
#[derive(Clone)]
pub struct OpenAiResponsesClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    max_output_tokens: u32,
}

impl OpenAiResponsesClient {
    pub fn from_env() -> Result<Self, ResponsesError> {
        let config = ProviderConfig::from_env("openai");
        config.validate().map_err(ResponsesError::InvalidConfig)?;
        let api_key = config
            .get_api_key()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ResponsesError::MissingCredential(config.api_key_env.clone()))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs.clamp(1, 300)))
            .redirect(Policy::none())
            .build()
            .map_err(|error| ResponsesError::InvalidConfig(error.to_string()))?;

        Ok(Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            api_key,
            max_output_tokens: config.max_tokens.clamp(1, 65_536),
        })
    }

    pub async fn create(
        &self,
        model: &str,
        reasoning_effort: &str,
        input: &[ResponsesInput],
    ) -> Result<String, ResponsesError> {
        let items: Vec<ResponsesItem> = input.iter().cloned().map(Into::into).collect();
        let mut payload = responses_request_payload(model, reasoning_effort, &items, &[], false)?;
        payload["max_output_tokens"] = json!(self.max_output_tokens);
        let mut response = self
            .http
            .post(format!("{}/responses", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| ResponsesError::Request(error.to_string()))?;
        let status = response.status();
        let body = read_bounded_body(&mut response).await?;

        if !status.is_success() {
            return Err(ResponsesError::Upstream {
                status: status.as_u16(),
                message: upstream_message(&body),
            });
        }

        let value: Value = serde_json::from_slice(&body)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        extract_output_text(&value)
    }

    pub async fn stream(
        &self,
        model: &str,
        reasoning_effort: &str,
        input: &[ResponsesInput],
    ) -> Result<OpenAiResponsesStream, ResponsesError> {
        let items: Vec<ResponsesItem> = input.iter().cloned().map(Into::into).collect();
        self.stream_with_tools(model, reasoning_effort, &items, &[])
            .await
    }

    /// Stream a turn that may advertise tools and carry typed transcript
    /// items (function calls and their outputs from earlier rounds).
    pub async fn stream_with_tools(
        &self,
        model: &str,
        reasoning_effort: &str,
        items: &[ResponsesItem],
        tools: &[ResponsesTool],
    ) -> Result<OpenAiResponsesStream, ResponsesError> {
        let mut payload = responses_request_payload(model, reasoning_effort, items, tools, true)?;
        payload["max_output_tokens"] = json!(self.max_output_tokens);
        payload["reasoning"]["summary"] = json!("auto");
        let mut response = self
            .http
            .post(format!("{}/responses", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| ResponsesError::Request(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = read_bounded_body(&mut response).await?;
            return Err(ResponsesError::Upstream {
                status: status.as_u16(),
                message: upstream_message(&body),
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSES_BODY_BYTES as u64)
        {
            return Err(ResponsesError::ResponseTooLarge {
                max: MAX_RESPONSES_BODY_BYTES,
            });
        }

        Ok(OpenAiResponsesStream {
            response,
            pending: Vec::new(),
            received_bytes: 0,
            parser: ResponsesStreamParser::new(),
        })
    }
}

impl OpenAiResponsesStream {
    pub async fn next_event(&mut self) -> Result<Option<ResponsesStreamEvent>, ResponsesError> {
        if self.parser.is_exhausted() {
            return Ok(None);
        }

        loop {
            if let Some(event) = take_sse_event(&mut self.pending) {
                if let Some(parsed) = self.parse_event(&event)? {
                    return Ok(Some(parsed));
                }
                continue;
            }

            match self
                .response
                .chunk()
                .await
                .map_err(|error| ResponsesError::Request(error.to_string()))?
            {
                Some(chunk) => {
                    self.received_bytes = self.received_bytes.saturating_add(chunk.len());
                    if self.received_bytes > MAX_RESPONSES_BODY_BYTES {
                        return Err(ResponsesError::ResponseTooLarge {
                            max: MAX_RESPONSES_BODY_BYTES,
                        });
                    }
                    self.pending.extend_from_slice(&chunk);
                }
                None => {
                    if self.pending.iter().any(|byte| !byte.is_ascii_whitespace()) {
                        return Err(ResponsesError::UnexpectedStreamEnd);
                    }
                    return Err(ResponsesError::UnexpectedStreamEnd);
                }
            }
        }
    }

    fn parse_event(
        &mut self,
        event: &[u8],
    ) -> Result<Option<ResponsesStreamEvent>, ResponsesError> {
        let event = std::str::from_utf8(event)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let mut event_name = None;
        let mut data = String::new();

        for line in event.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with(':') {
                continue;
            }
            if let Some(value) = line.strip_prefix("event:") {
                event_name = Some(value.trim());
            } else if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
            }
        }

        if data.is_empty() {
            return Ok(None);
        }
        self.parser.parse_event_impl(event_name, data.as_str())
    }
}

fn take_sse_event(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    let (index, delimiter_len) = match (lf, crlf) {
        (Some(left), Some(right)) => {
            if left.0 <= right.0 {
                left
            } else {
                right
            }
        }
        (Some(found), None) | (None, Some(found)) => found,
        (None, None) => return None,
    };

    let event = buffer[..index].to_vec();
    buffer.drain(..index + delimiter_len);
    Some(event)
}

async fn read_bounded_body(response: &mut reqwest::Response) -> Result<Vec<u8>, ResponsesError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSES_BODY_BYTES as u64)
    {
        return Err(ResponsesError::ResponseTooLarge {
            max: MAX_RESPONSES_BODY_BYTES,
        });
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ResponsesError::Request(error.to_string()))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSES_BODY_BYTES {
            return Err(ResponsesError::ResponseTooLarge {
                max: MAX_RESPONSES_BODY_BYTES,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn extract_output_text(value: &Value) -> Result<String, ResponsesError> {
    let mut text = String::new();
    for item in value
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) != Some("message")
            || item.get("role").and_then(Value::as_str) != Some("assistant")
        {
            continue;
        }
        for content in item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if content.get("type").and_then(Value::as_str) != Some("output_text") {
                continue;
            }
            let Some(chunk) = content.get("text").and_then(Value::as_str) else {
                continue;
            };
            if text.len().saturating_add(chunk.len()) > MAX_INLINE_PAYLOAD_BYTES {
                return Err(ResponsesError::OutputTooLarge {
                    max: MAX_INLINE_PAYLOAD_BYTES,
                });
            }
            text.push_str(chunk);
        }
    }
    if text.trim().is_empty() {
        return Err(ResponsesError::EmptyOutput);
    }
    Ok(text)
}

fn upstream_message(body: &[u8]) -> String {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "provider request failed".to_owned())
}

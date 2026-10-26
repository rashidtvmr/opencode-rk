
//! Bounded OpenAI Responses API client for native turns.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use opencode_rk_contracts::{MAX_INLINE_PAYLOAD_BYTES, MAX_REASONING_SUMMARY_BYTES};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::config::ProviderConfig;

/// Maximum transcript entries admitted to one provider request.
pub const MAX_RESPONSES_INPUT_MESSAGES: usize = 100;
/// Maximum UTF-8 transcript bytes admitted before JSON framing.
pub const MAX_RESPONSES_INPUT_BYTES: usize = 512 * 1024;
/// Hard cap on one upstream HTTP response body.
pub const MAX_RESPONSES_BODY_BYTES: usize = 1024 * 1024;
/// Hard cap on returned items in a standalone compacted context window.
pub const MAX_RESPONSES_COMPACT_OUTPUT_ITEMS: usize = 128;

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
    /// Opaque provider-owned state returned by native Responses compaction.
    ///
    /// The encrypted content is intentionally never interpreted locally.
    Compaction {
        id: String,
        encrypted_content: String,
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
            ResponsesItem::Compaction {
                id,
                encrypted_content,
            } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "compaction")?;
                map.serialize_entry("id", id)?;
                map.serialize_entry("encrypted_content", encrypted_content)?;
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
    if !reasoning_effort.is_empty()
        && !matches!(
            reasoning_effort,
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
        )
    {
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
    });
    // An omitted effort preserves the model/provider's default variant.
    if !reasoning_effort.is_empty() {
        payload["reasoning"] = json!({ "effort": reasoning_effort });
    }
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
    #[error("OpenAI OAuth refresh failed: {0}")]
    OAuthRefresh(String),
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
    #[error("provider transport protocol is unsupported: {0}")]
    UnsupportedProtocol(String),
    #[error("provider stream failed ({code}): {message}")]
    StreamFailed { code: String, message: String },
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
    usage: Option<ReportedUsage>,
    output_bytes: usize,
    reasoning_summary_bytes: usize,
    completed: bool,
}

/// Token counts exactly as the provider reported them for one turn.
///
/// `input_tokens` and `output_tokens` are taken only from values the
/// provider actually reported; when either is absent, null or malformed,
/// [`ReportedUsage::parse`] returns `None` and the whole usage stays
/// unreported instead of being synthesized. `cached_tokens` and
/// `reasoning_tokens` are `Option<u64>`: `Some(count)` only when the
/// provider reported the number verbatim (an explicit zero is preserved
/// as `Some(0)`); absent, null or malformed detail fields stay `None`
/// ("unreported") instead of being conflated with a provider-asserted
/// zero. Reported numbers pass through verbatim; the client never
/// "corrects" them, even when they look inconsistent. Serialization
/// emits `null` for unreported subsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedResponse {
    pub text: String,
    pub usage: Option<ReportedUsage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ReportedUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

/// Provider-reported count: a non-negative integer, tolerating JSON floats
/// with an integral value (`120.0`, how some compatible providers serialize
/// JS numbers). Anything else is "not reported".
fn reported_token(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if let Some(count) = value.as_u64() {
        return Some(count);
    }
    value
        .as_f64()
        .filter(|count| *count >= 0.0 && count.fract() == 0.0 && *count <= 9_007_199_254_740_991.0)
        .map(|count| count as u64)
}

impl ReportedUsage {
    fn parse(value: &Value) -> Option<Self> {
        let input_tokens = reported_token(value.get("input_tokens"))?;
        let output_tokens = reported_token(value.get("output_tokens"))?;
        Some(Self {
            input_tokens,
            output_tokens,
            cached_tokens: reported_token(value.pointer("/input_tokens_details/cached_tokens")),
            reasoning_tokens: reported_token(
                value.pointer("/output_tokens_details/reasoning_tokens"),
            ),
        })
    }
}

#[cfg(test)]
mod reported_usage_tests {
    use super::*;

    #[test]
    fn usage_is_provider_reported_and_subsets_are_not_invented() {
        let mut parser = ResponsesStreamParser::new();
        parser
            .parse_event(
                None,
                r#"{"type":"response.completed","response":{"usage":{"input_tokens":120,"output_tokens":30,"input_tokens_details":{"cached_tokens":40},"output_tokens_details":{"reasoning_tokens":10}}}}"#,
            )
            .unwrap();
        assert_eq!(
            parser.usage,
            Some(ReportedUsage {
                input_tokens: 120,
                output_tokens: 30,
                cached_tokens: Some(40),
                reasoning_tokens: Some(10),
            })
        );
        let mut missing = ResponsesStreamParser::new();
        missing
            .parse_event(None, r#"{"type":"response.completed","response":{}}"#)
            .unwrap();
        assert_eq!(missing.usage, None);
    }
}

impl ResponsesStreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Usage reported by the stream's terminal event, when the provider
    /// sent one. `None` means "provider did not report usage", never zero.
    pub fn usage(&self) -> Option<ReportedUsage> {
        self.usage
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
                Ok(Some(ResponsesStreamEvent::FunctionCall {
                    call_id,
                    name,
                    arguments,
                }))
            }
            Some("response.completed") => {
                self.usage = value
                    .pointer("/response/usage")
                    .and_then(ReportedUsage::parse);
                self.completed = true;
                Ok(Some(ResponsesStreamEvent::Completed {
                    stop_reason: ResponsesStopReason::Completed,
                }))
            }
            Some("response.incomplete") => {
                self.usage = value
                    .pointer("/response/usage")
                    .and_then(ReportedUsage::parse);
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
                let code = value
                    .pointer("/response/error/code")
                    .or_else(|| value.pointer("/error/code"))
                    .or_else(|| value.get("code"))
                    .and_then(Value::as_str)
                    .map(|code| {
                        code.chars()
                            .filter(|ch| !ch.is_control())
                            .take(256)
                            .collect::<String>()
                    })
                    .filter(|code| !code.is_empty())
                    .unwrap_or_else(|| "unknown".to_owned());
                let message = value
                    .pointer("/response/error/message")
                    .or_else(|| value.pointer("/error/message"))
                    .or_else(|| value.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("provider stream failed")
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(1024)
                    .collect();
                Err(ResponsesError::StreamFailed { code, message })
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
    buffered: bool,
    buffered_events: std::collections::VecDeque<ResponsesStreamEvent>,
    buffered_loaded: bool,
}

/// Validated, canonical context window returned by standalone compaction.
/// Output items stay opaque so message IDs, multimodal content, tool metadata,
/// future fields, and provider-owned encrypted state survive unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponsesCompactionWindow {
    /// Compact response ID, as returned by the API.
    pub id: String,
    /// Unix creation timestamp reported by the compact endpoint.
    pub created_at: u64,
    /// Provider-reported usage metadata, unchanged when present.
    pub usage: Option<Value>,
    /// Canonical compacted input window. Preserve every item and its order.
    pub output: Vec<Value>,
}

impl ResponsesCompactionWindow {
    /// Consume the response into the canonical next input window.
    pub fn into_output(self) -> Vec<Value> {
        self.output
    }
}

fn compaction_continuation_payload(
    model: &str,
    reasoning_effort: &str,
    window: &ResponsesCompactionWindow,
    tail: &[ResponsesItem],
    tools: &[ResponsesTool],
    stream: bool,
) -> Result<Value, ResponsesError> {
    let envelope = json!({
        "id": &window.id,
        "object": "response.compaction",
        "created_at": window.created_at,
        "usage": &window.usage,
        "output": &window.output,
    });
    let encoded = serde_json::to_vec(&envelope)
        .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
    parse_compaction_window(&encoded)?;

    let mut input = window.output.clone();
    input.extend(
        tail.iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?,
    );
    responses_request_payload(model, reasoning_effort, &input, tools, stream)
}

fn parse_compaction_window(body: &[u8]) -> Result<ResponsesCompactionWindow, ResponsesError> {
    if body.len() > MAX_RESPONSES_BODY_BYTES {
        return Err(ResponsesError::ResponseTooLarge {
            max: MAX_RESPONSES_BODY_BYTES,
        });
    }
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
    let invalid = || ResponsesError::InvalidJson("invalid Responses compaction object".into());
    if value.get("object").and_then(Value::as_str) != Some("response.compaction") {
        return Err(invalid());
    }
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
        return Err(invalid());
    }
    let created_at = value
        .get("created_at")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if output.len() > MAX_RESPONSES_COMPACT_OUTPUT_ITEMS {
        return Err(invalid());
    }
    let mut saw_compaction = false;
    for item in output {
        if !item.is_object() || item.get("type").and_then(Value::as_str).is_none() {
            return Err(invalid());
        }
        if item.get("type").and_then(Value::as_str) == Some("compaction") {
            saw_compaction = true;
            let item_id = item.get("id").and_then(Value::as_str).ok_or_else(invalid)?;
            let encrypted_content = item
                .get("encrypted_content")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if item_id.is_empty()
                || item_id.len() > 256
                || item_id.chars().any(char::is_control)
                || encrypted_content.is_empty()
                || encrypted_content.len() > MAX_INLINE_PAYLOAD_BYTES
            {
                return Err(invalid());
            }
        }
    }
    if !saw_compaction {
        return Err(invalid());
    }
    let usage = match value.get("usage") {
        None | Some(Value::Null) => None,
        Some(usage) if usage.is_object() => Some(usage.clone()),
        Some(_) => return Err(invalid()),
    };
    Ok(ResponsesCompactionWindow {
        id: id.to_owned(),
        created_at,
        usage,
        output: output.clone(),
    })
}

pub struct BufferedResponsesBody {
    events: Vec<ResponsesStreamEvent>,
    usage: Option<ReportedUsage>,
    output_items: Vec<Value>,
}
impl BufferedResponsesBody {
    pub fn parse(body: &str) -> Result<Self, ResponsesError> {
        if body.len() > MAX_RESPONSES_BODY_BYTES {
            return Err(ResponsesError::ResponseTooLarge {
                max: MAX_RESPONSES_BODY_BYTES,
            });
        }
        let value: Value = serde_json::from_str(body)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let kind = match value["status"].as_str() {
            Some("completed") => "response.completed",
            Some("incomplete") => "response.incomplete",
            Some("failed") => "response.failed",
            _ => return Err(ResponsesError::UnexpectedStreamEnd),
        };
        let mut parser = ResponsesStreamParser::new();
        let invalid = || ResponsesError::InvalidJson("invalid buffered Responses output".into());
        if value.get("error").is_some_and(|error| !error.is_null()) || kind == "response.failed" {
            let code = value
                .pointer("/error/code")
                .and_then(Value::as_str)
                .map(|code| {
                    code.chars()
                        .filter(|ch| !ch.is_control())
                        .take(256)
                        .collect::<String>()
                })
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| "unknown".to_owned());
            return Err(ResponsesError::StreamFailed {
                code,
                message: value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("provider response failed")
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(1024)
                    .collect(),
            });
        }
        let output = value["output"].as_array().ok_or_else(invalid)?;
        if output.len() > 128 {
            return Err(invalid());
        }
        let mut events = Vec::new();
        let mut calls = std::collections::BTreeSet::new();
        let mut argument_bytes: usize = 0;
        for item in output {
            // Compatible providers may omit item status. An explicit status
            // must agree with a completed envelope before any call can escape.
            if kind == "response.completed"
                && item
                    .get("status")
                    .is_some_and(|status| status.as_str() != Some("completed"))
            {
                return Err(invalid());
            }
            match item["type"].as_str() {
                Some("message") => {
                    if item["role"].as_str() != Some("assistant") {
                        return Err(invalid());
                    }
                    let content = item["content"].as_array().ok_or_else(invalid)?;
                    for part in content {
                        let text = match part["type"].as_str() {
                            Some("output_text") => part["text"].as_str(),
                            Some("refusal") => part["refusal"].as_str(),
                            _ => return Err(invalid()),
                        }
                        .ok_or_else(invalid)?;
                        if let Some(event) = parser.parse_event(
                            None,
                            &json!({"type":"response.output_text.delta","delta":text}).to_string(),
                        )? {
                            events.push(event);
                        }
                    }
                }
                Some("reasoning") => {
                    if let Some(summary) = item.get("summary") {
                        for part in summary.as_array().ok_or_else(invalid)? {
                            if part["type"].as_str() != Some("summary_text") {
                                return Err(invalid());
                            }
                            let text = part["text"].as_str().ok_or_else(invalid)?;
                            if let Some(event) = parser.parse_event(None, &json!({"type":"response.reasoning_summary_text.delta","delta":text}).to_string())? { events.push(event); }
                        }
                    }
                }
                Some("function_call") => {
                    if kind == "response.incomplete" {
                        continue;
                    }
                    let call_id = item["call_id"].as_str().ok_or_else(invalid)?;
                    let name = item["name"].as_str().ok_or_else(invalid)?;
                    let arguments = item["arguments"].as_str().ok_or_else(invalid)?;
                    if call_id.is_empty()
                        || call_id.len() > 256
                        || call_id.chars().any(char::is_control)
                        || name.is_empty()
                        || name.len() > 256
                        || name.chars().any(char::is_control)
                        || !calls.insert(call_id)
                    {
                        return Err(invalid());
                    }
                    argument_bytes = argument_bytes.saturating_add(arguments.len());
                    if argument_bytes > MAX_INLINE_PAYLOAD_BYTES {
                        return Err(ResponsesError::OutputTooLarge {
                            max: MAX_INLINE_PAYLOAD_BYTES,
                        });
                    }
                    let args: Value = serde_json::from_str(arguments).map_err(|_| invalid())?;
                    if !args.is_object() {
                        return Err(invalid());
                    }
                    // Incomplete output can contain partial calls. Never turn
                    // those into executable work; terminal semantics survive.
                    events.push(ResponsesStreamEvent::FunctionCall {
                        call_id: call_id.into(),
                        name: name.into(),
                        arguments: arguments.into(),
                    });
                }
                Some("compaction") => {
                    let id = item["id"].as_str().ok_or_else(invalid)?;
                    let encrypted_content =
                        item["encrypted_content"].as_str().ok_or_else(invalid)?;
                    if id.is_empty()
                        || id.len() > 256
                        || id.chars().any(char::is_control)
                        || encrypted_content.is_empty()
                        || encrypted_content.len() > MAX_INLINE_PAYLOAD_BYTES
                        || encrypted_content.chars().any(char::is_control)
                    {
                        return Err(invalid());
                    }
                    // Compaction is provider-owned opaque state. Retain the
                    // complete item, including future fields, but never turn
                    // it into executable work or an event.
                }
                _ => return Err(invalid()),
            }
        }
        if let Some(event) = parser.parse_event(
            Some(kind),
            &json!({"type": kind, "response": value}).to_string(),
        )? {
            events.push(event);
        }
        Ok(Self {
            events,
            usage: parser.usage,
            output_items: output.to_vec(),
        })
    }

    /// Usage reported by the buffered envelope, when the provider sent one.
    pub fn usage(&self) -> Option<ReportedUsage> {
        self.usage
    }

    pub fn into_events(self) -> Vec<ResponsesStreamEvent> {
        self.events
    }

    /// Complete output items exactly as returned by the Responses API.
    ///
    /// This is the continuation seam for provider-owned opaque items such as
    /// native compaction. Callers must preserve ordering and pass the items
    /// back unchanged; compaction items are never exposed as tool events.
    pub fn output_items(&self) -> &[Value] {
        &self.output_items
    }
}

/// One reusable, redirect-disabled OpenAI Responses client.
#[derive(Clone)]
pub struct OpenAiResponsesClient {
    http: reqwest::Client,
    base_url: String,
    credential: Arc<Mutex<RequestCredential>>,
    oauth_token_url: String,
    max_output_tokens: u32,
    buffered_model_ids: Vec<String>,
    /// Catalog-resolved provider model ID.  The public call still receives
    /// the user's qualified selection, but the wire payload uses this ID.
    model_id: Option<String>,
}

const OPENAI_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const OPENAI_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OAUTH_REFRESH_SKEW_MS: u64 = 60_000;

#[derive(Clone, Debug)]
enum RequestCredential {
    Anonymous,
    ApiKey(String),
    OAuth(crate::persisted_auth::PersistedOAuth),
}

#[derive(Debug, Deserialize)]
struct OAuthRefreshResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

/// Only unambiguous pre-execution admission failures are retried. Other 5xx
/// statuses stay non-retryable so callers see them immediately.
fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 503)
}

/// Integer `Retry-After` seconds, capped at 2s, never shorter than backoff.
fn retry_delay(attempt: u32, retry_after_secs: Option<u64>) -> Duration {
    let backoff = Duration::from_millis(200u64 << attempt.saturating_sub(1).min(2));
    retry_after_secs
        .map(|secs| Duration::from_millis(secs.saturating_mul(1000).min(2000)))
        .map_or(backoff, |retry_after| retry_after.max(backoff))
}

#[cfg(test)]
mod retry_policy_tests {
    use super::*;

    #[test]
    fn backoff_doubles_per_attempt() {
        assert_eq!(retry_delay(1, None), Duration::from_millis(200));
        assert_eq!(retry_delay(2, None), Duration::from_millis(400));
    }

    #[test]
    fn retry_after_is_capped_and_never_below_backoff() {
        assert_eq!(retry_delay(1, Some(1)), Duration::from_millis(1000));
        assert_eq!(retry_delay(2, Some(1)), Duration::from_millis(1000));
        assert_eq!(retry_delay(1, Some(9)), Duration::from_millis(2000));
        assert_eq!(retry_delay(2, Some(0)), Duration::from_millis(400));
    }

    #[test]
    fn only_429_and_503_are_retryable() {
        assert!(retryable_status(429));
        assert!(retryable_status(503));
        assert!(!retryable_status(500));
        assert!(!retryable_status(400));
    }
}

impl OpenAiResponsesClient {
    pub async fn from_persisted_env() -> Result<Self, ResponsesError> {
        let config = ProviderConfig::from_env("openai");
        config.validate().map_err(ResponsesError::InvalidConfig)?;
        if let Some(oauth) = crate::persisted_auth::load_oauth_credential("openai").await {
            return Self::from_oauth_config(config, oauth);
        }
        let key = crate::persisted_auth::api_key("openai", &config.api_key_env)
            .await
            .ok_or_else(|| ResponsesError::MissingCredential(config.api_key_env.clone()))?;
        Self::from_config(config, Some(key))
    }

    pub fn from_env() -> Result<Self, ResponsesError> {
        let config = ProviderConfig::from_env("openai");
        config.validate().map_err(ResponsesError::InvalidConfig)?;
        let api_key = config
            .get_api_key()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ResponsesError::MissingCredential(config.api_key_env.clone()))?;
        Self::from_config(config, Some(api_key))
    }

    pub fn from_config(
        config: ProviderConfig,
        api_key: Option<String>,
    ) -> Result<Self, ResponsesError> {
        config.validate().map_err(ResponsesError::InvalidConfig)?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs.clamp(1, 300)))
            .redirect(Policy::none())
            .build()
            .map_err(|error| ResponsesError::InvalidConfig(error.to_string()))?;

        Ok(Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            credential: Arc::new(Mutex::new(
                api_key.map_or(RequestCredential::Anonymous, RequestCredential::ApiKey),
            )),
            oauth_token_url: OPENAI_OAUTH_TOKEN_URL.to_owned(),
            max_output_tokens: config.max_tokens.clamp(1, 65_536),
            buffered_model_ids: Vec::new(),
            model_id: None,
        })
    }

    fn from_oauth_config(
        config: ProviderConfig,
        oauth: crate::persisted_auth::PersistedOAuth,
    ) -> Result<Self, ResponsesError> {
        Self::from_oauth_config_with_endpoint(config, oauth, OPENAI_OAUTH_TOKEN_URL)
    }

    /// Test/integration constructor retaining the production request path but
    /// directing the OAuth exchange to a disposable local fixture.
    pub fn from_oauth_config_with_endpoint(
        config: ProviderConfig,
        oauth: crate::persisted_auth::PersistedOAuth,
        oauth_token_url: impl Into<String>,
    ) -> Result<Self, ResponsesError> {
        config.validate().map_err(ResponsesError::InvalidConfig)?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs.clamp(1, 300)))
            .redirect(Policy::none())
            .build()
            .map_err(|error| ResponsesError::InvalidConfig(error.to_string()))?;
        let base_url = if config.base_url.trim_end_matches('/') == "https://api.openai.com/v1" {
            OPENAI_CODEX_BASE_URL.to_owned()
        } else {
            config.base_url.trim_end_matches('/').to_owned()
        };
        Ok(Self {
            http,
            base_url,
            credential: Arc::new(Mutex::new(RequestCredential::OAuth(oauth))),
            oauth_token_url: oauth_token_url.into(),
            max_output_tokens: config.max_tokens.clamp(1, 65_536),
            buffered_model_ids: Vec::new(),
            model_id: None,
        })
    }

    async fn credential_for_request(
        &self,
    ) -> Result<(Option<String>, Option<String>, bool), ResponsesError> {
        let mut credential = self.credential.lock().await;
        match &*credential {
            RequestCredential::Anonymous => Ok((None, None, false)),
            RequestCredential::ApiKey(key) => Ok((Some(key.clone()), None, false)),
            RequestCredential::OAuth(value)
                if value.expires_at > epoch_ms().saturating_add(OAUTH_REFRESH_SKEW_MS) =>
            {
                Ok((
                    Some(value.access_token.clone()),
                    value.account_id.clone(),
                    true,
                ))
            }
            RequestCredential::OAuth(value) => {
                let response = self
                    .http
                    .post(&self.oauth_token_url)
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .form(&[
                        ("grant_type", "refresh_token"),
                        ("refresh_token", value.refresh_token.as_str()),
                        ("client_id", OPENAI_OAUTH_CLIENT_ID),
                    ])
                    .send()
                    .await
                    .map_err(|error| ResponsesError::OAuthRefresh(error.to_string()))?;
                if !response.status().is_success() {
                    return Err(ResponsesError::OAuthRefresh(format!(
                        "refresh rejected with HTTP {}",
                        response.status()
                    )));
                }
                let token: OAuthRefreshResponse = response
                    .json()
                    .await
                    .map_err(|error| ResponsesError::OAuthRefresh(error.to_string()))?;
                let access = token
                    .access_token
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ResponsesError::OAuthRefresh(
                            "refresh response missing access_token".to_owned(),
                        )
                    })?;
                let refresh = token
                    .refresh_token
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| value.refresh_token.clone());
                let expires = epoch_ms()
                    .saturating_add(token.expires_in.unwrap_or(3600).saturating_mul(1000));
                crate::persisted_auth::save_refreshed_oauth(
                    "openai".to_owned(),
                    access.clone(),
                    refresh.clone(),
                    expires,
                    value.account_id.clone(),
                    value.enterprise_url.clone(),
                )
                .await
                .map_err(ResponsesError::OAuthRefresh)?;
                let account_id = value.account_id.clone();
                let enterprise_url = value.enterprise_url.clone();
                *credential = RequestCredential::OAuth(crate::persisted_auth::PersistedOAuth {
                    access_token: access.clone(),
                    refresh_token: refresh,
                    expires_at: expires,
                    account_id: account_id.clone(),
                    enterprise_url,
                });
                Ok((Some(access), account_id, true))
            }
        }
    }

    /// Apply models.dev transport resolution without changing credential
    /// handling.  The endpoint is validated by `ProviderConfig` before this
    /// method is called; the model ID is deliberately kept separate from the
    /// provider config because it is selected per request.
    pub fn with_resolved_transport(
        mut self,
        api: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        if let Some(api) = api.filter(|value| !value.trim().is_empty()) {
            self.base_url = api.trim_end_matches('/').to_owned();
        }
        self.model_id = model_id.filter(|value| !value.trim().is_empty());
        self
    }

    fn resolved_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.model_id.as_deref().unwrap_or(requested)
    }

    /// Explicit capability selection, evaluated per request before dispatch.
    /// This never retries an SSE failure or replays a partially executed round.
    pub fn with_buffered_models(mut self, models: Vec<String>) -> Self {
        self.buffered_model_ids = models;
        self
    }

    /// Dispatch one already-built payload, retrying only 429/503 up to three
    /// attempts. Network errors and all other non-success statuses are returned
    /// immediately; success returns before any body is consumed.
    async fn send_with_retry(&self, payload: &Value) -> Result<reqwest::Response, ResponsesError> {
        self.send_with_retry_path("responses", payload).await
    }

    async fn send_with_retry_path(
        &self,
        path: &str,
        payload: &Value,
    ) -> Result<reqwest::Response, ResponsesError> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut request = self
                .http
                .post(format!("{}/{path}", self.base_url))
                .json(payload);
            let (token, account_id, oauth) = self.credential_for_request().await?;
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            if oauth {
                request = request.header("originator", "opencode");
                if let Some(account_id) = account_id {
                    request = request.header("chatgpt-account-id", account_id);
                }
            }
            let response = request
                .send()
                .await
                .map_err(|error| ResponsesError::Request(error.to_string()))?;
            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            let retryable = retryable_status(status.as_u16());
            if retryable && attempt < MAX_ATTEMPTS {
                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.trim().parse::<u64>().ok());
                tokio::time::sleep(retry_delay(attempt, retry_after)).await;
                continue;
            }
            let mut response = response;
            let body = read_bounded_body(&mut response).await?;
            let message = upstream_message(&body);
            let message = if retryable {
                format!("{message} (retry exhausted after {MAX_ATTEMPTS} attempts)")
            } else {
                message
            };
            return Err(ResponsesError::Upstream {
                status: status.as_u16(),
                message,
            });
        }
    }

    /// Compact a complete native Responses input window through OpenAI's
    /// stateless `POST /responses/compact` endpoint. Callers must select this
    /// capability explicitly; this method does not infer provider support.
    pub async fn compact(
        &self,
        model: &str,
        input: &[ResponsesItem],
    ) -> Result<ResponsesCompactionWindow, ResponsesError> {
        if model.len() > MAX_RESPONSES_INPUT_BYTES {
            return Err(ResponsesError::InputTooLarge {
                max: MAX_RESPONSES_INPUT_BYTES,
                actual: model.len(),
            });
        }
        let payload = responses_request_payload(self.resolved_model(model), "", input, &[], false)?;
        self.compact_payload(&payload).await
    }

    /// Recompact the provider-owned window and complete typed tail without
    /// converting opaque items into local transcript records.
    pub async fn compact_with_compaction_window(
        &self,
        model: &str,
        window: &ResponsesCompactionWindow,
        tail: &[ResponsesItem],
    ) -> Result<ResponsesCompactionWindow, ResponsesError> {
        let payload = compaction_continuation_payload(
            self.resolved_model(model),
            "",
            window,
            tail,
            &[],
            false,
        )?;
        self.compact_payload(&payload).await
    }

    async fn compact_payload(
        &self,
        payload: &Value,
    ) -> Result<ResponsesCompactionWindow, ResponsesError> {
        let request_bytes = serde_json::to_vec(payload)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        if request_bytes.len() > MAX_RESPONSES_INPUT_BYTES {
            return Err(ResponsesError::InputTooLarge {
                max: MAX_RESPONSES_INPUT_BYTES,
                actual: request_bytes.len(),
            });
        }
        let mut response = self
            .send_with_retry_path("responses/compact", payload)
            .await?;
        let body = read_bounded_body(&mut response).await?;
        parse_compaction_window(&body)
    }

    pub async fn create(
        &self,
        model: &str,
        reasoning_effort: &str,
        input: &[ResponsesInput],
    ) -> Result<String, ResponsesError> {
        let items: Vec<ResponsesItem> = input.iter().cloned().map(Into::into).collect();
        self.create_with_items(model, reasoning_effort, &items)
            .await
    }

    /// Non-streaming turns retain typed call/result pairs from durable history.
    pub async fn create_with_items(
        &self,
        model: &str,
        reasoning_effort: &str,
        items: &[ResponsesItem],
    ) -> Result<String, ResponsesError> {
        Ok(self
            .create_with_items_and_usage(model, reasoning_effort, items)
            .await?
            .text)
    }

    /// Non-streaming response with provider-authoritative usage metadata.
    pub async fn create_with_items_and_usage(
        &self,
        model: &str,
        reasoning_effort: &str,
        items: &[ResponsesItem],
    ) -> Result<CompletedResponse, ResponsesError> {
        let mut payload = responses_request_payload(model, reasoning_effort, items, &[], false)?;
        payload["max_output_tokens"] = json!(self.max_output_tokens);
        let mut response = self.send_with_retry(&payload).await?;
        let body = read_bounded_body(&mut response).await?;
        let text = std::str::from_utf8(&body)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let value: Value = serde_json::from_str(text)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let parsed = BufferedResponsesBody::parse(text)?;
        Ok(CompletedResponse {
            text: extract_output_text(&value)?,
            usage: parsed.usage(),
        })
    }

    /// Generate a bounded, tool-free textual summary through the ordinary
    /// Responses API. Native context, when present, stays provider-owned.
    pub async fn summarize_with_context(
        &self,
        model: &str,
        window: Option<&ResponsesCompactionWindow>,
        items: &[ResponsesItem],
    ) -> Result<String, ResponsesError> {
        self.summarize_with_context_and_reasoning(model, "", window, items)
            .await
    }

    pub async fn summarize_with_context_and_reasoning(
        &self,
        model: &str,
        reasoning_effort: &str,
        window: Option<&ResponsesCompactionWindow>,
        items: &[ResponsesItem],
    ) -> Result<String, ResponsesError> {
        let mut payload = match window {
            Some(window) => compaction_continuation_payload(
                self.resolved_model(model),
                reasoning_effort,
                window,
                items,
                &[],
                false,
            )?,
            None => responses_request_payload(
                self.resolved_model(model),
                reasoning_effort,
                items,
                &[],
                false,
            )?,
        };
        payload["max_output_tokens"] = json!(self.max_output_tokens.min(4096));
        payload["tools"] = json!([]);
        payload["stream"] = json!(false);
        let mut response = self.send_with_retry(&payload).await?;
        let body = read_bounded_body(&mut response).await?;
        let value: Value = serde_json::from_slice(&body)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        if value["status"].as_str() != Some("completed")
            || value.get("error").is_some_and(|error| !error.is_null())
            || value["output"].as_array().is_none_or(|output| {
                output
                    .iter()
                    .any(|item| !matches!(item["type"].as_str(), Some("message" | "reasoning")))
            })
        {
            return Err(ResponsesError::InvalidJson(
                "summary must be a completed tool-free response".into(),
            ));
        }
        // Validate roles, part types, item statuses and UTF-8 before accepting
        // text. Partial or tool-bearing output must never replace a checkpoint.
        BufferedResponsesBody::parse(
            std::str::from_utf8(&body)
                .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?,
        )?;
        extract_output_text(&value)
    }

    /// Continue from standalone compaction output without converting opaque
    /// provider-owned items into the narrower local transcript enum.
    pub async fn create_with_compaction_window(
        &self,
        model: &str,
        reasoning_effort: &str,
        window: &ResponsesCompactionWindow,
        tail: &[ResponsesItem],
    ) -> Result<String, ResponsesError> {
        Ok(self
            .create_with_compaction_window_and_usage(model, reasoning_effort, window, tail)
            .await?
            .text)
    }

    pub async fn create_with_compaction_window_and_usage(
        &self,
        model: &str,
        reasoning_effort: &str,
        window: &ResponsesCompactionWindow,
        tail: &[ResponsesItem],
    ) -> Result<CompletedResponse, ResponsesError> {
        let mut payload = compaction_continuation_payload(
            self.resolved_model(model),
            reasoning_effort,
            window,
            tail,
            &[],
            false,
        )?;
        payload["max_output_tokens"] = json!(self.max_output_tokens);
        let mut response = self.send_with_retry(&payload).await?;
        let body = read_bounded_body(&mut response).await?;
        let text = std::str::from_utf8(&body)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let value: Value = serde_json::from_str(text)
            .map_err(|error| ResponsesError::InvalidJson(error.to_string()))?;
        let parsed = BufferedResponsesBody::parse(text)?;
        Ok(CompletedResponse {
            text: extract_output_text(&value)?,
            usage: parsed.usage(),
        })
    }

    async fn create_with_payload(&self, payload: &Value) -> Result<String, ResponsesError> {
        let mut response = self.send_with_retry(payload).await?;
        let body = read_bounded_body(&mut response).await?;
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
        let wire_model = self.resolved_model(model);
        let buffered = self
            .buffered_model_ids
            .iter()
            .any(|id| id == model || id == wire_model);
        let payload =
            responses_request_payload(wire_model, reasoning_effort, items, tools, !buffered)?;
        self.stream_with_payload(model, buffered, payload).await
    }

    /// Stream from a standalone compacted window while retaining each opaque
    /// provider output item exactly as returned.
    pub async fn stream_with_compaction_window(
        &self,
        model: &str,
        reasoning_effort: &str,
        window: &ResponsesCompactionWindow,
        tail: &[ResponsesItem],
        tools: &[ResponsesTool],
    ) -> Result<OpenAiResponsesStream, ResponsesError> {
        let wire_model = self.resolved_model(model);
        let buffered = self
            .buffered_model_ids
            .iter()
            .any(|id| id == model || id == wire_model);
        let payload = compaction_continuation_payload(
            wire_model,
            reasoning_effort,
            window,
            tail,
            tools,
            !buffered,
        )?;
        self.stream_with_payload(model, buffered, payload).await
    }

    async fn stream_with_payload(
        &self,
        _model: &str,
        buffered: bool,
        mut payload: Value,
    ) -> Result<OpenAiResponsesStream, ResponsesError> {
        if buffered {
            payload["stream"] = json!(false);
        }
        payload["max_output_tokens"] = json!(self.max_output_tokens);
        payload["reasoning"]["summary"] = json!("auto");
        let response = self.send_with_retry(&payload).await?;
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
            buffered,
            buffered_events: std::collections::VecDeque::new(),
            buffered_loaded: false,
        })
    }
}

impl OpenAiResponsesStream {
    pub fn usage(&self) -> Option<ReportedUsage> {
        self.parser.usage
    }

    pub async fn next_event(&mut self) -> Result<Option<ResponsesStreamEvent>, ResponsesError> {
        if self.buffered {
            if !self.buffered_loaded {
                // Buffer inside the cancellable event driver, not admission.
                // Fully validate before a single tool call becomes observable.
                self.buffered_loaded = true;
                let body = read_bounded_body(&mut self.response).await?;
                let text = std::str::from_utf8(&body)
                    .map_err(|_| ResponsesError::InvalidJson("invalid buffered UTF-8".into()))?;
                let buffered = BufferedResponsesBody::parse(text)?;
                self.parser.usage = buffered.usage();
                self.buffered_events = buffered.into_events().into();
            }
            return Ok(self.buffered_events.pop_front());
        }
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

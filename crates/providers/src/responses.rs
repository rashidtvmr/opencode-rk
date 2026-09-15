//! Bounded OpenAI Responses API client for native turns.

use std::time::Duration;

use opencode_rk_contracts::MAX_INLINE_PAYLOAD_BYTES;
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

#[derive(Debug, Error)]
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
        validate_request(model, reasoning_effort, input)?;

        let payload = json!({
            "model": model,
            "input": input,
            "reasoning": { "effort": reasoning_effort },
            "max_output_tokens": self.max_output_tokens,
        });
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
}

fn validate_request(
    model: &str,
    reasoning_effort: &str,
    input: &[ResponsesInput],
) -> Result<(), ResponsesError> {
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
        .try_fold(0_usize, |total, message| {
            total.checked_add(message.content.len())
        })
        .unwrap_or(usize::MAX);
    if bytes > MAX_RESPONSES_INPUT_BYTES {
        return Err(ResponsesError::InputTooLarge {
            max: MAX_RESPONSES_INPUT_BYTES,
            actual: bytes,
        });
    }
    Ok(())
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

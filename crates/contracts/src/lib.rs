//! Runtime-neutral domain and wire contracts shared by the OpenCode RK core.
#![forbid(unsafe_code)]
pub mod events;
pub mod opencode_event;

use std::{collections::BTreeMap, fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub const WIRE_SCHEMA_VERSION: u16 = 1;
pub const MAX_TITLE_BYTES: usize = 512;
pub const MAX_ERROR_MESSAGE_BYTES: usize = 8 * 1024;
pub const MAX_INLINE_PAYLOAD_BYTES: usize = 64 * 1024;
pub const MAX_DRAFT_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DRAFT_ATTACHMENTS: usize = 8;
pub const MAX_ATTACHMENT_NAME_BYTES: usize = 255;
pub const MAX_ATTACHMENT_MIME_BYTES: usize = 255;
/// Provider-visible reasoning summaries are transcript metadata, not hidden chain of thought.
/// Keep them small enough to fit one format-2 inline message part.
pub const MAX_REASONING_SUMMARY_BYTES: usize = 8 * 1024;
/// Maximum durable tool lifecycle records attached to one assistant message.
pub const MAX_ASSISTANT_TOOL_CALLS: usize = 128;
/// Maximum durable references attached to one assistant message.
pub const MAX_ASSISTANT_REFERENCES: usize = 64;
/// Maximum UTF-8 bytes in one durable tool identifier or name.
pub const MAX_ASSISTANT_TOOL_FIELD_BYTES: usize = 128;
/// Maximum UTF-8 bytes in one reference label or URL.
pub const MAX_ASSISTANT_ACTIVITY_FIELD_BYTES: usize = 1024;
pub const MAX_ARTIFACT_CONTENT_BYTES: usize = 64 * 1024;
pub const MAX_ARTIFACT_TITLE_BYTES: usize = 256;
pub const MAX_ARTIFACT_LANGUAGE_BYTES: usize = 64;
pub const MAX_ARTIFACTS_PER_SESSION: usize = 32;
pub const MAX_ARTIFACT_VERSIONS: usize = 16;
pub const MAX_ARTIFACT_TOTAL_BYTES: usize = 1024 * 1024;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl FromStr for $name {
            type Err = uuid::Error;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

uuid_id!(SessionId);
uuid_id!(MessageId);
uuid_id!(AgentId);
uuid_id!(ToolCallId);
uuid_id!(ApprovalId);
uuid_id!(AttachmentId);
uuid_id!(ArtifactId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);
impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        validate_slug("provider", value.into()).map(Self)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(String);
impl ModelId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err(ContractError::InvalidIdentifier {
                kind: "model",
                value,
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);
impl Timestamp {
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }
    #[must_use]
    pub const fn from_datetime(value: DateTime<Utc>) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
    #[must_use]
    pub fn to_rfc3339(self) -> String {
        self.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
}
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Active,
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub state: SessionState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub archived_at: Option<Timestamp>,
}
impl SessionSummary {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_bounded_text("title", &self.title, MAX_TITLE_BYTES)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub enum PayloadRef {
    Inline { text: String },
    Blob { hash: String, bytes: u64 },
}
impl PayloadRef {
    pub fn inline(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        validate_bounded_text("inline payload", &text, MAX_INLINE_PAYLOAD_BYTES)?;
        Ok(Self::Inline { text })
    }
    #[must_use]
    pub fn blob(hash: impl Into<String>, bytes: u64) -> Self {
        Self::Blob {
            hash: hash.into(),
            bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: MessageId,
    pub session_id: SessionId,
    pub role: MessageRole,
    pub body: PayloadRef,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DraftAttachment {
    pub id: AttachmentId,
    pub session_id: SessionId,
    pub name: String,
    pub mime: String,
    pub hash: String,
    pub bytes: u64,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssistantActivity {
    pub message_id: MessageId,
    pub reasoning_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<AssistantToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<AssistantReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssistantToolCall {
    pub call_id: String,
    pub name: String,
    pub state: String,
    pub ok: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssistantReference {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Writing,
    Code,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: ArtifactId,
    pub session_id: SessionId,
    pub source_message_id: MessageId,
    pub kind: ArtifactKind,
    pub title: String,
    pub language: Option<String>,
    pub current_version: u16,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactVersion {
    pub version: u16,
    pub bytes: u64,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDocument {
    #[serde(flatten)]
    pub summary: ArtifactSummary,
    pub content: String,
    pub versions: Vec<ArtifactVersion>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventCursor(u64);
impl EventCursor {
    #[must_use]
    pub const fn new(sequence: u64) -> Self {
        Self(sequence)
    }
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionedEnvelope<T> {
    pub schema_version: u16,
    pub data: T,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
impl<T> VersionedEnvelope<T> {
    #[must_use]
    pub fn current(data: T) -> Self {
        Self {
            schema_version: WIRE_SCHEMA_VERSION,
            data,
            extensions: BTreeMap::new(),
        }
    }
    pub fn validate_version(&self) -> Result<(), ContractError> {
        if self.schema_version == WIRE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(ContractError::UnsupportedSchemaVersion {
                expected: WIRE_SCHEMA_VERSION,
                actual: self.schema_version,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub native_core: bool,
    pub embedded_sqlite: bool,
    pub js_compatibility_host: bool,
    pub os_sandbox_backend: Option<String>,
    pub feature_profile: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub schema_version: u16,
    pub version: String,
    pub capabilities: CapabilityReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}
impl ApiError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Result<Self, ContractError> {
        let message = message.into();
        validate_bounded_text("error message", &message, MAX_ERROR_MESSAGE_BYTES)?;
        Ok(Self {
            code: code.into(),
            message,
            retryable,
            details: BTreeMap::new(),
        })
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ContractError {
    #[error("{field} exceeds {max_bytes} bytes")]
    TextTooLarge {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("invalid {kind} identifier: {value}")]
    InvalidIdentifier { kind: &'static str, value: String },
    #[error("unsupported schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { expected: u16, actual: u16 },
}

pub fn validate_bounded_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), ContractError> {
    if value.len() > max_bytes {
        Err(ContractError::TextTooLarge { field, max_bytes })
    } else {
        Ok(())
    }
}
fn validate_slug(kind: &'static str, value: String) -> Result<String, ContractError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(value)
    } else {
        Err(ContractError::InvalidIdentifier { kind, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additive_unknown_fields_survive_round_trip() {
        let json = r#"{"schema_version":1,"data":{"native_core":true,"embedded_sqlite":true,"js_compatibility_host":false,"os_sandbox_backend":null,"feature_profile":"lean"},"future_field":{"enabled":true}}"#;
        let envelope: VersionedEnvelope<CapabilityReport> = serde_json::from_str(json).unwrap();
        envelope.validate_version().unwrap();
        assert!(envelope.extensions.contains_key("future_field"));
        assert!(serde_json::to_value(envelope)
            .unwrap()
            .get("future_field")
            .is_some());
    }
    #[test]
    fn incompatible_version_fails_explicitly() {
        let envelope = VersionedEnvelope {
            schema_version: WIRE_SCHEMA_VERSION + 1,
            data: (),
            extensions: BTreeMap::new(),
        };
        assert!(matches!(
            envelope.validate_version(),
            Err(ContractError::UnsupportedSchemaVersion { .. })
        ));
    }
    #[test]
    fn inline_payload_is_bounded() {
        assert!(PayloadRef::inline("x".repeat(MAX_INLINE_PAYLOAD_BYTES + 1)).is_err());
    }
    #[test]
    fn provider_ids_reject_path_characters() {
        assert!(ProviderId::new("openai").is_ok());
        assert!(ProviderId::new("../../etc/passwd").is_err());
    }
}

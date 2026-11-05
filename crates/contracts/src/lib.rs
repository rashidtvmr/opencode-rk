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
    /// Construct from an epoch-millisecond value — the `DateTimeUtcFromMillis`
    /// wire form used by `session.view` (`packages/protocol/src/groups/
    /// session.ts:805-815`). `None` when the value is outside the representable
    /// datetime range.
    #[must_use]
    pub fn from_epoch_millis(millis: i64) -> Option<Self> {
        DateTime::from_timestamp_millis(millis).map(Self)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileDiffStatus {
    Added,
    Deleted,
    Modified,
}

/// Mirrors upstream `FileDiff.Info` (`packages/schema/src/file-diff.ts:6-12`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileDiffInfo {
    pub file: String,
    pub patch: String,
    pub additions: u32,
    pub deletions: u32,
    pub status: FileDiffStatus,
}

/// Mirrors upstream `Session.Revert` (`packages/schema/src/session-revert.ts:7-14`):
/// `messageID` required; `partID`, `snapshot` and `files` optional.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionRevert {
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    #[serde(rename = "partID", default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileDiffInfo>>,
    /// Real count of user messages from the revert boundary onward, computed
    /// server-side (`stage_session_revert`) and carried to the TUI revert card
    /// (`index.tsx:1561` counts `messagesFromRevert().filter(user).length`).
    /// Optional and skipped when absent so older persisted rows still decode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub state: SessionState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub archived_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<SessionRevert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewed_at: Option<Timestamp>,
    /// TAB-21: the session's last idle transition (`time_idle` upstream,
    /// `projector.ts:423-447`): written when a turn reaches a terminal state.
    /// The unread predicate is `idle_at.is_some() && (viewed_at.is_none() ||
    /// idle_at > viewed_at)` (`session-tabs.tsx:120-123`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_at: Option<Timestamp>,
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
    /// Local extension: a durable compaction marker row rendered in the
    /// transcript. Never a provider history role (rejected before replay).
    Compaction,
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

/// MSG-005: one attachment chip on a user card. Upstream `Prompt.FileAttachment`
/// (`packages/schema/src/prompt.ts:33-38`) carries the name/mime the chip shows;
/// the record keeps only that metadata plus an optional content hash. Base64/raw
/// bytes are never persisted here or in the transcript wire.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageFile {
    pub name: String,
    pub mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// MSG-005: one skill chip on a user card. Upstream `Prompt.SkillAttachment`
/// (`packages/schema/src/prompt.ts:64-69`) resolves to `{ id, name }`
/// (`packages/core/src/session/prompt.ts:57-72`); only those two facts ride the
/// record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageSkill {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: MessageId,
    pub session_id: SessionId,
    pub role: MessageRole,
    pub body: PayloadRef,
    pub created_at: Timestamp,
    /// MSG-005: message-level attachment chips (`Session.Message.User.files`,
    /// `packages/schema/src/session-message.ts:76-84`). Chips are message
    /// facts, not transcript parts. Empty for every non-user or chip-less row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<MessageFile>,
    /// MSG-005: message-level skill chips (`Session.Message.User.skills`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<MessageSkill>,
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
}

/// Execution facts a producer observed for one assistant message.
///
/// Every field is optional: a producer writes only what the turn actually
/// reported (`usage` counters and the terminal `stop_reason`), and a missing
/// value stays absent rather than defaulting to zero. `provider_id`/`model_id`
/// are the qualified model the turn ran on, captured in turn scope — they are
/// not inferable from the message body or the currently selected UI model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageExecution {
    pub message_id: MessageId,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub stop_reason: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    /// MSG-008: provider-active streamed time in milliseconds — the first output
    /// delta's timestamp minus the step's creation timestamp, matching upstream
    /// `time.streamed - time.created` (`routes/session/rows.ts:451-453`). `None`
    /// when the producer observed no first-delta stamp, so throughput stays
    /// absent rather than fabricated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamed_ms: Option<u64>,
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

/// Form identifiers use the upstream `frm_` namespace.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FormId(String);
impl FormId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if !value.starts_with("frm_") || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(ContractError::InvalidIdentifier { kind: "form", value });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn generate() -> Self {
        Self(format!("frm_{}", Uuid::new_v4().simple()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}
impl fmt::Display for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

/// User-facing form field, matching the server-produced OpenCode Form.Field schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub key: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(flatten)]
    pub properties: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormInfo {
    pub id: FormId,
    pub session_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, Value>>,
    pub fields: Vec<FormField>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormStatus { Pending, Answered, Cancelled }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormState {
    pub status: FormStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormDetail {
    #[serde(flatten)]
    pub form: FormInfo,
    pub state: FormState,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormReply { pub answer: BTreeMap<String, Value> }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormCancel { #[serde(default, skip_serializing_if = "Option::is_none")] pub message: Option<String> }

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
    fn form_contract_round_trip_and_id_validation() {
        let form = FormInfo {
            id: FormId::new("frm_example").unwrap(),
            session_id: "ses_test".into(),
            title: "Questions".into(),
            metadata: None,
            fields: vec![FormField { key: "q0".into(), field_type: "string".into(), properties: BTreeMap::new() }],
        };
        let encoded = serde_json::to_string(&form).unwrap();
        let decoded: FormInfo = serde_json::from_str(&encoded).unwrap();
        assert_eq!(form, decoded);
        assert!(FormId::new("invalid").is_err());
        assert_eq!(serde_json::to_value(FormStatus::Pending).unwrap(), "pending");
    }
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
    #[test]
    fn session_summary_without_revert_omits_wire_key() {
        let json = r#"{"id":"00000000-0000-4000-8000-000000000001","title":"t","state":"active","created_at":"2024-01-01T00:00:00.000Z","updated_at":"2024-01-01T00:00:00.000Z","archived_at":null}"#;
        let summary: SessionSummary = serde_json::from_str(json).unwrap();
        assert!(summary.revert.is_none());
        let value = serde_json::to_value(&summary).unwrap();
        assert!(value.get("revert").is_none());
    }
    #[test]
    fn session_summary_with_revert_round_trips_message_id() {
        let json = r#"{"id":"00000000-0000-4000-8000-000000000001","title":"t","state":"active","created_at":"2024-01-01T00:00:00.000Z","updated_at":"2024-01-01T00:00:00.000Z","archived_at":null,"revert":{"messageID":"00000000-0000-4000-8000-000000000002"}}"#;
        let summary: SessionSummary = serde_json::from_str(json).unwrap();
        assert_eq!(
            summary.revert.as_ref().unwrap().message_id,
            MessageId::from_str("00000000-0000-4000-8000-000000000002").unwrap()
        );
        let value = serde_json::to_value(&summary).unwrap();
        assert_eq!(
            value.get("revert").and_then(|r| r.get("messageID")),
            Some(&serde_json::json!("00000000-0000-4000-8000-000000000002"))
        );
    }
    #[test]
    fn session_revert_round_trips_snapshot_and_files() {
        let json = r#"{"id":"00000000-0000-4000-8000-000000000001","title":"t","state":"active","created_at":"2024-01-01T00:00:00.000Z","updated_at":"2024-01-01T00:00:00.000Z","archived_at":null,"revert":{"messageID":"00000000-0000-4000-8000-000000000002","partID":"p1","snapshot":"snap-1","files":[{"file":"a.rs","patch":"@@","additions":1,"deletions":2,"status":"modified"}]}}"#;
        let summary: SessionSummary = serde_json::from_str(json).unwrap();
        let revert = summary.revert.as_ref().unwrap();
        assert_eq!(revert.part_id.as_deref(), Some("p1"));
        assert_eq!(revert.snapshot.as_deref(), Some("snap-1"));
        let files = revert.files.as_ref().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file, "a.rs");
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 2);
        assert_eq!(files[0].status, FileDiffStatus::Modified);
        let value = serde_json::to_value(&summary).unwrap();
        assert_eq!(value.pointer("/revert/snapshot"), Some(&serde_json::json!("snap-1")));
        assert_eq!(
            value.pointer("/revert/files/0/status"),
            Some(&serde_json::json!("modified"))
        );
    }
    #[test]
    fn session_revert_omits_absent_optionals() {
        let revert = SessionRevert {
            message_id: MessageId::from_str("00000000-0000-4000-8000-000000000002").unwrap(),
            part_id: None,
            snapshot: None,
            files: None,
            count: None,
        };
        let value = serde_json::to_value(&revert).unwrap();
        assert!(value.get("partID").is_none());
        assert!(value.get("snapshot").is_none());
        assert!(value.get("files").is_none());
        assert!(value.get("count").is_none());
    }
    #[test]
    fn session_revert_round_trips_real_user_message_count() {
        // MSG-009: the server computes the boundary-onward user-message count
        // (`stage_session_revert`) and the TUI card reads it verbatim
        // (`index.tsx:1561`).
        let json = r#"{"messageID":"00000000-0000-4000-8000-000000000002","count":3}"#;
        let revert: SessionRevert = serde_json::from_str(json).unwrap();
        assert_eq!(revert.count, Some(3));
        let value = serde_json::to_value(&revert).unwrap();
        assert_eq!(value.get("count"), Some(&serde_json::json!(3)));
    }
}

//! WEB-001: canonical typed control-plane input decoding (MoveSession shape).
//!
//! Pure synchronous route-layer boundary: decodes the canonical JSON wire shape
//! exactly once into [`MoveSessionInput`] before any domain service runs.
//! Rejects oversize bodies before parsing; never logs input values.

/// Maximum retained control-plane input bytes (64 KiB).
pub const MAX_CONTROL_INPUT_BYTES: usize = 64 * 1024;

/// Opaque session identifier, byte-identical to the wire value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(String);

impl SessionId {
    /// Raw wire value. No normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque relative path, byte-identical to the wire value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelPath(String);

impl RelPath {
    /// Raw wire value. No normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque workspace identifier, byte-identical to the wire value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// Raw wire value. No normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Canonical typed MoveSession input, decoded once at the route layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveSessionInput {
    /// Required. Present `null` or missing key is an error, never coerced.
    pub session_id: SessionId,
    /// Required. Forwarded byte-identical.
    pub target_directory: RelPath,
    /// Optional. Absent key stays `None`; present `null` is an error.
    pub target_workspace: Option<WorkspaceId>,
}

/// Route-layer decode failures. Variants carry field names only, never values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDecodeError {
    /// Required key absent from the top-level object.
    MissingField(&'static str),
    /// Wrong JSON type (incl. present `null` for a required/optional field,
    /// non-object top level, invalid UTF-8, or invalid JSON).
    MalformedField(&'static str),
    /// Body longer than [`MAX_CONTROL_INPUT_BYTES`]; parse never attempted.
    BodyTooLarge,
}

impl std::fmt::Display for InputDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(name) => write!(f, "missing field: {name}"),
            Self::MalformedField(name) => write!(f, "malformed field: {name}"),
            Self::BodyTooLarge => write!(f, "body too large"),
        }
    }
}

impl std::error::Error for InputDecodeError {}

/// Decode canonical MoveSession input from raw request bytes.
///
/// Total, deterministic, synchronous over caller-owned bytes: same bytes plus
/// same cap always yield the same `Ok` value or `Err` variant. Unknown extra
/// keys are ignored (forward compatibility). Performs no I/O and logs nothing.
pub fn decode_move_session_input(bytes: &[u8]) -> Result<MoveSessionInput, InputDecodeError> {
    if bytes.len() > MAX_CONTROL_INPUT_BYTES {
        return Err(InputDecodeError::BodyTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| InputDecodeError::MalformedField("body"))?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| InputDecodeError::MalformedField("body"))?;
    let object = value
        .as_object()
        .ok_or(InputDecodeError::MalformedField("body"))?;

    let session_id = decode_required_string(object, "session_id").map(SessionId)?;
    let target_directory = decode_required_string(object, "target_directory").map(RelPath)?;
    let target_workspace =
        decode_optional_string(object, "target_workspace").map(|opt| opt.map(WorkspaceId))?;

    Ok(MoveSessionInput {
        session_id,
        target_directory,
        target_workspace,
    })
}

/// Required field: absent key => `MissingField`; present `null`/wrong type => `MalformedField`.
fn decode_required_string(
    object: &serde_json::Map<String, serde_json::Value>,
    name: &'static str,
) -> Result<String, InputDecodeError> {
    match object.get(name) {
        None => Err(InputDecodeError::MissingField(name)),
        Some(serde_json::Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(InputDecodeError::MalformedField(name)),
    }
}

/// Optional field: absent key => `None`; present `null`/wrong type => `MalformedField`.
fn decode_optional_string(
    object: &serde_json::Map<String, serde_json::Value>,
    name: &'static str,
) -> Result<Option<String>, InputDecodeError> {
    match object.get(name) {
        None => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(InputDecodeError::MalformedField(name)),
    }
}

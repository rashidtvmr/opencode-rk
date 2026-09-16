//! ACP-001: ACP v1 JSON-lines bridge over stdio (codec + session map).
//!
//! Pure boundary over caller-owned bytes and a caller-owned session map
//! (permission broker pattern: this module performs no filesystem I/O, no
//! network, spawns no thread, holds no globals/statics/cache). The caller owns
//! the stdio FDs; [`decode_frame`] turns one inbound line into a typed
//! request, [`encode_frame`] turns one response into a single `\n`-terminated
//! line. Codec functions are stateless; session state lives only in the
//! caller-owned [`AcpSessions`] map. Errors carry codes only, never frame
//! content.

use std::collections::BTreeMap;

/// Largest accepted frame in bytes, either direction (1 MiB).
pub const MAX_ACP_FRAME_BYTES: usize = 1_048_576;

/// Largest accepted prompt text in chars.
pub const MAX_PROMPT_CHARS: usize = 65_536;

/// Largest accepted session id in chars.
pub const MAX_SESSION_ID_CHARS: usize = 128;

/// Recommended session-map cap; enforced, no hidden eviction.
pub const MAX_SESSIONS: usize = 64;

/// Typed protocol error codes. Rendered, never frame content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpErrorCode {
    TooLarge,
    InvalidUtf8,
    BadFrame,
    UnknownMethod,
    UnknownSession,
    InvalidInput,
    TooManySessions,
}

impl AcpErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::BadFrame => "bad_frame",
            Self::UnknownMethod => "unknown_method",
            Self::UnknownSession => "unknown_session",
            Self::InvalidInput => "invalid_input",
            Self::TooManySessions => "too_many_sessions",
        }
    }
}

/// Protocol failure. Carries a code only, never input bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcpError {
    pub code: AcpErrorCode,
}

impl AcpError {
    fn new(code: AcpErrorCode) -> Self {
        Self { code }
    }
}

impl std::fmt::Display for AcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "acp error: {}", self.code.as_str())
    }
}

impl std::error::Error for AcpError {}

/// Typed inbound ACP v1 request, one per line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpRequest {
    Initialize,
    SessionNew,
    SessionLoad { session_id: String },
    SessionPrompt { session_id: String, text: String },
    GetModes,
    GetModels,
    GetVariants,
}

/// Typed outbound ACP v1 response, one line per value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpResponse {
    Ok { body: String },
    Error { code: AcpErrorCode },
}

/// One caller-visible session: id, mode, and its own prompt list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpSession {
    pub id: String,
    pub mode: String,
    pub prompts: Vec<String>,
}

/// Caller-owned in-memory session map. Deterministic id minting via counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcpSessions {
    sessions: BTreeMap<String, AcpSession>,
    next: u64,
}

impl AcpSessions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AcpSession> {
        self.sessions.get(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

/// Fixture-declared mode list, in declared order.
#[must_use]
pub fn modes_body() -> String {
    r#"["default","read-only","plan"]"#.to_owned()
}

/// Fixture-declared model list, in declared order.
#[must_use]
pub fn models_body() -> String {
    r#"["fixture-llm-a","fixture-llm-b"]"#.to_owned()
}

/// Fixture-declared variant list, in declared order.
#[must_use]
pub fn variants_body() -> String {
    r#"["default"]"#.to_owned()
}

/// Decode one inbound frame into a typed request.
///
/// Rejects oversize frames before retention, non-UTF8 bytes, multi-object
/// lines, malformed JSON, unknown methods, and out-of-bound field values.
/// Never mutates caller state.
pub fn decode_frame(line: &[u8]) -> Result<AcpRequest, AcpError> {
    if line.len() > MAX_ACP_FRAME_BYTES {
        return Err(AcpError::new(AcpErrorCode::TooLarge));
    }
    let text = std::str::from_utf8(line).map_err(|_| AcpError::new(AcpErrorCode::InvalidUtf8))?;
    let mut de = serde_json::Deserializer::from_str(text);
    let value: serde_json::Value = serde::de::Deserialize::deserialize(&mut de)
        .map_err(|_| AcpError::new(AcpErrorCode::BadFrame))?;
    de.end()
        .map_err(|_| AcpError::new(AcpErrorCode::BadFrame))?;
    let object = value
        .as_object()
        .ok_or(AcpError::new(AcpErrorCode::BadFrame))?;
    let method = object
        .get("method")
        .and_then(serde_json::Value::as_str)
        .ok_or(AcpError::new(AcpErrorCode::BadFrame))?;
    match method {
        "initialize" => Ok(AcpRequest::Initialize),
        "session/new" => Ok(AcpRequest::SessionNew),
        "session/load" => {
            let session_id = required_id(object, "session_id")?;
            Ok(AcpRequest::SessionLoad { session_id })
        }
        "session/prompt" => {
            let session_id = required_id(object, "session_id")?;
            let text = object
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or(AcpError::new(AcpErrorCode::InvalidInput))?;
            check_text(text)?;
            Ok(AcpRequest::SessionPrompt {
                session_id,
                text: text.to_owned(),
            })
        }
        "modes" => Ok(AcpRequest::GetModes),
        "models" => Ok(AcpRequest::GetModels),
        "variants" => Ok(AcpRequest::GetVariants),
        _ => Err(AcpError::new(AcpErrorCode::UnknownMethod)),
    }
}

fn required_id(
    object: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<String, AcpError> {
    let value = object
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or(AcpError::new(AcpErrorCode::InvalidInput))?;
    check_session_id(value)?;
    Ok(value.to_owned())
}

fn check_session_id(id: &str) -> Result<(), AcpError> {
    let len = id.chars().count();
    if len == 0 || len > MAX_SESSION_ID_CHARS {
        return Err(AcpError::new(AcpErrorCode::InvalidInput));
    }
    Ok(())
}

fn check_text(text: &str) -> Result<(), AcpError> {
    let len = text.chars().count();
    if len == 0 || len > MAX_PROMPT_CHARS {
        return Err(AcpError::new(AcpErrorCode::InvalidInput));
    }
    Ok(())
}

/// Encode one response as a single JSON object line plus trailing `\n`.
///
/// Oversize bodies fail before any byte is produced; nothing is emitted.
pub fn encode_frame(response: &AcpResponse) -> Result<Vec<u8>, AcpError> {
    let mut out = Vec::new();
    encode_frame_into(response, &mut out)?;
    Ok(out)
}

/// Encode into `out`, appending nothing on failure (no partial line).
pub fn encode_frame_into(response: &AcpResponse, out: &mut Vec<u8>) -> Result<(), AcpError> {
    let value = match response {
        AcpResponse::Ok { body } => {
            if body.len() > MAX_ACP_FRAME_BYTES {
                return Err(AcpError::new(AcpErrorCode::TooLarge));
            }
            serde_json::json!({"status": "ok", "body": body})
        }
        AcpResponse::Error { code } => {
            serde_json::json!({"status": "error", "code": code.as_str()})
        }
    };
    let mut line = serde_json::to_vec(&value).map_err(|_| AcpError::new(AcpErrorCode::BadFrame))?;
    line.push(b'\n');
    out.extend_from_slice(&line);
    Ok(())
}

/// Mint a new session with a caller-visible id. Enforces the 64-session cap;
/// never evicts.
pub fn session_new(sessions: &mut AcpSessions) -> Result<AcpSession, AcpError> {
    if sessions.sessions.len() >= MAX_SESSIONS {
        return Err(AcpError::new(AcpErrorCode::TooManySessions));
    }
    sessions.next = sessions.next.saturating_add(1);
    let id = format!("ses_{:06}", sessions.next);
    let session = AcpSession {
        id: id.clone(),
        mode: "default".to_owned(),
        prompts: Vec::new(),
    };
    sessions.sessions.insert(id, session.clone());
    Ok(session)
}

/// Resolve a session id. Read-only; unknown ids change nothing.
pub fn session_load(sessions: &AcpSessions, id: &str) -> Result<AcpSession, AcpError> {
    sessions
        .sessions
        .get(id)
        .cloned()
        .ok_or(AcpError::new(AcpErrorCode::UnknownSession))
}

/// Record a prompt against one session only. Failures leave the map unchanged.
pub fn session_prompt(sessions: &mut AcpSessions, id: &str, text: &str) -> Result<(), AcpError> {
    check_text(text)?;
    let session = sessions
        .sessions
        .get_mut(id)
        .ok_or(AcpError::new(AcpErrorCode::UnknownSession))?;
    session.prompts.push(text.to_owned());
    Ok(())
}

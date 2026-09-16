#![forbid(unsafe_code)]
//! HEAD-002: deterministic redacted session export.
//!
//! Caller-owned traits only; no filesystem, network, thread, statics, or cache.
//! Single sink write on success; every error leaves prior sink bytes untouched.

/// Maximum export size in bytes (16 MiB). Estimated before any sink write.
pub const EXPORT_MAX_BYTES: usize = 16_777_216;

const REDACTED: &str = "***";
const MAX_ID_CHARS: usize = 128;

/// One exported message. `fields` keeps first-seen order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMessage {
    pub role: String,
    pub content: String,
    pub fields: Vec<(String, String)>,
}

/// One session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSession {
    pub id: String,
    pub messages: Vec<ExportMessage>,
}

/// Caller-supplied session source. No filesystem access here.
pub trait SessionStore {
    fn load(&self, id: &str) -> Result<Option<ExportSession>, ExportError>;
}

/// Caller-supplied byte sink. Receives at most one write per call.
pub trait ExportSink {
    fn push(&mut self, data: &[u8]);
}

/// Export options. `tty` is caller-owned; empty id is `MissingId` either way
/// (interactive picker owned by a later UI slice, never implied here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOpts {
    pub tty: bool,
}

/// Export summary. `bytes` equals exactly the emitted length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    pub session_id: String,
    pub bytes: u64,
    pub messages: u64,
}

/// Typed export failures. Variants carry ids/counts only, never secret content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    MissingId,
    InvalidId,
    NotFound,
    TooLarge { estimated: u64 },
    StoreFailed,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingId => write!(f, "missing session id"),
            Self::InvalidId => write!(f, "invalid session id"),
            Self::NotFound => write!(f, "session not found"),
            Self::TooLarge { estimated } => write!(f, "export too large: {estimated} bytes"),
            Self::StoreFailed => write!(f, "session store failed"),
        }
    }
}

impl std::error::Error for ExportError {}

/// Export one session as canonical redacted JSON through the caller sink.
///
/// Lifecycle: validate id -> load session -> render+redact -> size check ->
/// single sink write -> summary. Any failure emits zero sink bytes.
pub fn export_json(
    session_id: &str,
    store: &dyn SessionStore,
    out: &mut dyn ExportSink,
    _opts: &ExportOpts,
) -> Result<ExportSummary, ExportError> {
    if session_id.is_empty() {
        return Err(ExportError::MissingId);
    }
    if session_id.chars().count() > MAX_ID_CHARS {
        return Err(ExportError::InvalidId);
    }
    let session = store.load(session_id)?.ok_or(ExportError::NotFound)?;
    let rendered = render(&session);
    if rendered.len() > EXPORT_MAX_BYTES {
        return Err(ExportError::TooLarge {
            estimated: rendered.len() as u64,
        });
    }
    out.push(&rendered);
    Ok(ExportSummary {
        session_id: session_id.to_owned(),
        bytes: rendered.len() as u64,
        messages: session.messages.len() as u64,
    })
}

fn render(session: &ExportSession) -> Vec<u8> {
    let mut buf = Vec::with_capacity(128 + session.messages.len() * 64);
    buf.extend_from_slice(b"{\"session_id\":");
    push_value(&mut buf, &session.id);
    buf.extend_from_slice(b",\"messages\":[");
    for (i, msg) in session.messages.iter().enumerate() {
        if i > 0 {
            buf.push(b',');
        }
        buf.extend_from_slice(b"{\"role\":");
        push_value(&mut buf, &msg.role);
        buf.extend_from_slice(b",\"content\":");
        push_value(&mut buf, &msg.content);
        for (key, value) in &msg.fields {
            buf.push(b',');
            push_string(&mut buf, key);
            buf.push(b':');
            if is_secret_key(key) || is_secret_value(value) {
                push_string(&mut buf, REDACTED);
            } else {
                push_string(&mut buf, value);
            }
        }
        buf.push(b'}');
    }
    buf.extend_from_slice(b"]}");
    buf
}

/// Push a string value with value-shape redaction applied.
fn push_value(buf: &mut Vec<u8>, value: &str) {
    if is_secret_value(value) {
        push_string(buf, REDACTED);
    } else {
        push_string(buf, value);
    }
}

/// Case-insensitive key match for
/// `^(token|secret|password|api[_-]?key|authorization|bearer|session[_-]?token)$`.
fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        "token" | "secret" | "password" | "authorization" | "bearer" => true,
        "apikey" | "api_key" | "api-key" => true,
        "sessiontoken" | "session_token" | "session-token" => true,
        _ => false,
    }
}

/// Case-insensitive value-prefix match for `^(sk-|bearer |xox[bpas]-)`.
fn is_secret_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("bearer ")
        || ["xoxb-", "xoxp-", "xoxa-", "xoxs-"]
            .iter()
            .any(|p| lower.starts_with(p))
}

/// Canonical minimal JSON string escaping (deterministic, no wall-clock).
fn push_string(buf: &mut Vec<u8>, s: &str) {
    buf.push(b'"');
    for c in s.chars() {
        match c {
            '"' => buf.extend_from_slice(b"\\\""),
            '\\' => buf.extend_from_slice(b"\\\\"),
            '\n' => buf.extend_from_slice(b"\\n"),
            '\r' => buf.extend_from_slice(b"\\r"),
            '\t' => buf.extend_from_slice(b"\\t"),
            '\u{08}' => buf.extend_from_slice(b"\\b"),
            '\u{0C}' => buf.extend_from_slice(b"\\f"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                buf.extend_from_slice(format!("\\u{code:04x}").as_bytes());
            }
            c => {
                let mut tmp = [0_u8; 4];
                buf.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
    }
    buf.push(b'"');
}

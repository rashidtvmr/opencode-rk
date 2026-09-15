//! WEB-002: domain-error to HTTP translation at the route boundary.
//!
//! Pure synchronous boundary: the control-plane domain service returns
//! [`DomainError`], which carries no transport types, and the route layer
//! calls [`translate`] exactly once to get an [`HttpError`] plus the exact
//! wire envelope from [`wire_body`]. Inner I/O strings and filesystem paths
//! are never serialized; each variant maps to a fixed redacted template.

/// Maximum bytes for a single translated error message template.
pub const MAX_ERROR_MESSAGE_BYTES: usize = 512;

/// Domain-only failure enum for the control-plane move-session service.
///
/// Payloads (`TargetUnreadable`, `Internal`) hold diagnostic detail for the
/// domain owner only. They are never serialized to the wire and never logged
/// by this module. No transport types appear here by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Named session does not exist.
    SessionNotFound,
    /// Move target already exists.
    TargetExists,
    /// Move target cannot be read. Holds the unreadable path for the domain
    /// owner; the wire template drops it.
    TargetUnreadable(String),
    /// Session is locked or conflicts with another operation.
    ConflictOrLocked,
    /// Unexpected failure. Holds the inner cause for the domain owner; the
    /// wire template drops it.
    Internal(String),
}

/// Route-boundary error shape: numeric status, stable code, redacted message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError {
    /// Numeric status (404, 409, 422, 500).
    pub status: u16,
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Fixed redacted template. Never contains paths, secrets, or I/O text.
    pub message: String,
}

/// Translate a domain failure to its route-boundary shape.
///
/// Pure and total: every variant maps to exactly one frozen pair. The match
/// is exhaustive with one arm per variant so that adding a sixth variant
/// breaks compilation instead of silently remapping.
pub fn translate(err: &DomainError) -> HttpError {
    match err {
        DomainError::SessionNotFound => HttpError {
            status: 404,
            code: "session_not_found",
            message: "session not found".to_string(),
        },
        DomainError::TargetExists => HttpError {
            status: 409,
            code: "target_exists",
            message: "target already exists".to_string(),
        },
        DomainError::TargetUnreadable(_) => HttpError {
            status: 422,
            code: "target_unreadable",
            message: "target is not readable".to_string(),
        },
        DomainError::ConflictOrLocked => HttpError {
            status: 409,
            code: "conflict_or_locked",
            message: "session is locked or conflicts with another operation".to_string(),
        },
        DomainError::Internal(_) => HttpError {
            status: 500,
            code: "internal",
            message: "internal error".to_string(),
        },
    }
}

/// Render the exact wire envelope for a domain failure.
///
/// Output is byte-exact JSON with exactly two keys in order:
/// `{"error":{"code":<code>,"message":<template>}}`.
/// Only the translated code and redacted template are serialized; domain
/// payloads are dropped. Templates are fixed ASCII with no escapable bytes,
/// so direct formatting is byte-identical to JSON encoding here.
pub fn wire_body(err: &DomainError) -> String {
    let http = translate(err);
    debug_assert!(http.message.len() <= MAX_ERROR_MESSAGE_BYTES);
    format!(
        "{{\"error\":{{\"code\":\"{}\",\"message\":\"{}\"}}}}",
        http.code, http.message
    )
}

/// Every mapped code, one entry per variant. The length assertion in tests
/// pins the fixed error cardinality: growth needs a contract update.
pub fn all_codes() -> [&'static str; 5] {
    [
        "session_not_found",
        "target_exists",
        "target_unreadable",
        "conflict_or_locked",
        "internal",
    ]
}

/// Domain move-session decision, transport-free.
///
/// Predicate order: missing session beats existing target, which beats an
/// unreadable target, which beats a lock conflict. Returns a unit outcome on
/// success; every failure is a typed [`DomainError`], never a status.
pub fn move_session(
    session_exists: bool,
    target_exists: bool,
    target_readable: bool,
    locked: bool,
) -> Result<(), DomainError> {
    if !session_exists {
        return Err(DomainError::SessionNotFound);
    }
    if target_exists {
        return Err(DomainError::TargetExists);
    }
    if !target_readable {
        return Err(DomainError::TargetUnreadable("target".to_string()));
    }
    if locked {
        return Err(DomainError::ConflictOrLocked);
    }
    Ok(())
}

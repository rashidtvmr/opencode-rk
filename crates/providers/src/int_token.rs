//! Token shape validation: length and charset only, never secret content.
//!
//! Rejects empty input with [`TokenShapeError::EmptyToken`]; rejects wrong
//! length (outside 16..=256) or disallowed characters with
//! [`TokenShapeError::BadToken`]. Allowed characters are ASCII alphanumeric
//! plus `.`, `_`, `-`, `~`. Returns the token length on success.

/// Token shape failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenShapeError {
    /// Token is empty.
    #[error("token is empty")]
    EmptyToken,
    /// Token has wrong length or disallowed characters.
    #[error("bad token shape")]
    BadToken,
}

/// Validate token shape (length + charset), return its length.
///
/// Rules: empty -> [`TokenShapeError::EmptyToken`]; length outside 16..=256
/// or any char outside `[A-Za-z0-9._~-]` -> [`TokenShapeError::BadToken`];
/// otherwise `Ok(len)`.
pub fn qualify_shape(t: &str) -> Result<usize, TokenShapeError> {
    if t.is_empty() {
        return Err(TokenShapeError::EmptyToken);
    }
    let len = t.len();
    if !(16..=256).contains(&len) {
        return Err(TokenShapeError::BadToken);
    }
    let ok = t
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'~'));
    if !ok {
        return Err(TokenShapeError::BadToken);
    }
    Ok(len)
}

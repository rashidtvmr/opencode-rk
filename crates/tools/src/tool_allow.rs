//! Tool allowlist (EXT-005 slice).
//!
//! Pure in-memory allowlist over tool names: no I/O, no clock.

use thiserror::Error;

/// Maximum names before [`allow_tool`] rejects new entries.
pub const MAX_TOOL_ALLOW: usize = 128;

/// Allowlist failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AllowError {
    #[error("tool name is empty")]
    EmptyName,
    #[error("too many entries: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Push `name` onto `list` unless already present (idempotent).
/// Rejects empty names and a full list.
pub fn allow_tool(list: &mut Vec<String>, name: &str) -> Result<(), AllowError> {
    if name.is_empty() {
        return Err(AllowError::EmptyName);
    }
    if list.iter().any(|n| n == name) {
        return Ok(());
    }
    if list.len() >= MAX_TOOL_ALLOW {
        return Err(AllowError::TooMany {
            max: MAX_TOOL_ALLOW,
            actual: list.len(),
        });
    }
    list.push(name.to_string());
    Ok(())
}

/// Exact-match membership check.
pub fn is_allowed(list: &[String], name: &str) -> bool {
    list.iter().any(|n| n == name)
}

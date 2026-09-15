//! Integration status mapping. Pure, no IO.

use thiserror::Error;

/// Integration health state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntState {
    Up,
    Down,
    Unknown,
}

/// Status validation failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum IntStatusError {
    #[error("integration id must not be empty")]
    EmptyId,
}

/// Map optional health flag to state.
pub fn report_status(healthy: Option<bool>) -> IntState {
    match healthy {
        Some(true) => IntState::Up,
        Some(false) => IntState::Down,
        None => IntState::Unknown,
    }
}

/// Static label for a state.
pub fn label_state(s: &IntState) -> &'static str {
    match s {
        IntState::Up => "up",
        IntState::Down => "down",
        IntState::Unknown => "unknown",
    }
}

/// Validate id, return trimmed copy.
pub fn require_id(id: &str) -> Result<String, IntStatusError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(IntStatusError::EmptyId);
    }
    Ok(trimmed.to_string())
}

//! Release gate state (REL slice).
//!
//! Pure gate: zero failures opens the gate, any failure count closes it.
//! No IO, no threads.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GateError {
    #[error("suite name must not be empty")]
    EmptyName,
}

pub fn gate_for(failures: u64) -> GateState {
    if failures == 0 {
        GateState::Open
    } else {
        GateState::Closed
    }
}

pub fn gate_label(g: &GateState) -> &'static str {
    match g {
        GateState::Open => "open",
        GateState::Closed => "closed",
    }
}

pub fn require_suite(s: &str) -> Result<String, GateError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(GateError::EmptyName);
    }
    Ok(trimmed.to_string())
}

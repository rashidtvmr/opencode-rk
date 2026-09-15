//! Pure extension lifecycle planner: validate name + phase, render plan string.
//! No loading, no execution, no IO.

use thiserror::Error;

/// Lifecycle phases an extension can transition to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtPhase {
    Install,
    Enable,
    Disable,
    Uninstall,
}

/// Lifecycle planning failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExtLifecycleError {
    #[error("extension name is empty")]
    EmptyName,
    #[error("unknown phase: {name}")]
    UnknownPhase { name: String },
}

/// Parse a phase name (case-insensitive, trimmed).
pub fn parse_phase(n: &str) -> Result<ExtPhase, ExtLifecycleError> {
    let t = n.trim();
    if t.eq_ignore_ascii_case("install") {
        Ok(ExtPhase::Install)
    } else if t.eq_ignore_ascii_case("enable") {
        Ok(ExtPhase::Enable)
    } else if t.eq_ignore_ascii_case("disable") {
        Ok(ExtPhase::Disable)
    } else if t.eq_ignore_ascii_case("uninstall") {
        Ok(ExtPhase::Uninstall)
    } else {
        Err(ExtLifecycleError::UnknownPhase { name: t.to_string() })
    }
}

/// Plan a lifecycle transition. Returns `"NAME:phase"` with lowercase phase.
pub fn plan_transition(name: &str, phase: &str) -> Result<String, ExtLifecycleError> {
    if name.is_empty() {
        return Err(ExtLifecycleError::EmptyName);
    }
    let p = parse_phase(phase)?;
    let s = match p {
        ExtPhase::Install => "install",
        ExtPhase::Enable => "enable",
        ExtPhase::Disable => "disable",
        ExtPhase::Uninstall => "uninstall",
    };
    Ok(format!("{name}:{s}"))
}

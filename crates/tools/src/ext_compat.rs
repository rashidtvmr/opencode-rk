//! EXT-005/009 extension compatibility verdict.
use thiserror::Error;

/// Compatibility verdict for an extension tool name.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CompatVerdict {
    /// Fully compatible, no shim needed.
    Ok,
    /// Legacy tool routed through the compat shim.
    Shimmed,
    /// Blocked (reserved for future policy; never returned by [`check_compat`]).
    Blocked,
}

/// Compatibility check failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompatError {
    /// Name is empty or whitespace-only.
    #[error("empty tool name")]
    EmptyName,
}

/// Check compatibility of an extension tool name.
///
/// Empty or whitespace-only names are rejected; legacy tools are
/// shimmed, all others are OK.
pub fn check_compat(name: &str, legacy: bool) -> Result<CompatVerdict, CompatError> {
    if name.trim().is_empty() {
        return Err(CompatError::EmptyName);
    }
    if legacy {
        Ok(CompatVerdict::Shimmed)
    } else {
        Ok(CompatVerdict::Ok)
    }
}

/// Stable label for a verdict: `ok` / `shimmed` / `blocked`.
pub fn verdict_label(v: &CompatVerdict) -> &'static str {
    match v {
        CompatVerdict::Ok => "ok",
        CompatVerdict::Shimmed => "shimmed",
        CompatVerdict::Blocked => "blocked",
    }
}

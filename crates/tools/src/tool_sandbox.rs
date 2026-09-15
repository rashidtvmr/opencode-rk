//! Per-tool sandbox flag (EXT-009 slice).
//!
//! Pure in-memory toggle over tool names: no I/O, no clock.

use thiserror::Error;

/// Tool with sandbox flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxFlag {
    pub tool: String,
    pub sandboxed: bool,
}

/// Sandbox flag failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
    #[error("tool name is empty")]
    EmptyTool,
}

/// Flip `tool` to `on`, returning new state. Missing tools are pushed
/// (sandboxed=`on`).
pub fn set_sandbox(
    flags: &mut Vec<SandboxFlag>,
    tool: &str,
    on: bool,
) -> Result<bool, SandboxError> {
    if tool.is_empty() {
        return Err(SandboxError::EmptyTool);
    }
    if let Some(f) = flags.iter_mut().find(|f| f.tool == tool) {
        f.sandboxed = on;
        return Ok(f.sandboxed);
    }
    flags.push(SandboxFlag {
        tool: tool.to_string(),
        sandboxed: on,
    });
    Ok(on)
}

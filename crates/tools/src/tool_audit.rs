//! Tool-use audit buffer (EXT-012 tool-audit slice).
//!
//! Pure append-only record over `Vec<ToolAudit>`: validates shape only, no
//! clock, no I/O, no side effects beyond pushing on success. Bounded by
//! [`MAX_TOOL_AUDIT`] so callers cannot grow the buffer without limit.

use thiserror::Error;

/// Audit entry for one tool invocation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAudit {
    pub tool: String,
    pub ok: bool,
    pub seq: u64,
}

/// Tool-audit failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolAuditError {
    #[error("tool name is empty")]
    EmptyTool,
    #[error("too many tool audits: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Maximum entries retained in the audit buffer.
pub const MAX_TOOL_AUDIT: usize = 512;

/// Record one tool outcome. Rejects empty names and a full buffer without
/// mutating; on success pushes with `seq = len + 1`, preserving order.
pub fn record_tool(
    buf: &mut Vec<ToolAudit>,
    tool: &str,
    ok: bool,
) -> Result<(), ToolAuditError> {
    if tool.is_empty() {
        return Err(ToolAuditError::EmptyTool);
    }
    if buf.len() >= MAX_TOOL_AUDIT {
        return Err(ToolAuditError::TooMany {
            max: MAX_TOOL_AUDIT,
            actual: buf.len(),
        });
    }
    buf.push(ToolAudit {
        tool: tool.to_owned(),
        ok,
        seq: buf.len() as u64 + 1,
    });
    Ok(())
}

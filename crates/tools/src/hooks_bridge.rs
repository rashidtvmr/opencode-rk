//! Pure pre/post-tool hook matcher registry (EXT-008).
//!
//! Answers which registered hooks fire for a tool name. No execution,
//! no I/O, no side effects.

use thiserror::Error;

/// When a hook fires relative to tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTiming {
    Pre,
    Post,
}

/// One hook registration: a tool-name pattern plus timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEntry {
    pub tool_pattern: String,
    pub timing: HookTiming,
    pub hook_id: String,
}

/// Validation failures for [`hooks_for`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HookBridgeError {
    #[error("hook pattern must not be empty")]
    EmptyPattern,
    #[error("hook id must not be empty")]
    EmptyHookId,
    #[error("too many hooks: max {max}, got {actual}")]
    TooManyHooks { max: usize, actual: usize },
}

/// Maximum number of hook registrations per query.
pub const MAX_HOOKS: usize = 64;

fn same_timing(a: &HookTiming, b: &HookTiming) -> bool {
    matches!(
        (a, b),
        (HookTiming::Pre, HookTiming::Pre) | (HookTiming::Post, HookTiming::Post)
    )
}

fn pattern_matches(pattern: &str, tool: &str) -> bool {
    if (pattern == "*") || (pattern == tool) {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        if prefix.ends_with('.') {
            return tool.starts_with(prefix);
        }
    }
    false
}

/// Return the ids of hooks firing for `tool` at `timing`, in slice order.
pub fn hooks_for(
    hooks: &[HookEntry],
    tool: &str,
    timing: &HookTiming,
) -> Result<Vec<String>, HookBridgeError> {
    for h in hooks {
        if h.tool_pattern.is_empty() {
            return Err(HookBridgeError::EmptyPattern);
        }
        if h.hook_id.is_empty() {
            return Err(HookBridgeError::EmptyHookId);
        }
    }
    if hooks.len() > MAX_HOOKS {
        return Err(HookBridgeError::TooManyHooks {
            max: MAX_HOOKS,
            actual: hooks.len(),
        });
    }
    Ok(hooks
        .iter()
        .filter(|h| same_timing(&h.timing, timing) && pattern_matches(&h.tool_pattern, tool))
        .map(|h| h.hook_id.clone())
        .collect())
}

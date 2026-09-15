//! Pure extension-hook registration registry (EXT-008 hooks half).
//!
//! Answers which hooks are registered and validates shape only. No matcher
//! semantics (see `hooks_bridge`), no execution, no I/O, no side effects.

use thiserror::Error;

/// One hook registration: id plus lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtHook {
    pub id: String,
    pub event: String,
}

/// Registration validation failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtHookError {
    #[error("hook id must not be empty")]
    EmptyId,
    #[error("hook event must be one of pre, post, error")]
    EmptyEvent,
    #[error("duplicate hook id: {id}")]
    DuplicateId { id: String },
    #[error("too many hooks: max {max}, got {actual}")]
    TooManyHooks { max: usize, actual: usize },
}

/// Maximum number of hook registrations.
pub const MAX_EXT_HOOKS: usize = 64;

/// Valid lifecycle events, in canonical order.
pub fn valid_events() -> &'static [&'static str] {
    &["pre", "post", "error"]
}

/// Insertion-ordered hook registry.
#[derive(Debug, Default)]
pub struct ExtHookRegistry {
    hooks: Vec<ExtHook>,
}

impl ExtHookRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a hook. Validates id, event, dup, capacity in that order.
    pub fn register(&mut self, hook: ExtHook) -> Result<(), ExtHookError> {
        if hook.id.is_empty() {
            return Err(ExtHookError::EmptyId);
        }
        if hook.event.is_empty() || !valid_events().contains(&hook.event.as_str()) {
            return Err(ExtHookError::EmptyEvent);
        }
        if self.hooks.iter().any(|h| h.id == hook.id) {
            return Err(ExtHookError::DuplicateId { id: hook.id });
        }
        if self.hooks.len() >= MAX_EXT_HOOKS {
            return Err(ExtHookError::TooManyHooks {
                max: MAX_EXT_HOOKS,
                actual: self.hooks.len() + 1,
            });
        }
        self.hooks.push(hook);
        Ok(())
    }

    /// Look up a hook by id.
    pub fn get(&self, id: &str) -> Option<&ExtHook> {
        self.hooks.iter().find(|h| h.id == id)
    }

    /// All hooks in insertion order.
    pub fn list(&self) -> &[ExtHook] {
        &self.hooks
    }
}

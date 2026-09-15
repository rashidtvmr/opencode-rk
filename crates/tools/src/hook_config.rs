//! Pure hook command config validation (EXT-008).
//!
//! Qualifies `(id, command)` pairs into owned [`HookConfig`] values.
//! No execution, no I/O, no side effects.

use thiserror::Error;

/// One qualified hook: stable id plus the command to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookConfig {
    pub id: String,
    pub command: String,
}

/// Validation failures for [`qualify_hooks`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HookConfigError {
    #[error("hook id must not be empty")]
    EmptyId,
    #[error("hook command must not be empty")]
    EmptyCommand,
    #[error("too many hook configs: max {max}, got {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Maximum number of hook configs per call.
pub const MAX_HOOK_CONFIGS: usize = 64;

/// Validate `items` in order, keeping order. Length checked first.
pub fn qualify_hooks(items: &[(&str, &str)]) -> Result<Vec<HookConfig>, HookConfigError> {
    if items.len() > MAX_HOOK_CONFIGS {
        return Err(HookConfigError::TooMany {
            max: MAX_HOOK_CONFIGS,
            actual: items.len(),
        });
    }
    items
        .iter()
        .map(|(id, command)| {
            if id.is_empty() {
                return Err(HookConfigError::EmptyId);
            }
            if command.is_empty() {
                return Err(HookConfigError::EmptyCommand);
            }
            Ok(HookConfig {
                id: id.to_string(),
                command: command.to_string(),
            })
        })
        .collect()
}

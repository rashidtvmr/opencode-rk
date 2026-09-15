//! Extension enable/disable registry (EXT-004 enablement slice).
//!
//! Pure in-memory toggle over [`ExtEntry`]: no I/O, no clock.

use thiserror::Error;

/// Maximum entries before [`set_enabled`] rejects new names.
pub const MAX_EXT_ENTRIES: usize = 128;

/// Named extension entry with enable flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtEntry {
    pub name: String,
    pub enabled: bool,
}

/// Enable failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnableError {
    #[error("extension name is empty")]
    EmptyName,
    #[error("too many entries: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Flip `name` to `on`, returning new state. Missing names are pushed
/// (enabled=`on`) unless the vec is at [`MAX_EXT_ENTRIES`].
pub fn set_enabled(
    entries: &mut Vec<ExtEntry>,
    name: &str,
    on: bool,
) -> Result<bool, EnableError> {
    if name.is_empty() {
        return Err(EnableError::EmptyName);
    }
    if let Some(e) = entries.iter_mut().find(|e| e.name == name) {
        e.enabled = on;
        return Ok(e.enabled);
    }
    if entries.len() >= MAX_EXT_ENTRIES {
        return Err(EnableError::TooMany {
            max: MAX_EXT_ENTRIES,
            actual: entries.len(),
        });
    }
    entries.push(ExtEntry {
        name: name.to_string(),
        enabled: on,
    });
    Ok(on)
}

//! EXT-005 legacy command alias resolution: old name -> canonical name.
use thiserror::Error;

/// Maximum number of alias entries accepted by [`resolve_alias`].
pub const MAX_ALIASES: usize = 64;

/// Alias resolution failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompatError {
    /// The queried name is empty.
    #[error("empty command name")]
    EmptyName,
    /// The alias table exceeds [`MAX_ALIASES`].
    #[error("too many aliases: max {max}, got {actual}")]
    TooManyAliases { max: usize, actual: usize },
    /// No alias maps this name.
    #[error("unknown alias: {name}")]
    UnknownAlias { name: String },
}

/// Resolve a legacy command name to its canonical name.
///
/// Exact match on the old name (first tuple element) returns the canonical
/// name (second element). Duplicate old names: first wins. A name that only
/// equals a canonical value is [`CompatError::UnknownAlias`]; no guessing.
pub fn resolve_alias(aliases: &[(&str, &str)], name: &str) -> Result<String, CompatError> {
    if name.is_empty() {
        return Err(CompatError::EmptyName);
    }
    if aliases.len() > MAX_ALIASES {
        return Err(CompatError::TooManyAliases { max: MAX_ALIASES, actual: aliases.len() });
    }
    for (old, canonical) in aliases {
        if *old == name {
            return Ok((*canonical).to_string());
        }
    }
    Err(CompatError::UnknownAlias { name: name.to_string() })
}

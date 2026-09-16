//! Provider key mapping. Pure, no IO.

use thiserror::Error;

/// Maximum entries accepted in a single lookup call.
pub const MAX_MAPPINGS: usize = 128;

/// Mapping lookup failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MappingError {
    #[error("key must not be empty")]
    EmptyKey,
    #[error("value must not be empty")]
    EmptyValue,
    #[error("too many mappings: max {max}, got {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Look up `key` in `map`. First match wins.
///
/// Contract: empty `key` arg -> [`MappingError::EmptyKey`]; any empty
/// stored key -> [`MappingError::EmptyKey`]; any empty stored value ->
/// [`MappingError::EmptyValue`]; `map.len() > MAX_MAPPINGS` ->
/// [`MappingError::TooMany`]. The frozen enum has no `Unknown` variant,
/// so an unknown key returns [`MappingError::EmptyValue`].
pub fn lookup(map: &[(&str, &str)], key: &str) -> Result<String, MappingError> {
    if key.is_empty() {
        return Err(MappingError::EmptyKey);
    }
    if map.len() > MAX_MAPPINGS {
        return Err(MappingError::TooMany {
            max: MAX_MAPPINGS,
            actual: map.len(),
        });
    }
    for (k, v) in map {
        if k.is_empty() {
            return Err(MappingError::EmptyKey);
        }
        if v.is_empty() {
            return Err(MappingError::EmptyValue);
        }
    }
    // ponytail: skipped trimming/normalization; add when keys need canonical form.
    map.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
        .ok_or(MappingError::EmptyValue)
}

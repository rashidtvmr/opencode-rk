//! REL tag qualification: `vX.Y.Z` release tag validation.
use thiserror::Error;

/// Tag qualification failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TagError {
    /// The tag string is empty.
    #[error("empty tag")]
    EmptyTag,
    /// The tag is not a valid `vX.Y.Z` tag.
    #[error("bad tag")]
    BadTag,
}

/// Validate a release tag: must start with `v`, remainder must be
/// dot-separated numeric parts (at least two, so a dot is required),
/// charset ASCII alphanumeric plus `.`; returns a copy on success.
pub fn qualify_tag(t: &str) -> Result<String, TagError> {
    if t.is_empty() {
        return Err(TagError::EmptyTag);
    }
    let rest = t.strip_prefix('v').ok_or(TagError::BadTag)?;
    if !rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') {
        return Err(TagError::BadTag);
    }
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() < 2 {
        return Err(TagError::BadTag);
    }
    if !parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(TagError::BadTag);
    }
    Ok(t.to_string())
}

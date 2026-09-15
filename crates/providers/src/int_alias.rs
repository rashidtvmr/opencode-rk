//! Alias resolution: first-match lookup over `(alias, target)` pairs.

/// Alias resolution failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AliasError {
    /// Alias argument or a stored alias was empty.
    #[error("alias is empty")]
    EmptyAlias,
    /// Stored target was empty, or alias was unknown.
    ///
    /// Unknown aliases map to [`AliasError::EmptyTarget`] because the
    /// frozen enum has no `Unknown` variant.
    #[error("alias target is empty")]
    EmptyTarget,
}

/// Resolve `alias` to its target.
///
/// Rules: empty `alias` -> [`AliasError::EmptyAlias`]; any empty stored
/// alias -> [`AliasError::EmptyAlias`]; any empty stored target ->
/// [`AliasError::EmptyTarget`]; first match wins; unknown alias ->
/// [`AliasError::EmptyTarget`].
pub fn resolve_alias(pairs: &[(&str, &str)], alias: &str) -> Result<String, AliasError> {
    if alias.is_empty() {
        return Err(AliasError::EmptyAlias);
    }
    for (a, t) in pairs {
        if a.is_empty() {
            return Err(AliasError::EmptyAlias);
        }
        if t.is_empty() {
            return Err(AliasError::EmptyTarget);
        }
    }
    for (a, t) in pairs {
        if *a == alias {
            return Ok((*t).to_owned());
        }
    }
    Err(AliasError::EmptyTarget)
}

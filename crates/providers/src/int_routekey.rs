//! Route-key qualification: bounded lowercase alnum-dash identifier.

/// Route-key validation failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RouteKeyError {
    /// Key was empty.
    #[error("route key is empty")]
    EmptyKey,
    /// Key violated charset, start, or length rules.
    #[error("bad route key")]
    BadKey,
}

/// Validate and copy a route key.
///
/// Rules: empty -> [`RouteKeyError::EmptyKey`]; otherwise lowercase
/// alphanumeric plus dash, first char alphanumeric, length 3..=48,
/// else [`RouteKeyError::BadKey`]; returns an owned copy.
pub fn qualify_routekey(k: &str) -> Result<String, RouteKeyError> {
    if k.is_empty() {
        return Err(RouteKeyError::EmptyKey);
    }
    let len = k.len();
    if !(3..=48).contains(&len) {
        return Err(RouteKeyError::BadKey);
    }
    let mut chars = k.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return Err(RouteKeyError::BadKey),
    }
    if !k
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(RouteKeyError::BadKey);
    }
    Ok(k.to_owned())
}

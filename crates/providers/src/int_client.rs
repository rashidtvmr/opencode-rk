//! Client-id qualification for integration repository operations (INT-002).
//!
//! Pure validation + normalization. No persistence, no network, no clock.

/// Upper bound on client-id length in bytes (ASCII-only charset).
pub const MAX_CLIENT_ID_LEN: usize = 64;

/// Typed client-id failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ClientError {
    #[error("empty client id")]
    EmptyId,
    #[error("bad client id")]
    BadId,
}

/// Validate `id` and return its lowercased canonical form.
///
/// Rules: non-empty; len <= 64; chars alnum/`-`/`_`; first char alnum.
pub fn qualify_client(id: &str) -> Result<String, ClientError> {
    if id.is_empty() {
        return Err(ClientError::EmptyId);
    }
    if id.len() > MAX_CLIENT_ID_LEN {
        return Err(ClientError::BadId);
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return Err(ClientError::BadId),
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ClientError::BadId);
    }
    Ok(id.to_ascii_lowercase())
}

/// Return `"client:ID"` for the qualified (lowercased) id.
pub fn client_tag(id: &str) -> Result<String, ClientError> {
    qualify_client(id).map(|qualified| format!("client:{qualified}"))
}

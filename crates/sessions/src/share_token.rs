use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TokenError {
    #[error("empty token")]
    EmptyToken,
    #[error("bad token")]
    BadToken,
}

pub fn qualify_token(t: &str) -> Result<String, TokenError> {
    let trimmed = t.trim();
    if trimmed.is_empty() {
        return Err(TokenError::EmptyToken);
    }
    let len = trimmed.len();
    if !(8..=64).contains(&len) {
        return Err(TokenError::BadToken);
    }
    let ok = trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok {
        return Err(TokenError::BadToken);
    }
    Ok(trimmed.to_owned())
}

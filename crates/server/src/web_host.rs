//! Host allowlist qualifier.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    EmptyHost,
    BadHost,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyHost => write!(f, "empty host"),
            Self::BadHost => write!(f, "bad host"),
        }
    }
}

impl std::error::Error for HostError {}

pub fn qualify_host(h: &str) -> Result<String, HostError> {
    if h.is_empty() {
        return Err(HostError::EmptyHost);
    }
    let len = h.len();
    if !(3..=64).contains(&len) {
        return Err(HostError::BadHost);
    }
    if !h
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return Err(HostError::BadHost);
    }
    let b = h.as_bytes();
    if !b[0].is_ascii_alphanumeric() || !b[len - 1].is_ascii_alphanumeric() {
        return Err(HostError::BadHost);
    }
    if h.contains("..") {
        return Err(HostError::BadHost);
    }
    Ok(h.to_ascii_lowercase())
}

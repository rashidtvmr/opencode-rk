//! Pure enterprise remote link builder (native half of opencode.enterprise-remote).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterpriseLink {
    pub host: String,
    pub api_key_slot: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterpriseError {
    EmptyHost,
    BadHost,
    EmptySlot,
}

impl std::fmt::Display for EnterpriseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyHost => write!(f, "empty host"),
            Self::BadHost => write!(f, "bad host"),
            Self::EmptySlot => write!(f, "empty slot"),
        }
    }
}

impl std::error::Error for EnterpriseError {}

pub fn build_link(host: &str, slot: &str) -> Result<EnterpriseLink, EnterpriseError> {
    if host.is_empty() {
        return Err(EnterpriseError::EmptyHost);
    }
    if !host.starts_with("https://") {
        return Err(EnterpriseError::BadHost);
    }
    if slot.is_empty() {
        return Err(EnterpriseError::EmptySlot);
    }
    Ok(EnterpriseLink {
        host: host.trim_end_matches('/').to_string(),
        api_key_slot: slot.to_string(),
    })
}

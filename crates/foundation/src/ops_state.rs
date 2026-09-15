//! Ops-state slice.
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpsState {
    pub name: String,
    pub rev: u64,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OpsStateError {
    #[error("name must not be empty")]
    EmptyName,
}

pub fn make_state(name: &str) -> Result<OpsState, OpsStateError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OpsStateError::EmptyName);
    }
    Ok(OpsState {
        name: trimmed.to_string(),
        rev: 0,
    })
}

pub fn bump(s: &mut OpsState) {
    s.rev = s.rev.saturating_add(1);
}

//! Ops-scope planner slice (opencode.configuration-runtime + repository-operations).
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpsScope {
    pub project: String,
    pub read_only: bool,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OpsScopeError {
    #[error("project must not be empty")]
    EmptyProject,
}

pub fn build_scope(project: &str, read_only: bool) -> Result<OpsScope, OpsScopeError> {
    let trimmed = project.trim();
    if trimmed.is_empty() {
        return Err(OpsScopeError::EmptyProject);
    }
    Ok(OpsScope {
        project: trimmed.to_string(),
        read_only,
    })
}

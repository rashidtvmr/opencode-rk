//! Turn-level agent role resolution (AGENT-MODE-ROLE).
//!
//! Given a requested agent name, loads the workspace agent definition files
//! from `<workspace>/.agents/agents/` (via `opencode_rk_agents::agent_files`)
//! and derives the role (system) prompt plus the provider tool-advertisement
//! restriction for the turn. Pure decision layer: the caller in the turn
//! stream prepends the returned prompt as a `System` item and filters the
//! advertised tool schema by the returned ids.
//!
//! Semantics:
//! - no agent name → `Ok(None)` (ordinary turn, unchanged).
//! - definitions directory absent → `Ok(None)`: agent selection is an
//!   optimization and must not break turns in workspaces without definitions.
//! - directory present but name unknown → [`RoleError::UnknownAgent`]
//!   (fail closed on a silent typo when the user did set up agents).
#![forbid(unsafe_code)]

use std::path::Path;

use opencode_rk_agents::agent_files::{self, AgentFileError};

/// Resolved role prompting for one turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRole {
    /// System/role text to prepend to the provider request.
    pub role_prompt: String,
    /// Tool ids that may be advertised for this turn (possibly empty).
    pub tools: Vec<String>,
}

/// Resolution failures surfaced to the caller as bad requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoleError {
    /// The definitions directory exists but no agent file matches the name.
    UnknownAgent(String),
    /// Loading or parsing the agent files failed.
    Load(String),
}

impl std::fmt::Display for RoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAgent(name) => write!(f, "unknown agent: {name}"),
            Self::Load(reason) => write!(f, "agent definitions unavailable: {reason}"),
        }
    }
}

impl std::error::Error for RoleError {}

impl From<AgentFileError> for RoleError {
    fn from(e: AgentFileError) -> Self {
        Self::Load(format!("{e:?}"))
    }
}

/// Resolve `agent` against `<workspace>/.agents/agents` for a turn whose
/// server-enabled tool set is `enabled`.
pub fn resolve_agent_role(
    workspace: &Path,
    agent: Option<&str>,
    enabled: &[&str],
) -> Result<Option<AgentRole>, RoleError> {
    let Some(name) = agent.map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    let dir = workspace.join(".agents").join("agents");
    if !dir.is_dir() {
        return Ok(None);
    }
    let snapshot = agent_files::load_agent_files(&dir)?;
    match snapshot.find(name) {
        None => Err(RoleError::UnknownAgent(name.to_string())),
        Some(def) => {
            let owned: Vec<String> = enabled.iter().map(|t| (*t).to_string()).collect();
            Ok(Some(AgentRole {
                role_prompt: def.role_prompt(),
                tools: def.advertised_tools(&owned),
            }))
        }
    }
}

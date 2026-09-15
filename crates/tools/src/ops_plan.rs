//! OPS-003 deploy-plan: (name, target) -> "NAME@target".
use thiserror::Error;

/// Deploy target for [`plan_deploy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployTarget {
    /// Local deployment.
    Local,
    /// Remote deployment.
    Remote,
}

impl DeployTarget {
    fn as_str(self) -> &'static str {
        match self {
            DeployTarget::Local => "local",
            DeployTarget::Remote => "remote",
        }
    }
}

/// Deploy-plan failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    /// The deploy name is empty.
    #[error("empty deploy name")]
    EmptyName,
    /// The target is not `local` or `remote`.
    #[error("unknown target: {name}")]
    UnknownTarget { name: String },
}

/// Parse a deploy target name, case-insensitive.
pub fn parse_target(s: &str) -> Result<DeployTarget, PlanError> {
    if s.eq_ignore_ascii_case("local") {
        Ok(DeployTarget::Local)
    } else if s.eq_ignore_ascii_case("remote") {
        Ok(DeployTarget::Remote)
    } else {
        Err(PlanError::UnknownTarget {
            name: s.to_string(),
        })
    }
}

/// Build a deploy plan string `"NAME@target"`.
///
/// Empty `name` is [`PlanError::EmptyName`]; `target` parses via
/// [`parse_target`] (case-insensitive, canonical lowercase output).
pub fn plan_deploy(name: &str, target: &str) -> Result<String, PlanError> {
    if name.is_empty() {
        return Err(PlanError::EmptyName);
    }
    let t = parse_target(target)?;
    Ok(format!("{}@{}", name, t.as_str()))
}

//! OPS-003 target-parse slice: qualify a deploy target name.
use thiserror::Error;

/// Qualified deploy target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployTarget2 {
    /// Local deployment.
    Local,
    /// Remote deployment.
    Remote,
}

/// Target-qualify failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeployTargetError {
    /// The target string is empty.
    #[error("empty target")]
    EmptyTarget,
    /// The target is not `local` or `remote`.
    #[error("unknown target: {name}")]
    UnknownTarget { name: String },
}

/// Qualify a deploy target name, case-insensitive.
///
/// Empty input is [`DeployTargetError::EmptyTarget`]; anything other than
/// `local`/`remote` (any case) is [`DeployTargetError::UnknownTarget`].
pub fn qualify_target(s: &str) -> Result<DeployTarget2, DeployTargetError> {
    if s.is_empty() {
        return Err(DeployTargetError::EmptyTarget);
    }
    if s.eq_ignore_ascii_case("local") {
        Ok(DeployTarget2::Local)
    } else if s.eq_ignore_ascii_case("remote") {
        Ok(DeployTarget2::Remote)
    } else {
        Err(DeployTargetError::UnknownTarget {
            name: s.to_string(),
        })
    }
}

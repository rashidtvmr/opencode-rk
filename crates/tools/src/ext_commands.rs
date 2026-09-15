//! EXT-010/011 extension command qualification.
use thiserror::Error;

/// Maximum number of extension commands accepted by [`qualify_commands`].
pub const MAX_EXT_CMDS: usize = 128;

/// Command qualification failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtCmdError {
    /// A command name is empty.
    #[error("empty command name")]
    EmptyName,
    /// A command name is malformed (missing leading `/`, too short, spaces).
    #[error("bad command name")]
    BadName,
    /// More than [`MAX_EXT_CMDS`] commands supplied.
    #[error("too many commands: max {max}, got {actual}")]
    TooManyCmds { max: usize, actual: usize },
}

/// Validate and copy extension command names, preserving order.
///
/// Each name must start with `/`, have length >= 2, and contain no spaces.
pub fn qualify_commands(cmds: &[&str]) -> Result<Vec<String>, ExtCmdError> {
    if cmds.len() > MAX_EXT_CMDS {
        return Err(ExtCmdError::TooManyCmds { max: MAX_EXT_CMDS, actual: cmds.len() });
    }
    let mut out = Vec::with_capacity(cmds.len());
    for c in cmds {
        if c.is_empty() {
            return Err(ExtCmdError::EmptyName);
        }
        if c.len() < 2 || !c.starts_with('/') || c.contains(char::is_whitespace) {
            return Err(ExtCmdError::BadName);
        }
        out.push((*c).to_string());
    }
    Ok(out)
}

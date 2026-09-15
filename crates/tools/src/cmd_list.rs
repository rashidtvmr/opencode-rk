//! In-memory custom-command name list (EXT-010).

use thiserror::Error;

pub const MAX_CMD_LIST: usize = 128;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CmdListError {
    #[error("empty name")]
    EmptyName,
    #[error("too many commands: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub fn add_command(list: &mut Vec<String>, name: &str) -> Result<(), CmdListError> {
    if name.is_empty() {
        return Err(CmdListError::EmptyName);
    }
    if has_command(list, name) {
        return Ok(());
    }
    if list.len() >= MAX_CMD_LIST {
        return Err(CmdListError::TooMany {
            max: MAX_CMD_LIST,
            actual: list.len() + 1,
        });
    }
    list.push(name.to_string());
    Ok(())
}

pub fn has_command(list: &[String], name: &str) -> bool {
    list.iter().any(|c| c == name)
}

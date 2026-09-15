use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BackupError {
    #[error("backup name must not be empty")]
    EmptyName,
    #[error("bad backup name")]
    BadName,
}

pub fn backup_name(name: &str, seq: u32) -> Result<String, BackupError> {
    if name.is_empty() {
        return Err(BackupError::EmptyName);
    }
    if name.len() > 128
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(BackupError::BadName);
    }
    Ok(format!("{name}-{seq:04}"))
}

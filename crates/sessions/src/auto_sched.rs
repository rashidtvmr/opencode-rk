use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedEntry {
    pub name: String,
    pub every_secs: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchedError {
    #[error("schedule name is empty")]
    EmptyName,
    #[error("interval must be non-zero")]
    ZeroInterval,
    #[error("too many schedules: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub const MAX_SCHED: usize = 64;

pub fn add_sched(
    list: &mut Vec<SchedEntry>,
    name: &str,
    every_secs: u64,
) -> Result<(), SchedError> {
    if name.is_empty() {
        return Err(SchedError::EmptyName);
    }
    if every_secs == 0 {
        return Err(SchedError::ZeroInterval);
    }
    if let Some(e) = list.iter_mut().find(|e| e.name == name) {
        e.every_secs = every_secs;
        return Ok(());
    }
    if list.len() >= MAX_SCHED {
        return Err(SchedError::TooMany {
            max: MAX_SCHED,
            actual: list.len(),
        });
    }
    list.push(SchedEntry {
        name: name.to_string(),
        every_secs,
    });
    Ok(())
}

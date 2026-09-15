use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpLimits {
    pub max_tasks: usize,
    pub max_mb: u64,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OpLimitsError {
    #[error("max_tasks must be non-zero")]
    ZeroTasks,
    #[error("max_mb must be non-zero")]
    ZeroMb,
}

pub fn check_limits(l: &OpLimits) -> Result<(), OpLimitsError> {
    if l.max_tasks == 0 {
        return Err(OpLimitsError::ZeroTasks);
    }
    if l.max_mb == 0 {
        return Err(OpLimitsError::ZeroMb);
    }
    Ok(())
}

#[must_use]
pub fn tasks_within(l: &OpLimits, n: usize) -> bool {
    n <= l.max_tasks
}

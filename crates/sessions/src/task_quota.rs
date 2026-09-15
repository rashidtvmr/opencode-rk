use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskQuota {
    pub limit: usize,
    pub used: usize,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TaskQuotaError {
    #[error("task limit must be non-zero")]
    ZeroLimit,
    #[error("task quota exceeded: limit {limit}, used {used}")]
    OverQuota { limit: usize, used: usize },
}

pub fn check_task_quota(q: &TaskQuota) -> Result<usize, TaskQuotaError> {
    if q.limit == 0 {
        return Err(TaskQuotaError::ZeroLimit);
    }
    if q.used > q.limit {
        return Err(TaskQuotaError::OverQuota {
            limit: q.limit,
            used: q.used,
        });
    }
    Ok(q.limit - q.used)
}

pub fn consume_task(q: &mut TaskQuota) -> Result<(), TaskQuotaError> {
    check_task_quota(q)?;
    if q.used == q.limit {
        return Err(TaskQuotaError::OverQuota {
            limit: q.limit,
            used: q.used,
        });
    }
    q.used = q.used.saturating_add(1);
    Ok(())
}

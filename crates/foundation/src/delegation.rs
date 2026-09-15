//! Lightweight delegation-budget planner (REQ-024).
//!
//! Pure planner: decides whether a subtask may spawn given memory/depth
//! limits. No spawning, no IO, no threads.

use thiserror::Error;

pub const MAX_DELEGATION_MB: u64 = 512;
pub const MAX_DELEGATION_DEPTH: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationRequest {
    pub task_kind: String,
    pub estimated_mb: u64,
    pub depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationPlan {
    pub allowed: bool,
    pub reason: String,
    pub max_depth: u32,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DelegationError {
    #[error("task kind must not be empty")]
    EmptyKind,
    #[error("delegation budget exceeded: limit {limit_mb} MB, asked {asked_mb} MB")]
    BudgetExceeded { limit_mb: u64, asked_mb: u64 },
    #[error("delegation max depth exceeded: max {max}, asked {asked}")]
    MaxDepthExceeded { max: u32, asked: u32 },
}

pub fn plan_delegation(req: &DelegationRequest) -> Result<DelegationPlan, DelegationError> {
    if req.task_kind.is_empty() {
        return Err(DelegationError::EmptyKind);
    }
    if req.estimated_mb > MAX_DELEGATION_MB {
        return Err(DelegationError::BudgetExceeded {
            limit_mb: MAX_DELEGATION_MB,
            asked_mb: req.estimated_mb,
        });
    }
    if req.depth > MAX_DELEGATION_DEPTH {
        return Err(DelegationError::MaxDepthExceeded {
            max: MAX_DELEGATION_DEPTH,
            asked: req.depth,
        });
    }
    Ok(DelegationPlan {
        allowed: true,
        reason: "ok".to_string(),
        max_depth: MAX_DELEGATION_DEPTH,
    })
}

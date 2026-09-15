use opencode_rk_foundation::delegation::{
    plan_delegation, DelegationError, DelegationRequest, MAX_DELEGATION_DEPTH, MAX_DELEGATION_MB,
};

#[test]
fn deleg_t01_allowed_small_task() {
    let req = DelegationRequest {
        task_kind: "summarize".to_string(),
        estimated_mb: 64,
        depth: 1,
    };
    let plan = plan_delegation(&req).expect("small task must be allowed");
    assert!(plan.allowed);
    assert_eq!(plan.reason, "ok");
    assert_eq!(plan.max_depth, MAX_DELEGATION_DEPTH);
}

#[test]
fn deleg_t02_empty_kind_rejected() {
    let req = DelegationRequest {
        task_kind: String::new(),
        estimated_mb: 10,
        depth: 0,
    };
    assert_eq!(
        plan_delegation(&req).unwrap_err(),
        DelegationError::EmptyKind
    );
}

#[test]
fn deleg_t03_budget_exceeded() {
    let req = DelegationRequest {
        task_kind: "big".to_string(),
        estimated_mb: MAX_DELEGATION_MB + 1,
        depth: 0,
    };
    assert_eq!(
        plan_delegation(&req).unwrap_err(),
        DelegationError::BudgetExceeded {
            limit_mb: MAX_DELEGATION_MB,
            asked_mb: MAX_DELEGATION_MB + 1,
        }
    );
}

#[test]
fn deleg_t04_depth_exceeded() {
    let req = DelegationRequest {
        task_kind: "nested".to_string(),
        estimated_mb: 10,
        depth: MAX_DELEGATION_DEPTH + 1,
    };
    assert_eq!(
        plan_delegation(&req).unwrap_err(),
        DelegationError::MaxDepthExceeded {
            max: MAX_DELEGATION_DEPTH,
            asked: MAX_DELEGATION_DEPTH + 1,
        }
    );
}

#[test]
fn deleg_t05_boundary_accepted() {
    let req = DelegationRequest {
        task_kind: "edge".to_string(),
        estimated_mb: MAX_DELEGATION_MB,
        depth: MAX_DELEGATION_DEPTH,
    };
    let plan = plan_delegation(&req).expect("boundary values must be allowed");
    assert!(plan.allowed);
    assert_eq!(plan.reason, "ok");
}

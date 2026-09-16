use opencode_rk_sessions::ops_limits::{check_limits, tasks_within, OpLimits, OpLimitsError};

#[test]
fn oplim_t01_ok() {
    let l = OpLimits {
        max_tasks: 4,
        max_mb: 64,
    };
    assert!(check_limits(&l).is_ok());
}

#[test]
fn oplim_t02_zero_tasks() {
    let l = OpLimits {
        max_tasks: 0,
        max_mb: 64,
    };
    assert!(matches!(check_limits(&l), Err(OpLimitsError::ZeroTasks)));
}

#[test]
fn oplim_t03_zero_mb() {
    let l = OpLimits {
        max_tasks: 4,
        max_mb: 0,
    };
    assert!(matches!(check_limits(&l), Err(OpLimitsError::ZeroMb)));
}

#[test]
fn oplim_t04_within() {
    let l = OpLimits {
        max_tasks: 4,
        max_mb: 64,
    };
    assert!(tasks_within(&l, 4));
    assert!(tasks_within(&l, 0));
}

#[test]
fn oplim_t05_over() {
    let l = OpLimits {
        max_tasks: 4,
        max_mb: 64,
    };
    assert!(!tasks_within(&l, 5));
}

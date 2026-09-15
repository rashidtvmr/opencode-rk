use opencode_rk_sessions::task_quota::{TaskQuota, TaskQuotaError, check_task_quota, consume_task};

#[test]
fn tsq_t01_remaining() {
    let q = TaskQuota { limit: 5, used: 2 };
    assert_eq!(check_task_quota(&q).unwrap(), 3);
}

#[test]
fn tsq_t02_consume() {
    let mut q = TaskQuota { limit: 5, used: 2 };
    consume_task(&mut q).unwrap();
    assert_eq!(q.used, 3);
    assert_eq!(check_task_quota(&q).unwrap(), 2);
}

#[test]
fn tsq_t03_over() {
    let q = TaskQuota { limit: 2, used: 3 };
    assert_eq!(
        check_task_quota(&q).unwrap_err(),
        TaskQuotaError::OverQuota { limit: 2, used: 3 }
    );
    let mut q2 = TaskQuota { limit: 2, used: 2 };
    assert_eq!(
        consume_task(&mut q2).unwrap_err(),
        TaskQuotaError::OverQuota { limit: 2, used: 2 }
    );
    assert_eq!(q2.used, 2);
}

#[test]
fn tsq_t04_zero() {
    let q = TaskQuota { limit: 0, used: 0 };
    assert_eq!(check_task_quota(&q).unwrap_err(), TaskQuotaError::ZeroLimit);
    let mut q2 = TaskQuota { limit: 0, used: 0 };
    assert_eq!(consume_task(&mut q2).unwrap_err(), TaskQuotaError::ZeroLimit);
    assert_eq!(q2.used, 0);
}

#[test]
fn tsq_t05_saturating() {
    let mut q = TaskQuota {
        limit: usize::MAX,
        used: usize::MAX,
    };
    assert_eq!(check_task_quota(&q).unwrap(), 0);
    assert_eq!(
        consume_task(&mut q).unwrap_err(),
        TaskQuotaError::OverQuota {
            limit: usize::MAX,
            used: usize::MAX,
        }
    );
    assert_eq!(q.used, usize::MAX);
}

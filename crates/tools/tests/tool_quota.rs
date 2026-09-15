use opencode_rk_tools::tool_quota::{QuotaError, ToolQuota, check_quota, consume_quota};

fn quota(tool: &str, limit: u64, used: u64) -> ToolQuota {
    ToolQuota {
        tool: tool.to_string(),
        limit,
        used,
    }
}

#[test]
fn quo_t01_remaining() {
    let q = quota("grep", 10, 3);
    assert_eq!(check_quota(&q), Ok(7));
    let empty = quota("", 10, 3);
    assert!(matches!(check_quota(&empty), Err(QuotaError::EmptyTool)));
}

#[test]
fn quo_t02_consume() {
    let mut q = quota("grep", 3, 0);
    consume_quota(&mut q).expect("consume ok");
    assert_eq!(q.used, 1);
    assert_eq!(check_quota(&q), Ok(2));
}

#[test]
fn quo_t03_over() {
    let q = quota("grep", 2, 3);
    match check_quota(&q) {
        Err(QuotaError::OverQuota { limit, used }) => {
            assert_eq!((limit, used), (2, 3));
        }
        other => panic!("expected OverQuota, got {other:?}"),
    }
    let mut q2 = quota("grep", 2, 3);
    match consume_quota(&mut q2) {
        Err(QuotaError::OverQuota { limit, used }) => {
            assert_eq!((limit, used), (2, 3));
        }
        other => panic!("expected OverQuota, got {other:?}"),
    }
    assert_eq!(q2.used, 3);
}

#[test]
fn quo_t04_zero() {
    let q = quota("grep", 0, 0);
    assert!(matches!(check_quota(&q), Err(QuotaError::ZeroLimit)));
    let mut q2 = quota("grep", 0, 0);
    assert!(matches!(consume_quota(&mut q2), Err(QuotaError::ZeroLimit)));
    assert_eq!(q2.used, 0);
}

#[test]
fn quo_t05_saturating() {
    let mut q = quota("grep", 2, 0);
    consume_quota(&mut q).expect("first ok");
    consume_quota(&mut q).expect("second ok");
    assert_eq!(q.used, 2);
    assert_eq!(check_quota(&q), Ok(0));
    match consume_quota(&mut q) {
        Err(QuotaError::OverQuota { limit, used }) => {
            assert_eq!((limit, used), (2, 2));
        }
        other => panic!("expected OverQuota at full, got {other:?}"),
    }
    assert_eq!(q.used, 2);
    let mut edge = quota("grep", u64::MAX, u64::MAX - 1);
    consume_quota(&mut edge).expect("saturating edge ok");
    assert_eq!(edge.used, u64::MAX);
    assert_eq!(check_quota(&edge), Ok(0));
}

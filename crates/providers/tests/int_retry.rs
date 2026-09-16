use opencode_rk_providers::int_retry::{plan_retry, RetryError, MAX_ATTEMPTS};

#[test]
fn intret_t01_plan() {
    let plan = plan_retry(3, 100).expect("3 attempts valid");
    assert_eq!(plan.attempts, 3);
    assert_eq!(plan.backoff_ms, 300);
}

#[test]
fn intret_t02_zero() {
    match plan_retry(0, 100) {
        Err(RetryError::ZeroAttempts) => {}
        other => panic!("expected ZeroAttempts, got {other:?}"),
    }
}

#[test]
fn intret_t03_overflow() {
    let plan = plan_retry(8, u64::MAX).expect("saturating backoff valid");
    assert_eq!(plan.attempts, 8);
    assert_eq!(plan.backoff_ms, 60_000);
}

#[test]
fn intret_t04_capped() {
    let plan = plan_retry(8, 10_000).expect("capped backoff valid");
    assert_eq!(plan.backoff_ms, 60_000);
}

#[test]
fn intret_t05_boundary() {
    assert_eq!(MAX_ATTEMPTS, 8);
    let ok = plan_retry(8, 100).expect("MAX_ATTEMPTS valid");
    assert_eq!(ok.attempts, 8);
    assert_eq!(ok.backoff_ms, 800);
    match plan_retry(9, 100) {
        Err(RetryError::TooManyAttempts { max, asked }) => {
            assert_eq!(max, 8);
            assert_eq!(asked, 9);
        }
        other => panic!("expected TooManyAttempts, got {other:?}"),
    }
}

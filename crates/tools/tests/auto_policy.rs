use opencode_rk_tools::auto_policy::{AutoPolicyError, make_policy};

#[test]
fn pol_t01_valid() {
    let p = make_policy(3, 100).expect("valid policy");
    assert_eq!(p.max_retries, 3);
    assert_eq!(p.backoff_ms, 100);
}

#[test]
fn pol_t02_too_many() {
    let err = make_policy(11, 100).expect_err("too many rejected");
    assert!(matches!(err, AutoPolicyError::TooManyRetries));
}

#[test]
fn pol_t03_zero_backoff() {
    let err = make_policy(3, 0).expect_err("zero backoff rejected");
    assert!(matches!(err, AutoPolicyError::ZeroBackoff));
}

#[test]
fn pol_t04_boundary_10_ok() {
    let p = make_policy(10, 50).expect("boundary 10 accepted");
    assert_eq!(p.max_retries, 10);
    assert_eq!(p.backoff_ms, 50);
}

#[test]
fn pol_t05_11_err() {
    let err = make_policy(11, 50).expect_err("11 rejected");
    assert_eq!(err, AutoPolicyError::TooManyRetries);
}

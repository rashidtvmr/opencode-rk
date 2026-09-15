use opencode_rk_foundation::ops_health::{check_health, is_healthy};

#[test]
fn hlt_t01_healthy() {
    let h = check_health(0, 3);
    assert!(h.ok);
}

#[test]
fn hlt_t02_unhealthy() {
    let h = check_health(2, 3);
    assert!(!h.ok);
}

#[test]
fn hlt_t03_checked_kept() {
    let h = check_health(0, 7);
    assert_eq!(h.checked, 7);
}

#[test]
fn hlt_t04_zero() {
    let h = check_health(0, 0);
    assert!(h.ok);
    assert_eq!(h.checked, 0);
}

#[test]
fn hlt_t05_predicate() {
    let healthy = check_health(0, 1);
    assert!(is_healthy(&healthy));
    let sick = check_health(1, 1);
    assert!(!is_healthy(&sick));
}

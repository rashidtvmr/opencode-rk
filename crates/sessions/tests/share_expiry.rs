use opencode_rk_sessions::share_expiry::{check_expiry, expiry_at, ExpiryError};

#[test]
fn exp_t01_future() {
    assert_eq!(expiry_at(1000, 60).unwrap(), 1060);
    assert_eq!(expiry_at(0, 1).unwrap(), 1);
    assert_eq!(expiry_at(u64::MAX, 1).unwrap(), u64::MAX);
}

#[test]
fn exp_t02_zero_ttl() {
    assert!(matches!(expiry_at(1000, 0), Err(ExpiryError::ZeroTtl)));
    assert!(matches!(expiry_at(0, 0), Err(ExpiryError::ZeroTtl)));
}

#[test]
fn exp_t03_valid() {
    assert!(check_expiry(1000, 1060).is_ok());
    assert!(check_expiry(500, 1060).is_ok());
}

#[test]
fn exp_t04_expired() {
    assert!(matches!(
        check_expiry(1061, 1060),
        Err(ExpiryError::Expired {
            now: 1061,
            exp: 1060
        })
    ));
}

#[test]
fn exp_t05_boundary_now_eq_exp_ok() {
    assert!(check_expiry(1060, 1060).is_ok());
}

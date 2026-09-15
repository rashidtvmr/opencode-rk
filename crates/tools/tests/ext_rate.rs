use opencode_rk_tools::ext_rate::{RateError, RateWindow, check_window, consume};

#[test]
fn rate_t01_remaining() {
    let w = RateWindow { limit: 10, used: 3 };
    assert_eq!(check_window(&w), Ok(7));
}

#[test]
fn rate_t02_consume() {
    let mut w = RateWindow { limit: 3, used: 0 };
    consume(&mut w).expect("consume ok");
    assert_eq!(w.used, 1);
    assert_eq!(check_window(&w), Ok(2));
}

#[test]
fn rate_t03_over_rejected() {
    let w = RateWindow { limit: 2, used: 3 };
    match check_window(&w) {
        Err(RateError::OverLimit { limit, used }) => {
            assert_eq!((limit, used), (2, 3));
        }
        other => panic!("expected OverLimit, got {other:?}"),
    }
    let mut w2 = RateWindow { limit: 2, used: 3 };
    match consume(&mut w2) {
        Err(RateError::OverLimit { limit, used }) => {
            assert_eq!((limit, used), (2, 3));
        }
        other => panic!("expected OverLimit, got {other:?}"),
    }
    assert_eq!(w2.used, 3);
}

#[test]
fn rate_t04_zero_limit() {
    let w = RateWindow { limit: 0, used: 0 };
    assert!(matches!(check_window(&w), Err(RateError::ZeroLimit)));
    let mut w2 = RateWindow { limit: 0, used: 0 };
    assert!(matches!(consume(&mut w2), Err(RateError::ZeroLimit)));
    assert_eq!(w2.used, 0);
}

#[test]
fn rate_t05_consume_fills() {
    let mut w = RateWindow { limit: 2, used: 0 };
    consume(&mut w).expect("first ok");
    consume(&mut w).expect("second ok");
    assert_eq!(w.used, 2);
    assert_eq!(check_window(&w), Ok(0));
    match consume(&mut w) {
        Err(RateError::OverLimit { limit, used }) => {
            assert_eq!((limit, used), (2, 2));
        }
        other => panic!("expected OverLimit at full, got {other:?}"),
    }
    assert_eq!(w.used, 2);
}

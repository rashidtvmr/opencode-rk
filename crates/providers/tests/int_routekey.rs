use opencode_rk_providers::int_routekey::{qualify_routekey, RouteKeyError};

#[test]
fn rk_t01_valid() {
    assert_eq!(qualify_routekey("abc").expect("valid"), "abc");
    assert_eq!(qualify_routekey("a1-b2").expect("valid"), "a1-b2");
    assert_eq!(
        qualify_routekey("route-42-ok").expect("valid"),
        "route-42-ok"
    );
}

#[test]
fn rk_t02_empty() {
    match qualify_routekey("") {
        Err(RouteKeyError::EmptyKey) => {}
        other => panic!("expected EmptyKey, got {other:?}"),
    }
}

#[test]
fn rk_t03_bad_upper() {
    match qualify_routekey("Abc") {
        Err(RouteKeyError::BadKey) => {}
        other => panic!("expected BadKey, got {other:?}"),
    }
    match qualify_routekey("-abc") {
        Err(RouteKeyError::BadKey) => {}
        other => panic!("expected BadKey, got {other:?}"),
    }
    match qualify_routekey("a_b") {
        Err(RouteKeyError::BadKey) => {}
        other => panic!("expected BadKey, got {other:?}"),
    }
}

#[test]
fn rk_t04_too_short() {
    match qualify_routekey("ab") {
        Err(RouteKeyError::BadKey) => {}
        other => panic!("expected BadKey, got {other:?}"),
    }
}

#[test]
fn rk_t05_too_long() {
    let long = "a".repeat(49);
    match qualify_routekey(&long) {
        Err(RouteKeyError::BadKey) => {}
        other => panic!("expected BadKey, got {other:?}"),
    }
    let maxed = "a".repeat(48);
    assert_eq!(qualify_routekey(&maxed).expect("48 ok"), maxed);
}

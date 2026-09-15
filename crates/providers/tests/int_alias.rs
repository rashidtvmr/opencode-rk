use opencode_rk_providers::int_alias::{AliasError, resolve_alias};

#[test]
fn als_t01_hit() {
    let pairs = [("a", "x"), ("b", "y")];
    assert_eq!(resolve_alias(&pairs, "a").expect("hit"), "x");
    assert_eq!(resolve_alias(&pairs, "b").expect("hit"), "y");
}

#[test]
fn als_t02_empty() {
    match resolve_alias(&[("a", "x")], "") {
        Err(AliasError::EmptyAlias) => {}
        other => panic!("expected EmptyAlias, got {other:?}"),
    }
    match resolve_alias(&[("", "x")], "a") {
        Err(AliasError::EmptyAlias) => {}
        other => panic!("expected EmptyAlias, got {other:?}"),
    }
}

#[test]
fn als_t03_empty_target() {
    match resolve_alias(&[("a", "")], "a") {
        Err(AliasError::EmptyTarget) => {}
        other => panic!("expected EmptyTarget, got {other:?}"),
    }
}

#[test]
fn als_t04_unknown_is_emptytarget() {
    match resolve_alias(&[("a", "x")], "zzz") {
        Err(AliasError::EmptyTarget) => {}
        other => panic!("expected EmptyTarget, got {other:?}"),
    }
}

#[test]
fn als_t05_first_wins() {
    let pairs = [("a", "x"), ("a", "y")];
    assert_eq!(resolve_alias(&pairs, "a").expect("first wins"), "x");
}

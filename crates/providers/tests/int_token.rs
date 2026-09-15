use opencode_rk_providers::int_token::{TokenShapeError, qualify_shape};

#[test]
fn tks_t01_len() {
    let ok = qualify_shape("abcdefghijklmnop").expect("16 valid chars");
    assert_eq!(ok, 16);
    let mixed = qualify_shape("abCD09.-_~efGH12").expect("mixed charset valid");
    assert_eq!(mixed, 16);
    let max = qualify_shape(&"a".repeat(256)).expect("256 valid");
    assert_eq!(max, 256);
}

#[test]
fn tks_t02_empty() {
    match qualify_shape("") {
        Err(TokenShapeError::EmptyToken) => {}
        other => panic!("expected EmptyToken, got {other:?}"),
    }
}

#[test]
fn tks_t03_bad_chars() {
    match qualify_shape("abcdefghijklmnop!") {
        Err(TokenShapeError::BadToken) => {}
        other => panic!("expected BadToken, got {other:?}"),
    }
    match qualify_shape("abcdefghijklm op") {
        Err(TokenShapeError::BadToken) => {}
        other => panic!("expected BadToken, got {other:?}"),
    }
}

#[test]
fn tks_t04_too_short() {
    match qualify_shape("abc") {
        Err(TokenShapeError::BadToken) => {}
        other => panic!("expected BadToken, got {other:?}"),
    }
    match qualify_shape(&"a".repeat(15)) {
        Err(TokenShapeError::BadToken) => {}
        other => panic!("expected BadToken, got {other:?}"),
    }
}

#[test]
fn tks_t05_too_long() {
    match qualify_shape(&"a".repeat(257)) {
        Err(TokenShapeError::BadToken) => {}
        other => panic!("expected BadToken, got {other:?}"),
    }
}

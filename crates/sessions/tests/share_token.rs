use opencode_rk_sessions::share_token::{TokenError, qualify_token};

#[test]
fn tok_t01_valid() {
    assert_eq!(qualify_token("abcDEF12").unwrap(), "abcDEF12");
    assert_eq!(qualify_token("ab-c_dE12").unwrap(), "ab-c_dE12");
    assert_eq!(qualify_token("  abcDEF12  ").unwrap(), "abcDEF12");
}

#[test]
fn tok_t02_empty() {
    assert!(matches!(
        qualify_token(""),
        Err(TokenError::EmptyToken)
    ));
    assert!(matches!(
        qualify_token("   "),
        Err(TokenError::EmptyToken)
    ));
}

#[test]
fn tok_t03_bad_chars() {
    assert!(matches!(
        qualify_token("has space!"),
        Err(TokenError::BadToken)
    ));
    assert!(matches!(
        qualify_token("has/slash1"),
        Err(TokenError::BadToken)
    ));
    assert!(matches!(
        qualify_token("tok!bad12"),
        Err(TokenError::BadToken)
    ));
}

#[test]
fn tok_t04_too_short() {
    assert!(matches!(
        qualify_token("abc1234"),
        Err(TokenError::BadToken)
    ));
}

#[test]
fn tok_t05_too_long() {
    let long = "a".repeat(65);
    assert!(matches!(
        qualify_token(&long),
        Err(TokenError::BadToken)
    ));
}

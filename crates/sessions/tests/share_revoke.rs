use opencode_rk_sessions::share_revoke::{revoke_token, RevokeError};

#[test]
fn rev_t01_revoke() {
    let mut tokens = vec!["abc".to_owned(), "def".to_owned()];
    assert!(revoke_token(&mut tokens, "abc").is_ok());
    assert_eq!(tokens, vec!["def".to_owned()]);
}

#[test]
fn rev_t02_empty() {
    let mut tokens = vec!["abc".to_owned()];
    assert!(matches!(
        revoke_token(&mut tokens, ""),
        Err(RevokeError::EmptyToken)
    ));
    assert_eq!(tokens, vec!["abc".to_owned()]);
}

#[test]
fn rev_t03_unknown() {
    let mut tokens = vec!["abc".to_owned()];
    assert!(matches!(
        revoke_token(&mut tokens, "zzz"),
        Err(RevokeError::UnknownToken)
    ));
    assert_eq!(tokens, vec!["abc".to_owned()]);
}

#[test]
fn rev_t04_removes_only_one() {
    let mut tokens = vec!["a".to_owned(), "a".to_owned(), "b".to_owned()];
    assert!(revoke_token(&mut tokens, "a").is_ok());
    assert_eq!(tokens, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn rev_t05_empty_list_unknown() {
    let mut tokens: Vec<String> = Vec::new();
    assert!(matches!(
        revoke_token(&mut tokens, "zzz"),
        Err(RevokeError::UnknownToken)
    ));
    assert!(tokens.is_empty());
}

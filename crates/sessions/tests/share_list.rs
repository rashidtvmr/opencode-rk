use opencode_rk_sessions::share_list::{count_tokens, has_token};

#[test]
fn shl_t01_count() {
    let tokens = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    assert_eq!(count_tokens(&tokens), 3);
}

#[test]
fn shl_t02_has() {
    let tokens = vec!["abc".to_owned(), "def".to_owned()];
    assert!(has_token(&tokens, "abc"));
}

#[test]
fn shl_t03_empty_false() {
    let tokens = vec!["abc".to_owned()];
    assert!(!has_token(&tokens, ""));
}

#[test]
fn shl_t04_missing() {
    let tokens = vec!["abc".to_owned()];
    assert!(!has_token(&tokens, "zzz"));
}

#[test]
fn shl_t05_grows() {
    let mut tokens: Vec<String> = Vec::new();
    assert_eq!(count_tokens(&tokens), 0);
    tokens.push("a".to_owned());
    tokens.push("b".to_owned());
    assert_eq!(count_tokens(&tokens), 2);
    assert!(has_token(&tokens, "b"));
}

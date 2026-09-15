use opencode_rk_sessions::share_scope::{MAX_SCOPES, ScopeError, grant_scope, has_scope};

#[test]
fn scp_t01_grant() {
    let mut list = Vec::new();
    grant_scope(&mut list, "read").unwrap();
    assert!(has_scope(&list, "read"));
}

#[test]
fn scp_t02_empty() {
    let mut list = Vec::new();
    assert!(matches!(
        grant_scope(&mut list, ""),
        Err(ScopeError::EmptyScope)
    ));
}

#[test]
fn scp_t03_dup_ok() {
    let mut list = vec!["read".to_owned()];
    grant_scope(&mut list, "read").unwrap();
    assert_eq!(list.len(), 1);
}

#[test]
fn scp_t04_overflow() {
    let mut list: Vec<String> = (0..MAX_SCOPES).map(|i| format!("s{i}")).collect();
    let err = grant_scope(&mut list, "extra").unwrap_err();
    assert!(matches!(
        err,
        ScopeError::TooMany { max, actual } if max == MAX_SCOPES && actual == MAX_SCOPES
    ));
}

#[test]
fn scp_t05_missing() {
    let list = vec!["read".to_owned()];
    assert!(!has_scope(&list, "write"));
    assert!(!has_scope(&[], "read"));
}

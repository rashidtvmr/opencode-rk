use opencode_rk_tools::tool_allow::{AllowError, MAX_TOOL_ALLOW, allow_tool, is_allowed};

#[test]
fn alw_t01_allow() {
    let mut list = Vec::new();
    assert_eq!(allow_tool(&mut list, "read"), Ok(()));
    assert_eq!(list, vec!["read".to_string()]);
}

#[test]
fn alw_t02_empty() {
    let mut list = Vec::new();
    assert_eq!(allow_tool(&mut list, ""), Err(AllowError::EmptyName));
    assert!(list.is_empty());
}

#[test]
fn alw_t03_dup_ok() {
    let mut list = vec!["read".to_string()];
    assert_eq!(allow_tool(&mut list, "read"), Ok(()));
    assert_eq!(list.len(), 1);
}

#[test]
fn alw_t04_overflow() {
    let mut list: Vec<String> = (0..MAX_TOOL_ALLOW).map(|i| format!("tool-{i}")).collect();
    assert_eq!(
        allow_tool(&mut list, "one-more"),
        Err(AllowError::TooMany {
            max: MAX_TOOL_ALLOW,
            actual: MAX_TOOL_ALLOW
        })
    );
    assert_eq!(list.len(), MAX_TOOL_ALLOW);
}

#[test]
fn alw_t05_denied() {
    let list = vec!["read".to_string()];
    assert!(is_allowed(&list, "read"));
    assert!(!is_allowed(&list, "write"));
    assert!(!is_allowed(&[], "read"));
}

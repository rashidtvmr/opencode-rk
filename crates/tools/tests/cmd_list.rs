use opencode_rk_tools::cmd_list::{MAX_CMD_LIST, add_command, has_command};

#[test]
fn cml_t01_add_has() {
    let mut list = Vec::new();
    add_command(&mut list, "review").unwrap();
    assert!(has_command(&list, "review"));
}

#[test]
fn cml_t02_empty() {
    let mut list = Vec::new();
    let err = add_command(&mut list, "").unwrap_err();
    assert_eq!(err, opencode_rk_tools::cmd_list::CmdListError::EmptyName);
}

#[test]
fn cml_t03_dup_ok() {
    let mut list = Vec::new();
    add_command(&mut list, "review").unwrap();
    add_command(&mut list, "review").unwrap();
    assert_eq!(list.len(), 1);
}

#[test]
fn cml_t04_overflow() {
    let mut list: Vec<String> = (0..MAX_CMD_LIST).map(|i| format!("cmd-{i}")).collect();
    let err = add_command(&mut list, "one-more").unwrap_err();
    assert_eq!(
        err,
        opencode_rk_tools::cmd_list::CmdListError::TooMany {
            max: MAX_CMD_LIST,
            actual: MAX_CMD_LIST + 1
        }
    );
}

#[test]
fn cml_t05_missing() {
    let list: Vec<String> = vec!["review".to_string()];
    assert!(!has_command(&list, "nope"));
}

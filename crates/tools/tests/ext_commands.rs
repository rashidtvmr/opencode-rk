use opencode_rk_tools::ext_commands::{ExtCmdError, MAX_EXT_CMDS, qualify_commands};

#[test]
fn extcmd_t01_valid() {
    let out = qualify_commands(&["/review", "/ship"]).expect("valid");
    assert_eq!(out, vec!["/review".to_string(), "/ship".to_string()]);
}

#[test]
fn extcmd_t02_empty_rejected() {
    assert_eq!(qualify_commands(&[""]), Err(ExtCmdError::EmptyName));
}

#[test]
fn extcmd_t03_bad_no_slash() {
    assert_eq!(qualify_commands(&["review"]), Err(ExtCmdError::BadName));
    assert_eq!(qualify_commands(&["/"]), Err(ExtCmdError::BadName));
}

#[test]
fn extcmd_t04_spaces_rejected() {
    assert_eq!(qualify_commands(&["/my cmd"]), Err(ExtCmdError::BadName));
}

#[test]
fn extcmd_t05_overflow() {
    let many: Vec<String> = (0..MAX_EXT_CMDS + 1).map(|i| format!("/c{i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    assert_eq!(
        qualify_commands(&refs),
        Err(ExtCmdError::TooManyCmds {
            max: MAX_EXT_CMDS,
            actual: MAX_EXT_CMDS + 1
        })
    );
}

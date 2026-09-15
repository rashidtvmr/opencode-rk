use opencode_rk_tools::tool_sandbox::{SandboxError, SandboxFlag, set_sandbox};

#[test]
fn sbx_t01_add() {
    let mut flags = Vec::new();
    assert_eq!(set_sandbox(&mut flags, "exec", true), Ok(true));
    assert_eq!(flags.len(), 1);
    assert_eq!(
        flags[0],
        SandboxFlag { tool: "exec".to_string(), sandboxed: true }
    );
}

#[test]
fn sbx_t02_toggle() {
    let mut flags = vec![SandboxFlag { tool: "exec".to_string(), sandboxed: true }];
    assert_eq!(set_sandbox(&mut flags, "exec", false), Ok(false));
    assert_eq!(flags.len(), 1);
    assert!(!flags[0].sandboxed);
}

#[test]
fn sbx_t03_empty() {
    let mut flags = Vec::new();
    assert_eq!(set_sandbox(&mut flags, "", true), Err(SandboxError::EmptyTool));
    assert!(flags.is_empty());
}

#[test]
fn sbx_t04_missing_adds() {
    let mut flags = Vec::new();
    assert_eq!(set_sandbox(&mut flags, "read", false), Ok(false));
    assert_eq!(flags.len(), 1);
    assert_eq!(
        flags[0],
        SandboxFlag { tool: "read".to_string(), sandboxed: false }
    );
}

#[test]
fn sbx_t05_state() {
    let mut flags = Vec::new();
    assert_eq!(set_sandbox(&mut flags, "exec", true), Ok(true));
    assert_eq!(set_sandbox(&mut flags, "exec", true), Ok(true));
    assert_eq!(flags.len(), 1);
    assert!(flags[0].sandboxed);
    assert_eq!(set_sandbox(&mut flags, "exec", false), Ok(false));
    assert!(!flags[0].sandboxed);
}

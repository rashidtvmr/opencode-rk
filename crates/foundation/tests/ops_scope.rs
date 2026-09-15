use opencode_rk_foundation::ops_scope::{OpsScopeError, build_scope};

#[test]
fn ops_t01_valid() {
    let scope = build_scope("my-project", false).unwrap();
    assert_eq!(scope.project, "my-project");
    assert!(!scope.read_only);
}

#[test]
fn ops_t02_readonly_kept() {
    let scope = build_scope("my-project", true).unwrap();
    assert!(scope.read_only);
}

#[test]
fn ops_t03_empty_rejected() {
    assert!(matches!(
        build_scope("", false),
        Err(OpsScopeError::EmptyProject)
    ));
}

#[test]
fn ops_t04_trims() {
    let scope = build_scope("  my-project  ", false).unwrap();
    assert_eq!(scope.project, "my-project");
}

#[test]
fn ops_t05_whitespace_rejected() {
    assert!(matches!(
        build_scope("   ", false),
        Err(OpsScopeError::EmptyProject)
    ));
}

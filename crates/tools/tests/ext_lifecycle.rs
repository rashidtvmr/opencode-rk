use opencode_rk_tools::ext_lifecycle::{ExtLifecycleError, plan_transition};

#[test]
fn ext_t01_install() {
    assert_eq!(
        plan_transition("myext", "install").unwrap(),
        "myext:install"
    );
}

#[test]
fn ext_t02_enable() {
    assert_eq!(plan_transition("myext", "enable").unwrap(), "myext:enable");
}

#[test]
fn ext_t03_unknown_phase() {
    assert_eq!(
        plan_transition("myext", "bogus"),
        Err(ExtLifecycleError::UnknownPhase {
            name: "bogus".to_string()
        })
    );
}

#[test]
fn ext_t04_empty_name() {
    assert_eq!(
        plan_transition("", "install"),
        Err(ExtLifecycleError::EmptyName)
    );
}

#[test]
fn ext_t05_case_insensitive() {
    assert_eq!(
        plan_transition("myext", "  INSTALL ").unwrap(),
        "myext:install"
    );
    assert_eq!(
        plan_transition("myext", "Disable").unwrap(),
        "myext:disable"
    );
    assert_eq!(
        plan_transition("myext", "UNINSTALL").unwrap(),
        "myext:uninstall"
    );
}

use opencode_rk_providers::int_status::{label_state, report_status, require_id, IntState};

#[test]
fn intst_t01_up() {
    assert!(matches!(report_status(Some(true)), IntState::Up));
}

#[test]
fn intst_t02_down() {
    assert!(matches!(report_status(Some(false)), IntState::Down));
}

#[test]
fn intst_t03_unknown() {
    assert!(matches!(report_status(None), IntState::Unknown));
}

#[test]
fn intst_t04_labels() {
    assert_eq!(label_state(&IntState::Up), "up");
    assert_eq!(label_state(&IntState::Down), "down");
    assert_eq!(label_state(&IntState::Unknown), "unknown");
}

#[test]
fn intst_t05_empty_id() {
    assert!(require_id("").is_err());
    assert!(require_id("   ").is_err());
    assert_eq!(require_id("  abc  ").unwrap(), "abc");
}

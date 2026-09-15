use opencode_rk_sessions::ui_005::{SteerAction, SteerError, SteerState};

#[test]
fn ui005_t01_interrupt_when_busy() {
    let mut s = SteerState::new();
    s.busy = true;
    let out = s.request(SteerAction::Interrupt).unwrap();
    assert_eq!(out, "interrupted");
    assert!(!s.busy);
    assert_eq!(s.last_action.as_deref(), Some("interrupted"));
}

#[test]
fn ui005_t02_resume_when_idle() {
    let mut s = SteerState::new();
    assert!(!s.busy);
    let out = s.request(SteerAction::Resume).unwrap();
    assert_eq!(out, "resumed");
    assert!(s.busy);
    assert_eq!(s.last_action.as_deref(), Some("resumed"));
}

#[test]
fn ui005_t03_interrupt_idle_rejected() {
    let mut s = SteerState::new();
    assert!(!s.busy);
    assert!(matches!(
        s.request(SteerAction::Interrupt),
        Err(SteerError::NotBusy)
    ));
    assert!(!s.busy);
    assert_eq!(s.last_action, None);
}

#[test]
fn ui005_t04_resume_busy_rejected() {
    let mut s = SteerState::new();
    s.busy = true;
    assert!(matches!(
        s.request(SteerAction::Resume),
        Err(SteerError::AlreadyBusy)
    ));
    assert!(matches!(
        s.request(SteerAction::Retry),
        Err(SteerError::AlreadyBusy)
    ));
    assert!(s.busy);
}

#[test]
fn ui005_t05_retry_cycle() {
    let mut s = SteerState::new();
    let out = s.request(SteerAction::Retry).unwrap();
    assert_eq!(out, "retried");
    assert!(s.busy);
    assert_eq!(s.last_action.as_deref(), Some("retried"));
    let out = s.request(SteerAction::Interrupt).unwrap();
    assert_eq!(out, "interrupted");
    assert!(!s.busy);
    assert_eq!(s.last_action.as_deref(), Some("interrupted"));
    let out = s.request(SteerAction::Resume).unwrap();
    assert_eq!(out, "resumed");
    assert!(s.busy);
    assert_eq!(s.last_action.as_deref(), Some("resumed"));
}

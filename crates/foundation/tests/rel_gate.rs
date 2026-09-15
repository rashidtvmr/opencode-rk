use opencode_rk_foundation::rel_gate::{gate_for, gate_label, require_suite, GateError, GateState};

#[test]
fn gate_t01_open() {
    assert_eq!(gate_for(0), GateState::Open);
}

#[test]
fn gate_t02_closed() {
    assert_eq!(gate_for(1), GateState::Closed);
    assert_eq!(gate_for(u64::MAX), GateState::Closed);
}

#[test]
fn gate_t03_labels() {
    assert_eq!(gate_label(&GateState::Open), "open");
    assert_eq!(gate_label(&GateState::Closed), "closed");
}

#[test]
fn gate_t04_empty() {
    assert_eq!(require_suite(""), Err(GateError::EmptyName));
    assert_eq!(require_suite("   "), Err(GateError::EmptyName));
}

#[test]
fn gate_t05_trims() {
    assert_eq!(require_suite("  suite-a  "), Ok("suite-a".to_string()));
    assert_eq!(require_suite("x"), Ok("x".to_string()));
}

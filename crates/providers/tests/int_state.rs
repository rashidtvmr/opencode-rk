use opencode_rk_providers::int_state::{next_state, ConnState};

#[test]
fn cst_t01_open() {
    assert_eq!(next_state(&ConnState::Idle, true), ConnState::Open);
}

#[test]
fn cst_t02_idle_stays() {
    assert_eq!(next_state(&ConnState::Idle, false), ConnState::Idle);
}

#[test]
fn cst_t03_shut() {
    assert_eq!(next_state(&ConnState::Open, false), ConnState::Shut);
}

#[test]
fn cst_t04_shut_sticky() {
    assert_eq!(next_state(&ConnState::Shut, true), ConnState::Shut);
    assert_eq!(next_state(&ConnState::Shut, false), ConnState::Shut);
}

#[test]
fn cst_t05_open_stays() {
    assert_eq!(next_state(&ConnState::Open, true), ConnState::Open);
}

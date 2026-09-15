use opencode_rk_foundation::ops_guard::{enter, leave, new_guard, GuardError, OpsGuard};

#[test]
fn guard_t01_enter() {
    let mut g: OpsGuard = new_guard();
    assert!(!g.locked);
    assert!(enter(&mut g).is_ok());
    assert!(g.locked);
}

#[test]
fn guard_t02_double_enter_locked() {
    let mut g = new_guard();
    enter(&mut g).unwrap();
    assert!(matches!(enter(&mut g), Err(GuardError::Locked)));
    assert!(g.locked);
}

#[test]
fn guard_t03_leave_unlocks() {
    let mut g = new_guard();
    enter(&mut g).unwrap();
    leave(&mut g);
    assert!(!g.locked);
}

#[test]
fn guard_t04_leave_idempotent() {
    let mut g = new_guard();
    leave(&mut g);
    assert!(!g.locked);
    leave(&mut g);
    assert!(!g.locked);
}

#[test]
fn guard_t05_cycle() {
    let mut g = new_guard();
    enter(&mut g).unwrap();
    leave(&mut g);
    assert!(enter(&mut g).is_ok());
    assert!(g.locked);
}

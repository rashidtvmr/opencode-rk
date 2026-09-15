use opencode_rk_foundation::ops_state::{bump, make_state, OpsStateError};

#[test]
fn ops2_t01_make() {
    let s = make_state("alpha").unwrap();
    assert_eq!(s.name, "alpha");
    assert_eq!(s.rev, 0);
}

#[test]
fn ops2_t02_empty() {
    assert!(matches!(make_state(""), Err(OpsStateError::EmptyName)));
    assert!(matches!(make_state("   "), Err(OpsStateError::EmptyName)));
}

#[test]
fn ops2_t03_bump() {
    let mut s = make_state("alpha").unwrap();
    bump(&mut s);
    assert_eq!(s.rev, 1);
}

#[test]
fn ops2_t04_saturates() {
    let mut s = make_state("alpha").unwrap();
    s.rev = u64::MAX;
    bump(&mut s);
    assert_eq!(s.rev, u64::MAX);
}

#[test]
fn ops2_t05_trims() {
    let s = make_state("  alpha  ").unwrap();
    assert_eq!(s.name, "alpha");
}

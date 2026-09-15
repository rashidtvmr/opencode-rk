use opencode_rk_foundation::ops_lock::{acquire, release_lock, DeployLock, LockError};

fn unlocked() -> DeployLock {
    DeployLock {
        held: false,
        owner: String::new(),
    }
}

#[test]
fn lck_t01_acquire() {
    let mut l = unlocked();
    assert!(acquire(&mut l, "alice").is_ok());
    assert!(l.held);
    assert_eq!(l.owner, "alice");
}

#[test]
fn lck_t02_empty() {
    let mut l = unlocked();
    assert!(matches!(acquire(&mut l, ""), Err(LockError::EmptyOwner)));
    assert!(!l.held);
}

#[test]
fn lck_t03_locked() {
    let mut l = unlocked();
    acquire(&mut l, "alice").unwrap();
    assert!(matches!(acquire(&mut l, "bob"), Err(LockError::Locked)));
    assert_eq!(l.owner, "alice");
}

#[test]
fn lck_t04_release() {
    let mut l = unlocked();
    acquire(&mut l, "alice").unwrap();
    release_lock(&mut l);
    assert!(!l.held);
    assert!(l.owner.is_empty());
}

#[test]
fn lck_t05_reacquire() {
    let mut l = unlocked();
    acquire(&mut l, "alice").unwrap();
    release_lock(&mut l);
    assert!(acquire(&mut l, "bob").is_ok());
    assert!(l.held);
    assert_eq!(l.owner, "bob");
}

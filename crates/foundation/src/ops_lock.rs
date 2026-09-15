//! Ops deploy-lock slice: named lock with owner.
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeployLock {
    pub held: bool,
    pub owner: String,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LockError {
    #[error("owner must not be empty")]
    EmptyOwner,
    #[error("deploy lock is held")]
    Locked,
}

pub fn acquire(l: &mut DeployLock, owner: &str) -> Result<(), LockError> {
    if owner.is_empty() {
        return Err(LockError::EmptyOwner);
    }
    if l.held {
        return Err(LockError::Locked);
    }
    l.held = true;
    l.owner = owner.to_string();
    Ok(())
}

pub fn release_lock(l: &mut DeployLock) {
    l.held = false;
    l.owner.clear();
}

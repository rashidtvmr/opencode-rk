//! Ops guard slice: exclusive enter/leave mutex flag.
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpsGuard {
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GuardError {
    #[error("guard is locked")]
    Locked,
}

#[must_use]
pub fn new_guard() -> OpsGuard {
    OpsGuard { locked: false }
}

pub fn enter(g: &mut OpsGuard) -> Result<(), GuardError> {
    if g.locked {
        return Err(GuardError::Locked);
    }
    g.locked = true;
    Ok(())
}

pub fn leave(g: &mut OpsGuard) {
    g.locked = false;
}

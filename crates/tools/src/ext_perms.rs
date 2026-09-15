//! Extension permission grants (EXT permissions slice).
//!
//! Pure allow-list over [`ExtPerm`]: validates shape only, no I/O, no ambient
//! authority. Empty identifiers rejected fail-closed; the buffer is bounded by
//! [`MAX_EXT_PERMS`]; duplicate `(ext, perm)` grants are idempotent.

use thiserror::Error;

/// Maximum number of `(ext, perm)` entries held in one grant buffer.
pub const MAX_EXT_PERMS: usize = 256;

/// One granted permission: extension `ext` may exercise `perm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtPerm {
    pub ext: String,
    pub perm: String,
}

/// Permission-grant failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermError {
    #[error("extension id is empty")]
    EmptyExt,
    #[error("permission is empty")]
    EmptyPerm,
    #[error("too many extension permissions: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

/// Grant `(ext, perm)` into `buf`. Empty `ext`/`perm` rejected first, then
/// duplicate grants return `Ok(())` without growing the buffer, then the
/// capacity bound is enforced before pushing.
pub fn grant_perm(buf: &mut Vec<ExtPerm>, ext: &str, perm: &str) -> Result<(), PermError> {
    if ext.is_empty() {
        return Err(PermError::EmptyExt);
    }
    if perm.is_empty() {
        return Err(PermError::EmptyPerm);
    }
    if buf.iter().any(|p| p.ext == ext && p.perm == perm) {
        return Ok(());
    }
    if buf.len() >= MAX_EXT_PERMS {
        return Err(PermError::TooMany {
            max: MAX_EXT_PERMS,
            actual: buf.len(),
        });
    }
    buf.push(ExtPerm {
        ext: ext.to_string(),
        perm: perm.to_string(),
    });
    Ok(())
}

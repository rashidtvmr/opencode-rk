//! EXT secure permission grants: validate names, bound count, grant all.
//! Pure, no IO. Check order: length bound first, then per-name validation.

use thiserror::Error;

/// A granted secure permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurePerm {
    pub name: String,
    pub granted: bool,
}

/// Secure permission grant failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SecureError {
    #[error("empty permission name")]
    EmptyName,
    #[error("too many permissions: max {max}, got {actual}")]
    TooManyPerms { max: usize, actual: usize },
}

/// Maximum number of permissions grantable in one call.
pub const MAX_SECURE_PERMS: usize = 64;

/// Grant all named permissions. Preserves input order, marks granted=true.
/// Rejects oversized batches before inspecting names; any empty name fails.
pub fn grant_all(names: &[&str]) -> Result<Vec<SecurePerm>, SecureError> {
    if names.len() > MAX_SECURE_PERMS {
        return Err(SecureError::TooManyPerms {
            max: MAX_SECURE_PERMS,
            actual: names.len(),
        });
    }
    let mut out = Vec::with_capacity(names.len());
    for n in names {
        if n.is_empty() {
            return Err(SecureError::EmptyName);
        }
        out.push(SecurePerm {
            name: (*n).to_string(),
            granted: true,
        });
    }
    Ok(out)
}

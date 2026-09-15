//! Pure plugin manifest validation: name/version/permissions shape only.
//! No loading, no execution, no IO.

use thiserror::Error;

/// Maximum number of permissions a manifest may declare.
pub const MAX_PERMISSIONS: usize = 32;

/// Validated plugin identity plus requested permission names.
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub permissions: Vec<String>,
}

/// Manifest shape violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("plugin name is empty")]
    EmptyName,
    #[error("version must be numeric X.Y.Z")]
    BadVersion,
    #[error("permission entry is empty")]
    EmptyPermission,
    #[error("too many permissions: max {max}, got {actual}")]
    TooManyPermissions { max: usize, actual: usize },
}

fn version_ok(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Validate manifest shape. Pure, no IO.
pub fn validate_manifest(m: &PluginManifest) -> Result<(), ManifestError> {
    if m.name.is_empty() {
        return Err(ManifestError::EmptyName);
    }
    if !version_ok(&m.version) {
        return Err(ManifestError::BadVersion);
    }
    if m.permissions.iter().any(|p| p.is_empty()) {
        return Err(ManifestError::EmptyPermission);
    }
    if m.permissions.len() > MAX_PERMISSIONS {
        return Err(ManifestError::TooManyPermissions {
            max: MAX_PERMISSIONS,
            actual: m.permissions.len(),
        });
    }
    Ok(())
}

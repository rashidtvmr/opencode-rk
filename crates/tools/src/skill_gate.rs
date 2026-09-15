//! Safe materialization helpers for bundled skill files.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillExtractError {
    #[error("invalid bundled-skill relative path: {0:?}")]
    InvalidPath(PathBuf),
    #[error("unsafe bundled-skill parent path: {0:?}")]
    UnsafeParent(PathBuf),
    #[error("skill extraction I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Materialize one bundled skill file below a caller-owned extraction root.
///
/// The destination is created exactly once. Absolute paths and non-normal path
/// components are rejected before touching the filesystem, existing files are
/// never replaced, and final-component symlinks are not followed. The caller is
/// expected to provide a private extraction root; any nested directories are
/// created owner-only and are rejected if an existing component is a symlink.
pub fn extract_skill_file(
    root: &Path,
    relative_path: &Path,
    bytes: &[u8],
) -> Result<PathBuf, SkillExtractError> {
    validate_relative_path(relative_path)?;
    ensure_private_root(root)?;

    let target = root.join(relative_path);
    ensure_parent_directories(root, relative_path)?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
        #[cfg(any(target_os = "linux", target_os = "android"))]
        options.custom_flags(0o400000); // O_NOFOLLOW
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        options.custom_flags(0x100); // O_NOFOLLOW
    }

    let mut file = options.open(&target)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            drop(file);
            let _ = fs::remove_file(&target);
            return Err(SkillExtractError::UnsafeParent(target));
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }

    if let Err(error) = file.write_all(bytes) {
        drop(file);
        let _ = fs::remove_file(&target);
        return Err(SkillExtractError::Io(error));
    }

    Ok(target)
}

fn validate_relative_path(path: &Path) -> Result<(), SkillExtractError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(SkillExtractError::InvalidPath(path.to_path_buf()));
    }

    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SkillExtractError::InvalidPath(path.to_path_buf()));
    }

    Ok(())
}

fn ensure_private_root(root: &Path) -> Result<(), SkillExtractError> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SkillExtractError::UnsafeParent(root.to_path_buf()));
    }
    Ok(())
}

fn ensure_parent_directories(root: &Path, relative_path: &Path) -> Result<(), SkillExtractError> {
    let Some(parent) = relative_path.parent() else {
        return Ok(());
    };
    let mut current = root.to_path_buf();

    for component in parent.components() {
        let Component::Normal(name) = component else {
            return Err(SkillExtractError::InvalidPath(relative_path.to_path_buf()));
        };
        current.push(name);

        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(SkillExtractError::UnsafeParent(current));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&current, fs::Permissions::from_mode(0o700))?;
                }
            }
            Err(error) => return Err(SkillExtractError::Io(error)),
        }
    }

    Ok(())
}

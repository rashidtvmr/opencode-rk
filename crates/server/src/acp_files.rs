//! ACP-002: root-scoped text-file read/write plus typed capability rejects.
//!
//! Pure boundary over caller-supplied traits (permission broker): this module
//! performs no direct filesystem I/O, no network, spawns no thread, holds no
//! globals/statics/cache. Reads/writes land inside one caller-owned `root`
//! only; `rel` must be a root-relative `a/b.txt` path (no `..`, no absolute,
//! no empty, max [`MAX_REL_CHARS`] chars). Content either direction capped at
//! [`MAX_ACP_FILE_BYTES`]; rejected before retention. Errors carry paths and
//! variant names only, never file content.

use std::path::Path;

/// Largest accepted file content in bytes, either direction (1 MiB).
pub const MAX_ACP_FILE_BYTES: usize = 1_048_576;

/// Largest accepted `rel` length in chars.
pub const MAX_REL_CHARS: usize = 512;

/// Closed ACP-adjacent capability set; exactly one variant per request kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    FileRead,
    FileWrite,
    Terminal,
    Auth,
    ModeSwitch,
    HistoryRestore,
    UpdateStream,
}

/// File access failures. Variants carry paths/variant names only, never content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpFileError {
    /// Capability outside this slice; names the rejected variant.
    Unsupported { capability: Capability },
    /// `rel` escapes the root, is absolute/empty, or exceeds [`MAX_REL_CHARS`].
    PathNotAllowed,
    /// No such file on read.
    NotFound,
    /// Content exceeds [`MAX_ACP_FILE_BYTES`] either direction.
    TooLarge,
    /// File bytes are not valid UTF-8 on read.
    NotText,
}

impl std::fmt::Display for AcpFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { capability } => {
                write!(f, "unsupported capability: {capability:?}")
            }
            Self::PathNotAllowed => write!(f, "path not allowed"),
            Self::NotFound => write!(f, "file not found"),
            Self::TooLarge => write!(f, "file exceeds max size"),
            Self::NotText => write!(f, "file is not valid text"),
        }
    }
}

impl std::error::Error for AcpFileError {}

/// Caller-supplied read side of the permission broker (fixture dir in tests;
/// production wiring is the integrator's).
pub trait FileReader {
    fn read_bytes(&mut self, rel: &str) -> Result<Vec<u8>, AcpFileError>;
}

/// Caller-supplied write side. Implementations must be all-or-nothing:
/// a failed call writes nothing.
pub trait FileWriter {
    fn write_bytes(&mut self, rel: &str, bytes: &[u8]) -> Result<u64, AcpFileError>;
}

/// True only for [`Capability::FileRead`] / [`Capability::FileWrite`].
#[must_use]
pub fn is_supported(cap: Capability) -> bool {
    matches!(cap, Capability::FileRead | Capability::FileWrite)
}

/// Gate a capability: read/write pass; anything else is a named reject.
pub fn require(cap: Capability) -> Result<(), AcpFileError> {
    if is_supported(cap) {
        Ok(())
    } else {
        Err(AcpFileError::Unsupported { capability: cap })
    }
}

/// Validate `rel` as a root-relative path: non-empty, <= 512 chars, no
/// absolute form, no `..` segment, no empty segment, no backslash drive form.
fn check_rel(rel: &str) -> Result<(), AcpFileError> {
    if rel.is_empty() || rel.chars().count() > MAX_REL_CHARS {
        return Err(AcpFileError::PathNotAllowed);
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(AcpFileError::PathNotAllowed);
    }
    // Windows drive / UNC forms never root-relative.
    if rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return Err(AcpFileError::PathNotAllowed);
    }
    if rel.contains('\\') {
        return Err(AcpFileError::PathNotAllowed);
    }
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(AcpFileError::PathNotAllowed);
        }
    }
    Ok(())
}

/// Read one root-scoped text file via the caller-supplied trait.
pub fn read_text(_root: &Path, rel: &str, fs: &mut dyn FileReader) -> Result<String, AcpFileError> {
    check_rel(rel)?;
    let bytes = fs.read_bytes(rel)?;
    if bytes.len() > MAX_ACP_FILE_BYTES {
        return Err(AcpFileError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| AcpFileError::NotText)
}

/// Write one root-scoped text file via the caller-supplied trait only.
/// Returns bytes written; oversize writes nothing.
pub fn write_text(
    _root: &Path,
    rel: &str,
    text: &str,
    fs: &mut dyn FileWriter,
) -> Result<u64, AcpFileError> {
    check_rel(rel)?;
    if text.len() > MAX_ACP_FILE_BYTES {
        return Err(AcpFileError::TooLarge);
    }
    fs.write_bytes(rel, text.as_bytes())
}

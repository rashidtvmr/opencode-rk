//! OPS-006: offline repository-reference normalization plus branch-isolated
//! cache identity. Pure computation: no clone/fetch/checkout, no file lock,
//! no network transport, no filesystem mutation, no env reads, no logging.
//!
//! Contract (tasks/OPS-006.md):
//! - Inputs: `raw: &str`, `branch: Option<&str>`, `cache_root: &RelPath`.
//! - Outputs: `NormalizedRef { canonical, branch, cache_path, cache_id }`,
//!   `cache_path = cache_root.join(canonical).join(branch)` (construction
//!   only), `cache_id` = 64-char hex of a stable hash of
//!   `canonical + '\0' + branch`.

use thiserror::Error;

pub const MAX_RAW_BYTES: usize = 256;
pub const MAX_BRANCH_BYTES: usize = 128;
pub const MAX_ROOT_BYTES: usize = 1024;
pub const MAX_CACHE_PATH_BYTES: usize = 2048;
const DEFAULT_BRANCH: &str = "main";

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RefError {
    #[error("malformed or unsupported reference")]
    Invalid,
    #[error("unsafe branch")]
    UnsafeBranch,
    #[error("bad cache root")]
    BadCacheRoot,
}

/// Bounded relative path built from caller-supplied segments only.
/// Rejects empty roots, absolute paths, and `..` segments. Never touches fs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelPath {
    raw: String,
}

impl RelPath {
    pub fn new(root: &str) -> Result<Self, RefError> {
        if root.is_empty() || root.len() > MAX_ROOT_BYTES {
            return Err(RefError::BadCacheRoot);
        }
        validate_rel_segments(root).map_err(|_| RefError::BadCacheRoot)?;
        Ok(Self {
            raw: root.to_string(),
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    #[must_use]
    pub fn join(&self, segment: &str) -> Self {
        let mut out = String::with_capacity(self.raw.len() + 1 + segment.len());
        out.push_str(&self.raw);
        if !self.raw.ends_with('/') {
            out.push('/');
        }
        out.push_str(segment);
        Self { raw: out }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedRef {
    pub canonical: String,
    pub branch: String,
    pub cache_path: RelPath,
    pub cache_id: String,
}

pub fn normalize_reference(
    raw: &str,
    branch: Option<&str>,
    cache_root: &RelPath,
) -> Result<NormalizedRef, RefError> {
    if raw.is_empty() || raw.len() > MAX_RAW_BYTES {
        return Err(RefError::Invalid);
    }
    let canonical = canonicalize(raw).ok_or(RefError::Invalid)?;
    if canonical.len() > MAX_RAW_BYTES {
        return Err(RefError::Invalid);
    }
    let branch = match branch {
        None => DEFAULT_BRANCH.to_string(),
        Some(b) => validate_branch(b).map_err(|_| RefError::UnsafeBranch)?,
    };
    if cache_root.as_str().is_empty()
        || cache_root.as_str().len() > MAX_ROOT_BYTES
        || cache_root.as_str().split('/').any(|s| s == "..")
    {
        return Err(RefError::BadCacheRoot);
    }
    let cache_path = cache_root.join(&canonical).join(&branch);
    if cache_path.as_str().len() > MAX_CACHE_PATH_BYTES {
        return Err(RefError::Invalid);
    }
    let cache_id = cache_identity(&canonical, &branch);
    Ok(NormalizedRef {
        canonical,
        branch,
        cache_path,
        cache_id,
    })
}

/// Failure hint carrying only the failure kind plus the ref basename.
/// Never echoes the full raw ref, credentials, or env bytes.
#[must_use]
pub fn failure_hint(raw: &str, err: &RefError) -> String {
    let kind = match err {
        RefError::Invalid => "invalid",
        RefError::UnsafeBranch => "unsafe-branch",
        RefError::BadCacheRoot => "bad-cache-root",
    };
    let mut hint = String::with_capacity(kind.len() + 1 + 64);
    hint.push_str(kind);
    hint.push(':');
    hint.push_str(&safe_basename(raw));
    hint
}

/// Last `/`-delimited segment of the ref, capped at 64 bytes. Pure.
#[must_use]
pub fn safe_basename(raw: &str) -> String {
    let tail = raw.rsplit('/').next().unwrap_or("");
    let tail = tail.rsplit('\\').next().unwrap_or(tail);
    let tail: String = tail
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect();
    if tail.is_empty() {
        "(empty)".to_string()
    } else {
        tail
    }
}

fn canonicalize(raw: &str) -> Option<String> {
    if raw.bytes().any(|b| b == 0) {
        return None;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = strip_url_prefix(trimmed) {
        let no_query = rest.split(['?', '#']).next().unwrap_or("");
        let no_query = no_query.trim_end_matches('/');
        let no_query = no_query.strip_suffix(".git").unwrap_or(no_query);
        let no_query = no_query.trim();
        if no_query.is_empty() {
            return None;
        }
        return validate_shorthand_shape(no_query);
    }
    validate_shorthand_shape(trimmed)
}

fn strip_url_prefix(s: &str) -> Option<&str> {
    for prefix in [
        "https://github.com/",
        "http://github.com/",
        "https://www.github.com/",
        "http://www.github.com/",
        "git@github.com:",
        "ssh://git@github.com/",
        "git://github.com/",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    None
}

fn validate_shorthand_shape(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > MAX_RAW_BYTES {
        return None;
    }
    if s.starts_with('/') || s.ends_with('/') || s.contains("//") {
        return None;
    }
    if s.bytes()
        .any(|b| b == 0 || b == b'\\' || b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
    {
        return None;
    }
    if s.contains("..") {
        return None;
    }
    if s.bytes().any(|b| !is_ref_byte(b)) {
        return None;
    }
    let mut parts = s.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    if owner.starts_with(['.', '-']) || repo.starts_with(['.', '-']) {
        return None;
    }
    if owner.ends_with('.') || repo.ends_with('.') {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn is_ref_byte(b: u8) -> bool {
    // ponytail: ASCII alnum + -_.~/% kept; add when upstream widens forms.
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/' | b'%')
}

fn validate_branch(branch: &str) -> Result<String, ()> {
    if branch.is_empty() || branch.len() > MAX_BRANCH_BYTES {
        return Err(());
    }
    if branch.bytes().any(|b| b == 0) {
        return Err(());
    }
    if branch.contains('/') || branch.contains('\\') || branch.contains("..") {
        return Err(());
    }
    if branch.starts_with('.') || branch.ends_with('.') || branch.ends_with(".lock") {
        return Err(());
    }
    if branch
        .bytes()
        .any(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
    {
        return Err(());
    }
    if branch.bytes().any(|b| {
        !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'+' | b'%'))
    }) {
        return Err(());
    }
    Ok(branch.to_string())
}

fn validate_rel_segments(path: &str) -> Result<(), ()> {
    if path.starts_with('/') || path.contains('\\') || path.bytes().any(|b| b == 0) {
        return Err(());
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(());
        }
    }
    Ok(())
}

/// 64-hex-char stable identity of `canonical + '\0' + branch`.
/// FNV-1a 64-bit expanded deterministically to 32 bytes. No I/O, no env.
fn cache_identity(canonical: &str, branch: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut words = [0u64; 4];
    for (i, w) in words.iter_mut().enumerate() {
        let mut h: u64 =
            0xcbf29ce484222325 ^ (0x9e3779b97f4a7c15u64.wrapping_add(i as u64 * 0x100000001b3));
        for b in canonical
            .bytes()
            .chain(std::iter::once(0))
            .chain(branch.bytes())
            .chain(std::iter::once(i as u8))
        {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
            h ^= h >> 29;
            h = h.wrapping_mul(0xbf58476d1ce4e5b9);
        }
        *w = h;
    }
    let mut out = String::with_capacity(64);
    for w in words {
        for byte in w.to_be_bytes() {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0xf) as usize] as char);
        }
    }
    out
}

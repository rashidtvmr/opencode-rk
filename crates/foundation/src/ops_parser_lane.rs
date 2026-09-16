//! OPS-006 lane: offline repo-ref normalization, branch-isolated identity.
//! Distinct from repo_ref.rs (OPS-002) and repo_ref_ext.rs; lane-owned API
//! uses ops_lane_ prefix throughout to avoid symbol collision.
//!
//! Pure computation: validate/normalize `owner/repo` plus URL aliases,
//! reject unsafe branches, derive cache path and stable identity from the
//! caller-supplied root. No clone/fetch, no fs mutation, no net, no clock,
//! no env reads, no logging of ref bytes.

use std::fmt;

/// Byte caps per tasks/OPS-006.md resource bounds.
pub const OPS_LANE_MAX_RAW_BYTES: usize = 256;
pub const OPS_LANE_MAX_BRANCH_BYTES: usize = 128;
pub const OPS_LANE_MAX_ROOT_BYTES: usize = 1024;
pub const OPS_LANE_MAX_CACHE_PATH_BYTES: usize = 2048;
const OPS_LANE_DEFAULT_BRANCH: &str = "main";

/// Typed failure. Static messages only, never echo input bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpsLaneError {
    Invalid,
    UnsafeBranch,
    BadCacheRoot,
}

impl fmt::Display for OpsLaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "malformed or unsupported reference"),
            Self::UnsafeBranch => write!(f, "unsafe branch"),
            Self::BadCacheRoot => write!(f, "bad cache root"),
        }
    }
}

impl std::error::Error for OpsLaneError {}

/// Bounded relative path from caller-supplied segments only. No fs touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsLaneRoot {
    raw: String,
}

impl OpsLaneRoot {
    pub fn new(root: &str) -> Result<Self, OpsLaneError> {
        if root.is_empty() || root.len() > OPS_LANE_MAX_ROOT_BYTES {
            return Err(OpsLaneError::BadCacheRoot);
        }
        ops_lane_validate_root(root).map_err(|_| OpsLaneError::BadCacheRoot)?;
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

/// Validated reference plus branch-isolated cache identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsLaneRef {
    pub canonical: String,
    pub branch: String,
    pub cache_path: OpsLaneRoot,
    pub cache_id: String,
}

/// Normalize one reference against a caller-supplied cache root.
///
/// Pure: constructs (never creates) `cache_path =
/// root.join(canonical).join(branch)` and a stable 64-hex `cache_id` over
/// `canonical + '\0' + branch`. Zero I/O, zero env reads.
#[must_use = "validation result must be checked"]
pub fn ops_lane_normalize(
    raw: &str,
    branch: Option<&str>,
    cache_root: &OpsLaneRoot,
) -> Result<OpsLaneRef, OpsLaneError> {
    if raw.is_empty() || raw.len() > OPS_LANE_MAX_RAW_BYTES {
        return Err(OpsLaneError::Invalid);
    }
    let canonical = ops_lane_canonicalize(raw).ok_or(OpsLaneError::Invalid)?;
    if canonical.len() > OPS_LANE_MAX_RAW_BYTES {
        return Err(OpsLaneError::Invalid);
    }
    let branch = match branch {
        None => OPS_LANE_DEFAULT_BRANCH.to_string(),
        Some(b) => ops_lane_validate_branch(b).map_err(|_| OpsLaneError::UnsafeBranch)?,
    };
    if cache_root.as_str().is_empty()
        || cache_root.as_str().len() > OPS_LANE_MAX_ROOT_BYTES
        || cache_root.as_str().split('/').any(|s| s == "..")
    {
        return Err(OpsLaneError::BadCacheRoot);
    }
    let cache_path = cache_root.join(&canonical).join(&branch);
    if cache_path.as_str().len() > OPS_LANE_MAX_CACHE_PATH_BYTES {
        return Err(OpsLaneError::Invalid);
    }
    let cache_id = ops_lane_identity(&canonical, &branch);
    Ok(OpsLaneRef {
        canonical,
        branch,
        cache_path,
        cache_id,
    })
}

/// Failure hint carrying only the failure kind plus the ref basename.
/// Never echoes the full raw ref, credentials, or env bytes.
#[must_use]
pub fn ops_lane_failure_hint(raw: &str, err: &OpsLaneError) -> String {
    let kind = match err {
        OpsLaneError::Invalid => "invalid",
        OpsLaneError::UnsafeBranch => "unsafe-branch",
        OpsLaneError::BadCacheRoot => "bad-cache-root",
    };
    let mut hint = String::with_capacity(kind.len() + 1 + 64);
    hint.push_str(kind);
    hint.push(':');
    hint.push_str(&ops_lane_basename(raw));
    hint
}

/// Last `/`-delimited segment of the ref, capped at 64 chars. Pure.
#[must_use]
pub fn ops_lane_basename(raw: &str) -> String {
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

fn ops_lane_canonicalize(raw: &str) -> Option<String> {
    if raw.bytes().any(|b| b == 0) {
        return None;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = ops_lane_strip_url_prefix(trimmed) {
        let no_query = rest.split(['?', '#']).next().unwrap_or("");
        let no_query = no_query.trim_end_matches('/');
        let no_query = no_query.strip_suffix(".git").unwrap_or(no_query);
        let no_query = no_query.trim();
        if no_query.is_empty() {
            return None;
        }
        return ops_lane_shorthand(no_query);
    }
    ops_lane_shorthand(trimmed)
}

fn ops_lane_strip_url_prefix(s: &str) -> Option<&str> {
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

fn ops_lane_shorthand(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > OPS_LANE_MAX_RAW_BYTES {
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
    if s.bytes().any(|b| !ops_lane_ref_byte(b)) {
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
    // ponytail: owner/repo only; wider forms add when upstream requires them.
    Some(format!("{owner}/{repo}"))
}

fn ops_lane_ref_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/' | b'%')
}

fn ops_lane_validate_branch(branch: &str) -> Result<String, ()> {
    if branch.is_empty() || branch.len() > OPS_LANE_MAX_BRANCH_BYTES {
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

fn ops_lane_validate_root(path: &str) -> Result<(), ()> {
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

/// 64-hex stable identity of `canonical + '\0' + branch`.
/// FNV-1a 64-bit expanded deterministically to 32 bytes. No I/O, no env.
fn ops_lane_identity(canonical: &str, branch: &str) -> String {
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

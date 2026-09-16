//! OPS-002 pure repository-reference normalization.
//!
//! One workspace, offline only: a reference string plus optional branch and
//! cache root yields a normalized reference, a typed validation failure, and
//! a branch-isolated cache path plus stable cache identity. No clone, no
//! fetch, no filesystem mutation, no network, no clock, no global state.
//! The only environment read is the GitHub-default-remote host override for
//! the bare `owner/repo` shorthand; every other accepted form ignores the
//! environment entirely.
//!
//! Accepted forms: local path (`./`, `/`, `~` prefixed), `owner/repo`
//! shorthand, `https://host/owner/repo`, `git@host:owner/repo(.git)`.
//! Anything else is [`RefError::UnsupportedForm`] with absence semantics.
//! A trailing `@` with no branch (`owner/repo@`) is [`RefError::BranchRequired`].

use std::path::{Path, PathBuf};

/// Maximum accepted input length in bytes (raw ref and branch each).
pub const MAX_REF_BYTES: usize = 4096;
/// Maximum accepted branch length in bytes.
const MAX_BRANCH_BYTES: usize = 255;
/// Host assumed for bare `owner/repo` references without override.
const DEFAULT_HOST: &str = "github.com";
/// Branch assumed when none is supplied.
const DEFAULT_BRANCH: &str = "main";
/// Single environment variable consulted, shorthand form only.
const SHORTHAND_HOST_VAR: &str = "OPS002_GITHUB_HOST";

/// Unvalidated repository reference: raw string plus optional branch.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoRef {
    pub raw: String,
    pub branch: Option<String>,
}

/// Validated, fully-qualified repository reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedRef {
    pub canonical: String,
    pub branch: String,
    pub cache_path: PathBuf,
    pub cache_id: String,
}

/// Typed validation failure. Messages are static shape-only strings; they
/// never echo input bytes, so rejected secrets cannot leak through logs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefError {
    Empty,
    UnsupportedForm,
    UnsafeBranch,
    BranchRequired,
    InputTooLong,
}

impl std::fmt::Display for RefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty repository reference"),
            Self::UnsupportedForm => write!(f, "unsupported reference form"),
            Self::UnsafeBranch => write!(f, "unsafe branch name"),
            Self::BranchRequired => write!(f, "branch required"),
            Self::InputTooLong => write!(f, "reference exceeds 4096 bytes"),
        }
    }
}

impl std::error::Error for RefError {}

/// Normalize one repository reference against a cache root.
///
/// Pure: constructs (never creates) a branch-isolated `cache_path` under
/// `cache_root` and a stable hex `cache_id` over `(canonical, branch)`.
/// Reads at most one environment variable ([`SHORTHAND_HOST_VAR`], shorthand
/// form only) and performs no other I/O.
#[must_use = "validation result must be checked"]
pub fn normalize_ref(
    raw: &str,
    branch: Option<&str>,
    cache_root: &Path,
) -> Result<NormalizedRef, RefError> {
    if raw.len() > MAX_REF_BYTES {
        return Err(RefError::InputTooLong);
    }
    if branch.is_some_and(|b| b.len() > MAX_REF_BYTES) {
        return Err(RefError::InputTooLong);
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RefError::Empty);
    }
    if trimmed.contains('\0') {
        return Err(RefError::UnsupportedForm);
    }
    if let Some(head) = trimmed.strip_suffix('@') {
        if head.is_empty() {
            return Err(RefError::UnsupportedForm);
        }
        return Err(RefError::BranchRequired);
    }
    let (host, path) = if let Some(rest) = trimmed.strip_prefix("https://") {
        parse_host_path(rest)?
    } else if trimmed.starts_with("git@") {
        parse_scp_form(trimmed)?
    } else if trimmed.contains("://") || trimmed.starts_with("//") || trimmed.contains('@') {
        return Err(RefError::UnsupportedForm);
    } else if is_local_ref(trimmed) {
        parse_local_ref(trimmed)?
    } else {
        parse_bare_ref(trimmed)?
    };
    let branch = match branch {
        Some(b) => validate_branch(b)?,
        None => DEFAULT_BRANCH.to_string(),
    };
    let canonical = format!("{host}/{path}");
    let mut cache_path = cache_root.to_path_buf();
    cache_path.push(sanitize_segment(&host));
    for seg in path.split('/') {
        cache_path.push(sanitize_segment(seg));
    }
    cache_path.push(sanitize_segment(&branch));
    if !cache_path.starts_with(cache_root) {
        return Err(RefError::UnsafeBranch);
    }
    let cache_id = cache_identity(&canonical, &branch);
    Ok(NormalizedRef {
        canonical,
        branch,
        cache_path,
        cache_id,
    })
}

fn parse_host_path(rest: &str) -> Result<(String, String), RefError> {
    let (raw_host, raw_path) = rest.split_once('/').ok_or(RefError::UnsupportedForm)?;
    let host = validate_host(raw_host)?;
    let path = clean_repo_path(raw_path)?;
    Ok((host, path))
}

fn parse_scp_form(trimmed: &str) -> Result<(String, String), RefError> {
    let after = &trimmed["git@".len()..];
    let (raw_host, raw_path) = after.split_once(':').ok_or(RefError::UnsupportedForm)?;
    let host = validate_host(raw_host)?;
    let path = clean_repo_path(raw_path)?;
    Ok((host, path))
}

fn parse_bare_ref(trimmed: &str) -> Result<(String, String), RefError> {
    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.iter().any(|s| s.is_empty()) {
        return Err(RefError::UnsupportedForm);
    }
    if segments.len() == 2 {
        let owner = validate_segment(segments[0])?;
        let repo = validate_segment(segments[1])?;
        Ok((shorthand_host(), format!("{owner}/{repo}")))
    } else if segments.len() > 2 && segments[0].contains('.') {
        let host = validate_host(segments[0])?;
        let mut parts = Vec::with_capacity(segments.len() - 1);
        for s in &segments[1..] {
            parts.push(validate_segment(s)?);
        }
        Ok((host, parts.join("/")))
    } else {
        Err(RefError::UnsupportedForm)
    }
}

fn parse_local_ref(trimmed: &str) -> Result<(String, String), RefError> {
    let no_trail = trimmed.trim_end_matches('/');
    let no_git = no_trail.strip_suffix(".git").unwrap_or(no_trail);
    let kept: Vec<&str> = no_git
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if kept.is_empty() {
        return Err(RefError::UnsupportedForm);
    }
    Ok(("local".to_string(), kept.join("/")))
}

fn clean_repo_path(raw_path: &str) -> Result<String, RefError> {
    let no_trail = raw_path.trim_end_matches('/');
    let no_git = no_trail.strip_suffix(".git").unwrap_or(no_trail);
    let segments: Vec<&str> = no_git.split('/').collect();
    if segments.len() < 2 || segments.iter().any(|s| s.is_empty()) {
        return Err(RefError::UnsupportedForm);
    }
    let mut parts = Vec::with_capacity(segments.len());
    for s in segments {
        parts.push(validate_segment(s)?);
    }
    Ok(parts.join("/"))
}

fn is_local_ref(trimmed: &str) -> bool {
    trimmed.starts_with('.') || trimmed.starts_with('/') || trimmed.starts_with('~')
}

/// Host for the `owner/repo` shorthand: single env read, validated, else default.
fn shorthand_host() -> String {
    if let Ok(raw) = std::env::var(SHORTHAND_HOST_VAR) {
        let t = raw.trim().to_ascii_lowercase();
        if validate_host(&t).is_ok() {
            return t;
        }
    }
    DEFAULT_HOST.to_string()
}

fn validate_host(raw: &str) -> Result<String, RefError> {
    if raw.is_empty() || raw.len() > MAX_REF_BYTES {
        return Err(RefError::UnsupportedForm);
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with(['.', '-']) || lower.ends_with(['.', '-']) {
        return Err(RefError::UnsupportedForm);
    }
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            continue;
        }
        return Err(RefError::UnsupportedForm);
    }
    Ok(lower)
}

fn validate_segment(segment: &str) -> Result<String, RefError> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(RefError::UnsupportedForm);
    }
    if segment.starts_with(['.', '-']) {
        return Err(RefError::UnsupportedForm);
    }
    // ponytail: ASCII alnum + -_. only; widen when upstream forms require it.
    for c in segment.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            continue;
        }
        return Err(RefError::UnsupportedForm);
    }
    Ok(segment.to_string())
}

fn validate_branch(branch: &str) -> Result<String, RefError> {
    if branch.is_empty() || branch.len() > MAX_BRANCH_BYTES {
        return Err(RefError::UnsafeBranch);
    }
    if branch == "." || branch == ".." || branch.contains("..") {
        return Err(RefError::UnsafeBranch);
    }
    if branch.contains('/') || branch.contains('\\') {
        return Err(RefError::UnsafeBranch);
    }
    for c in branch.chars() {
        if c.is_control() || c.is_whitespace() {
            return Err(RefError::UnsafeBranch);
        }
    }
    Ok(branch.to_string())
}

/// Defense-in-depth projection: strip separators so a validated component can
/// never escape `cache_root`; the `starts_with` check fails closed.
fn sanitize_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for c in segment.chars() {
        if c == '/' || c == '\\' || c == '\0' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    if out == ".." {
        out = String::from("__");
    }
    out
}

/// Stable hex identity of `canonical + '\0' + branch`. FNV-1a 64-bit, std only.
fn cache_identity(canonical: &str, branch: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut h: u64 = 0xcbf29ce484222325;
    for b in canonical
        .bytes()
        .chain(std::iter::once(0))
        .chain(branch.bytes())
    {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    let mut out = String::with_capacity(16);
    for byte in h.to_be_bytes() {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

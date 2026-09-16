//! OPS-008 pure repository-reference normalizer.
//!
//! Offline only: validates one repository reference string plus an optional
//! branch and derives a branch-isolated cache path and a stable cache
//! identity. No network, no filesystem, no process-environment reads, no
//! clock. All bounds are caller-visible constants; time is O(n) in the input
//! length with O(1) extra memory beyond the returned structs.

use thiserror::Error;

/// Maximum accepted reference-string length in bytes.
pub const MAX_INPUT_BYTES: usize = 2048;
/// Maximum accepted branch length in bytes.
pub const MAX_BRANCH_BYTES: usize = 255;
/// Host assumed for bare `owner/repo` references.
pub const DEFAULT_HOST: &str = "github.com";
/// Branch assumed when none is supplied.
pub const DEFAULT_BRANCH: &str = "main";

/// Unvalidated repository reference: host plus path plus optional branch.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoRef {
    pub host: String,
    pub path: String,
    pub branch: Option<String>,
}

/// Validated, fully-qualified repository reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedRef {
    pub canonical: String,
    pub host: String,
    pub path: String,
    pub branch: String,
}

/// Typed validation failure. Messages are static shape-only strings; they
/// never echo input bytes, so rejected secrets cannot leak through logs.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepoError {
    #[error("malformed repository reference")]
    Malformed,
    #[error("unsafe branch name")]
    UnsafeBranch,
    #[error("repository reference exceeds 2048 bytes")]
    InputTooLong,
}

/// Normalize one repository reference with default host resolution.
///
/// Accepts `owner/repo`, `host/owner/repo`, and full `https://` URL forms.
/// Never reads the process environment; see [`normalize_ref_with_base`] for
/// the explicit-base wrapper.
#[must_use = "validation result must be checked"]
pub fn normalize_ref(input: &str, branch: Option<&str>) -> Result<NormalizedRef, RepoError> {
    normalize_inner(input, branch, None)
}

/// Normalize with an explicit caller-supplied default host.
///
/// `base` replaces [`DEFAULT_HOST`] for short `owner/repo` forms only; full
/// `https://` URLs and explicit `host/owner/repo` forms keep their own host.
/// `None` behaves exactly like [`normalize_ref`]. Never reads the process
/// environment; the override must be passed in.
pub fn normalize_ref_with_base(
    input: &str,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<NormalizedRef, RepoError> {
    normalize_inner(input, branch, base)
}

/// Branch-isolated cache path: `root/host/path@branch` with `/` separators.
/// The branch is sanitized to `[A-Za-z0-9._-]` (`_` otherwise) so the result
/// is always a safe single path segment tail.
#[must_use]
pub fn cache_path(root: &str, norm: &NormalizedRef) -> String {
    let clean_root = root.trim_end_matches('/');
    let tail = format!(
        "{}/{}@{}",
        norm.host,
        norm.path,
        sanitize_branch(&norm.branch)
    );
    if clean_root.is_empty() {
        format!("/{tail}")
    } else {
        format!("{clean_root}/{tail}")
    }
}

/// Stable cache identity: `host/path@branch`. Same inputs always give the
/// identical string.
#[must_use]
pub fn cache_identity(norm: &NormalizedRef) -> String {
    format!("{}/{}@{}", norm.host, norm.path, norm.branch)
}

fn normalize_inner(
    input: &str,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<NormalizedRef, RepoError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(RepoError::InputTooLong);
    }
    let trimmed = input.trim_matches(|c: char| c.is_ascii_whitespace());
    if trimmed.is_empty() {
        return Err(RepoError::Malformed);
    }
    let default_host = match base {
        Some(b) => {
            let t = b.trim_matches(|c: char| c.is_ascii_whitespace());
            validate_host(t)?;
            t.to_ascii_lowercase()
        }
        None => DEFAULT_HOST.to_string(),
    };
    let (host, path) = if let Some(rest) = trimmed.strip_prefix("https://") {
        parse_url_ref(rest)?
    } else if trimmed.contains("://") || trimmed.contains('@') {
        return Err(RepoError::Malformed);
    } else {
        parse_bare_ref(trimmed, &default_host)?
    };
    let branch = match branch {
        Some(b) => validate_branch(b)?,
        None => DEFAULT_BRANCH.to_string(),
    };
    let canonical = format!("{host}/{path}@{branch}");
    Ok(NormalizedRef {
        canonical,
        host,
        path,
        branch,
    })
}

fn parse_url_ref(rest: &str) -> Result<(String, String), RepoError> {
    let slash = rest.find('/').ok_or(RepoError::Malformed)?;
    let (raw_host, raw_path) = (&rest[..slash], &rest[slash + 1..]);
    validate_host(raw_host)?;
    let path = parse_path_segments(raw_path)?;
    Ok((raw_host.to_ascii_lowercase(), path))
}

fn parse_bare_ref(trimmed: &str, default_host: &str) -> Result<(String, String), RepoError> {
    let no_trail = trimmed.trim_end_matches('/');
    let no_git = no_trail.strip_suffix(".git").unwrap_or(no_trail);
    let segments: Vec<&str> = no_git.split('/').collect();
    if segments.iter().any(|s| s.is_empty()) {
        return Err(RepoError::Malformed);
    }
    if segments.len() == 2 {
        for s in &segments {
            validate_segment(s)?;
        }
        Ok((default_host.to_string(), segments.join("/")))
    } else if segments.len() >= 3 {
        validate_host(segments[0])?;
        for s in &segments[1..] {
            validate_segment(s)?;
        }
        Ok((segments[0].to_ascii_lowercase(), segments[1..].join("/")))
    } else {
        Err(RepoError::Malformed)
    }
}

fn parse_path_segments(raw_path: &str) -> Result<String, RepoError> {
    let no_trail = raw_path.trim_end_matches('/');
    let no_git = no_trail.strip_suffix(".git").unwrap_or(no_trail);
    let segments: Vec<&str> = no_git.split('/').collect();
    if segments.len() < 2 || segments.iter().any(|s| s.is_empty()) {
        return Err(RepoError::Malformed);
    }
    for s in &segments {
        validate_segment(s)?;
    }
    Ok(segments.join("/"))
}

fn validate_host(host: &str) -> Result<(), RepoError> {
    if host.is_empty() || host == "." || host == ".." {
        return Err(RepoError::Malformed);
    }
    for c in host.chars() {
        if c.is_control() || c.is_whitespace() {
            return Err(RepoError::Malformed);
        }
        match c {
            ':' | '?' | '#' | '@' | '\\' => return Err(RepoError::Malformed),
            _ => {}
        }
    }
    Ok(())
}

fn validate_segment(segment: &str) -> Result<(), RepoError> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(RepoError::Malformed);
    }
    for c in segment.chars() {
        if c.is_control() || c.is_whitespace() {
            return Err(RepoError::Malformed);
        }
        match c {
            ':' | '?' | '*' | '~' | '^' | '#' | '@' | '[' | '\\' => {
                return Err(RepoError::Malformed)
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_branch(branch: &str) -> Result<String, RepoError> {
    if branch.is_empty() || branch.len() > MAX_BRANCH_BYTES {
        return Err(RepoError::UnsafeBranch);
    }
    if branch == "." || branch == ".." {
        return Err(RepoError::UnsafeBranch);
    }
    if branch.starts_with('-') || branch.starts_with('/') {
        return Err(RepoError::UnsafeBranch);
    }
    if branch.contains("..") {
        return Err(RepoError::UnsafeBranch);
    }
    for c in branch.chars() {
        if c.is_control() || c.is_whitespace() {
            return Err(RepoError::UnsafeBranch);
        }
        match c {
            '~' | '^' | ':' | '?' | '*' | '[' => return Err(RepoError::UnsafeBranch),
            _ => {}
        }
    }
    Ok(branch.to_string())
}

fn sanitize_branch(branch: &str) -> String {
    branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

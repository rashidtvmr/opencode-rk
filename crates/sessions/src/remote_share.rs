//! Remote share target selection (SHARE-005, pure planner).
//!
//! Picks which configured remote a share publishes to. No I/O, no network,
//! no persistence; validates the candidate list then returns the matched URL.

use thiserror::Error;

/// A named remote share endpoint.
pub struct RemoteTarget {
    /// Logical name used to select this remote.
    pub name: String,
    /// Publish URL. Must start with `https://`.
    pub url: String,
}

/// Failure modes for [`select_target`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RemoteShareError {
    /// An entry name was empty, or the selector `name` was empty.
    /// (Unknown selector maps to [`RemoteShareError::BadUrl`]; the frozen
    /// enum has no `Unknown` variant.)
    #[error("empty remote name")]
    EmptyName,
    /// An entry URL was empty.
    #[error("empty remote url")]
    EmptyUrl,
    /// An entry URL did not start with `https://`, or the selector matched
    /// no entry (frozen enum has no `Unknown` variant).
    #[error("bad remote url")]
    BadUrl,
    /// More targets than [`MAX_REMOTE_TARGETS`].
    #[error("too many remote targets: max {max}, got {actual}")]
    TooManyTargets {
        /// Allowed maximum ([`MAX_REMOTE_TARGETS`]).
        max: usize,
        /// Actual number supplied.
        actual: usize,
    },
}

/// Maximum number of remote targets accepted by [`select_target`].
pub const MAX_REMOTE_TARGETS: usize = 16;

/// Return the URL of the target named `name`.
///
/// Validation order: overflow, then every entry (empty name, empty url,
/// non-`https://` url), then the selector (empty, unknown).
/// Unknown `name` returns [`RemoteShareError::BadUrl`]: the frozen error
/// enum has no `Unknown` variant.
pub fn select_target(ts: &[RemoteTarget], name: &str) -> Result<String, RemoteShareError> {
    if ts.len() > MAX_REMOTE_TARGETS {
        return Err(RemoteShareError::TooManyTargets {
            max: MAX_REMOTE_TARGETS,
            actual: ts.len(),
        });
    }
    for t in ts {
        if t.name.is_empty() {
            return Err(RemoteShareError::EmptyName);
        }
        if t.url.is_empty() {
            return Err(RemoteShareError::EmptyUrl);
        }
        if !t.url.starts_with("https://") {
            return Err(RemoteShareError::BadUrl);
        }
    }
    if name.is_empty() {
        return Err(RemoteShareError::EmptyName);
    }
    ts.iter()
        .find(|t| t.name == name)
        .map(|t| t.url.clone())
        .ok_or(RemoteShareError::BadUrl)
}

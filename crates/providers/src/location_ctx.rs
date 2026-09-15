//! INT-009: pure project-location context resolution.
//!
//! Caller-supplied hints only. No filesystem, git, database, environment, or
//! child-process access. Same `(ctx/pin, defaults, vcs, ids)` inputs always
//! yield byte-identical [`LocationCtx`] outputs.

/// Maximum retained directory bytes (`DirHint`, defaults, pins).
pub const MAX_DIR_LEN: usize = 256;
/// Maximum retained workspace-identity bytes (`WsHint`).
pub const MAX_WS_LEN: usize = 64;

/// Caller-supplied absolute directory hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirHint {
    /// Absolute path, 1..=[`MAX_DIR_LEN`] bytes, charset `[A-Za-z0-9/._-]`.
    pub path: String,
}

/// Caller-supplied branded workspace-identity hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsHint {
    /// Identity, 1..=[`MAX_WS_LEN`] bytes, `wrk`-prefixed, charset
    /// `[A-Za-z0-9_-]`.
    pub id: String,
}

/// Caller-supplied VCS metadata (never discovered here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcsInfo {
    /// Clone remote URL, if the caller knows one.
    pub remote: Option<String>,
    /// Branch name, if the caller knows one.
    pub branch: Option<String>,
}

/// Caller-supplied identity inputs beyond the VCS remote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdentHints {
    /// Cached common-directory identity, if the caller knows one.
    pub common_dir: Option<String>,
    /// Root-commit identity, if the caller knows one.
    pub root_commit: Option<String>,
}

/// Request-scoped location hints (best-effort reads).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestCtx {
    /// Explicit directory hint; malformed values fall back, never error.
    pub dir: Option<DirHint>,
    /// Explicit workspace hint; malformed values drop to `None`.
    pub ws: Option<WsHint>,
}

/// Caller-pinned session binding (strict reads).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPin {
    /// Pinned directory; malformed values reject the whole call.
    pub directory: String,
    /// Pinned workspace identity; returned verbatim.
    pub workspace_id: Option<String>,
}

/// Deterministic project identity, highest precedence first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    /// No identity input supplied.
    Global,
    /// Root-commit identity (no remote or common directory won).
    RootCommit(String),
    /// Cached common-directory identity.
    CommonDir(String),
    /// Explicit non-file VCS remote (top precedence).
    Remote(String),
}

/// Resolved project identity/location context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocationCtx {
    /// Canonical directory: explicit hint, default, or pinned value.
    pub directory: String,
    /// Branded workspace identity, if a valid one was supplied.
    pub workspace_id: Option<String>,
    /// Caller-supplied VCS metadata, passed through untouched.
    pub vcs: Option<VcsInfo>,
    /// Deterministic identity per precedence rules.
    pub identity: Identity,
}

/// Resolution failure (request-path malformed hints fall back instead).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CtxError {
    /// Session pin carried a malformed directory.
    #[error("invalid session pin")]
    InvalidPin,
    /// Caller-supplied default directory was malformed.
    #[error("invalid default directory")]
    InvalidDefault,
}

/// Validate a directory string: 1..=[`MAX_DIR_LEN`] bytes, absolute,
/// charset `[A-Za-z0-9/._-]`.
fn valid_dir(s: &str) -> bool {
    let len = s.len();
    if len == 0 || len > MAX_DIR_LEN || !s.starts_with('/') {
        return false;
    }
    s.bytes().all(|b| {
        b.is_ascii_alphanumeric() || b == b'/' || b == b'.' || b == b'_' || b == b'-'
    })
}

/// Validate a workspace id: 1..=[`MAX_WS_LEN`] bytes, `wrk`-prefixed,
/// charset `[A-Za-z0-9_-]`.
fn valid_ws(s: &str) -> bool {
    let len = s.len();
    if len == 0 || len > MAX_WS_LEN || !s.starts_with("wrk") {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Derive identity: non-file `remote` > `CommonDir` > `RootCommit` > `Global`.
///
/// `file:`-scheme remotes contribute no identity and fall through. Empty
/// payloads count as absent.
fn derive_identity(vcs: &Option<VcsInfo>, ids: &IdentHints) -> Identity {
    if let Some(v) = vcs {
        if let Some(remote) = &v.remote {
            if !remote.is_empty() && !remote.starts_with("file:") {
                return Identity::Remote(remote.clone());
            }
        }
    }
    if let Some(common) = &ids.common_dir {
        if !common.is_empty() {
            return Identity::CommonDir(common.clone());
        }
    }
    if let Some(root) = &ids.root_commit {
        if !root.is_empty() {
            return Identity::RootCommit(root.clone());
        }
    }
    Identity::Global
}

/// Resolve a request-scoped location.
///
/// Explicit `dir` wins when valid; malformed hints fall back to
/// `default_dir` deterministically. Explicit `ws` is kept only when valid.
/// Malformed `default_dir` yields [`CtxError::InvalidDefault`].
pub fn resolve_request(
    ctx: &RequestCtx,
    default_dir: &str,
    vcs: Option<VcsInfo>,
    ids: &IdentHints,
) -> Result<LocationCtx, CtxError> {
    if !valid_dir(default_dir) {
        return Err(CtxError::InvalidDefault);
    }
    let directory = ctx
        .dir
        .as_ref()
        .map(|h| h.path.as_str())
        .filter(|p| valid_dir(p))
        .unwrap_or(default_dir)
        .to_owned();
    let workspace_id = ctx
        .ws
        .as_ref()
        .map(|h| h.id.as_str())
        .filter(|id| valid_ws(id))
        .map(str::to_owned);
    let identity = derive_identity(&vcs, ids);
    Ok(LocationCtx {
        directory,
        workspace_id,
        vcs,
        identity,
    })
}

/// Resolve a session-scoped location.
///
/// Returns the pinned directory/workspace and ignores `req` entirely:
/// session routes never accept request context. A malformed pinned
/// directory yields [`CtxError::InvalidPin`]; request hints are never
/// consulted as rescue.
pub fn resolve_session(
    pin: &SessionPin,
    req: &RequestCtx,
    vcs: Option<VcsInfo>,
    ids: &IdentHints,
) -> Result<LocationCtx, CtxError> {
    let _ = req;
    if !valid_dir(&pin.directory) {
        return Err(CtxError::InvalidPin);
    }
    let identity = derive_identity(&vcs, ids);
    Ok(LocationCtx {
        directory: pin.directory.clone(),
        workspace_id: pin.workspace_id.clone(),
        vcs,
        identity,
    })
}

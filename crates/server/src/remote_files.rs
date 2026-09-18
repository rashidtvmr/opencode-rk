//! NET-010: authorized remote file, diff, and artifact workflow validation.
//!
//! Pure boundary over caller-owned storage/transport: no filesystem I/O,
//! no network, no clock, no threads, no globals. Caller owns workspace
//! roots and byte buffers; this module validates paths, version
//! preconditions, transfer budgets, cancellation, and the artifact
//! approval path before any side effect. Errors carry variant names
//! only, never file content, workspace layout, or secret bytes.
#![forbid(unsafe_code)]

/// Largest accepted `rel` length in chars.
pub const MAX_REMOTE_REL_CHARS: usize = 512;
/// Largest accepted single file content in bytes either direction (1 MiB).
pub const MAX_REMOTE_FILE_BYTES: usize = 1_048_576;
/// Largest accepted single upload/download payload in bytes (8 MiB).
pub const MAX_TRANSFER_BYTES: usize = 8_388_608;
/// Largest accepted workspace id length in chars.
pub const MAX_WORKSPACE_ID_CHARS: usize = 128;

/// Remote path failures. Display strings are fixed; they never echo the
/// rejected path, workspace id, or file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePathError {
    /// Empty rel or workspace id, or over the char cap.
    Empty,
    TooLong,
    /// Absolute form, `..`/`.`/empty segment, drive/UNC form, or interior NUL.
    NotAllowed,
    /// Secret basename (`.env`, key material, credential files).
    SecretDenied,
    /// Caller-flagged symlink component; remote must not follow it.
    SymlinkDenied,
    /// Workspace id not in the caller-supplied allowlist.
    UnknownWorkspace,
}

impl std::fmt::Display for RemotePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty path"),
            Self::TooLong => write!(f, "path too long"),
            Self::NotAllowed => write!(f, "path not allowed"),
            Self::SecretDenied => write!(f, "secret path denied"),
            Self::SymlinkDenied => write!(f, "symlink denied"),
            Self::UnknownWorkspace => write!(f, "unknown workspace"),
        }
    }
}

impl std::error::Error for RemotePathError {}

/// Version-preconditioned edit failures. Never carry content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// No `expected_version` supplied; concurrent edits must name one.
    VersionRequired,
    /// `expected_version` does not equal the current version.
    VersionMismatch,
    /// Content length exceeds [`MAX_REMOTE_FILE_BYTES`].
    TooLarge,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VersionRequired => write!(f, "edit version required"),
            Self::VersionMismatch => write!(f, "edit version mismatch"),
            Self::TooLarge => write!(f, "edit exceeds max size"),
        }
    }
}

impl std::error::Error for EditError {}

/// Upload/download budget failures. Never carry payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    /// Payload exceeds [`MAX_TRANSFER_BYTES`].
    TooLarge,
    /// `used + len` would exceed the caller-supplied quota.
    QuotaExceeded,
    /// Cancel flag set before or during the transfer.
    Cancelled,
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "transfer exceeds max size"),
            Self::QuotaExceeded => write!(f, "transfer quota exceeded"),
            Self::Cancelled => write!(f, "transfer cancelled"),
        }
    }
}

impl std::error::Error for TransferError {}

/// Artifact apply/run failures. Only one failure: a separate remote
/// bypass path is never permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactError {
    RemoteBypassDenied,
}

impl std::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "remote bypass denied")
    }
}

impl std::error::Error for ArtifactError {}

/// Which approval/sandbox route a mutation takes. Remote artifacts must
/// resolve to the same path as a local file edit; [`ApprovalPath::RemoteBypass`]
/// is the forbidden separate route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPath {
    LocalFileEdit,
    /// Marker: artifact from a remote view still funnels through the local
    /// approval/sandbox path. Normalized to [`ApprovalPath::LocalFileEdit`].
    RemoteArtifactSamePath,
    RemoteBypass,
}

/// One proposed edit with an optimistic-concurrency precondition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedEdit {
    /// Must equal the current version; `None` is rejected.
    pub expected_version: Option<u64>,
    /// Proposed content length in bytes, checked before retention.
    pub content_len: usize,
}

/// Caller-tracked byte quota for uploads/downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    pub used_bytes: u64,
    pub quota_bytes: u64,
}

/// Caller-owned cancellation flag, polled before a transfer starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelToken {
    pub cancelled: bool,
}

/// Remote file/diff view state: caller-reported current version, the base
/// the viewer diffed against, and whether the server flagged a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteView {
    pub version: u64,
    pub base_version: u64,
    pub conflict: bool,
}

/// Basenames never servable over remote, case-insensitive exact match.
fn is_secret_segment(seg: &str) -> bool {
    const SECRETS: [&str; 6] = [
        ".env",
        "id_rsa",
        "id_ed25519",
        ".netrc",
        ".aws",
        "credentials",
    ];
    SECRETS.iter().any(|s| seg.eq_ignore_ascii_case(s))
}

/// Validate one workspace id against the caller allowlist: non-empty,
/// within [`MAX_WORKSPACE_ID_CHARS`] chars, `A-Za-z0-9_-` only, and an
/// exact member of `allowed`.
fn check_workspace(workspace: &str, allowed: &[&str]) -> Result<(), RemotePathError> {
    if workspace.is_empty() {
        return Err(RemotePathError::Empty);
    }
    if workspace.chars().count() > MAX_WORKSPACE_ID_CHARS {
        return Err(RemotePathError::TooLong);
    }
    if !workspace
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(RemotePathError::NotAllowed);
    }
    if allowed.iter().any(|a| *a == workspace) {
        Ok(())
    } else {
        Err(RemotePathError::UnknownWorkspace)
    }
}

/// Validate one root-relative `rel` without touching the filesystem.
///
/// Denies: empty/oversize, absolute forms, drive/UNC forms, backslashes,
/// NUL bytes, `.`/`..`/empty segments, secret basenames, and any path
/// where the caller flags a symlink component (`is_symlink`).
pub fn check_rel(rel: &str, is_symlink: bool) -> Result<(), RemotePathError> {
    if is_symlink {
        return Err(RemotePathError::SymlinkDenied);
    }
    if rel.is_empty() {
        return Err(RemotePathError::Empty);
    }
    if rel.chars().count() > MAX_REMOTE_REL_CHARS {
        return Err(RemotePathError::TooLong);
    }
    if rel.bytes().any(|b| b == 0) {
        return Err(RemotePathError::NotAllowed);
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(RemotePathError::NotAllowed);
    }
    if rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return Err(RemotePathError::NotAllowed);
    }
    if rel.contains('\\') {
        return Err(RemotePathError::NotAllowed);
    }
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(RemotePathError::NotAllowed);
        }
        if is_secret_segment(seg) {
            return Err(RemotePathError::SecretDenied);
        }
    }
    Ok(())
}

/// Validate a full remote path: workspace allowlist first, then `rel`.
/// The workspace check runs before any path inspection so unknown ids
/// reveal nothing about path validity.
pub fn check_remote_path(
    workspace: &str,
    allowed: &[&str],
    rel: &str,
    is_symlink: bool,
) -> Result<(), RemotePathError> {
    check_workspace(workspace, allowed)?;
    check_rel(rel, is_symlink)
}

/// Gate one version-preconditioned edit. Requires an explicit
/// `expected_version` equal to `current_version`; on success returns the
/// next version (`current + 1`, saturating). Content over
/// [`MAX_REMOTE_FILE_BYTES`] is rejected before anything is retained.
pub fn check_edit(current_version: u64, edit: &VersionedEdit) -> Result<u64, EditError> {
    let expected = edit.expected_version.ok_or(EditError::VersionRequired)?;
    if expected != current_version {
        return Err(EditError::VersionMismatch);
    }
    if edit.content_len > MAX_REMOTE_FILE_BYTES {
        return Err(EditError::TooLarge);
    }
    Ok(current_version.saturating_add(1))
}

/// Gate one upload/download: cancellation first (deterministic stop),
/// then the hard per-transfer cap, then the caller quota.
pub fn check_transfer(
    len: usize,
    quota: &Quota,
    cancel: &CancelToken,
) -> Result<(), TransferError> {
    if cancel.cancelled {
        return Err(TransferError::Cancelled);
    }
    if len > MAX_TRANSFER_BYTES {
        return Err(TransferError::TooLarge);
    }
    if quota.used_bytes.saturating_add(len as u64) > quota.quota_bytes {
        return Err(TransferError::QuotaExceeded);
    }
    Ok(())
}

/// Cancellation cleanup: release caller-held staged bytes. Returns the
/// released count and zeroes the holder so a cancelled transfer retains
/// nothing. Pure ledger helper; the caller drops its own buffer.
pub fn apply_cancel(bytes_held: &mut u64) -> u64 {
    let released = *bytes_held;
    *bytes_held = 0;
    released
}

/// Force artifacts through the same approval/sandbox route as local
/// edits. `RemoteArtifactSamePath` normalizes to `LocalFileEdit`;
/// `RemoteBypass` is denied so no separate remote route exists.
pub fn check_artifact(path: ApprovalPath) -> Result<ApprovalPath, ArtifactError> {
    match path {
        ApprovalPath::LocalFileEdit | ApprovalPath::RemoteArtifactSamePath => {
            Ok(ApprovalPath::LocalFileEdit)
        }
        ApprovalPath::RemoteBypass => Err(ArtifactError::RemoteBypassDenied),
    }
}

/// Validate remote view/diff state: the `conflict` flag must agree with
/// the version comparison (`version != base_version` implies conflict).
/// Returns `Err` on a contradictory flag; message is a fixed string.
pub fn check_view(view: &RemoteView) -> Result<(), &'static str> {
    let diverged = view.version != view.base_version;
    if diverged == view.conflict {
        Ok(())
    } else {
        Err("conflict flag mismatch")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_view_preserves_version_conflict() {
        let v = RemoteView { version: 7, base_version: 7, conflict: false };
        assert!(check_view(&v).is_ok());
        assert_eq!(v.version, 7);
        let c = RemoteView { version: 8, base_version: 7, conflict: true };
        assert!(check_view(&c).is_ok());
        assert!(c.conflict);
    }

    #[test]
    fn view_conflict_flag_mismatch_rejected() {
        let v = RemoteView { version: 8, base_version: 7, conflict: false };
        assert!(check_view(&v).is_err());
    }

    #[test]
    fn traversal_denied_without_leakage() {
        for rel in ["../secret.txt", "/etc/passwd", "a/../../b", "a//b", "C:x"] {
            let err = check_rel(rel, false).expect_err("traversal must be denied");
            let msg = format!("{err}");
            assert!(!msg.contains(rel), "leak: {msg}");
            assert!(!msg.contains("passwd"), "leak: {msg}");
            assert!(!msg.contains("secret"), "leak: {msg}");
        }
        // Empty rel: any string contains ""; assert deny only, plus fixed message.
        let err = check_rel("", false).expect_err("empty must be denied");
        assert_eq!(format!("{err}"), "empty path");
        let err = check_rel(".env", false).expect_err("secret path denied");
        assert!(!format!("{err}").contains(".env"));
    }

    #[test]
    fn symlink_escape_denied() {
        assert_eq!(check_rel("a/b.txt", true), Err(RemotePathError::SymlinkDenied));
    }

    #[test]
    fn unknown_workspace_denied() {
        let allowed = ["ws1"];
        assert_eq!(
            check_remote_path("ws-nope", &allowed, "a.txt", false),
            Err(RemotePathError::UnknownWorkspace)
        );
        assert!(check_remote_path("ws1", &allowed, "a.txt", false).is_ok());
    }

    #[test]
    fn concurrent_edit_needs_version() {
        let cur = 5u64;
        assert_eq!(
            check_edit(cur, &VersionedEdit { expected_version: None, content_len: 10 }),
            Err(EditError::VersionRequired)
        );
        assert_eq!(
            check_edit(cur, &VersionedEdit { expected_version: Some(4), content_len: 10 }),
            Err(EditError::VersionMismatch)
        );
        assert_eq!(
            check_edit(cur, &VersionedEdit { expected_version: Some(5), content_len: 10 }),
            Ok(6)
        );
    }

    #[test]
    fn upload_over_cap_rejected() {
        let q = Quota { used_bytes: 0, quota_bytes: u64::MAX };
        let live = CancelToken { cancelled: false };
        assert_eq!(check_transfer(MAX_TRANSFER_BYTES + 1, &q, &live), Err(TransferError::TooLarge));
        let tight = Quota { used_bytes: 900, quota_bytes: 1000 };
        assert_eq!(check_transfer(200, &tight, &live), Err(TransferError::QuotaExceeded));
        assert!(check_transfer(100, &tight, &live).is_ok());
    }

    #[test]
    fn cancel_cleans_up() {
        let q = Quota { used_bytes: 0, quota_bytes: u64::MAX };
        let dead = CancelToken { cancelled: true };
        assert_eq!(check_transfer(10, &q, &dead), Err(TransferError::Cancelled));
        let mut held = 4096u64;
        let released = apply_cancel(&mut held);
        assert_eq!(released, 4096);
        assert_eq!(held, 0);
    }

    #[test]
    fn artifact_path_same_as_local() {
        assert_eq!(check_artifact(ApprovalPath::RemoteArtifactSamePath), Ok(ApprovalPath::LocalFileEdit));
        assert_eq!(check_artifact(ApprovalPath::LocalFileEdit), Ok(ApprovalPath::LocalFileEdit));
        assert_eq!(
            check_artifact(ApprovalPath::RemoteBypass),
            Err(ArtifactError::RemoteBypassDenied)
        );
    }
}

//! PROV-022: fail-closed authentication-storage planning and File0600 backend.
//!
//! Planning remains side-effect free. The explicit file operations below accept
//! caller-owned bytes for one bounded operation, use directory-relative nofollow
//! traversal, and never claim secure erasure.
#![forbid(unsafe_code)]

use std::{
    borrow::Borrow,
    fmt,
    path::{Component, Path, PathBuf},
};

use serde::ser::Serializer;
use serde::Serialize;
use thiserror::Error;

// rustix 1.1.4 imports for directory-relative, nofollow atomic file operations.
use rustix::{
    fd::OwnedFd,
    fs::{
        fchmod, fstat, fsync, linkat, mkdirat, open, openat, readlinkat, statat, unlinkat,
        AtFlags, FileType, Mode, OFlags,
    },
    io::Errno,
    process::geteuid,
};

/// Maximum path material accepted by this boundary, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Maximum provider identifier accepted by this boundary, in bytes.
pub const MAX_PROVIDER_ID_BYTES: usize = 128;
/// Maximum secret buffer size accepted by a caller-owned operation.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
/// Owner-only Unix file mode required by the file backend.
pub const FILE_MODE_0600: u32 = 0o600;
/// Directory mode for credential storage.
pub const DIR_MODE_0700: u32 = 0o700;
/// Temporary-file suffix callers should use before the atomic rename commit.
pub const ATOMIC_TEMP_SUFFIX: &str = ".tmp";

/// Borrowed, size-checked secret view for one caller-owned operation.
///
/// The bytes are never copied, formatted, serialized, or retained by this
/// module. The caller controls the backing buffer and its lifetime.
pub struct SealedSecret<'a>(&'a [u8]);

impl<'a> SealedSecret<'a> {
    /// Borrow and validate a caller-owned secret buffer.
    pub fn new(bytes: &'a [u8]) -> Result<Self, StoreError> {
        validate_secret(bytes)?;
        Ok(Self(bytes))
    }

    /// Borrow the bytes for the caller-owned backend operation.
    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.0
    }
}

impl AsRef<[u8]> for SealedSecret<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl fmt::Debug for SealedSecret<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SealedSecret(<redacted>)")
    }
}

/// Explicit credential-storage backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum StoreBackend {
    Keyring,
    File0600,
}

impl StoreBackend {
    /// Resolve a preferred backend from caller-supplied capability state.
    ///
    /// `keyring_available` is an explicit observation supplied by the caller;
    /// this function never probes the host keyring.
    #[must_use]
    pub const fn resolve(self, keyring_available: bool) -> Self {
        match (self, keyring_available) {
            (Self::Keyring, true) => Self::Keyring,
            (Self::Keyring, false) | (Self::File0600, _) => Self::File0600,
        }
    }

    /// Whether this backend has the owner-only file-mode requirement.
    #[must_use]
    pub const fn mode(self) -> Option<u32> {
        match self {
            Self::Keyring => None,
            Self::File0600 => Some(FILE_MODE_0600),
        }
    }
}

/// Typed failures with fixed, redacted codes.
///
/// Variants carry no caller path, provider text, OS error, or credential
/// material. This makes `Debug`, `Display`, and serialization safe for status
/// and diagnostic surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Error)]
#[serde(rename_all = "snake_case")]
pub enum StoreError {
    #[error("path_not_allowed")]
    PathNotAllowed,
    #[error("empty_provider")]
    EmptyProvider,
    #[error("provider_too_long")]
    ProviderTooLong,
    #[error("secret_too_large")]
    TooLarge,
    #[error("keyring_unavailable")]
    KeyringUnavailable,
    #[error("symlink_detected")]
    SymlinkDetected,
    #[error("parent_not_directory")]
    ParentNotDirectory,
    #[error("file_exists")]
    FileExists,
}

impl StoreError {
    /// Stable code suitable for redacted logs and wire status.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PathNotAllowed => "path_not_allowed",
            Self::EmptyProvider => "empty_provider",
            Self::ProviderTooLong => "provider_too_long",
            Self::TooLarge => "secret_too_large",
            Self::KeyringUnavailable => "keyring_unavailable",
            Self::SymlinkDetected => "symlink_detected",
            Self::ParentNotDirectory => "parent_not_directory",
            Self::FileExists => "file_exists",
        }
    }
}

/// A destination rooted at the caller-supplied storage root.
///
/// The root and relative path are retained only as operation metadata. The
/// type is syntactically checked and can be passed to a caller-owned writer;
/// construction performs no filesystem access or canonicalization.
#[derive(Clone, Eq, PartialEq)]
pub struct AllowlistedPath {
    root: PathBuf,
    relative: PathBuf,
}

impl fmt::Debug for AllowlistedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllowlistedPath")
            .field("label", &self.label())
            .finish()
    }
}

impl Serialize for AllowlistedPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
    }
}

impl AllowlistedPath {
    /// Construct a checked relative destination under `root`.
    pub fn new(root: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref();
        let dest = dest.as_ref();
        validate_dest(root, dest)?;
        Ok(Self {
            root: root.to_path_buf(),
            relative: dest.to_path_buf(),
        })
    }

    /// Alias spelling for callers that distinguish the storage root from the
    /// relative destination at the call site.
    pub fn from_relative(
        root: impl AsRef<Path>,
        dest: impl AsRef<Path>,
    ) -> Result<Self, StoreError> {
        Self::new(root, dest)
    }

    /// Caller-owned storage root. No access is performed.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Checked relative destination. No access is performed.
    #[must_use]
    pub fn relative(&self) -> &Path {
        &self.relative
    }

    /// Lexically joined destination for a caller-owned operation.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.relative)
    }

    /// Redacted destination label. The storage root is intentionally omitted.
    #[must_use]
    pub fn label(&self) -> &str {
        self.relative
            .to_str()
            .expect("AllowlistedPath rejects non-UTF-8 paths")
    }
}

/// Request for a storage plan. It contains no credential bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct StoreRequest {
    pub provider_id: String,
    pub backend: StoreBackend,
    pub dest: AllowlistedPath,
}

impl fmt::Debug for StoreRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoreRequest")
            .field("provider_id", &self.provider_id)
            .field("backend", &self.backend)
            .field("dest", &self.dest)
            .finish()
    }
}

impl Serialize for StoreRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct RedactedRequest<'a> {
            provider_id: &'a str,
            backend: StoreBackend,
            dest: &'a AllowlistedPath,
        }

        RedactedRequest {
            provider_id: &self.provider_id,
            backend: self.backend,
            dest: &self.dest,
        }
        .serialize(serializer)
    }
}

/// Redacted storage operation plan. The caller retains the request and owns
/// the subsequent file/keyring operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StorePlan {
    pub provider_id: String,
    pub backend: StoreBackend,
    pub dest_label: String,
    pub mode: u32,
}

/// Redacted refresh-persistence plan. `atomic` requires temp-file plus rename
/// semantics when the caller executes the plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RefreshPlan {
    pub provider_id: String,
    pub atomic: bool,
}

/// Redacted status observation supplied by this planning boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoreStatus {
    pub provider_id: String,
    pub backend: StoreBackend,
    pub exists: bool,
    pub error: Option<StoreError>,
}

fn validate_provider(provider_id: &str) -> Result<(), StoreError> {
    if provider_id.is_empty() {
        return Err(StoreError::EmptyProvider);
    }
    if provider_id.len() > MAX_PROVIDER_ID_BYTES || provider_id.contains('\0') {
        return Err(StoreError::ProviderTooLong);
    }
    Ok(())
}

fn path_text(path: &Path) -> Option<&str> {
    path.to_str().filter(|value| !value.is_empty())
}

fn path_len_ok(path: &Path) -> bool {
    path_text(path).is_some_and(|value| value.len() <= MAX_PATH_BYTES)
}

fn valid_root(root: &Path) -> bool {
    if !path_len_ok(root) || root.as_os_str().is_empty() {
        return false;
    }
    root.components().all(|component| {
        !matches!(component, Component::ParentDir)
            && !component
                .as_os_str()
                .to_string_lossy()
                .contains(['\\', '\0'])
    })
}

fn valid_relative_destination(dest: &Path) -> bool {
    if !path_len_ok(dest) || dest.is_absolute() {
        return false;
    }

    let text = dest.to_str().unwrap_or("");
    if text.starts_with('/')
        || text.ends_with('/')
        || text.contains("//")
        || text.contains('\\')
        || text.contains('\0')
    {
        return false;
    }

    let mut count = 0usize;
    for component in dest.components() {
        count = count.saturating_add(1);
        if !matches!(component, Component::Normal(_)) {
            return false;
        }
        let text = component.as_os_str().to_str().unwrap_or("");
        if text.is_empty() || text.contains(['\\', '\0', ':']) {
            return false;
        }
    }
    count != 0
}

/// Validate a destination without touching the filesystem.
///
/// `dest` must be a non-empty, UTF-8, relative path made only of normal path
/// components. The root is checked for parent traversal before lexical join.
/// This is a planning boundary, not a symlink or filesystem-authority check;
/// the caller-owned writer must enforce those capabilities when executing.
pub fn validate_dest(root: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<(), StoreError> {
    let root = root.as_ref();
    let dest = dest.as_ref();
    if !valid_root(root) || !valid_relative_destination(dest) {
        return Err(StoreError::PathNotAllowed);
    }
    let joined = root.join(dest);
    if !path_len_ok(&joined) || !joined.starts_with(root) {
        return Err(StoreError::PathNotAllowed);
    }
    Ok(())
}

fn checked_plan_request(request: &StoreRequest) -> Result<StorePlan, StoreError> {
    validate_provider(&request.provider_id)?;
    Ok(StorePlan {
        provider_id: request.provider_id.clone(),
        backend: request.backend,
        dest_label: request.dest.label().to_owned(),
        mode: request.backend.mode().unwrap_or(FILE_MODE_0600),
    })
}

/// Build a deterministic storage plan from either an owned or borrowed request.
/// No backend is opened and no secret bytes are accepted or retained.
pub fn plan_store<R>(request: R) -> Result<StorePlan, StoreError>
where
    R: Borrow<StoreRequest>,
{
    checked_plan_request(request.borrow())
}

/// Validate a caller-owned secret buffer before it is handed to a backend
/// operation. The returned slice remains owned by the caller.
pub fn validate_secret(secret: &[u8]) -> Result<(), StoreError> {
    if secret.len() > MAX_SECRET_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(())
}

/// Build a storage plan while validating, but never retaining, caller bytes.
pub fn plan_store_with_secret<R>(request: R, secret: &[u8]) -> Result<StorePlan, StoreError>
where
    R: Borrow<StoreRequest>,
{
    let _sealed = SealedSecret::new(secret)?;
    plan_store(request)
}

/// Build an atomic refresh persistence plan.
pub fn plan_refresh_persist(provider_id: impl AsRef<str>) -> Result<RefreshPlan, StoreError> {
    let provider_id = provider_id.as_ref();
    validate_provider(provider_id)?;
    Ok(RefreshPlan {
        provider_id: provider_id.to_owned(),
        atomic: true,
    })
}

/// Return a status projection without probing a backend.
///
/// Keyring availability is intentionally not inferred from the host. In this
/// isolated boundary a requested keyring operation is represented honestly as
/// the caller-owned `File0600` fallback plus a fixed failure code. Callers with
/// an explicit keyring capability may use [`status_with_keyring`].
#[must_use]
pub fn status_of(provider_id: impl AsRef<str>, backend: StoreBackend) -> StoreStatus {
    status_with_keyring(provider_id, backend, false)
}

/// Build status from explicit caller-supplied keyring availability.
#[must_use]
pub fn status_with_keyring(
    provider_id: impl AsRef<str>,
    backend: StoreBackend,
    keyring_available: bool,
) -> StoreStatus {
    let provider_id = provider_id.as_ref();
    if let Err(error) = validate_provider(provider_id) {
        return StoreStatus {
            provider_id: String::new(),
            backend,
            exists: false,
            error: Some(error),
        };
    }

    let effective = backend.resolve(keyring_available);
    let error = (backend == StoreBackend::Keyring && effective == StoreBackend::File0600)
        .then_some(StoreError::KeyringUnavailable);
    StoreStatus {
        provider_id: provider_id.to_owned(),
        backend: effective,
        exists: false,
        error,
    }
}

// ============================================================================
// Atomic File0600 Backend Implementation (APP-001-FILE0600-BACKEND-W1)
//
// Design notes (no TOCTOU overclaim):
// - Every leaf operation is directory-relative (openat/mkdirat/fstatat/
//   linkat/unlinkat) against a chain of opened directory fds, so the target
//   name and the final credentials directory are resolved without following
//   symlinks.
// - Ancestor symlink components are rejected, except for macOS's exact system
//   alias `/var -> /private/var`; ownership alone never grants trust.
// - Commit uses linkat(temp -> final name), which fails EEXIST when any name
//   already exists, including an attacker-created symlink, and never replaces
//   or follows it. This is the portable no-replace install primitive on macOS
//   and Linux (renameat2 RENAME_NOREPLACE is Linux-only; plain rename silently
//   replaces, so rename is not used for the commit).
// - A crash between linkat and unlinkat leaves the same durable bytes under
//   both names; a subsequent write unlinks the stale bounded temp name. No
//   secure erase is claimed; unlink only removes the directory entry.
// - Failures map to fixed redacted StoreError codes; no path, OS error or
//   credential text is retained.
// ============================================================================

/// Result of an atomic credential write operation.
#[derive(Debug)]
pub struct CredentialWriteResult {
    /// Path to the written credential file.
    pub credential_path: PathBuf,
    /// Whether the operation succeeded.
    pub success: bool,
}

/// Maximum bytes read by a single bounded nofollow load.
pub const MAX_LOAD_BYTES: u64 = MAX_SECRET_BYTES as u64;

/// Map a redaction-safe errno onto the fixed error surface.
fn map_errno(err: Errno) -> StoreError {
    match err {
        Errno::LOOP => StoreError::SymlinkDetected,
        Errno::EXIST => StoreError::FileExists,
        Errno::NOTDIR | Errno::ISDIR => StoreError::ParentNotDirectory,
        _ => StoreError::PathNotAllowed,
    }
}


/// Reject anything but an absolute, normal-component directory path.
fn checked_dir_path(dir: &Path) -> Result<Vec<std::ffi::OsString>, StoreError> {
    if !dir.is_absolute() || !path_len_ok(dir) {
        return Err(StoreError::PathNotAllowed);
    }
    let mut components = Vec::new();
    for component in dir.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => components.push(name.to_os_string()),
            _ => return Err(StoreError::PathNotAllowed),
        }
    }
    if components.is_empty() {
        return Err(StoreError::PathNotAllowed);
    }
    Ok(components)
}

/// Single-component name check for the leaf file.
fn checked_file_name(path: &Path) -> Result<std::ffi::OsString, StoreError> {
    let name = path.file_name().ok_or(StoreError::PathNotAllowed)?;
    let text = name.to_str().ok_or(StoreError::PathNotAllowed)?;
    if text.is_empty()
        || text.len() > MAX_PROVIDER_ID_BYTES
        || text.contains(['/', '\\', '\0', ':'])
    {
        return Err(StoreError::PathNotAllowed);
    }
    Ok(name.to_os_string())
}

/// A directory is trusted for descent when it is a real directory owned by
/// root or the effective uid, and either not group/other writable or the
/// classic root-owned sticky world-writable hop (e.g. `/tmp`, macOS
/// `/private/var/folders`). A non-sticky shared-writable directory is an
/// attacker planting ground and is rejected.
fn trusted_dir_stat(stat: &rustix::fs::Stat) -> bool {
    let file_type = FileType::from_raw_mode(stat.st_mode);
    let mode = stat.st_mode as u32;
    file_type.is_dir()
        && (stat.st_uid == 0 || stat.st_uid == geteuid().as_raw())
        && (mode & 0o022 == 0 || (stat.st_uid == 0 && mode & 0o1000 != 0))
}

/// Is `stat` an intermediate symlink candidate?
fn trusted_symlink_stat(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode).is_symlink()
}

/// Accept only macOS's fixed system `/var -> /private/var` alias.
/// Ownership alone is insufficient: an owned symlink can still redirect into
/// an attacker-controlled tree.
fn trusted_symlink_target(current: &OwnedFd, name: &std::ffi::OsStr) -> bool {
    #[cfg(target_os = "macos")]
    {
        if name != "var" {
            return false;
        }
        let target = match readlinkat(current, name, Vec::with_capacity(32)) {
            Ok(target) => target,
            Err(_) => return false,
        };
        target.as_bytes() == b"/private/var" || target.as_bytes() == b"private/var"
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (current, name);
        false
    }
}

/// Descend one directory component from `current` with a nofollow open.
/// A final-component symlink is always rejected. Intermediate symlinked
/// components are rejected except for the narrow macOS `/var` alias. Mode and
/// ownership checks for the final credentials directory happen after opening.
fn descend(
    current: &OwnedFd,
    name: &std::ffi::OsStr,
    final_dir: bool,
) -> Result<Option<OwnedFd>, StoreError> {
    match openat(
        current,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) {
        Ok(fd) => {
            if trusted_dir_fd(&fd)? {
                Ok(Some(fd))
            } else {
                Err(StoreError::PathNotAllowed)
            }
        }
        Err(Errno::NOENT) => Ok(None),
        Err(Errno::LOOP) if final_dir => Err(StoreError::SymlinkDetected),
        // macOS returns ENOTDIR (not ELOOP) when O_NOFOLLOW|O_DIRECTORY hits a
        // symlink component; treat it identically as a candidate hop.
        Err(Errno::LOOP) | Err(Errno::NOTDIR) if !final_dir => {
            // Inspect the link nofollow; follow at most this one component.
            let link_stat = statat(current, name, AtFlags::SYMLINK_NOFOLLOW).map_err(map_errno)?;
            if !trusted_symlink_stat(&link_stat) || !trusted_symlink_target(current, name) {
                return Err(StoreError::SymlinkDetected);
            }
            let fd = openat(
                current,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(map_errno)?;
            if trusted_dir_fd(&fd)? {
                Ok(Some(fd))
            } else {
                Err(StoreError::PathNotAllowed)
            }
        }
        Err(errno) => Err(map_errno(errno)),
    }
}

/// fstat an opened fd and report whether it is a directory owned by root or
/// the effective uid and not group/other writable.
fn trusted_dir_fd(fd: &OwnedFd) -> Result<bool, StoreError> {
    let stat = fstat(fd).map_err(map_errno)?;
    let result = trusted_dir_stat(&stat);
    Ok(result)
}

/// Walk from the filesystem root to `dir` using only directory-relative
/// nofollow opens (only the narrow macOS `/var` alias is excepted). Missing
/// components are created once at mode 0700 and re-resolved nofollow. The
/// final directory must end up owned by the effective uid with mode no wider
/// than 0700. Returns an open fd for the final directory.
fn open_secure_dir(dir: &Path) -> Result<OwnedFd, StoreError> {
    let components = checked_dir_path(dir)?;
    let mut current = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(map_errno)?;
    let last = components.len() - 1;
    for (index, name) in components.iter().enumerate() {
        let final_dir = index == last;
        let fd = match descend(&current, name, final_dir)? {
            Some(fd) => fd,
            None => {
                // Absent: one bounded creation, then re-resolve nofollow.
                match mkdirat(&current, name, Mode::RWXU) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(err) => return Err(map_errno(err)),
                }
                match descend(&current, name, final_dir)? {
                    Some(fd) => fd,
                    None => return Err(StoreError::PathNotAllowed),
                }
            }
        };
        if final_dir {
            let stat = fstat(&fd).map_err(map_errno)?;
            if !FileType::from_raw_mode(stat.st_mode).is_dir()
                || stat.st_uid != geteuid().as_raw()
                || (stat.st_mode as u32) & 0o7777 > DIR_MODE_0700
            {
                return Err(StoreError::PathNotAllowed);
            }
        }
        current = fd;
    }
    Ok(current)
}

/// Stat a leaf name relative to a directory fd without following a symlink.
fn lookup_relative(
    dir_fd: &OwnedFd,
    name: &std::ffi::OsStr,
) -> Result<Option<rustix::fs::Stat>, StoreError> {
    match statat(dir_fd, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some(stat)),
        Err(Errno::NOENT) => Ok(None),
        Err(err) => Err(map_errno(err)),
    }
}

/// Reject a leaf that is a symlink.
fn reject_symlink_leaf(stat: &rustix::fs::Stat) -> Result<(), StoreError> {
    if FileType::from_raw_mode(stat.st_mode).is_symlink() {
        return Err(StoreError::SymlinkDetected);
    }
    Ok(())
}

/// Create an exclusive, owner-only temp name in `dir_fd`, retrying once
/// against a stale regular temp file. Never follows a symlinked temp name.
fn create_temp_exclusive(
    dir_fd: &OwnedFd,
    leaf: &std::ffi::OsStr,
) -> Result<(std::fs::File, std::ffi::OsString), StoreError> {
    let leaf_text = leaf.to_str().ok_or(StoreError::PathNotAllowed)?;
    for attempt in 0..2u32 {
        let temp_name = std::ffi::OsString::from(format!(
            "{leaf_text}.{}.{}.tmp",
            std::process::id(),
            attempt
        ));
        match openat(
            dir_fd,
            &temp_name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                // Close any umask hole and verify the fd itself.
                fchmod(&fd, Mode::RUSR | Mode::WUSR).map_err(map_errno)?;
                let stat = fstat(&fd).map_err(map_errno)?;
                if !FileType::from_raw_mode(stat.st_mode).is_file() {
                    return Err(StoreError::PathNotAllowed);
                }
                if stat.st_uid != geteuid().as_raw() || stat.st_nlink != 1 {
                    return Err(StoreError::PathNotAllowed);
                }
                return Ok((std::fs::File::from(fd), temp_name));
            }
            Err(Errno::EXIST) => {
                // Something already holds this temp name. Never follow it.
                match lookup_relative(dir_fd, &temp_name)? {
                    Some(stat) if FileType::from_raw_mode(stat.st_mode).is_symlink() => {
                        return Err(StoreError::SymlinkDetected);
                    }
                    Some(stat) if FileType::from_raw_mode(stat.st_mode).is_file() => {
                        unlinkat(dir_fd, &temp_name, AtFlags::empty()).map_err(map_errno)?;
                    }
                    Some(_) | None => continue,
                }
            }
            Err(err) => return Err(map_errno(err)),
        }
    }
    Err(StoreError::FileExists)
}

/// Write data to the exclusive temp file, fully fsync it, re-verify the fd,
/// then commit with a nofollow no-replace linkat and drop the temp name.
fn commit_temp(
    dir_fd: &OwnedFd,
    mut temp_file: std::fs::File,
    temp_name: &std::ffi::OsStr,
    leaf: &std::ffi::OsStr,
    data: &[u8],
) -> Result<(), StoreError> {
    use std::io::Write;

    temp_file
        .write_all(data)
        .map_err(|_| StoreError::PathNotAllowed)?;
    temp_file.flush().map_err(|_| StoreError::PathNotAllowed)?;
    // Durability: full fsync of content and metadata before the commit.
    temp_file
        .sync_all()
        .map_err(|_| StoreError::PathNotAllowed)?;

    // Re-verify the very fd we wrote (regular, owned, 0600, single link).
    let stat = fstat(&temp_file).map_err(map_errno)?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_uid != geteuid().as_raw()
        || stat.st_nlink != 1
        || (stat.st_mode as u32) & 0o7777 != FILE_MODE_0600
    {
        return Err(StoreError::PathNotAllowed);
    }
    drop(temp_file);

    // Re-check the final name nofollow immediately before commit; the
    // linkat below closes the remaining window atomically (EEXIST).
    if let Some(stat) = lookup_relative(dir_fd, leaf)? {
        reject_symlink_leaf(&stat)?;
        return Err(StoreError::FileExists);
    }

    // No-replace, no-follow commit: linkat fails EEXIST when the name exists
    // at all, including as a symlink, and never overwrites it.
    linkat(dir_fd, temp_name, dir_fd, leaf, AtFlags::empty()).map_err(map_errno)?;

    // Drop the temp name; bytes remain durable under the final name.
    unlinkat(dir_fd, temp_name, AtFlags::empty()).map_err(map_errno)?;
    Ok(())
}

/// Write data atomically to `dest_path`: exclusive bounded temp file, full
/// write+fsync, no-replace nofollow commit, parent-directory fsync. An
/// existing regular file or symlink at the destination is rejected, not
/// replaced. Performs no secure erase.
pub fn write_atomic_file(
    dest_path: &Path,
    data: &[u8],
) -> Result<CredentialWriteResult, StoreError> {
    validate_secret(data)?;
    let parent = dest_path.parent().ok_or(StoreError::PathNotAllowed)?;
    let leaf = checked_file_name(dest_path)?;
    let dir_fd = open_secure_dir(parent)?;

    if let Some(stat) = lookup_relative(&dir_fd, &leaf)? {
        reject_symlink_leaf(&stat)?;
        return Err(StoreError::FileExists);
    }

    let (temp_file, temp_name) = create_temp_exclusive(&dir_fd, &leaf)?;
    let result = commit_temp(&dir_fd, temp_file, &temp_name, &leaf, data);

    // On failure remove the temp entry; content erase is not claimed.
    if result.is_err() {
        unlinkat(&dir_fd, &temp_name, AtFlags::empty()).ok();
    }
    result?;

    // Persist the final directory entry.
    fsync(&dir_fd).map_err(map_errno)?;
    Ok(CredentialWriteResult {
        credential_path: dest_path.to_path_buf(),
        success: true,
    })
}

/// Remove a credential file via a directory-relative nofollow unlink.
/// Rejects symlinks; performs no secure erase, only entry removal.
pub fn remove_credential(path: &Path) -> Result<(), StoreError> {
    let parent = path.parent().ok_or(StoreError::PathNotAllowed)?;
    let leaf = checked_file_name(path)?;
    let dir_fd = open_secure_dir(parent)?;

    match lookup_relative(&dir_fd, &leaf)? {
        None => return Err(StoreError::PathNotAllowed),
        Some(stat) => {
            reject_symlink_leaf(&stat)?;
            if !FileType::from_raw_mode(stat.st_mode).is_file() {
                return Err(StoreError::ParentNotDirectory);
            }
        }
    }

    unlinkat(&dir_fd, &leaf, AtFlags::empty()).map_err(map_errno)?;
    fsync(&dir_fd).map_err(map_errno)?;
    Ok(())
}

/// Bounded nofollow load of a credential file. Opens relative to a verified
/// directory chain, re-validates the fd (regular, owned by euid, exactly mode
/// 0600, single link) and caps reads at [`MAX_LOAD_BYTES`].
pub fn load_credential(path: &Path) -> Result<Vec<u8>, StoreError> {
    let parent = path.parent().ok_or(StoreError::PathNotAllowed)?;
    let leaf = checked_file_name(path)?;
    let dir_fd = open_secure_dir(parent)?;

    let fd = openat(
        &dir_fd,
        &leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(map_errno)?;
    let mut file = std::fs::File::from(fd);

    let stat = fstat(&file).map_err(map_errno)?;
    let file_type = FileType::from_raw_mode(stat.st_mode);
    if file_type.is_symlink() {
        return Err(StoreError::SymlinkDetected);
    }
    if !file_type.is_file() {
        return Err(StoreError::PathNotAllowed);
    }
    if stat.st_uid != geteuid().as_raw() || stat.st_nlink != 1 {
        return Err(StoreError::PathNotAllowed);
    }
    if (stat.st_mode as u32) & 0o7777 != FILE_MODE_0600 {
        return Err(StoreError::PathNotAllowed);
    }
    if stat.st_size > MAX_LOAD_BYTES.try_into().unwrap_or(i64::MAX) {
        return Err(StoreError::TooLarge);
    }

    use std::io::Read;
    let mut buffer = Vec::new();
    file.by_ref()
        .take(MAX_LOAD_BYTES + 1)
        .read_to_end(&mut buffer)
        .map_err(|_| StoreError::PathNotAllowed)?;
    if buffer.len() as u64 > MAX_LOAD_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn test_fixture_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("disposable test dir")
    }

    #[test]
    fn atomic_write_creates_file_with_correct_permissions() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");

        let result = write_atomic_file(&cred_path, b"test-credential-data");
        assert!(result.is_ok(), "atomic write should succeed");

        let result = result.unwrap();
        assert!(result.success);

        // Verify file exists
        assert!(cred_path.exists(), "credential file should exist");

        // Verify permissions
        let mode = cred_path.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "credential file should have mode 0600");

        // Verify content
        let content = std::fs::read(&cred_path).unwrap();
        assert_eq!(content, b"test-credential-data");
    }

    #[test]
    fn atomic_write_creates_parent_directory_with_mode_0700() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("nested").join("openai.json");

        let result = write_atomic_file(&cred_path, b"test-data");
        assert!(result.is_ok());

        let cred_dir = dir.path().join("credentials");
        let nested_dir = cred_dir.join("nested");

        // Check credentials directory mode
        let cred_dir_mode = cred_dir.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(cred_dir_mode, 0o700, "credentials dir should have mode 0700");

        // Check nested directory mode
        let nested_mode = nested_dir.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(nested_mode, 0o700, "nested dir should have mode 0700");
    }

    #[test]
    fn atomic_write_rejects_symlink_destination() {
        let dir = test_fixture_dir();

        // Create symlink pointing outside
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"outside content").unwrap();

        let cred_path = dir.path().join("credentials").join("openai.json");
        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&outside, &cred_path).unwrap();

        let result = write_atomic_file(&cred_path, b"test-data");
        assert_eq!(result.unwrap_err(), StoreError::SymlinkDetected);

        // Verify original file unchanged
        let outside_content = std::fs::read(&outside).unwrap();
        assert_eq!(outside_content, b"outside content");
    }

    #[test]
    fn atomic_write_fails_if_file_exists() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");

        // Create existing file
        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(&cred_path, b"existing content").unwrap();

        let result = write_atomic_file(&cred_path, b"new data");
        assert_eq!(result.unwrap_err(), StoreError::FileExists);
    }

    #[test]
    fn atomic_write_creates_no_temp_artifacts() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");

        let result = write_atomic_file(&cred_path, b"test-data");
        assert!(result.is_ok());

        // Check no temp files
        let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
        for entry in files {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.contains(".tmp"),
                "should not leave temp files: {name}"
            );
        }
    }

    #[test]
    fn remove_credential_removes_file_and_syncs_parent() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");

        // Create file
        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(&cred_path, b"test-data").unwrap();
        std::fs::set_permissions(&cred_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert!(cred_path.exists());

        let result = remove_credential(&cred_path);
        assert!(result.is_ok());

        // Verify file removed
        assert!(!cred_path.exists(), "credential file should be removed");
    }

    #[test]
    fn load_credential_returns_data() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");

        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(&cred_path, b"secret-data").unwrap();
        std::fs::set_permissions(&cred_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let data = load_credential(&cred_path);
        assert!(data.is_ok());
        assert_eq!(data.unwrap(), b"secret-data");
    }

    #[test]
    fn load_credential_rejects_symlink() {
        let dir = test_fixture_dir();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"outside content").unwrap();

        let cred_path = dir.path().join("credentials").join("openai.json");
        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&outside, &cred_path).unwrap();

        let result = load_credential(&cred_path);
        assert_eq!(result.unwrap_err(), StoreError::SymlinkDetected);
    }

    #[test]
    fn stale_temp_is_swept_before_commit() {
        let dir = test_fixture_dir();
        let cred_dir = dir.path().join("credentials");
        std::fs::create_dir_all(&cred_dir).unwrap();
        std::fs::set_permissions(&cred_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cred_path = cred_dir.join("openai.json");
        // Pre-create the attempt-0 temp name as a stale regular file.
        let stale = cred_dir.join(format!("openai.json.{}.0.tmp", std::process::id()));
        std::fs::write(&stale, b"stale").unwrap();
        let result = write_atomic_file(&cred_path, b"fresh");
        assert!(result.is_ok(), "stale regular temp must be swept");
        assert_eq!(std::fs::read(&cred_path).unwrap(), b"fresh");
        assert!(!stale.exists(), "temp name must be gone after commit");
    }

    #[test]
    fn symlinked_temp_name_is_rejected() {
        let dir = test_fixture_dir();
        let cred_dir = dir.path().join("credentials");
        std::fs::create_dir_all(&cred_dir).unwrap();
        std::fs::set_permissions(&cred_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"outside").unwrap();
        let cred_path = cred_dir.join("openai.json");
        let trap = cred_dir.join(format!("openai.json.{}.0.tmp", std::process::id()));
        std::os::unix::fs::symlink(&outside, &trap).unwrap();
        let result = write_atomic_file(&cred_path, b"payload");
        assert_eq!(result.unwrap_err(), StoreError::SymlinkDetected);
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside", "not written through");
        assert!(!cred_path.exists(), "nothing committed");
    }

    #[test]
    fn write_rejects_oversize_secret_before_touching_fs() {
        let dir = test_fixture_dir();
        let cred_path = dir.path().join("credentials").join("openai.json");
        let big = vec![0u8; MAX_SECRET_BYTES + 1];
        assert_eq!(
            write_atomic_file(&cred_path, &big).unwrap_err(),
            StoreError::TooLarge
        );
        let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert!(entries.is_empty(), "nothing written");
    }

    #[test]
    fn remove_credential_rejects_symlink() {
        let dir = test_fixture_dir();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"outside content").unwrap();

        let cred_path = dir.path().join("credentials").join("openai.json");
        std::fs::create_dir_all(cred_path.parent().unwrap()).unwrap();
        std::fs::set_permissions(cred_path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&outside, &cred_path).unwrap();

        let result = remove_credential(&cred_path);
        assert_eq!(result.unwrap_err(), StoreError::SymlinkDetected);

        // Verify original file unchanged
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside content");
    }
}

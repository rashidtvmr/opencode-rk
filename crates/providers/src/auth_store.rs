//! PROV-022: fail-closed authentication-storage planning boundary.
//!
//! This module deliberately stops at typed, caller-owned plans. It does not
//! open a keyring, read or write a file, inspect the environment, use the
//! network, or retain credential bytes. A caller with the required capability
//! executes the returned plan and owns the secret buffer and its lifetime.
#![forbid(unsafe_code)]

use std::{
    borrow::Borrow,
    fmt,
    path::{Component, Path, PathBuf},
};

use serde::ser::Serializer;
use serde::Serialize;
use thiserror::Error;

/// Maximum path material accepted by this boundary, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Maximum provider identifier accepted by this boundary, in bytes.
pub const MAX_PROVIDER_ID_BYTES: usize = 128;
/// Maximum secret buffer size accepted by a caller-owned operation.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;
/// Owner-only Unix file mode required by the file backend.
pub const FILE_MODE_0600: u32 = 0o600;
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

/// Explicit credential-storage backend. No backend is opened by this module.
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

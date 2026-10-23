//! Native first-run configuration import: shown-once offer state, redacted
//! preview, conflict-preserving merge, and confirmed atomic apply.
//!
//! No UI, discovery, normalizer or broker dependency lives here. The caller
//! supplies an already-normalized JSON patch, an explicit own-config destination
//! and source metadata (provenance + write-overlap guard). Wiring into
//! `main`/`tui`/`native_setup` is still pending; the integration test includes
//! this file with `#[path]` so it compiles without registration.
//!
//! Contract for that wiring:
//! - startup: `show_offer(&state_path)` -> render at most one visible offer;
//! - accept/decline: `mark_offer_decision(...)` -> later restarts stay silent;
//! - Settings "Import...": `reopen_offer(...)` then show again;
//! - preview: `preview(existing.as_ref(), &patch)` -> display `redacted` and
//!   `conflicts`, never raw `merged`;
//! - apply: `apply_confirmed(&our_dest, &patch, true, &sources, &broker)`.
//!   The broker adapter implements `ConfigWritePolicy`; this is user consent
//!   for a config write, not a tool approval or permission grant.
//! - never pass an `our_dest` inside any `sources` path: the application
//!   refuses with `OverlappingSource`. In particular the discovered OpenCode
//!   global tree (`~/.config/opencode`) is a source, not our destination.
//!
//! Guarantees: source bytes are never written; a malformed existing config is
//! reported without overwrite; imported commands, hooks, MCP command metadata
//! and other settings are written as inert data and never executed or
//! activated by this module; the destination is replaced by temp-file rename
//! (atomic on Unix, fail-closed elsewhere).
//!
//! Source-specific unsupported fields (for example hooks/plugins that a
//! particular upstream app cannot represent faithfully) are the normalizer's
//! responsibility: it must report them explicitly, not silently drop them.

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};

pub const STATE_VERSION: u64 = 1;
pub const MAX_PATCH_BYTES: usize = 1024 * 1024;
pub const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_STATE_BYTES: usize = 64 * 1024;
pub const MAX_DEPTH: usize = 32;
pub const MAX_NODES: usize = 20_000;
pub const MAX_KEY_BYTES: usize = 256;
pub const MAX_STRING_BYTES: usize = 64 * 1024;
pub const MAX_SOURCES: usize = 64;
pub const REDACTED: &str = "<redacted>";

const MAX_SOURCE_LABEL_BYTES: usize = 256;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One discovered configuration source. `path` may be a directory or a file.
/// It is provenance and a write boundary only; this module never reads or
/// expands it. Boundaries are compared after resolving symlinks (nearest
/// existing ancestor for paths that do not exist yet).
#[derive(Clone, Debug)]
pub struct SourceMeta {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Preview {
    /// Real merged config. Not for display: contains secret values.
    pub merged: Value,
    /// Safe-to-render copy of `merged` with secret values replaced.
    pub redacted: Value,
    /// JSON-pointer paths where the existing value won (existing config wins).
    pub conflicts: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ApplyReport {
    pub dest: PathBuf,
    pub bytes_written: u64,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferDecision {
    Accepted,
    Declined,
}

impl OfferDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}

/// Broker/policy seam. The caller adapts its existing permission broker.
/// Authorizes a user-initiated config import write; it is not a tool approval
/// and grants no runtime permission.
pub trait ConfigWritePolicy {
    fn authorize_config_import(
        &self,
        dest: &Path,
        sources: &[SourceMeta],
    ) -> Result<(), ImportError>;
}

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidPatch(&'static str),
    InvalidExisting(&'static str),
    PatchTooLarge,
    ConfigTooLarge,
    StateTooLarge,
    MalformedExistingConfig,
    ConfirmationRequired,
    UnsafePath(&'static str),
    UnsafeSource(&'static str),
    OverlappingSource,
    PolicyDenied(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "config import I/O error: {error}"),
            Self::Json(error) => write!(f, "config import JSON error: {error}"),
            Self::InvalidPatch(reason) => write!(f, "invalid import patch: {reason}"),
            Self::InvalidExisting(reason) => write!(f, "invalid existing config: {reason}"),
            Self::PatchTooLarge => {
                write!(f, "import patch exceeds {MAX_PATCH_BYTES} bytes")
            }
            Self::ConfigTooLarge => {
                write!(f, "config exceeds {MAX_CONFIG_BYTES} bytes")
            }
            Self::StateTooLarge => {
                write!(f, "import state exceeds {MAX_STATE_BYTES} bytes")
            }
            Self::MalformedExistingConfig => write!(
                f,
                "existing config is missing or not a JSON object; refusing to overwrite"
            ),
            Self::ConfirmationRequired => {
                write!(f, "config import requires explicit confirmation")
            }
            Self::UnsafePath(what) => {
                write!(f, "{what} must be an absolute, normalized file path")
            }
            Self::UnsafeSource(reason) => {
                write!(f, "unsafe configuration source: {reason}")
            }
            Self::OverlappingSource => write!(
                f,
                "refusing to write the destination inside a configuration source tree"
            ),
            Self::PolicyDenied(reason) => {
                write!(f, "config import write denied by policy: {reason}")
            }
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Whether the one-time startup offer should be shown. Only a complete,
/// recognized marker suppresses it: `version == 1` and decision
/// `"accepted"`/`"declined"`. Missing, truncated, unknown-version or
/// unknown-decision state is treated as offer-pending.
pub fn show_offer(state_path: &Path) -> Result<bool, ImportError> {
    validate_owned_path(state_path, "state path")?;
    let state = load_state(state_path)?;
    Ok(!state.as_ref().map(is_decided_state).unwrap_or(false))
}

fn is_decided_state(state: &Value) -> bool {
    state.get("version").and_then(Value::as_u64) == Some(STATE_VERSION)
        && matches!(
            state.get("decision").and_then(Value::as_str),
            Some("accepted" | "declined")
        )
}

/// Persist the visible offer decision. This is the shown-once marker.
pub fn mark_offer_decision(
    state_path: &Path,
    decision: OfferDecision,
) -> Result<(), ImportError> {
    validate_owned_path(state_path, "state path")?;
    atomic_write_value(
        state_path,
        &state_value(Some(decision)),
        MAX_STATE_BYTES,
        ImportError::StateTooLarge,
    )?;
    Ok(())
}

/// Settings action: clear the marker so the import offer can be shown again.
pub fn reopen_offer(state_path: &Path) -> Result<(), ImportError> {
    validate_owned_path(state_path, "state path")?;
    atomic_write_value(
        state_path,
        &state_value(None),
        MAX_STATE_BYTES,
        ImportError::StateTooLarge,
    )?;
    Ok(())
}

/// Redacted, conflict-reporting merge of an already-normalized patch.
///
/// Existing user values always win on scalar/array conflicts; the conflicting
/// JSON pointer is reported unless both values are already identical (repeat
/// imports are idempotent and produce no spurious conflicts). Object keys are
/// merged recursively. Commands/hooks are ordinary inert data here; this
/// module never executes them.
pub fn preview(
    existing: Option<&Value>,
    patch: &Value,
) -> Result<Preview, ImportError> {
    if let Some(existing) = existing {
        check_value(existing, MAX_CONFIG_BYTES, BoundKind::Existing)?;
    }
    check_value(patch, MAX_PATCH_BYTES, BoundKind::Patch)?;

    let mut merged = match existing {
        Some(value) => value.clone(),
        None => Value::Object(Map::new()),
    };
    if !merged.is_object() {
        return Err(ImportError::MalformedExistingConfig);
    }

    let mut conflicts = Vec::new();
    merge_into(&mut merged, patch, "", &mut conflicts);
    let redacted = redact(&merged);
    Ok(Preview {
        merged,
        redacted,
        conflicts,
    })
}

/// Read the destination config for preview. Missing means first import; a
/// malformed or non-object file is an error so apply can fail without writing.
pub fn read_existing_config(dest: &Path) -> Result<Option<Value>, ImportError> {
    validate_owned_path(dest, "config destination")?;
    let Some(bytes) = read_capped(dest, MAX_CONFIG_BYTES, ImportError::ConfigTooLarge)?
    else {
        return Ok(None);
    };
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| ImportError::MalformedExistingConfig)?;
    if !value.is_object() {
        return Err(ImportError::MalformedExistingConfig);
    }
    Ok(Some(value))
}

/// Apply only with explicit confirmation and a broker/policy grant.
///
/// Reads the destination itself (no caller-supplied stale state), refuses when
/// the destination overlaps any source tree, writes atomically to `dest` only,
/// and never touches a source file or activates imported commands/hooks.
pub fn apply_confirmed(
    dest: &Path,
    patch: &Value,
    confirmed: bool,
    sources: &[SourceMeta],
    policy: &dyn ConfigWritePolicy,
) -> Result<ApplyReport, ImportError> {
    if !confirmed {
        return Err(ImportError::ConfirmationRequired);
    }
    validate_owned_path(dest, "config destination")?;
    validate_sources(sources)?;
    check_no_source_overlap(dest, sources)?;

    let existing = read_existing_config(dest)?;
    let preview = preview(existing.as_ref(), patch)?;
    policy.authorize_config_import(dest, sources)?;

    let bytes_written = atomic_write_value(
        dest,
        &preview.merged,
        MAX_CONFIG_BYTES,
        ImportError::ConfigTooLarge,
    )?;
    Ok(ApplyReport {
        dest: dest.to_path_buf(),
        bytes_written,
        conflicts: preview.conflicts,
    })
}

/// Compare a source file against bytes captured before import. The caller owns
/// the path; this only reads, bounded by the baseline length.
pub fn source_bytes_unchanged(
    path: &Path,
    prior: &[u8],
) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut current = Vec::new();
    Read::by_ref(&mut file)
        .take(prior.len() as u64 + 1)
        .read_to_end(&mut current)?;
    Ok(current == prior)
}

fn state_value(decision: Option<OfferDecision>) -> Value {
    let mut state = Map::new();
    state.insert("version".to_owned(), Value::from(STATE_VERSION));
    if let Some(decision) = decision {
        state.insert(
            "decision".to_owned(),
            Value::from(decision.as_str()),
        );
    }
    Value::Object(state)
}

fn load_state(state_path: &Path) -> Result<Option<Value>, ImportError> {
    let Some(bytes) =
        read_capped(state_path, MAX_STATE_BYTES, ImportError::StateTooLarge)?
    else {
        return Ok(None);
    };
    Ok(serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object))
}

fn read_capped(
    path: &Path,
    cap: usize,
    too_large: ImportError,
) -> Result<Option<Vec<u8>>, ImportError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(too_large);
    }
    Ok(Some(bytes))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundKind {
    Patch,
    Existing,
}

impl BoundKind {
    fn invalid(self, reason: &'static str) -> ImportError {
        match self {
            Self::Patch => ImportError::InvalidPatch(reason),
            Self::Existing => ImportError::InvalidExisting(reason),
        }
    }

    fn too_large(self) -> ImportError {
        match self {
            Self::Patch => ImportError::PatchTooLarge,
            Self::Existing => ImportError::ConfigTooLarge,
        }
    }
}

struct CappedCounter {
    written: usize,
    cap: usize,
}

impl Write for CappedCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written += buf.len();
        if self.written > self.cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "serialized size cap",
            ));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn check_value(
    value: &Value,
    max_bytes: usize,
    kind: BoundKind,
) -> Result<(), ImportError> {
    if !value.is_object() {
        return match kind {
            BoundKind::Patch => Err(ImportError::InvalidPatch(
                "root must be a JSON object",
            )),
            BoundKind::Existing => Err(ImportError::MalformedExistingConfig),
        };
    }
    let mut nodes = 0usize;
    walk(value, 0, &mut nodes, kind)?;
    let mut counter = CappedCounter {
        written: 0,
        cap: max_bytes,
    };
    serde_json::to_writer(&mut counter, value).map_err(|_| kind.too_large())?;
    Ok(())
}

fn walk(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
    kind: BoundKind,
) -> Result<(), ImportError> {
    if depth > MAX_DEPTH {
        return Err(kind.invalid("nesting exceeds limit"));
    }
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(kind.invalid("too many values"));
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.len() > MAX_KEY_BYTES {
                    return Err(kind.invalid("object key too long"));
                }
                walk(child, depth + 1, nodes, kind)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                walk(child, depth + 1, nodes, kind)?;
            }
        }
        Value::String(text) => {
            if text.len() > MAX_STRING_BYTES {
                return Err(kind.invalid("string value too long"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn merge_into(
    target: &mut Value,
    patch: &Value,
    base: &str,
    conflicts: &mut Vec<String>,
) {
    let (Some(target_map), Some(patch_map)) = (target.as_object_mut(), patch.as_object())
    else {
        return;
    };
    for (key, incoming) in patch_map {
        let pointer = join_pointer(base, key);
        if !target_map.contains_key(key) {
            target_map.insert(key.clone(), incoming.clone());
            continue;
        }
        let existing = target_map.get_mut(key).expect("key checked above");
        if existing.is_object() && incoming.is_object() {
            merge_into(existing, incoming, &pointer, conflicts);
        } else if *existing != *incoming {
            conflicts.push(pointer);
        }
    }
}

fn join_pointer(base: &str, key: &str) -> String {
    let escaped = key.replace('~', "~0").replace('/', "~1");
    format!("{base}/{escaped}")
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = Map::new();
            for (key, child) in map {
                let value = if is_secret_key(key) {
                    Value::String(REDACTED.to_owned())
                } else {
                    redact(child)
                };
                redacted.insert(key.clone(), value);
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase().replace('-', "_").replace('.', "_");
    let compact = lower.replace('_', "");
    matches!(
        compact.as_str(),
        "apikey"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "authtoken"
            | "secret"
            | "clientsecret"
            | "password"
            | "passwd"
            | "authorization"
            | "cookie"
            | "credential"
            | "credentials"
            | "privatekey"
            | "accesskey"
    ) || lower.ends_with("_api_key")
        || lower.ends_with("_access_key")
        || lower.ends_with("_private_key")
        || lower.ends_with("_client_secret")
}

fn validate_owned_path(path: &Path, what: &'static str) -> Result<(), ImportError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(ImportError::UnsafePath(what));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ImportError::UnsafePath(what));
    }
    Ok(())
}

fn validate_sources(sources: &[SourceMeta]) -> Result<(), ImportError> {
    if sources.len() > MAX_SOURCES {
        return Err(ImportError::UnsafeSource("too many sources"));
    }
    for source in sources {
        let label = source.label.trim();
        if label.is_empty() {
            return Err(ImportError::UnsafeSource("source label is empty"));
        }
        if label.len() > MAX_SOURCE_LABEL_BYTES {
            return Err(ImportError::UnsafeSource("source label too long"));
        }
        if !source.path.is_absolute() {
            return Err(ImportError::UnsafeSource(
                "source path must be absolute",
            ));
        }
        if source.path.file_name().is_none()
            || source.path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::CurDir
                )
            })
        {
            return Err(ImportError::UnsafeSource(
                "source path must be normalized",
            ));
        }
    }
    Ok(())
}

fn check_no_source_overlap(
    dest: &Path,
    sources: &[SourceMeta],
) -> Result<(), ImportError> {
    if fs::symlink_metadata(dest)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(ImportError::UnsafePath(
            "config destination must not be a symlink",
        ));
    }
    let resolved_dest = resolve_boundary(dest)?;
    for source in sources {
        let resolved_source = resolve_boundary(&source.path)?;
        if resolved_dest == resolved_source
            || resolved_dest.starts_with(&resolved_source)
            || resolved_source.starts_with(&resolved_dest)
        {
            return Err(ImportError::OverlappingSource);
        }
    }
    Ok(())
}

/// Resolve symlinks for an existing path, or for the nearest existing ancestor
/// plus the not-yet-created tail. This closes lexical symlink escapes without
/// requiring the destination (or a source) to already exist.
fn resolve_boundary(path: &Path) -> Result<PathBuf, ImportError> {
    match fs::canonicalize(path) {
        Ok(resolved) => return Ok(resolved),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error.into());
        }
        Err(_) => {}
    }
    let mut missing: Vec<OsString> = Vec::new();
    let mut cursor = path;
    loop {
        match fs::canonicalize(cursor) {
            Ok(mut resolved) => {
                for part in missing.iter().rev() {
                    resolved.push(part.as_os_str());
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or(ImportError::UnsafePath(
                    "path has no existing ancestor",
                ))?;
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or(ImportError::UnsafePath(
                    "path has no existing ancestor",
                ))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn atomic_write_value(
    dest: &Path,
    value: &Value,
    cap: usize,
    too_large: ImportError,
) -> Result<u64, ImportError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() > cap {
        return Err(too_large);
    }
    atomic_write_bytes(dest, &bytes)
}

fn atomic_write_bytes(dest: &Path, bytes: &[u8]) -> Result<u64, ImportError> {
    validate_owned_path(dest, "config destination")?;
    let parent = dest
        .parent()
        .ok_or(ImportError::UnsafePath("destination has no parent"))?;
    fs::create_dir_all(parent)?;

    let file_name = dest
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or(ImportError::UnsafePath("destination has no file name"))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = parent.join(format!(
        ".{file_name}.{}.{sequence}.tmp",
        std::process::id()
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp_path)?;
    let _guard = TempGuard {
        path: temp_path.clone(),
    };
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp_path, dest)?;
    #[cfg(unix)]
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(bytes.len() as u64)
}

struct TempGuard {
    path: PathBuf,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

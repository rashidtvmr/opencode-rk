//! Consent-gated planning for importing local provider credentials.
//!
//! This module is deliberately a pure boundary. It does not expand a home
//! directory, read a file, inspect ambient environment, copy bytes, or write a
//! destination. The caller supplies already obtained schema and permission
//! facts, then owns the later storage operation.

use std::{
    borrow::Borrow,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Maximum credential-file input accepted by [`validate_schema`].
pub const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;
/// Maximum configured source path length.
pub const MAX_SOURCE_PATH_BYTES: usize = 1024;

/// Redacted destination label for Codex credentials.
pub const CODEX_DEST_LABEL: &str = "protected://credentials/codex";
/// Redacted destination label for Claude Code credentials.
pub const CLAUDE_DEST_LABEL: &str = "protected://credentials/claude-code";
/// Implicit Codex source label. It is a label only; this module never resolves it.
pub const CODEX_DEFAULT_SOURCE_LABEL: &str = "~/.codex/auth.json";
/// Implicit Claude Code source label. It is a label only; this module never resolves it.
pub const CLAUDE_DEFAULT_SOURCE_LABEL: &str = "~/.claude/.credentials.json";

/// Explicitly selected local credential source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImportSource {
    /// The standard Codex auth file, without resolving the home directory.
    CodexDefault,
    /// The standard Claude Code credentials file, without resolving the home directory.
    ClaudeDefault,
    /// A caller-configured relative path below its separately supplied root.
    ///
    /// The path is never opened here. Absolute paths and traversal components
    /// are rejected by [`plan_import`].
    ConfigDir { path: PathBuf },
}

/// Human authority for a credential import.
///
/// Only [`UserConsent::Granted`] permits planning. Consent is intentionally
/// represented separately from source and schema facts, so a source selection
/// cannot act as implicit approval.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum UserConsent {
    /// No explicit human grant exists.
    NotGranted,
    /// An explicit human grant exists for this import request.
    Granted,
}

impl UserConsent {
    /// Compatibility spelling for callers that model a missing grant as absent.
    #[allow(non_upper_case_globals)]
    pub const Absent: Self = Self::NotGranted;
    /// Compatibility spelling for callers that model a missing grant as none.
    #[allow(non_upper_case_globals)]
    pub const None: Self = Self::NotGranted;
    /// Compatibility spelling for an explicit grant.
    #[allow(non_upper_case_globals)]
    pub const Explicit: Self = Self::Granted;
    /// Compatibility spelling for a confirmed grant.
    #[allow(non_upper_case_globals)]
    pub const Confirmed: Self = Self::Granted;
    /// Compatibility spelling for an explicit denial.
    #[allow(non_upper_case_globals)]
    pub const Denied: Self = Self::NotGranted;
}

/// Caller request. It contains no credential material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportRequest {
    pub source: ImportSource,
    pub consent: UserConsent,
}

/// Credential shape discovered by schema validation.
///
/// Values contain no token bytes. The actual credential remains caller-owned
/// and is not returned by this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CredentialKind {
    ApiKey,
    OAuth,
}

/// Caller-supplied permission metadata for the source file.
///
/// `mode` is the Unix permission bit field. Permission checks are pure and do
/// not call `metadata`, `stat`, or any other filesystem API.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathMetadata {
    pub mode: u32,
}

impl PathMetadata {
    #[must_use]
    pub const fn new(mode: u32) -> Self {
        Self { mode }
    }
}

/// Common aliases for callers whose permission broker uses a more specific
/// metadata name. All aliases retain the same small, secret-free shape.
pub type FileMetadata = PathMetadata;
pub type FilePermissions = PathMetadata;
pub type PathMeta = PathMetadata;

impl From<u32> for PathMetadata {
    fn from(mode: u32) -> Self {
        Self { mode }
    }
}

impl From<&PathMetadata> for PathMetadata {
    fn from(metadata: &PathMetadata) -> Self {
        *metadata
    }
}

impl From<bool> for PathMetadata {
    fn from(owner_only: bool) -> Self {
        Self {
            mode: if owner_only { 0o600 } else { 0o644 },
        }
    }
}

/// Redacted import plan. Execution and protected storage belong to a later
/// slice. No source path or credential bytes are retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportPlan {
    pub provider: String,
    pub kind: CredentialKind,
    pub dest_label: String,
}

/// Secret-free validation and planning failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error, Serialize, Deserialize)]
pub enum ImportError {
    #[error("explicit consent is required")]
    ConsentRequired,
    #[error("credential source is not allowlisted")]
    PathNotAllowed,
    #[error("credential schema is invalid")]
    BadSchema,
    #[error("credential source permissions are too permissive")]
    TooPermissive,
    #[error("credential input exceeds the byte limit")]
    TooLarge,
}

/// Validate the caller-supplied source file permission mode.
///
/// Any group or other permission bit is rejected. This is stricter than only
/// rejecting the world-read bit and keeps credentials owner-only (`0600`, or a
/// stricter owner-only mode). No filesystem metadata is read here.
pub fn check_permissions<M>(path_meta: M) -> Result<(), ImportError>
where
    M: Into<PathMetadata>,
{
    let metadata = path_meta.into();
    if metadata.mode & 0o077 != 0 {
        return Err(ImportError::TooPermissive);
    }
    Ok(())
}

/// Validate a bounded Codex or Claude Code credential JSON shape.
///
/// The parser recognizes documented local-file forms and returns only the
/// authentication kind. Unknown fields are ignored, while a root object that
/// contains no complete recognized credential shape is rejected. Secrets are
/// inspected only as transient parsed values and never copied into the result
/// or an error.
pub fn validate_schema(bytes: &[u8]) -> Result<CredentialKind, ImportError> {
    if bytes.len() > MAX_CREDENTIAL_BYTES {
        return Err(ImportError::TooLarge);
    }

    let value: Value = serde_json::from_slice(bytes).map_err(|_| ImportError::BadSchema)?;
    let object = value.as_object().ok_or(ImportError::BadSchema)?;

    let api_key = ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "api_key", "apiKey"]
        .into_iter()
        .any(|field| non_empty_string(object.get(field)));

    let oauth = has_oauth_pair(object, "access_token", "refresh_token")
        || has_oauth_pair(object, "accessToken", "refreshToken")
        || object
            .get("tokens")
            .and_then(Value::as_object)
            .is_some_and(|tokens| {
                has_oauth_pair(tokens, "access_token", "refresh_token")
                    || has_oauth_pair(tokens, "accessToken", "refreshToken")
            })
        || object
            .get("claudeAiOauth")
            .and_then(Value::as_object)
            .is_some_and(|oauth| {
                has_oauth_pair(oauth, "accessToken", "refreshToken")
                    || has_oauth_pair(oauth, "access_token", "refresh_token")
            })
        || object
            .get("oauth")
            .and_then(Value::as_object)
            .is_some_and(|oauth| {
                has_oauth_pair(oauth, "accessToken", "refreshToken")
                    || has_oauth_pair(oauth, "access_token", "refresh_token")
            });

    match (api_key, oauth) {
        (true, false) => Ok(CredentialKind::ApiKey),
        (false, true) => Ok(CredentialKind::OAuth),
        // A file containing both credential forms is ambiguous. Refuse it
        // rather than selecting one and potentially leaving another secret
        // subject to a later, unintended import.
        _ => Err(ImportError::BadSchema),
    }
}

/// Build a deterministic, redacted plan after consent, source, schema, and
/// permission gates pass.
///
/// `request`, `schema`, and `permissions` accept owned values or references to
/// make the boundary convenient for callers while retaining no input state.
/// Configured paths must be relative, traversal-free paths that a later caller
/// resolves below its explicitly supplied configuration root.
pub fn plan_import<R, S, P>(
    request: R,
    schema: S,
    permissions: P,
) -> Result<ImportPlan, ImportError>
where
    R: Borrow<ImportRequest>,
    S: Borrow<CredentialKind>,
    P: Into<PathMetadata>,
{
    let request = request.borrow();

    if request.consent != UserConsent::Granted {
        return Err(ImportError::ConsentRequired);
    }

    let (provider, dest_label) = source_target(&request.source)?;
    check_permissions(permissions)?;

    Ok(ImportPlan {
        provider: provider.to_owned(),
        kind: *schema.borrow(),
        dest_label: dest_label.to_owned(),
    })
}

fn non_empty_string(value: Option<&Value>) -> bool {
    value.is_some_and(|value| value.as_str().is_some_and(|text| !text.is_empty()))
}

fn has_oauth_pair(object: &serde_json::Map<String, Value>, access: &str, refresh: &str) -> bool {
    non_empty_string(object.get(access)) && non_empty_string(object.get(refresh))
}

fn source_target(source: &ImportSource) -> Result<(&'static str, &'static str), ImportError> {
    match source {
        ImportSource::CodexDefault => Ok(("codex", CODEX_DEST_LABEL)),
        ImportSource::ClaudeDefault => Ok(("claude-code", CLAUDE_DEST_LABEL)),
        ImportSource::ConfigDir { path } => {
            if !valid_config_path(path) {
                return Err(ImportError::PathNotAllowed);
            }

            // A configured root is caller-owned. Recognized provider markers
            // preserve the native provider identity; an otherwise valid custom
            // root remains explicit rather than guessed as either provider.
            let provider = if path_has_component(path, "claude")
                || path_has_component(path, ".claude")
                || path
                    .file_name()
                    .is_some_and(|name| name == ".credentials.json")
            {
                "claude-code"
            } else if path_has_component(path, "codex")
                || path_has_component(path, ".codex")
                || path.file_name().is_some_and(|name| name == "auth.json")
            {
                "codex"
            } else {
                "configured"
            };
            let label = if provider == "claude-code" {
                CLAUDE_DEST_LABEL
            } else if provider == "codex" {
                CODEX_DEST_LABEL
            } else {
                "protected://credentials/configured"
            };
            Ok((provider, label))
        }
    }
}

fn valid_config_path(path: &Path) -> bool {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.as_os_str().to_string_lossy().len() > MAX_SOURCE_PATH_BYTES
    {
        return false;
    }
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn path_has_component(path: &Path, expected: &str) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy() == expected)
    })
}

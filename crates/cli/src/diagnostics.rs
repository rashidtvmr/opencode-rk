#![forbid(unsafe_code)]

//! CLI diagnostics helpers (APP-011, slice: diagnostics).
//!
//! Standalone (`std` only, no external crates) so it compiles under
//! `rustc --edition 2021 --test crates/cli/src/diagnostics.rs`.
//! Covers: capability probes (provider/daemon/tools), config-precedence
//! ordering, last-valid fallback for invalid config, and redacted export
//! (secrets/transcripts dropped unless explicitly selected).

use std::{collections::BTreeMap, fs};

/// Bounded export limits (AGENTS.md: no unbounded retained output).
pub const MAX_EXPORT_FIELDS: usize = 128;
pub const MAX_EXPORT_VALUE_BYTES: usize = 8 * 1024;
pub const MAX_EXPORT_TRANSCRIPTS: usize = 256;

/// What subsystem a capability probe describes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProbeKind {
    Provider,
    Daemon,
    Tools,
}

/// Health of one probed subsystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeStatus {
    Ok,
    Unconfigured,
    Disabled,
    Error,
}

/// One capability probe result: provider connectivity, daemon liveness, or
/// tool inventory. `measured_workers` is the observed active worker/connection
/// count; [`CapabilityProbe::active_workers`] clamps it to zero while the
/// service is disabled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityProbe {
    pub kind: ProbeKind,
    pub name: String,
    pub status: ProbeStatus,
    pub detail: String,
    measured_workers: usize,
}

impl CapabilityProbe {
    pub fn new(
        kind: ProbeKind,
        name: impl Into<String>,
        status: ProbeStatus,
        detail: impl Into<String>,
        measured_workers: usize,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            status,
            detail: detail.into(),
            measured_workers,
        }
    }

    /// Constructor for a disabled service: always reports zero workers.
    pub fn disabled(kind: ProbeKind, name: impl Into<String>) -> Self {
        Self::new(kind, name, ProbeStatus::Disabled, "disabled", 0)
    }

    /// Measured active workers/connections. Disabled (or unconfigured)
    /// services report zero regardless of any stale counter.
    pub fn active_workers(&self) -> usize {
        match self.status {
            ProbeStatus::Disabled | ProbeStatus::Unconfigured => 0,
            ProbeStatus::Ok | ProbeStatus::Error => self.measured_workers,
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self.status, ProbeStatus::Ok)
    }
}

/// Where an effective config value came from, lowest → highest precedence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ConfigPrecedence {
    BuiltInDefault,
    ConfigFile,
    Environment,
    CliFlag,
}

impl ConfigPrecedence {
    pub fn rank(self) -> u8 {
        match self {
            Self::BuiltInDefault => 0,
            Self::ConfigFile => 1,
            Self::Environment => 2,
            Self::CliFlag => 3,
        }
    }

    /// Pick the winning `(value, source)` pair: highest precedence wins;
    /// ties keep the first entry so reload order is deterministic.
    pub fn resolve<'a>(
        candidates: &'a [(&'a str, ConfigPrecedence)],
    ) -> Option<(&'a str, ConfigPrecedence)> {
        let mut best: Option<(&'a str, ConfigPrecedence)> = None;
        for &(value, source) in candidates {
            match best {
                None => best = Some((value, source)),
                Some((_, current)) if source.rank() > current.rank() => {
                    best = Some((value, source));
                }
                _ => {}
            }
        }
        best
    }
}

/// Holds the last valid config; rejected updates keep serving the previous
/// valid state and count the failure for repair UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LastValid<T: Clone> {
    valid: T,
    rejected_updates: u64,
}

impl<T: Clone> LastValid<T> {
    pub fn new(initial: T) -> Self {
        Self {
            valid: initial,
            rejected_updates: 0,
        }
    }

    pub fn current(&self) -> &T {
        &self.valid
    }

    pub fn rejected_updates(&self) -> u64 {
        self.rejected_updates
    }

    /// Validate `candidate` with `is_valid`. On success it becomes current;
    /// on failure the previous valid value is preserved. Returns whether
    /// the candidate was accepted.
    pub fn try_update(&mut self, candidate: T, is_valid: bool) -> bool {
        if is_valid {
            self.valid = candidate;
            true
        } else {
            self.rejected_updates = self.rejected_updates.saturating_add(1);
            false
        }
    }

    /// Same as [`LastValid::try_update`] but runs a validator closure.
    pub fn try_update_with(&mut self, candidate: T, validate: impl FnOnce(&T) -> bool) -> bool {
        let ok = validate(&candidate);
        self.try_update(candidate, ok)
    }
}

/// Export selection: secrets and transcripts are excluded unless opted in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportOptions {
    pub include_secrets: bool,
    pub include_transcripts: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_secrets: false,
            include_transcripts: false,
        }
    }
}

/// Redacted diagnostic bundle ready for display/upload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedExport {
    pub fields: BTreeMap<String, String>,
    pub transcripts: Vec<String>,
    pub redacted_field_count: usize,
    pub truncated: bool,
}

/// Returns a human-readable name for the active OS sandbox backend.
///
/// - `"landlock"` when Landlock is detected (Linux >= 5.13 with Landlock in /proc/filesystems)
/// - `"unavailable on this platform"` otherwise
///
/// Never returns `"not yet implemented"` — that was the old hardcoded lie
/// in the doctor output. This function provides the honest live answer.
#[must_use]
pub fn sandbox_backend_name() -> &'static str {
    if landlock_available() {
        "landlock"
    } else {
        "unavailable on this platform"
    }
}

fn landlock_available() -> bool {
    kernel_at_least_5_13() && proc_filesystems_has_landlock()
}

fn kernel_at_least_5_13() -> bool {
    let version = match fs::read_to_string("/proc/version") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let after = match version.find("Linux version ") {
        Some(pos) => &version[pos + "Linux version ".len()..],
        None => return false,
    };
    let ver = after.split_whitespace().next().unwrap_or("");
    let parts: Vec<&str> = ver.split('.').take(2).collect();
    if parts.len() < 2 {
        return false;
    }
    let major: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let minor: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    major > 5 || (major == 5 && minor >= 13)
}

fn proc_filesystems_has_landlock() -> bool {
    let content = match fs::read_to_string("/proc/filesystems") {
        Ok(c) => c,
        Err(_) => return false,
    };
    content
        .lines()
        .any(|line| line.trim().contains("landlock"))
}

pub const REDACTED_MARKER: &str = "[redacted]";

/// True when a field name looks like a secret (case-insensitive substring).
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "api_key",
        "apikey",
        "auth",
        "credential",
        "private",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn truncate_to_bytes(s: &str, max_bytes: usize) -> (String, bool) {
    if s.len() <= max_bytes {
        return (s.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_owned(), true)
}

/// Build a redacted export: secret values become `[redacted]` and
/// transcripts are dropped unless the corresponding option is set.
/// Bounds: at most [`MAX_EXPORT_FIELDS`] fields, values capped at
/// [`MAX_EXPORT_VALUE_BYTES`] bytes, transcripts capped at
/// [`MAX_EXPORT_TRANSCRIPTS`] entries.
pub fn redacted_export(
    fields: &BTreeMap<String, String>,
    transcripts: &[String],
    options: ExportOptions,
) -> RedactedExport {
    let mut out = BTreeMap::new();
    let mut redacted_field_count = 0usize;
    let mut truncated = false;
    for (key, value) in fields.iter().take(MAX_EXPORT_FIELDS) {
        let secret = !options.include_secrets && is_secret_key(key);
        let rendered = if secret {
            redacted_field_count += 1;
            REDACTED_MARKER.to_owned()
        } else {
            let (cut, was_cut) = truncate_to_bytes(value, MAX_EXPORT_VALUE_BYTES);
            truncated |= was_cut;
            cut
        };
        out.insert(key.clone(), rendered);
    }
    truncated |= fields.len() > MAX_EXPORT_FIELDS;
    let transcripts_out = if options.include_transcripts {
        let n = transcripts.len().min(MAX_EXPORT_TRANSCRIPTS);
        truncated |= transcripts.len() > MAX_EXPORT_TRANSCRIPTS;
        transcripts[..n].to_vec()
    } else {
        truncated |= !transcripts.is_empty();
        Vec::new()
    };
    RedactedExport {
        fields: out,
        transcripts: transcripts_out,
        redacted_field_count,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("provider".to_owned(), "openai".to_owned()),
            ("OPENAI_API_KEY".to_owned(), "sk-live-123".to_owned()),
            ("auth_token".to_owned(), "tok-abc".to_owned()),
        ])
    }

    #[test]
    fn invalid_config_keeps_last_valid() {
        let mut holder = LastValid::new("good-v1".to_owned());
        assert!(!holder.try_update("bad-value".to_owned(), false));
        assert_eq!(holder.current(), "good-v1");
        assert_eq!(holder.rejected_updates(), 1);
        assert!(holder.try_update("good-v2".to_owned(), true));
        assert_eq!(holder.current(), "good-v2");
    }

    #[test]
    fn invalid_config_validator_closure_keeps_last_valid() {
        let mut holder = LastValid::new(3u32);
        let accepted = holder.try_update_with(0, |v| *v > 0);
        assert!(!accepted);
        assert_eq!(*holder.current(), 3);
    }

    #[test]
    fn export_redacts_secret_by_default() {
        let out = redacted_export(&sample_fields(), &[], ExportOptions::default());
        assert_eq!(
            out.fields.get("provider").map(String::as_str),
            Some("openai")
        );
        assert_eq!(
            out.fields.get("OPENAI_API_KEY").map(String::as_str),
            Some(REDACTED_MARKER)
        );
        assert_eq!(
            out.fields.get("auth_token").map(String::as_str),
            Some(REDACTED_MARKER)
        );
        assert_eq!(out.redacted_field_count, 2);
        assert!(!out.fields.values().any(|v| v.contains("sk-live-123")));
    }

    #[test]
    fn export_excludes_transcripts_unless_explicit() {
        let transcripts = vec!["user: hi".to_owned()];
        let default_out = redacted_export(&sample_fields(), &transcripts, ExportOptions::default());
        assert!(default_out.transcripts.is_empty());
        let explicit = redacted_export(
            &sample_fields(),
            &transcripts,
            ExportOptions {
                include_secrets: false,
                include_transcripts: true,
            },
        );
        assert_eq!(explicit.transcripts, transcripts);
        let with_secrets = redacted_export(
            &sample_fields(),
            &[],
            ExportOptions {
                include_secrets: true,
                include_transcripts: false,
            },
        );
        assert_eq!(
            with_secrets
                .fields
                .get("OPENAI_API_KEY")
                .map(String::as_str),
            Some("sk-live-123")
        );
    }

    #[test]
    fn disabled_service_reports_zero_workers() {
        for kind in [ProbeKind::Provider, ProbeKind::Daemon, ProbeKind::Tools] {
            let probe = CapabilityProbe::disabled(kind, "svc");
            assert_eq!(probe.status, ProbeStatus::Disabled);
            assert_eq!(probe.active_workers(), 0);
        }
        // Even a stale nonzero counter must read as zero while disabled.
        let mut stale = CapabilityProbe::disabled(ProbeKind::Daemon, "daemon");
        stale.measured_workers = 99;
        assert_eq!(stale.active_workers(), 0);
        let live = CapabilityProbe::new(ProbeKind::Daemon, "daemon", ProbeStatus::Ok, "ok", 4);
        assert_eq!(live.active_workers(), 4);
    }

    #[test]
    fn sandbox_backend_name_honest_not_yet_implemented() {
        let name = sandbox_backend_name();
        assert!(
            !name.contains("not yet implemented"),
            "must not claim 'not yet implemented': got {name:?}"
        );
        assert!(!name.is_empty());
        // On Linux, should be either "landlock" or "unavailable on this platform"
        assert!(
            name == "landlock" || name == "unavailable on this platform",
            "unexpected backend name: {name:?}"
        );
    }

    #[test]
    fn config_precedence_cli_beats_env_beats_file() {
        let candidates = [
            ("file-val", ConfigPrecedence::ConfigFile),
            ("env-val", ConfigPrecedence::Environment),
            ("cli-val", ConfigPrecedence::CliFlag),
            ("default-val", ConfigPrecedence::BuiltInDefault),
        ];
        assert_eq!(
            ConfigPrecedence::resolve(&candidates),
            Some(("cli-val", ConfigPrecedence::CliFlag))
        );
        let without_cli = &candidates[..2];
        assert_eq!(
            ConfigPrecedence::resolve(without_cli),
            Some(("env-val", ConfigPrecedence::Environment))
        );
        assert!(ConfigPrecedence::resolve(&[]).is_none());
    }
}

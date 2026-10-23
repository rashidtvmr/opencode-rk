//! Native import sources discovery: finds existing OpenCode, Codex, and Claude
//! Code configuration and agent definitions, normalizes them into a single
//! import plan, and converts that plan into an [`ImportDialog`] ready for
//! first-run or Settings-triggered import.
//!
//! Design summary:
//! - Captures only the documented environment variables (no auth files,
//!   no keychain, no command execution): `HOME`, `XDG_CONFIG_HOME`,
//!   `OPENCODE_CONFIG` / `OPENCODE_CONFIG_DIR` / `OPENCODE_CONFIG_CONTENT`,
//!   `CODEX_HOME`, `CLAUDE_CONFIG_DIR`.
//! - Discovers OpenCode global config + ancestor project configs, plus
//!   Codex and Claude Code settings and agent markdown directories.
//! - Delegates parsing/normalizing to `opencode_rk_foundation::config_import`
//!   and provider loading to `opencode_rk_providers::imported_config`.
//! - Produces `SourceMeta` entries pointing at exact source files (never
//!   ancestry roots, the whole HOME, or the destination).
//! - Invalid/unreadable sources are recorded as bounded warnings on the plan;
//!   an empty plan (no meaningful sources) yields `None`, so no empty/fake
//!   import offer is ever presented.
//! - `ImportPlan.value` + source provenance is handed to
//!   `ImportDialog::startup` with an explicit `state_path` inside the data dir
//!   and the canonical own-config destination reader.
//!
//! This module is feature-gated `native` and registered in `main.rs`.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use opencode_rk_foundation::config_import::{
    self, ConfigKind, DiscoveredDocument, ImportPlan, NormalizedSource,
    OpenCodeRoots, CodexRoots, ClaudeRoots,
};
use opencode_rk_providers::imported_config;

use crate::native_config_import::SourceMeta;
use crate::native_import_dialog::ImportDialog;

/// Bounded warning about a source that could not be fully imported.
#[derive(Clone, Debug)]
pub struct ImportWarning {
    /// Human-readable source label (path or env var name).
    pub source: String,
    /// Bounded diagnostic text (never raw secret values).
    pub message: String,
}

/// Result of discovery: a plan ready for preview/apply, or `None` when no
/// meaningful sources were found (and thus no import offer should appear).
pub struct ImportOffer {
    /// The merged import plan (secrets already redacted inside `value`).
    pub plan: ImportPlan,
    /// Bounded warnings for unreadable/malformed/unsupported sources.
    pub warnings: Vec<ImportWarning>,
    /// Source provenance for the destination writer (native_config_import).
    pub sources: Vec<SourceMeta>,
}

/// Environment snapshot captured only for the documented keys.
/// Constructed by [`EnvSnapshot::capture`] or built explicitly in tests.
#[derive(Clone, Debug)]
pub struct EnvSnapshot {
    pub home: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub opencode_config: Option<PathBuf>,
    pub opencode_config_dir: Option<PathBuf>,
    pub opencode_config_content: Option<String>,
    pub codex_home: Option<PathBuf>,
    pub claude_config_dir: Option<PathBuf>,
}

impl EnvSnapshot {
    /// Capture only the documented environment variables at the call instant.
    /// No other env vars are read. No auth files or keychains are touched.
    #[allow(dead_code)]
    pub fn capture() -> Self {
        let get_path = |var: &str| std::env::var_os(var).map(PathBuf::from);
        Self {
            home: get_path("HOME"),
            xdg_config_home: get_path("XDG_CONFIG_HOME"),
            opencode_config: get_path("OPENCODE_CONFIG"),
            opencode_config_dir: get_path("OPENCODE_CONFIG_DIR"),
            opencode_config_content: std::env::var_os("OPENCODE_CONFIG_CONTENT")
                .map(|s| s.to_string_lossy().into_owned()),
            codex_home: get_path("CODEX_HOME"),
            claude_config_dir: get_path("CLAUDE_CONFIG_DIR"),
        }
    }

    /// Filter a root directory: must be absolute and non-empty.
    fn valid_root(path: &Option<PathBuf>) -> bool {
        path.as_ref()
            .map(|p| p.is_absolute() && !p.as_os_str().is_empty())
            .unwrap_or(false)
    }

    /// Resolve the OpenCode global config directory.
    /// First explicit `OPENCODE_CONFIG_DIR`, then `~/.config/opencode`.
    fn opencode_global_dir(&self) -> Option<PathBuf> {
        if Self::valid_root(&self.opencode_config_dir) {
            return self.opencode_config_dir.clone();
        }
        self.home.as_ref().map(|h| h.join(".config/opencode"))
    }

    /// Resolve the Codex home directory from `CODEX_HOME` or `~/.codex`.
    fn codex_home_dir(&self) -> Option<PathBuf> {
        if Self::valid_root(&self.codex_home) {
            return self.codex_home.clone();
        }
        self.home.as_ref().map(|h| h.join(".codex"))
    }

    /// Resolve the Claude Code config directory from `CLAUDE_CONFIG_DIR`
    /// or `~/.claude`.
    fn claude_config_dir(&self) -> Option<PathBuf> {
        if Self::valid_root(&self.claude_config_dir) {
            return self.claude_config_dir.clone();
        }
        self.home.as_ref().map(|h| h.join(".claude"))
    }
}

/// Maximum number of bounded warnings retained per offer.
const MAX_WARNINGS: usize = 32;

/// Discover, normalize and merge importable configuration from the actual
/// environment snapshot and cwd.
///
/// Returns `None` when no meaningful sources were found, so callers never
/// present an empty/fake import offer.
///
/// `state_path` and `dest` must be absolute, normalized paths inside the
/// caller's data directory; `dest` must not overlap any discovered source tree.
pub fn discover_import(
    cwd: &Path,
    env: &EnvSnapshot,
    dest: &Path,
) -> Result<Option<ImportOffer>, String> {
    let mut warnings: Vec<ImportWarning> = Vec::new();

    // --- Build discovery roots from the explicit env snapshot only ---
    let opencode_roots = OpenCodeRoots {
        cwd: cwd.to_path_buf(),
        global_dir: env.opencode_global_dir(),
        explicit_config: env.opencode_config.clone(),
        explicit_dir: env.opencode_config_dir.clone(),
        explicit_content: env.opencode_config_content.clone(),
        disable_project: false,
    };

    let codex_roots = CodexRoots {
        codex_home: env.codex_home_dir(),
        project_dir: Some(cwd.to_path_buf()),
    };

    let claude_roots = ClaudeRoots {
        home: env.home.clone(),
        project_dir: Some(cwd.to_path_buf()),
    };

    // --- OpenCode config documents ---
    let oc_entries = config_import::discover_opencode(&opencode_roots);
    let mut docs: Vec<DiscoveredDocument> = config_import::read_sources(&oc_entries);

    // --- OpenCode markdown agent/cmd dirs ---
    let oc_agent_dirs = config_import::discover_agent_dirs(&opencode_roots);
    docs.extend(config_import::read_markdown_dirs(
        ConfigKind::OpenCodeAgentMd,
        &oc_agent_dirs,
    ));

    // --- Codex config ---
    let codex_entries = config_import::discover_codex(&codex_roots);
    docs.extend(config_import::read_sources(&codex_entries));

    // --- Claude Code settings + agent docs ---
    // When CLAUDE_CONFIG_DIR is set, it may point to an arbitrary directory
    // name (not necessarily `.claude`). We construct explicit SourceEntry
    // objects for that directory so the path is correct regardless of name.
    // When only HOME is available, we use the foundation's discover_claude
    // which appends `.claude/settings.json` to `home`.
    let mut claude_entries = config_import::discover_claude(&claude_roots);
    if let Some(claude_dir) = env.claude_config_dir() {
        // Override: remove the auto-discovered ~/.claude/settings.json and
        // add the explicit CLAUDE_CONFIG_DIR path instead.
        let default_path = env.home.as_ref().map(|h| h.join(".claude/settings.json"));
        claude_entries.retain(|e| Some(&e.path) != default_path.as_ref());
        claude_entries.push(config_import::SourceEntry::new(
            ConfigKind::ClaudeJson,
            claude_dir.join("settings.json"),
        ));
    }
    docs.extend(config_import::read_sources(&claude_entries));

    let claude_agent_dirs = config_import::discover_claude_agent_dirs(&claude_roots);
    docs.extend(config_import::read_markdown_dirs(
        ConfigKind::ClaudeAgentMd,
        &claude_agent_dirs,
    ));

    // --- Normalize each document into a NormalizedSource ---
    let mut normalized: Vec<NormalizedSource> = Vec::new();
    for doc in &docs {
        if let Some(ref read_err) = doc.read_error {
            // Missing files are normal (not every source exists); only record
            // genuine read errors (permissions, oversize, etc.).
            if !is_not_found(read_err) {
                push_warning(&mut warnings, &doc.origin, read_err);
            }
            continue;
        }
        match config_import::normalize(doc.kind, &doc.text, &doc.origin) {
            Ok(source) => {
                for field in &source.unsupported {
                    push_warning(&mut warnings, &field.origin, &field.field);
                }
                let is_meaningful = !source.base.is_empty()
                    || !source.providers.is_empty()
                    || !source.models.is_empty()
                    || !source.agents.is_empty()
                    || !source.commands.is_empty();
                if is_meaningful {
                    normalized.push(source);
                }
            }
            Err(error) => {
                push_warning(&mut warnings, &doc.origin, &error.to_string());
            }
        }
    }

    if normalized.is_empty() && warnings.is_empty() {
        return Ok(None);
    }

    // --- Merge into a single plan (lowest precedence first) ---
    let plan: ImportPlan = match config_import::merge_plans(&normalized) {
        Ok(plan) => plan,
        Err(error) => {
            push_warning(&mut warnings, "merge", &error.to_string());
            return Ok(None);
        }
    };

    if plan.value.as_object().map_or(true, |m| m.is_empty()) {
        return Ok(None);
    }

    // --- Build SourceMeta with exact source file paths ---
    let sources = build_source_meta(&docs);

    // --- Check that dest does not overlap any source ---
    validate_dest_not_in_sources(dest, &sources)?;

    Ok(Some(ImportOffer {
        plan,
        warnings: trim_warnings(warnings),
        sources,
    }))
}

/// Build `SourceMeta` entries with exact source file paths from discovered
/// documents. The `path` points at the actual file (or inline marker), never
/// at an ancestry root or the whole HOME directory.
fn build_source_meta(docs: &[DiscoveredDocument]) -> Vec<SourceMeta> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut sources = Vec::new();
    for doc in docs {
        if doc.read_error.is_some() && is_not_found(&doc.read_error.clone().unwrap_or_default()) {
            continue;
        }
        let origin = doc.origin.trim();
        if origin.is_empty() || !seen.insert(origin.to_string()) {
            continue;
        }
        let path = if doc.origin == "OPENCODE_CONFIG_CONTENT" {
            PathBuf::from("<OPENCODE_CONFIG_CONTENT>")
        } else {
            PathBuf::from(&doc.origin)
        };
        sources.push(SourceMeta {
            label: doc.kind.label().to_string(),
            path,
        });
    }
    sources
}

/// True when the read error string indicates the file was absent (normal).
fn is_not_found(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("does not exist")
}

/// Append a bounded warning, suppressing duplicates.
fn push_warning(warnings: &mut Vec<ImportWarning>, source: &str, message: &str) {
    if warnings.len() >= MAX_WARNINGS {
        return;
    }
    let source_trim = source.trim();
    let message_trim = message.trim();
    if source_trim.is_empty() && message_trim.is_empty() {
        return;
    }
    if warnings
        .iter()
        .any(|w| w.source == source_trim && w.message == message_trim)
    {
        return;
    }
    warnings.push(ImportWarning {
        source: source_trim.to_string(),
        message: message_trim.to_string(),
    });
}

fn trim_warnings(mut warnings: Vec<ImportWarning>) -> Vec<ImportWarning> {
    warnings.truncate(MAX_WARNINGS);
    warnings
}

/// Reject if the destination is inside or co-located with any source tree.
///
/// - Exact file match: dest is the same file as a source file.
/// - Source tree overlap: dest is inside a source directory (lexical).
/// - Same-directory clash: dest shares its parent directory with a source
///   file (e.g., both in `~/.config/opencode/`), since writing there would
///   risk clobbering or shadowing import sources.
///
/// The symlink-safe check is also performed at apply time by
/// `native_config_import::apply_confirmed`.
fn validate_dest_not_in_sources(dest: &Path, sources: &[SourceMeta]) -> Result<(), String> {
    let dest_parent = dest.parent();
    for source in sources {
        let source_path = &source.path;
        if source_path == dest
            || dest.starts_with(source_path)
            || source_path.starts_with(dest)
        {
            return Err("destination overlaps a configuration source tree".to_string());
        }
        // Same-parent-directory clash detection: dest is a file that would
        // sit next to a source file in the same config directory.
        if let Some(dest_p) = dest_parent {
            if let Some(source_p) = source_path.parent() {
                if dest_p == source_p {
                    return Err(format!(
                        "destination sits in the same directory as a source file ({})",
                        source_p.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Convert a discovery result into a ready-to-show `ImportDialog`.
///
/// `data_dir` is the application data directory; the offer-state path is
/// `data_dir/import-offer.json` and the destination is the canonical
/// own config (`opencode.json` under the own config dir).
///
/// Returns `Ok(None)` when no offer should be shown (no sources or the state
/// marker already records a prior decision).
pub fn offer_dialog(
    offer: ImportOffer,
    data_dir: &Path,
) -> Result<Option<ImportDialog>, crate::native_config_import::ImportError> {
    let state_path = data_dir.join("import-offer.json");
    let dest = imported_config::own_config_path().ok_or_else(|| {
        crate::native_config_import::ImportError::UnsafePath(
            "cannot resolve own config destination",
        )
    })?;

    let ImportOffer {
        plan,
        sources,
        ..
    } = offer;

    ImportDialog::startup(&state_path, &dest, sources, plan.value).map(Some)
}

/// Build a grouped label set for the import dialog from an `ImportOffer`,
/// useful for previewing groups before showing the dialog.
///
/// Groups are ordered: OpenCode, then Codex, then Claude — never arbitrary
/// silent replacement. Conflicts and ordering are explained by the plan's
/// `provenance` and `conflicts`, not by reordering sources.
#[allow(dead_code)]
pub fn source_groups(offer: &ImportOffer) -> Vec<(String, Vec<String>)> {
    let opencode: Vec<String> = offer
        .sources
        .iter()
        .filter(|s| s.label.contains("opencode"))
        .map(|s| s.path.to_string_lossy().into_owned())
        .collect();
    let codex: Vec<String> = offer
        .sources
        .iter()
        .filter(|s| s.label.contains("codex"))
        .map(|s| s.path.to_string_lossy().into_owned())
        .collect();
    let claude: Vec<String> = offer
        .sources
        .iter()
        .filter(|s| s.label.contains("claude"))
        .map(|s| s.path.to_string_lossy().into_owned())
        .collect();
    let mut groups = Vec::new();
    if !opencode.is_empty() {
        groups.push(("OpenCode".to_string(), opencode));
    }
    if !codex.is_empty() {
        groups.push(("Codex".to_string(), codex));
    }
    if !claude.is_empty() {
        groups.push(("Claude Code".to_string(), claude));
    }
    groups
}
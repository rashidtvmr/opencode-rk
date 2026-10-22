//! C1/C3/C4: discovery and source-specific normalization for migrated
//! configuration. Pure-by-contract: no secret resolution, no process
//! execution, no environment reads, no mutation of source files.
//!
//! Research basis (comment cites, quoted key names only):
//! - OpenCode v2 (`https://opencode.ai/v2/docs/config`, `.../docs/agents`):
//!   JSON/JSONC, global `~/.config/opencode/opencode.json(c)`, project
//!   `opencode.json(c)` and `.opencode/opencode.json(c)` searched from the
//!   current directory to the filesystem root, direct files first
//!   (farthest-to-closest) then `.opencode` dirs, every `.opencode` layer
//!   overriding direct layers; `OPENCODE_CONFIG`/`..._DIR`/`..._CONTENT`
//!   environment inputs; `agents/<name>.md` frontmatter equals an `agents`
//!   entry and the markdown body is the prompt; legacy singular
//!   `agent`/`command`/`provider` keys still load alongside the V2 plurals.
//! - Codex (`https://developers.openai.com/codex/config-reference/`):
//!   `~/.codex/config.toml` user level, `.codex/config.toml` project scope
//!   (provider/auth/profile selection keys are ignored there), `model`,
//!   `model_provider`, `model_providers.<id>` (`name`, `base_url`,
//!   `env_key`, `wire_api`), `profiles`, `agents.<name>` (`description`,
//!   `config_file`).
//! - Claude Code (`https://code.claude.com/docs/en/settings` and
//!   `.../docs/en/sub-agents`): strict JSON settings `~/.claude/settings.json`
//!   < `.claude/settings.json` < `.claude/settings.local.json`, subagent
//!   files `.claude/agents/<name>.md` (and `~/.claude/agents`) with
//!   `name`/`description`/`tools`/`model` frontmatter and the body as prompt.
//!
//! Output is an RK-shaped `serde_json::Value` plus per-key provenance,
//! conflicting values and explicit unsupported-field notes. Raw credentials
//! are never carried into the plan (only `{env:VAR}` references survive as
//! `api_key_env`); `{file:...}` is never read; commands, hooks and shell
//! configuration are imported as inert definitions at most and never executed.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

mod adapters;
pub(crate) use adapters::{
    normalize_codex, normalize_claude, normalize_opencode, parse_json_doc, parse_markdown_frontmatter,
    parse_toml_doc,
};

/// Maximum bytes accepted for one configuration document.
pub const MAX_DOC_BYTES: usize = 256 * 1024;
/// Maximum markdown agent definition bytes (matches the runtime agent crate).
pub const MAX_AGENT_MD_BYTES: usize = 64 * 1024;
/// Maximum agent/command markdown files read per directory scan.
pub const MAX_MD_FILES_PER_DIR: usize = 64;
/// Maximum agent definitions accumulated per document.
pub const MAX_AGENTS_PER_DOC: usize = 128;
/// Maximum command definitions accumulated per document.
pub const MAX_COMMANDS_PER_DOC: usize = 128;
/// Maximum providers per document.
pub const MAX_PROVIDERS_PER_DOC: usize = 64;

/// Dotted path used for one merged top-level key.
pub const KEY_MODEL: &str = "model";
pub const KEY_PROVIDERS: &str = "providers";
pub const KEY_AGENTS: &str = "agents";
pub const KEY_COMMANDS: &str = "commands";

/// Where a configuration document came from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConfigKind {
    /// OpenCode `opencode.json(c)` (JSON or JSONC syntax).
    #[default]
    OpenCodeJson,
    /// Codex `config.toml`.
    CodexToml,
    /// Claude Code settings JSON (strict).
    ClaudeJson,
    /// OpenCode markdown agent definition.
    OpenCodeAgentMd,
    /// Claude Code markdown agent definition.
    ClaudeAgentMd,
}

impl ConfigKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenCodeJson => "opencode.json(c)",
            Self::CodexToml => "codex config.toml",
            Self::ClaudeJson => "claude settings.json",
            Self::OpenCodeAgentMd => "opencode agents/<name>.md",
            Self::ClaudeAgentMd => "claude agents/<name>.md",
        }
    }

    #[must_use]
    pub fn is_agent_markdown(self) -> bool {
        matches!(self, Self::OpenCodeAgentMd | Self::ClaudeAgentMd)
    }
}

/// Maximum top-level keys in a single configuration document object.
pub const MAX_TOP_LEVEL_KEYS: usize = 256;

/// Error returned by [`normalize`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportError {
    /// Document exceeded the size limit for its kind.
    TooLarge { what: String, got: usize, max: usize },
    /// Document could not be parsed into the expected structure.
    Malformed { origin: String, detail: String },
    /// A required source path component was missing.
    MissingPath { origin: String, detail: String },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { what, got, max } => {
                write!(f, "{what} exceeds the maximum of {max} bytes (got {got})")
            }
            Self::Malformed { origin, detail } => {
                write!(f, "{origin}: {detail}")
            }
            Self::MissingPath { origin, detail } => {
                write!(f, "{origin}: {detail}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Unsupported field note surfaced in the import plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnsupportedField {
    pub origin: String,
    pub field: String,
    pub reason: String,
}

/// One discovered configuration document, read in full, bytes unchanged.
#[derive(Clone)]
pub struct DiscoveredDocument {
    pub kind: ConfigKind,
    /// Human-sourced label (path or environment variable name).
    pub origin: String,
    pub text: String,
    /// Set when reading failed; normalization reports it as an error note.
    pub read_error: Option<String>,
}

impl fmt::Debug for DiscoveredDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscoveredDocument")
            .field("kind", &self.kind)
            .field("origin", &self.origin)
            .field("text", &format!("<redacted {} bytes>", self.text.len()))
            .field("read_error", &self.read_error)
            .finish()
    }
}

/// A caller-created source enumeration entry for [`read_sources`].
/// If `text` is `Some`, the caller supplies inline content and no file read
/// occurs; `read_error` may carry a pre-existing error instead.
#[derive(Clone)]
pub struct SourceEntry {
    pub kind: ConfigKind,
    pub path: PathBuf,
    pub origin: String,
    /// Inline content for `OPENCODE_CONFIG_CONTENT` sources instead of a file.
    pub text: Option<String>,
    /// Pre-recorded read error, or `None` when content is readable.
    pub read_error: Option<String>,
}

impl fmt::Debug for SourceEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceEntry")
            .field("kind", &self.kind)
            .field("path", &self.path)
            .field("origin", &self.origin)
            .field("text", &self.text.as_ref().map(|t| format!("<redacted {} bytes>", t.len())))
            .field("read_error", &self.read_error)
            .finish()
    }
}

impl SourceEntry {
    #[must_use]
    pub fn new(kind: ConfigKind, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            kind,
            origin: path.display().to_string(),
            path,
            text: None,
            read_error: None,
        }
    }
}

/// Per-source normalized output before cross-source merging.
/// Preserves source provenance: `origin` names the file/env source, `kind`
/// names the flavor (OpenCode/Codex/Claude). Raw credential literals are
/// never carried into the plan; only env references survive as
/// `api_key_env`.
#[derive(Clone, Debug, Default)]
pub struct NormalizedSource {
    pub origin: String,
    pub kind: ConfigKind,
    pub base: BTreeMap<String, Value>,
    pub providers: BTreeMap<String, Value>,
    pub models: BTreeMap<String, Value>,
    pub agents: BTreeMap<String, Value>,
    pub commands: BTreeMap<String, Value>,
    pub unsupported: Vec<UnsupportedField>,
}

/// Maximum number of sources accepted into a single [`ImportPlan`].
pub const MAX_PLAN_SOURCES: usize = 256;
/// Maximum total object keys and array items across all source plans.
pub const MAX_PLAN_KEYS: usize = 4096;

/// A leaf-level conflict between two or more sources during merging.
/// Only the final (winning) value is redacted; the losing origin is preserved
/// for diagnostics without leaking raw credentials.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImportConflict {
    pub field: String,
    /// Origins of the sources that contributed different values, lowest-first.
    pub origins: Vec<String>,
    /// Whether the conflicting value is a credential (redacted in `value`).
    pub credential: bool,
}

/// Merged cross-source import plan. `value` is the RK-shaped JSON ready for the
/// daemon config; `provenance` maps encoded metadata paths to winning origins;
/// `conflicts` lists unequal leaf metadata paths; `sources` preserves provenance.
#[derive(Clone, Debug, Serialize)]
pub struct ImportPlan {
    pub value: Value,
    /// Winning origin keyed by metadata paths. Segments join on `.`; within
    /// a segment `%` encodes as `%25` and `.` as `%2E`. Not runtime key paths.
    pub provenance: BTreeMap<String, String>,
    /// Conflicts where two sources provided unequal values at the same metadata path.
    pub conflicts: Vec<ImportConflict>,
    /// Per-source normalized data for UI preview.
    pub sources: Vec<PlanSource>,
    /// Aggregated unsupported-field notes from every source.
    pub unsupported: Vec<UnsupportedField>,
}

/// A source's contribution to [`ImportPlan`], redacted for safe display.
#[derive(Clone, Debug, Serialize)]
pub struct PlanSource {
    pub origin: String,
    pub kind: String,
    pub base: BTreeMap<String, Value>,
    pub providers: BTreeMap<String, Value>,
    pub models: BTreeMap<String, Value>,
    pub agents: BTreeMap<String, Value>,
    pub commands: BTreeMap<String, Value>,
}

fn reject_oversize(len: usize, max: usize, what: &str) -> Result<(), ImportError> {
    if len > max {
        return Err(ImportError::TooLarge {
            what: what.to_string(),
            got: len,
            max,
        });
    }
    Ok(())
}

/// Read caller-listed sources in order. Read failures are recorded on the
/// entry (`read_error`) rather than aborting the whole sweep, so one bad file
/// cannot hide the rest. Source bytes are never modified.
pub fn read_sources(entries: &[SourceEntry]) -> Vec<DiscoveredDocument> {
    entries
        .iter()
        .map(|entry| {
            if let Some(text) = &entry.text {
                DiscoveredDocument {
                    kind: entry.kind,
                    origin: entry.origin.clone(),
                    text: text.clone(),
                    read_error: entry.read_error.clone(),
                }
            } else {
                read_discovered(entry.kind, &entry.path, &entry.origin)
            }
        })
        .collect()
}

fn read_discovered(kind: ConfigKind, path: &Path, origin: &str) -> DiscoveredDocument {
    let max = if kind.is_agent_markdown() {
        MAX_AGENT_MD_BYTES
    } else {
        MAX_DOC_BYTES
    };
    match fs::metadata(path) {
        Ok(meta) => {
            if meta.len() as usize > max {
                return DiscoveredDocument {
                    kind,
                    origin: origin.to_string(),
                    text: String::new(),
                    read_error: Some(format!("exceeds the maximum size of {max} bytes")),
                };
            }
            match fs::read_to_string(path) {
                Ok(text) => DiscoveredDocument {
                    kind,
                    origin: origin.to_string(),
                    text,
                    read_error: None,
                },
                Err(error) => DiscoveredDocument {
                    kind,
                    origin: origin.to_string(),
                    text: String::new(),
                    read_error: Some(error.to_string()),
                },
            }
        }
        Err(error) => DiscoveredDocument {
            kind,
            origin: origin.to_string(),
            text: String::new(),
            read_error: Some(error.to_string()),
        },
    }
}

/// Read `*.md` agent definitions from a caller-supplied directory, nearest
/// files first (lexicographic), bounded. Never mutates the directory.
pub fn read_markdown_dirs(
    kind: ConfigKind,
    dirs: &[(PathBuf, String)],
) -> Vec<DiscoveredDocument> {
    let mut documents = Vec::new();
    for (dir, origin) in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue, // optional directory: absence is normal
        };
        let mut files: BTreeSet<PathBuf> = BTreeSet::new();
        let mut truncated = false;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if files.insert(path.clone()) && files.len() > MAX_MD_FILES_PER_DIR {
                files.remove(&path);
                truncated = true;
                break;
            }
        }
        if truncated {
            documents.push(DiscoveredDocument {
                kind,
                origin: format!("{origin} (directory truncated at {MAX_MD_FILES_PER_DIR} files)"),
                text: String::new(),
                read_error: Some(format!(
                    "directory has more than {MAX_MD_FILES_PER_DIR} markdown files; only the first bound were read"
                )),
            });
        }
        for path in &files {
            documents.push(read_discovered(
                kind,
                path,
                &format!("{}/{}", origin.trim_end_matches('/'), file_stem_text(path)),
            ));
        }
    }
    documents
}

fn file_stem_text(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unnamed>".to_string())
}

/// Normalize one document of `kind` into the per-source catalog output.
/// Never touches the filesystem; `text` is the whole document.
pub fn normalize(kind: ConfigKind, text: &str, origin: &str) -> Result<NormalizedSource, ImportError> {
    let max = if kind.is_agent_markdown() {
        MAX_AGENT_MD_BYTES
    } else {
        MAX_DOC_BYTES
    };
    reject_oversize(text.len(), max, origin)?;
    let mut source = NormalizedSource {
        origin: origin.to_string(),
        kind,
        base: BTreeMap::new(),
        providers: BTreeMap::new(),
        models: BTreeMap::new(),
        agents: BTreeMap::new(),
        commands: BTreeMap::new(),
        unsupported: Vec::new(),
    };
    let out = match kind {
        ConfigKind::OpenCodeJson => {
            let document = parse_json_doc(text)
                .map_err(|detail| ImportError::Malformed { origin: origin.to_string(), detail })?;
            enforce_document_counts(&document, origin)?;
            normalize_opencode(&document, origin)
        }
        ConfigKind::CodexToml => {
            let document = parse_toml_doc(text)
                .map_err(|detail| ImportError::Malformed { origin: origin.to_string(), detail })?;
            enforce_document_counts(&document, origin)?;
            normalize_codex(&document, origin)
        }
        ConfigKind::ClaudeJson => {
            let document = serde_json::from_str::<Value>(text).map_err(|error| {
                ImportError::Malformed {
                    origin: origin.to_string(),
                    detail: format!("strict JSON required for Claude settings: {error}"),
                }
            })?;
            enforce_document_counts(&document, origin)?;
            normalize_claude(&document, origin)
        }
        ConfigKind::OpenCodeAgentMd | ConfigKind::ClaudeAgentMd => {
            let (pairs, body) = parse_markdown_frontmatter(text).map_err(|detail| {
                ImportError::Malformed { origin: origin.to_string(), detail }
            })?;
            let mut local = adapters::SourceOutput::default();
            let flavor = if kind == ConfigKind::OpenCodeAgentMd {
                "opencode-agent-file"
            } else {
                "claude-agent-file"
            };
            let _stored = adapters::agent_markdown_entry(&pairs, &body, origin, flavor, &mut local);
            local
        }
    };
    source.base = out.base;
    source.providers = out.providers;
    source.models = out.models;
    source.agents = out.agents;
    source.commands = out.commands;
    source.unsupported = out.unsupported;
    if source.agents.len() > MAX_AGENTS_PER_DOC {
        return Err(ImportError::TooLarge {
            what: format!("{origin} agent entries"),
            got: source.agents.len(),
            max: MAX_AGENTS_PER_DOC,
        });
    }
    if source.commands.len() > MAX_COMMANDS_PER_DOC {
        return Err(ImportError::TooLarge {
            what: format!("{origin} command entries"),
            got: source.commands.len(),
            max: MAX_COMMANDS_PER_DOC,
        });
    }
    if source.providers.len() > MAX_PROVIDERS_PER_DOC {
        return Err(ImportError::TooLarge {
            what: format!("{origin} provider entries"),
            got: source.providers.len(),
            max: MAX_PROVIDERS_PER_DOC,
        });
    }
    Ok(source)
}

/// Reject a few hostile shapes early so every adapter sees bounded input.
fn enforce_document_counts(document: &Value, _origin: &str) -> Result<(), ImportError> {
    if let Some(map) = document.as_object() {
        if map.len() > MAX_TOP_LEVEL_KEYS {
            return Err(ImportError::TooLarge {
                what: "top-level keys".to_string(),
                got: map.len(),
                max: MAX_TOP_LEVEL_KEYS,
            });
        }
    }
    Ok(())
}

/// Maximum nesting depth accepted while counting and merging normalized values.
pub const MAX_PLAN_DEPTH: usize = 32;

/// Merge ordered normalized sources (lowest precedence first) into a single
/// [`ImportPlan`]. Sources are processed in array order; later sources win at
/// each leaf. Runtime maps remain nested objects; dotted paths are used only
/// for provenance and conflict metadata. Objects merge recursively by key,
/// while arrays and scalars replace wholesale. Raw credentials are never
/// included in `value` or conflict output.
pub fn merge_plans(sources: &[NormalizedSource]) -> Result<ImportPlan, ImportError> {
    if sources.len() > MAX_PLAN_SOURCES {
        return Err(ImportError::TooLarge {
            what: "plan sources".to_string(),
            got: sources.len(),
            max: MAX_PLAN_SOURCES,
        });
    }
    let mut value = Map::new();
    let mut provenance = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut plan_sources = Vec::with_capacity(sources.len());
    let mut all_unsupported = Vec::new();
    let mut key_count = 0usize;

    for source in sources {
        for map in [&source.base, &source.providers, &source.models, &source.agents, &source.commands] {
            for entry in map.values() {
                count_plan_nodes(entry, 0, &mut key_count)?;
            }
            key_count = key_count.saturating_add(map.len());
            if key_count > MAX_PLAN_KEYS {
                return Err(ImportError::TooLarge {
                    what: "plan keys".to_string(),
                    got: key_count,
                    max: MAX_PLAN_KEYS,
                });
            }
        }
        all_unsupported.extend_from_slice(&source.unsupported);
        let (providers, provider_notes) = canonicalize_providers(&source.providers, &source.origin);
        all_unsupported.extend(provider_notes);
        plan_sources.push(PlanSource {
            origin: source.origin.clone(),
            kind: source.kind.label().to_string(),
            base: redact_map(&source.base),
            providers: redact_catalogue_map(&providers),
            models: redact_catalogue_map(&source.models),
            agents: redact_catalogue_map(&source.agents),
            commands: redact_catalogue_map(&source.commands),
        });

        for (key, entry) in &source.base {
            let Some(safe_entry) = redact_field(key, entry) else {
                continue;
            };
            merge_value_at(
                &mut value,
                key,
                &safe_entry,
                &metadata_segment(key),
                is_secret_field(key) || is_credential_reference_field(key),
                &mut provenance,
                &mut conflicts,
                &source.origin,
            );
        }
        merge_catalogue(&mut value, "providers", &providers, &mut provenance, &mut conflicts, &source.origin);
        merge_catalogue(&mut value, "models", &source.models, &mut provenance, &mut conflicts, &source.origin);
        merge_catalogue(&mut value, "agents", &source.agents, &mut provenance, &mut conflicts, &source.origin);
        merge_catalogue(&mut value, "commands", &source.commands, &mut provenance, &mut conflicts, &source.origin);
    }

    Ok(ImportPlan {
        value: Value::Object(value),
        provenance,
        conflicts,
        sources: plan_sources,
        unsupported: all_unsupported,
    })
}

fn count_plan_nodes(value: &Value, depth: usize, count: &mut usize) -> Result<(), ImportError> {
    if *count > MAX_PLAN_KEYS {
        return Err(ImportError::TooLarge {
            what: "plan keys".to_string(),
            got: *count,
            max: MAX_PLAN_KEYS,
        });
    }
    if depth > MAX_PLAN_DEPTH {
        return Err(ImportError::TooLarge {
            what: "plan nesting depth".to_string(),
            got: depth,
            max: MAX_PLAN_DEPTH,
        });
    }
    match value {
        Value::Object(map) => {
            *count = count.saturating_add(map.len());
            for child in map.values() {
                count_plan_nodes(child, depth + 1, count)?;
            }
        }
        Value::Array(items) => {
            *count = count.saturating_add(items.len());
            for child in items {
                count_plan_nodes(child, depth + 1, count)?;
            }
        }
        _ => {}
    }
    if *count > MAX_PLAN_KEYS {
        return Err(ImportError::TooLarge {
            what: "plan keys".to_string(),
            got: *count,
            max: MAX_PLAN_KEYS,
        });
    }
    Ok(())
}

/// Canonical provider credentials at the public merge boundary. This also
/// protects caller-built `NormalizedSource` values from bypassing adapters.
fn canonicalize_providers(
    entries: &BTreeMap<String, Value>,
    origin: &str,
) -> (BTreeMap<String, Value>, Vec<UnsupportedField>) {
    let mut providers = BTreeMap::new();
    let mut unsupported = Vec::new();
    for (id, entry) in entries {
        if !adapters::provider_id_valid(id) {
            unsupported.push(UnsupportedField {
                origin: origin.to_string(),
                field: format!("providers.{id}"),
                reason: adapters::unsupported_provider_id_reason(id).to_string(),
            });
            continue;
        }
        providers.insert(id.clone(), canonicalize_provider_entry(entry, id, origin, &mut unsupported));
    }
    (providers, unsupported)
}

fn is_credential_reference_field(key: &str) -> bool {
    matches!(key, "api_key_env" | "apiKeyEnv")
}

fn canonicalize_provider_entry(
    entry: &Value,
    id: &str,
    origin: &str,
    unsupported: &mut Vec<UnsupportedField>,
) -> Value {
    let Some(object) = entry.as_object() else {
        return redact_sensitive(entry);
    };
    let mut canonical = Map::new();
    let mut api_key_env = None;
    let mut api_key_env_alias = None;
    for (key, value) in object {
        if key == "api_key" || key == "apiKey" {
            match value.as_str().map(adapters::secret_ref) {
                Some(adapters::SecretRef::EnvRef(name)) => {
                    api_key_env_alias.get_or_insert(name);
                }
                _ => {
                    unsupported.push(UnsupportedField {
                        origin: origin.to_string(),
                        field: format!("providers.{id}.{key}"),
                        reason: "credential must be a valid {env:VAR} reference; raw credentials are not imported".to_string(),
                    });
                }
            }
        } else if key == "api_key_env" || key == "apiKeyEnv" {
            match value.as_str().filter(|name| valid_env_name(name)) {
                Some(name) => {
                    api_key_env.get_or_insert_with(|| name.to_string());
                }
                None => {
                    unsupported.push(UnsupportedField {
                        origin: origin.to_string(),
                        field: format!("providers.{id}.{key}"),
                        reason: "environment variable name is not valid; configure the credential natively".to_string(),
                    });
                }
            }
        } else {
            if let Some(safe_value) = redact_field(key, value) {
                canonical.insert(key.clone(), safe_value);
            }
        }
    }
    if let Some(name) = api_key_env.or(api_key_env_alias) {
        canonical.insert("api_key_env".to_string(), Value::String(name));
    }
    Value::Object(canonical)
}

fn merge_catalogue(
    root: &mut Map<String, Value>,
    category: &str,
    entries: &BTreeMap<String, Value>,
    provenance: &mut BTreeMap<String, String>,
    conflicts: &mut Vec<ImportConflict>,
    origin: &str,
) {
    if entries.is_empty() {
        return;
    }
    let catalogue = root
        .entry(category.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !catalogue.is_object() {
        // The runtime catalogue has a fixed object shape even for malformed
        // caller-constructed normalized sources with a colliding base key.
        *catalogue = Value::Object(Map::new());
    }
    if let Some(catalogue) = catalogue.as_object_mut() {
        for (id, entry) in entries {
            let path = metadata_child(category, id);
            let safe_entry = redact_sensitive(entry);
            merge_value_at(catalogue, id, &safe_entry, &path, false, provenance, conflicts, origin);
        }
    }
}

fn merge_value_at(
    values: &mut Map<String, Value>,
    key: &str,
    new_value: &Value,
    path: &str,
    credential: bool,
    provenance: &mut BTreeMap<String, String>,
    conflicts: &mut Vec<ImportConflict>,
    origin: &str,
) {
    let Some(existing) = values.get_mut(key) else {
        let value = new_value.clone();
        record_value_provenance(&value, path, provenance, origin);
        values.insert(key.to_string(), value);
        return;
    };

    if let (Value::Object(existing), Value::Object(new_value)) = (&mut *existing, new_value) {
        provenance.insert(path.to_string(), origin.to_string());
        for (child_key, child_value) in new_value {
            let child_path = metadata_child(path, child_key);
            merge_value_at(
                existing,
                child_key,
                child_value,
                &child_path,
                credential || is_secret_field(child_key) || is_credential_reference_field(child_key),
                provenance,
                conflicts,
                origin,
            );
        }
        return;
    }

    if *existing != *new_value {
        let previous_origin = provenance.get(path).cloned().unwrap_or_default();
        let is_credential = credential || contains_secret_field(existing) || contains_secret_field(new_value);
        record_conflict(conflicts, path, &previous_origin, origin, is_credential);
    }
    clear_value_provenance(path, provenance);
    *existing = new_value.clone();
    record_value_provenance(existing, path, provenance, origin);
}

fn contains_secret_field(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, child)| {
            is_secret_field(key) || is_credential_reference_field(key) || contains_secret_field(child)
        }),
        Value::Array(items) => items.iter().any(contains_secret_field),
        _ => false,
    }
}

fn record_value_provenance(
    value: &Value,
    path: &str,
    provenance: &mut BTreeMap<String, String>,
    origin: &str,
) {
    provenance.insert(path.to_string(), origin.to_string());
    if let Value::Object(map) = value {
        for (key, child) in map {
            record_value_provenance(child, &metadata_child(path, key), provenance, origin);
        }
    }
}

fn clear_value_provenance(path: &str, provenance: &mut BTreeMap<String, String>) {
    let descendants = format!("{path}.");
    provenance.retain(|candidate, _| !candidate.starts_with(&descendants));
}

fn record_conflict(
    conflicts: &mut Vec<ImportConflict>,
    path: &str,
    previous: &str,
    origin: &str,
    credential: bool,
) {
    if previous.is_empty() || previous == origin {
        return;
    }
    let conflict = if let Some(conflict) = conflicts.iter_mut().find(|conflict| conflict.field == path) {
        conflict.credential |= credential;
        conflict
    } else {
        conflicts.push(ImportConflict {
            field: path.to_string(),
            origins: Vec::new(),
            credential,
        });
        conflicts.last_mut().expect("just inserted conflict")
    };
    for contributing_origin in [previous, origin] {
        if !contributing_origin.is_empty() && !conflict.origins.iter().any(|item| item == contributing_origin) {
            conflict.origins.push(contributing_origin.to_string());
        }
    }
}

/// Encode a metadata path segment; `%` is escaped first to make `%2E` injective.
/// Runtime map keys are never encoded. `.` joins path components only here.
fn metadata_child(path: &str, key: &str) -> String {
    let segment = metadata_segment(key);
    if path.is_empty() {
        segment
    } else {
        format!("{path}.{segment}")
    }
}

fn metadata_segment(key: &str) -> String {
    key.replace('%', "%25").replace('.', "%2E")
}

/// Redact credential fields recursively before values enter the runtime plan.
/// Valid env references are preserved, never resolved.
fn redact_sensitive(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, value) in map {
                if let Some(safe_value) = redact_field(key, value) {
                    out.insert(key.clone(), safe_value);
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive).collect()),
        _ => value.clone(),
    }
}

fn is_secret_field(key: &str) -> bool {
    if matches!(key, "api_key_env" | "apiKeyEnv") {
        return false;
    }
    let normalized = key.to_ascii_lowercase().replace('_', "").replace('-', "");
    matches!(
        normalized.as_str(),
        "apikey" | "token" | "accesstoken" | "refreshtoken" | "authtoken"
            | "bearertoken" | "secret" | "clientsecret" | "password" | "passwd"
            | "authorization" | "cookie" | "credential" | "credentials"
            | "privatekey" | "accesskey"
    ) || ["apikey", "accesskey", "privatekey", "clientsecret"]
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

fn redact_field(key: &str, value: &Value) -> Option<Value> {
    if matches!(key, "api_key_env" | "apiKeyEnv") {
        return value
            .as_str()
            .filter(|name| valid_env_name(name))
            .map(|name| Value::String(name.to_string()));
    }
    if is_secret_field(key) {
        return Some(match value {
            Value::String(secret) => redact_string_value(secret),
            _ => Value::String("[redacted]".to_string()),
        });
    }
    Some(redact_sensitive(value))
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Redact each normalized source map for the safe plan preview.
fn redact_map(map: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    map.iter()
        .filter_map(|(key, value)| redact_field(key, value).map(|safe| (key.clone(), safe)))
        .collect()
}

/// Redact all provider entries in a map (used for PlanSource display).
fn redact_catalogue_map(entries: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    entries.iter().map(|(key, value)| (key.clone(), redact_sensitive(value))).collect()
}

/// Apply secret_ref logic to a string value for safe display.
fn redact_string_value(s: &str) -> Value {
    let trimmed = s.trim();
    if let Some(name) = trimmed.strip_prefix("{env:").and_then(|name| name.strip_suffix('}')) {
        if valid_env_name(name) {
            return Value::String(s.to_string());
        }
    }
    Value::String("[redacted]".to_string())
}

/// Pure discovery: OpenCode candidate files, lowest precedence first.
///
/// Mirrors the upstream order: global files, `OPENCODE_CONFIG`, direct
/// project files from the farthest ancestor downwards, `.opencode`
/// directories in the same order, `OPENCODE_CONFIG_DIR`, and
/// `OPENCODE_CONFIG_CONTENT` last. Missing files stay in the list; readers
/// record their absence on the entry.
pub fn discover_opencode(inputs: &OpenCodeRoots) -> Vec<SourceEntry> {
    let mut entries = Vec::new();
    if let Some(global_dir) = &inputs.global_dir {
        for file in ["opencode.json", "opencode.jsonc"] {
            entries.push(SourceEntry::new(
                ConfigKind::OpenCodeJson,
                global_dir.join(file),
            ));
        }
    }
    if let Some(config) = &inputs.explicit_config {
        entries.push(SourceEntry::new(ConfigKind::OpenCodeJson, config));
    }
    if !inputs.disable_project {
        let mut ancestors = ancestor_chain(&inputs.cwd);
        ancestors.reverse(); // filesystem-root-most first
        for dir in &ancestors {
            for file in ["opencode.json", "opencode.jsonc"] {
                entries.push(SourceEntry::new(
                    ConfigKind::OpenCodeJson,
                    dir.join(file),
                ));
            }
        }
        for dir in &ancestors {
            for file in ["opencode.json", "opencode.jsonc"] {
                entries.push(SourceEntry::new(
                    ConfigKind::OpenCodeJson,
                    dir.join(".opencode").join(file),
                ));
            }
        }
    }
    if let Some(explicit_dir) = &inputs.explicit_dir {
        for file in ["opencode.json", "opencode.jsonc"] {
            entries.push(SourceEntry::new(
                ConfigKind::OpenCodeJson,
                explicit_dir.join(file),
            ));
        }
    }
    if let Some(content) = &inputs.explicit_content {
        entries.push(SourceEntry {
            kind: ConfigKind::OpenCodeJson,
            path: PathBuf::from("<OPENCODE_CONFIG_CONTENT>"),
            origin: "OPENCODE_CONFIG_CONTENT".to_string(),
            text: Some(content.clone()),
            read_error: None,
        });
    }
    entries
}

/// Agent-definition and command-template directories downstream of the
/// same discovery walk, per flavor. Home/global first, nearest project last.
pub fn discover_agent_dirs(inputs: &OpenCodeRoots) -> Vec<(PathBuf, String)> {
    let mut dirs = Vec::new();
    if let Some(global_dir) = &inputs.global_dir {
        dirs.push((global_dir.join("agents"), "global agents".to_string()));
        dirs.push((global_dir.join("command"), "global command legacy".to_string()));
        dirs.push((global_dir.join("commands"), "global commands".to_string()));
    }
    if !inputs.disable_project {
        let ancestors: Vec<String> = ancestor_chain(&inputs.cwd)
            .into_iter()
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect();
        for raw in &ancestors {
            for base in ["agent", "agents"] {
                dirs.push((
                    PathBuf::from(raw).join(".opencode").join(base),
                    format!("{} (.opencode/{base})", ancestors_label(raw)),
                ));
            }
        }
        for raw in &ancestors {
            for base in ["mode", "modes"] {
                dirs.push((
                    PathBuf::from(raw).join(".opencode").join(base),
                    format!("{} (.opencode/{base}, legacy primary agents)", ancestors_label(raw)),
                ));
            }
        }
        for raw in &ancestors {
            for base in ["command", "commands"] {
                dirs.push((
                    PathBuf::from(raw).join(".opencode").join(base),
                    format!("{} (.opencode/{base})", ancestors_label(raw)),
                ));
            }
        }
    }
    if let Some(explicit_dir) = &inputs.explicit_dir {
        for base in ["agent", "agents"] {
            dirs.push((explicit_dir.join(base), format!("{}/{{agent,agents}}", explicit_dir.display())));
        }
    }
    dirs
}

fn ancestors_label(raw: &str) -> String {
    if raw.is_empty() {
        "<filesystem root>".to_string()
    } else {
        raw.to_string()
    }
}

/// Pure Codex discovery: user config then project config (project entries
/// are marked untrusted so the caller can surface the trust requirement).
pub fn discover_codex(inputs: &CodexRoots) -> Vec<SourceEntry> {
    let mut entries = Vec::new();
    if let Some(home) = &inputs.codex_home {
        entries.push(SourceEntry::new(
            ConfigKind::CodexToml,
            home.join("config.toml"),
        ));
    }
    if let Some(project_dir) = &inputs.project_dir {
        let entry = SourceEntry::new(ConfigKind::CodexToml, project_dir.join(".codex/config.toml"));
        entries.push(entry);
    }
    entries
}

/// Pure Claude Code discovery: user, shared project and project-local
/// settings (project files come from the working directory only, matching
/// the documented behavior), plus agent directories (project walk to root,
/// then user).
pub fn discover_claude(inputs: &ClaudeRoots) -> Vec<SourceEntry> {
    let mut entries = Vec::new();
    if let Some(home) = &inputs.home {
        entries.push(SourceEntry::new(
            ConfigKind::ClaudeJson,
            home.join(".claude/settings.json"),
        ));
    }
    if let Some(project_dir) = &inputs.project_dir {
        entries.push(SourceEntry::new(
            ConfigKind::ClaudeJson,
            project_dir.join(".claude/settings.json"),
        ));
        entries.push(SourceEntry::new(
            ConfigKind::ClaudeJson,
            project_dir.join(".claude/settings.local.json"),
        ));
    }
    entries
}

/// Claude agent definition directories: every `.claude/agents` between the
/// working directory and the filesystem root (closest wins), then user.
pub fn discover_claude_agent_dirs(inputs: &ClaudeRoots) -> Vec<(PathBuf, String)> {
    let mut dirs = Vec::new();
    if let Some(project_dir) = &inputs.project_dir {
        for dir in ancestor_chain(project_dir) {
            dirs.push((
                dir.join(".claude/agents"),
                dir.to_string_lossy().into_owned(),
            ));
        }
    }
    if let Some(home) = &inputs.home {
        dirs.push((home.join(".claude/agents"), "user agents".to_string()));
    }
    dirs
}

fn ancestor_chain(start: &Path) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    let mut current: PathBuf = start.to_path_buf();
    loop {
        chain.push(current.clone());
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    chain
}

/// Caller-supplied discovery inputs for OpenCode sources. No direct
/// environment reads happen here; the caller passes the values.
pub struct OpenCodeRoots {
    pub cwd: PathBuf,
    pub global_dir: Option<PathBuf>,
    pub explicit_config: Option<PathBuf>,
    pub explicit_dir: Option<PathBuf>,
    pub explicit_content: Option<String>,
    pub disable_project: bool,
}

/// Caller-supplied discovery inputs for Codex sources.
pub struct CodexRoots {
    pub codex_home: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

/// Caller-supplied discovery inputs for Claude Code sources.
pub struct ClaudeRoots {
    pub home: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

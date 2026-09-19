//! Agent definition file loader for custom agent discovery.
//!
//! Scans a directory for `.md` (with optional YAML frontmatter) and `.json`
//! agent definition files. Enforces deterministic ordering, per-file size
//! bounds, total byte budget, and fail-closed symlink traversal.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Maximum number of agent definition files to load.
pub const MAX_FILES: usize = 64;

/// Maximum bytes per individual agent file.
pub const MAX_FILE_BYTES: usize = 64 * 1024;

/// Total byte budget across all loaded agent files.
pub const TOTAL_BUDGET: usize = 512 * 1024;

/// A single parsed agent definition.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDef {
    /// Agent name (unique identifier).
    pub name: String,
    /// Model to use (e.g. "gpt-4", "claude-3").
    pub model: Option<String>,
    /// The prompt / system body content.
    pub body: Vec<u8>,
    /// Sampling temperature (0.0–2.0).
    pub temperature: Option<f64>,
    /// Allowed tools (allowlist).
    pub tools: Vec<String>,
    /// Agent mode: "build" or "plan".
    pub mode: Option<String>,
    /// Absolute path to the source file.
    pub path: PathBuf,
}

/// A snapshot of all loaded agent definitions, in deterministic order.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentFileSnapshot {
    pub defs: Vec<AgentDef>,
}

/// Errors that can occur during agent file loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentFileError {
    /// Root path is not a directory.
    NotADirectory,
    /// Symlink escape detected during traversal.
    SymlinkEscape(PathBuf),
    /// Too many files found (exceeds MAX_FILES).
    TooManyFiles(usize),
    /// Total byte budget exceeded.
    BudgetExceeded { loaded: usize, file: PathBuf },
    /// Duplicate agent name across files.
    DuplicateName(String),
    /// IO error during traversal or read.
    Io(String),
    /// JSON parse error.
    Json(String),
}

impl From<io::Error> for AgentFileError {
    fn from(e: io::Error) -> Self {
        AgentFileError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AgentFileError {
    fn from(e: serde_json::Error) -> Self {
        AgentFileError::Json(e.to_string())
    }
}

/// Canonicalize a path, rejecting any symlink escape from the root.
fn canonicalize_reject_escape(root: &Path, path: &Path) -> Result<PathBuf, AgentFileError> {
    let canonical = path.canonicalize().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            AgentFileError::Io(format!("{}: not found", path.display()))
        } else {
            AgentFileError::Io(format!("{}: {}", path.display(), e))
        }
    })?;
    let root_canonical = root.canonicalize()?;
    if !canonical.starts_with(&root_canonical) {
        return Err(AgentFileError::SymlinkEscape(path.to_path_buf()));
    }
    Ok(canonical)
}

/// Parse optional YAML frontmatter delimited by `---`.
///
/// Returns a map of frontmatter key-value pairs and the body bytes.
/// If no frontmatter is present, returns an empty map and the full bytes.
/// Supports single-line `key: value` and multi-line YAML lists (`key:\n  - item`).
fn parse_frontmatter(raw: &[u8]) -> (Vec<(String, String)>, Vec<u8>) {
    let text = match std::str::from_utf8(raw) {
        Ok(t) => t,
        Err(_) => return (vec![], raw.to_vec()),
    };

    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return (vec![], raw.to_vec());
    }

    let after_open = &trimmed[3..];
    if let Some(end_idx) = after_open.find("\n---") {
        let block = &after_open[..end_idx];
        let body_start = 3 + end_idx + 4;
        let body_raw = if body_start < text.len() {
            &text[body_start..]
        } else {
            ""
        };
        let body = body_raw.strip_prefix('\n').unwrap_or(body_raw);

        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut current_key: Option<String> = None;
        let mut list_buf: Vec<String> = Vec::new();

        for line in block.lines() {
            let trimmed_line = line.trim();
            if trimmed_line.starts_with("- ") {
                // Continuation of a YAML list under current_key.
                if current_key.is_some() {
                    let item = trimmed_line[2..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !item.is_empty() {
                        list_buf.push(item);
                    }
                }
                continue;
            }

            // Flush any accumulated list from previous key.
            if let Some(key) = current_key.take() {
                if !list_buf.is_empty() {
                    let val = list_buf.join(", ");
                    list_buf = Vec::new();
                    pairs.push((key, val));
                    continue;
                }
            }

            if let Some((key, val)) = trimmed_line.split_once(':') {
                let key = key.trim().to_string();
                let val = val.trim();
                if val.is_empty() {
                    // Could be start of a multi-line list; hold key.
                    current_key = Some(key);
                } else {
                    let val = val.trim_matches('"').trim_matches('\'').to_string();
                    if !key.is_empty() {
                        pairs.push((key, val));
                    }
                }
            }
        }

        // Flush trailing list.
        if let Some(key) = current_key.take() {
            if !list_buf.is_empty() {
                let val = list_buf.join(", ");
                pairs.push((key, val));
            }
        }

        (pairs, body.as_bytes().to_vec())
    } else {
        (vec![], raw.to_vec())
    }
}

/// Parse a list-of-strings value from a frontmatter line.
/// Accepts `tools: [a, b]` or `tools: a, b` or `tools: "a"`.
fn parse_list_value(val: &str) -> Vec<String> {
    let val = val.trim();
    if val.starts_with('[') && val.ends_with(']') {
        let inner = &val[1..val.len() - 1];
        inner
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        val.split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Parse an agent definition from a markdown file (frontmatter + body).
fn parse_markdown(raw: &[u8], path: &PathBuf) -> Result<AgentDef, AgentFileError> {
    let (pairs, body) = parse_frontmatter(raw);
    let mut name = None;
    let mut model = None;
    let mut temperature = None;
    let mut mode = None;
    let mut tools = Vec::new();

    for (key, val) in &pairs {
        match key.as_str() {
            "name" => name = Some(val.clone()),
            "model" => model = Some(val.clone()),
            "temperature" => {
                temperature = val.parse::<f64>().ok();
            }
            "mode" => mode = Some(val.clone()),
            "tools" => tools = parse_list_value(val),
            _ => {}
        }
    }

    let name = name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    Ok(AgentDef {
        name,
        model,
        body,
        temperature,
        tools,
        mode,
        path: path.clone(),
    })
}

/// Parse an agent definition from a JSON file.
fn parse_json(raw: &[u8], path: &PathBuf) -> Result<AgentDef, AgentFileError> {
    let v: serde_json::Value = serde_json::from_slice(raw)?;

    let name = v
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    let model = v.get("model").and_then(|v| v.as_str()).map(String::from);

    // Accept "prompt" or "body" field.
    let body = v
        .get("prompt")
        .or_else(|| v.get("body"))
        .and_then(|v| v.as_str())
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();

    let temperature = v.get("temperature").and_then(|v| v.as_f64());

    let mode = v.get("mode").and_then(|v| v.as_str()).map(String::from);

    let tools = v
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(AgentDef {
        name,
        model,
        body,
        temperature,
        tools,
        mode,
        path: path.clone(),
    })
}

/// Load agent definition files from the given directory.
///
/// Scans for `.md` and `.json` files. Files are loaded in alphabetical order.
/// Markdown files may contain YAML frontmatter with agent metadata.
/// JSON files must contain at minimum a `name` and `prompt`/`body` field.
pub fn load_agent_files(root: &Path) -> Result<AgentFileSnapshot, AgentFileError> {
    if !root.is_dir() {
        return Err(AgentFileError::NotADirectory);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            match path.extension().and_then(|e| e.to_str()) {
                Some("md") | Some("json") => candidates.push(path),
                _ => {}
            }
        }
    }

    candidates.sort();

    if candidates.len() > MAX_FILES {
        return Err(AgentFileError::TooManyFiles(candidates.len()));
    }

    let mut defs = Vec::new();
    let mut total_bytes: usize = 0;
    let mut seen_names: HashSet<String> = HashSet::new();

    for path in &candidates {
        let canon = canonicalize_reject_escape(root, path)?;
        let raw = fs::read(&canon)?;
        if raw.len() > MAX_FILE_BYTES {
            continue;
        }
        if total_bytes + raw.len() > TOTAL_BUDGET {
            return Err(AgentFileError::BudgetExceeded {
                loaded: total_bytes,
                file: path.clone(),
            });
        }

        let def = match path.extension().and_then(|e| e.to_str()) {
            Some("md") => parse_markdown(&raw, path)?,
            Some("json") => parse_json(&raw, path)?,
            _ => continue,
        };

        if !seen_names.insert(def.name.clone()) {
            return Err(AgentFileError::DuplicateName(def.name.clone()));
        }

        total_bytes += raw.len();
        defs.push(def);
    }

    Ok(AgentFileSnapshot { defs })
}

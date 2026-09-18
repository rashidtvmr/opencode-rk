//! AGENTS.md / CLAUDE.md / rules-folder loader for system prompt injection.
//!
//! Scans the workspace root for `AGENTS.md`, `CLAUDE.md`, and `rules/*.md` files
//! containing optional YAML frontmatter globs. Enforces deterministic ordering,
//! byte-budget bounds, and fail-closed symlink traversal.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Maximum number of rule files to load.
pub const MAX_FILES: usize = 64;

/// Maximum bytes per individual rule file.
pub const MAX_FILE_BYTES: usize = 64 * 1024;

/// Total byte budget across all loaded rules.
pub const TOTAL_BUDGET: usize = 512 * 1024;

/// A single loaded rule entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleEntry {
    /// Absolute path to the source file.
    pub path: PathBuf,
    /// Optional glob pattern from frontmatter (e.g. `src/**`).
    pub glob: Option<String>,
    /// Raw bytes of the rule content (body after frontmatter, or full file if no frontmatter).
    pub bytes: Vec<u8>,
}

/// A snapshot of all loaded rules, in deterministic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesSnapshot {
    pub entries: Vec<RuleEntry>,
}

/// Errors that can occur during rule loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesError {
    /// Workspace root does not exist or is not a directory.
    NotADirectory,
    /// Symlink escape detected during traversal.
    SymlinkEscape(PathBuf),
    /// Too many files found (exceeds MAX_FILES).
    TooManyFiles(usize),
    /// Total byte budget exceeded.
    BudgetExceeded { loaded: usize, file: PathBuf },
    /// IO error during traversal or read.
    Io(String),
}

impl From<io::Error> for RulesError {
    fn from(e: io::Error) -> Self {
        RulesError::Io(e.to_string())
    }
}

/// Parse optional YAML frontmatter delimited by `---`.
///
/// Returns `(glob, body_bytes)`. If no frontmatter is present, returns
/// `(None, full_bytes)`.
fn parse_frontmatter(raw: &[u8]) -> (Option<String>, Vec<u8>) {
    let text = match std::str::from_utf8(raw) {
        Ok(t) => t,
        Err(_) => return (None, raw.to_vec()),
    };

    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return (None, raw.to_vec());
    }

    // Find closing `---`
    let after_open = &trimmed[3..];
    if let Some(end_idx) = after_open.find("\n---") {
        let block = &after_open[..end_idx];
        let body_start = 3 + end_idx + 4; // skip first ---\n...\n---
        // Find the newline after closing --- and skip it
        let body_raw = if body_start < text.len() {
            &text[body_start..]
        } else {
            ""
        };
        let body = body_raw.strip_prefix('\n').unwrap_or(body_raw);

        // Parse globs: from block, find line starting with "globs:"
        let mut glob = None;
        for line in block.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("globs:") {
                let val = rest.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    glob = Some(val.to_string());
                }
            }
        }

        (glob, body.as_bytes().to_vec())
    } else {
        // Unclosed frontmatter — treat entire content as body.
        (None, raw.to_vec())
    }
}

/// Canonicalize a path, rejecting any symlink escape from the workspace root.
fn canonicalize_reject_escape(root: &Path, path: &Path) -> Result<PathBuf, RulesError> {
    // Resolve symlinks manually to detect escape.
    let canonical = path.canonicalize().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            RulesError::Io(format!("{}: not found", path.display()))
        } else {
            RulesError::Io(format!("{}: {}", path.display(), e))
        }
    })?;
    let root_canonical = root.canonicalize()?;
    if !canonical.starts_with(&root_canonical) {
        return Err(RulesError::SymlinkEscape(path.to_path_buf()));
    }
    Ok(canonical)
}

/// Load rules from the workspace root directory.
///
/// Scans for `AGENTS.md`, `CLAUDE.md`, and `rules/*.md`. Files are loaded
/// alphabetically. Frontmatter with `globs:` field is parsed; remaining bytes
/// are the rule body.
pub fn load_rules(workspace_root: &Path) -> Result<RulesSnapshot, RulesError> {
    if !workspace_root.is_dir() {
        return Err(RulesError::NotADirectory);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Top-level files
    for name in &["AGENTS.md", "CLAUDE.md"] {
        let p = workspace_root.join(name);
        if p.is_file() {
            candidates.push(p);
        }
    }

    // rules/*.md
    let rules_dir = workspace_dir(workspace_root, "rules");
    if let Some(dir) = rules_dir {
        let mut md_files: Vec<PathBuf> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                    && e.path()
                        .extension()
                        .map(|ext| ext == "md")
                        .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect();
        candidates.append(&mut md_files);
    }

    // Deterministic: sort alphabetically by full path.
    candidates.sort();

    if candidates.len() > MAX_FILES {
        return Err(RulesError::TooManyFiles(candidates.len()));
    }

    let mut entries = Vec::new();
    let mut total_bytes: usize = 0;

    for path in &candidates {
        let canon = canonicalize_reject_escape(workspace_root, path)?;
        let raw = fs::read(&canon)?;
        if raw.len() > MAX_FILE_BYTES {
            // Skip oversized files — still count toward file limit but not budget.
            continue;
        }
        if total_bytes + raw.len() > TOTAL_BUDGET {
            return Err(RulesError::BudgetExceeded {
                loaded: total_bytes,
                file: path.clone(),
            });
        }

        let (glob, body) = parse_frontmatter(&raw);
        total_bytes += body.len();

        entries.push(RuleEntry {
            path: path.clone(),
            glob,
            bytes: body,
        });
    }

    Ok(RulesSnapshot { entries })
}

/// Helper: returns Some(path) if it's a directory, None otherwise.
fn workspace_dir(root: &Path, name: &str) -> Option<PathBuf> {
    let p = root.join(name);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static WORKSPACE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Create a temporary workspace and return its path.
    fn make_workspace() -> PathBuf {
        let id = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(format!(
            "/tmp/rules_loader_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create workspace");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    /// T01: AGENTS.md is discovered.
    #[test]
    fn t01_agents_md_discovered() {
        let ws = make_workspace();
        fs::write(ws.join("AGENTS.md"), "# Rules").unwrap();
        let snap = load_rules(&ws).unwrap();
        assert_eq!(snap.entries.len(), 1);
        assert!(snap.entries[0].path.ends_with("AGENTS.md"));
        assert_eq!(snap.entries[0].bytes, b"# Rules");
        assert!(snap.entries[0].glob.is_none());
        cleanup(&ws);
    }

    /// T02: Frontmatter glob is parsed.
    #[test]
    fn t02_frontmatter_glob_parsed() {
        let ws = make_workspace();
        fs::write(
            ws.join("AGENTS.md"),
            "---\nglobs: \"src/**\"\n---\nRule body here",
        )
        .unwrap();
        let snap = load_rules(&ws).unwrap();
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].glob.as_deref(), Some("src/**"));
        assert_eq!(snap.entries[0].bytes, b"Rule body here");
        cleanup(&ws);
    }

    /// T03: Bounds enforced — oversized file is skipped.
    #[test]
    fn t03_oversize_file_skipped() {
        let ws = make_workspace();
        // Write a file larger than MAX_FILE_BYTES
        let big_content = "x".repeat(MAX_FILE_BYTES + 1);
        fs::write(ws.join("AGENTS.md"), &big_content).unwrap();
        let snap = load_rules(&ws).unwrap();
        assert_eq!(snap.entries.len(), 0, "oversized file should be skipped");
        cleanup(&ws);
    }

    /// T04: Symlink escape is rejected.
    #[test]
    fn t04_symlink_escape_rejected() {
        let ws = make_workspace();
        let outside = PathBuf::from("/tmp/rules_escape_target");
        fs::write(&outside, "# escape").unwrap();
        let symlink = ws.join("AGENTS.md");
        std::os::unix::fs::symlink(&outside, &symlink).unwrap();
        let result = load_rules(&ws);
        assert!(
            matches!(result, Err(RulesError::SymlinkEscape(_))),
            "symlink escape must be rejected, got: {:?}",
            result
        );
        let _ = fs::remove_file(&outside);
        cleanup(&ws);
    }

    /// T05: Deterministic ordering — entries are alphabetically sorted by path.
    #[test]
    fn t05_deterministic_ordering() {
        let ws = make_workspace();
        // Create files in non-alphabetical order of creation
        fs::write(ws.join("CLAUDE.md"), "claude").unwrap();
        fs::create_dir_all(ws.join("rules")).unwrap();
        fs::write(ws.join("rules/b.md"), "b").unwrap();
        fs::write(ws.join("rules/a.md"), "a").unwrap();
        fs::write(ws.join("AGENTS.md"), "agents").unwrap();

        let snap = load_rules(&ws).unwrap();
        let names: Vec<&str> = snap
            .entries
            .iter()
            .map(|e| {
                e.path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            })
            .collect();
        // AGENTS.md < CLAUDE.md < a.md < b.md
        assert_eq!(names, vec!["AGENTS.md", "CLAUDE.md", "a.md", "b.md"]);
        cleanup(&ws);
    }

    /// T06: Empty workspace produces empty snapshot.
    #[test]
    fn t06_empty_workspace_empty_snapshot() {
        let ws = make_workspace();
        let snap = load_rules(&ws).unwrap();
        assert_eq!(snap.entries.len(), 0);
        cleanup(&ws);
    }
}

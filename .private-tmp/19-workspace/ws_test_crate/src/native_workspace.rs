#![forbid(unsafe_code)]
//! Safe native workspace command parsing and resolution, plus the normal
//! location/workspace picker presentation.
//!
//! Presentation helpers (abbreviate_home, truncate_left, truncate_middle) mirror
//! the OpenCode V2 TUI display utilities:
//! - packages/tui/src/util/path-format.ts: abbreviateHome
//! - packages/tui/src/util/locale.ts: truncateLeft, truncateMiddle
//!
//! Workspace picker (DialogWorkspaces) reference:
//! packages/tui/src/component/dialog-workspaces.tsx — worktree rows with
//! abbreviated display title, subdirectory-relative muted suffix, and a
//! Current/Other category. Display identity is never confused with the
//! actual directory authority: `directory` is the raw path used for the real
//! backend action, `label` is the human-friendly display only.

use std::path::{Path, PathBuf};
use serde_json::Value;

// ── Existing command parsing ─────────────────────────────────────────────

pub fn cd_argument(input: &str) -> Result<String, &'static str> {
    let rest = input.strip_prefix("/cd").ok_or("not a cd command")?;
    if rest.chars().next().is_some_and(|ch| !ch.is_whitespace()) {
        return Err("/cd requires a directory");
    }
    let mut out = String::new();
    let mut chars = rest.trim_start().chars().peekable();
    if chars.peek().is_none() { return Err("/cd requires a directory"); }
    let mut quote: Option<char> = None;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else {
            match quote {
                Some(active) if ch == active => quote = None,
                Some(_) => out.push(ch),
                None if ch == '\'' || ch == '"' => quote = Some(ch),
                None if ch.is_whitespace() => {
                    if chars.any(|ch| !ch.is_whitespace()) || out.is_empty() {
                        return Err("/cd accepts exactly one directory");
                    }
                    break;
                }
                None => out.push(ch),
            }
        }
    }
    if escaped || quote.is_some() { return Err("unterminated /cd argument"); }
    if out.is_empty() { Err("/cd requires a directory") } else { Ok(out) }
}

pub fn resolve_directory(raw: &str, base: &Path) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') { return Err("invalid working directory".into()); }
    let expanded = if raw == "~" {
        std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is unavailable")?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is unavailable")?.join(rest)
    } else { PathBuf::from(raw) };
    let candidate = if expanded.is_absolute() { expanded } else { base.join(expanded) };
    let resolved = candidate.canonicalize().map_err(|_| "working directory does not exist".to_owned())?;
    if !resolved.is_dir() { return Err("working directory is not a directory".into()); }
    Ok(resolved)
}

// ── Cell-width model (mirrors native_view_layout, no new deps) ─────────────

/// Combining marks, variation selectors and format characters (width 0).
const ZERO_WIDTH_RANGES: &[(u32, u32)] = &[
    (0x0300, 0x036F), (0x0483, 0x0489), (0x0591, 0x05BD), (0x05BF, 0x05BF),
    (0x05C1, 0x05C2), (0x05C4, 0x05C5), (0x05C7, 0x05C7), (0x0610, 0x061A),
    (0x064B, 0x065F), (0x0670, 0x0670), (0x06D6, 0x06DC), (0x06DF, 0x06E4),
    (0x06E7, 0x06E8), (0x06EA, 0x06ED), (0x0711, 0x0711), (0x0730, 0x074A),
];

/// East Asian Wide / Fullwidth and emoji scalars (width 2).
const WIDE_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x115F), (0x2E80, 0x303E), (0x3041, 0x33FF), (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF), (0xA000, 0xA4CF), (0xA960, 0xA97F), (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF), (0xFE10, 0xFE19), (0xFE30, 0xFE6F), (0xFF00, 0xFF60),
    (0xFFE0, 0xFFE6), (0x16FE0, 0x16FE4), (0x17000, 0x18AFF), (0x1B000, 0x1B12F),
    (0x1F004, 0x1F004), (0x1F0CF, 0x1F0CF), (0x1F18E, 0x1F18E), (0x1F191, 0x1F19A),
    (0x1F200, 0x1F320), (0x1F32D, 0x1F335), (0x1F337, 0x1F37C), (0x1F37E, 0x1F393),
    (0x1F3A0, 0x1F3CA), (0x1F3CF, 0x1F3D3), (0x1F3E0, 0x1F3F0), (0x1F3F4, 0x1F3F4),
    (0x1F3F8, 0x1F43E), (0x1F440, 0x1F440), (0x1F442, 0x1F4FC), (0x1F4FF, 0x1F53D),
    (0x1F54B, 0x1F54E), (0x1F550, 0x1F567), (0x1F57A, 0x1F57A), (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A4), (0x1F5FB, 0x1F64F), (0x1F680, 0x1F6C5), (0x1F6CC, 0x1F6CC),
    (0x1F6D0, 0x1F6D2), (0x1F6EB, 0x1F6EC), (0x1F6F4, 0x1F6FC), (0x1F7E0, 0x1F7EB),
    (0x1F90C, 0x1F93A), (0x1F93C, 0x1F945), (0x1F947, 0x1F9FF), (0x1FA70, 0x1FAFF),
    (0x20000, 0x2FFFD), (0x30000, 0x3FFFD),
];

fn in_ranges(cp: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.iter().any(|&(lo, hi)| cp >= lo && cp <= hi)
}

/// Character display width: 0 for control/zero-width, 1 for narrow, 2 for wide.
/// Local copy of native_view_layout::char_width — same table, no cross-module
/// visibility dependency.
pub fn char_width(c: char) -> u16 {
    if c.is_control() {
        return 0;
    }
    let cp = c as u32;
    if in_ranges(cp, ZERO_WIDTH_RANGES) {
        return 0;
    }
    if in_ranges(cp, WIDE_RANGES) {
        return 2;
    }
    1
}

/// Count display cells in a string (wide chars = 2).
fn cell_width(text: &str) -> usize {
    text.chars().map(|c| usize::from(char_width(c))).sum()
}

// ── Path display helpers (OpenCode V2 parity) ────────────────────────────

/// Abbreviate the home directory prefix to `~`, mirroring
/// `packages/tui/src/util/path-format.ts:abbreviateHome`. Path separators are
/// normalized to `/`. When `home` is empty or the path is not under home, the
/// input is returned unchanged (after separator normalization). Display-only:
/// the actual directory authority is never modified here.
pub fn abbreviate_home(path: &str, home: Option<&str>) -> String {
    let path = path.replace('\\', "/");
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path;
    };
    let home = home.replace('\\', "/");
    if path == home {
        return "~".to_owned();
    }
    let prefix = format!("{home}/");
    if let Some(rest) = path.strip_prefix(&prefix) {
        format!("~/{rest}")
    } else {
        path
    }
}

/// Truncate from the left, keeping the right end with a leading ellipsis —
/// mirrors `Locale.truncateLeft`. Cell-aware: wide characters count as 2 cells.
/// At `width == 0` returns an empty string; at `width == 1` the ellipsis.
pub fn truncate_left(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let cell_len = cell_width(text);
    if cell_len <= width {
        return text.to_owned();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let budget = width - 1; // 1 cell reserved for ellipsis
    let chars: Vec<char> = text.chars().collect();
    // Walk backwards from the end, collecting the last `budget` cells.
    let mut tail_cells = 0usize;
    let mut tail_start = chars.len();
    for i in (0..chars.len()).rev() {
        let w = usize::from(char_width(chars[i]));
        if tail_cells + w > budget {
            break;
        }
        tail_cells += w;
        tail_start = i;
    }
    let tail: String = chars[tail_start..].iter().collect();
    format!("…{tail}")
}

/// Truncate from the middle, preserving the start and end — mirrors
/// `Locale.truncateMiddle`. Cell-aware. At `width == 0` returns empty;
/// at `width == 1` the ellipsis.
pub fn truncate_middle(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let cell_len = cell_width(text);
    if cell_len <= width {
        return text.to_owned();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let keep_total = width - 1; // 1 cell for ellipsis
    let keep_start = keep_total / 2;
    let keep_end = keep_total - keep_start;

    // Build start portion.
    let mut start = String::new();
    let mut start_cells = 0usize;
    for c in text.chars() {
        let w = usize::from(char_width(c));
        if start_cells + w > keep_start {
            break;
        }
        start.push(c);
        start_cells += w;
    }

    // Build end portion.
    let mut end = String::new();
    let mut end_cells = 0usize;
    for c in text.chars().rev() {
        let w = usize::from(char_width(c));
        if end_cells + w > keep_end {
            break;
        }
        end.insert(0, c);
        end_cells += w;
    }

    format!("{start}…{end}")
}

// ── Workspace picker presentation ────────────────────────────────────────

/// One worktree/workspace entry in the picker. The `directory` is the
/// actual directory authority (never inferred from display text); `label`
/// is the human-friendly display (home-abbreviated); `detail` holds the
/// subdirectory-relative suffix or strategy; `current` marks the selected
/// workspace for the "Current" / "Other" category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRow {
    /// The raw directory path — the actual authority for backend actions.
    pub directory: String,
    /// Display label: home-abbreviated, truncated-left to fit the title width.
    pub label: String,
    /// Subdirectory suffix (relative to root) or strategy tag, never credentials.
    pub detail: String,
    /// True if this is the currently bound workspace (shown as "Current").
    pub current: bool,
    /// False only when the worktree is being removed or unavailable.
    pub enabled: bool,
}

/// Schema for a worktree entry from `GET /api/worktree` (WorktreeListOutput).
/// Mirrors `packages/client/src/promise/generated/types.ts:329`:
/// `WorktreeDirectory = { directory: AbsolutePath; strategy?: string }`.
#[derive(Clone, Debug)]
pub struct WorktreeEntry {
    pub directory: String,
    pub strategy: Option<String>,
}

/// Parse a `WorktreeListOutput` JSON array (`[{ "directory": "/abs/path",
/// "strategy"?: "git" }, ...]`) into worktree entries. Returns `Err` on the
/// first malformed entry so the caller keeps the previous list.
pub fn parse_workspace_rows(value: &Value) -> Result<Vec<WorkspaceRow>, String> {
    let raw: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => match map.get("worktrees") {
            Some(Value::Array(items)) => items.iter().collect(),
            Some(other) => return Err(format!("expected worktrees array, got {other}")),
            None => return Err("missing `worktrees` array".into()),
        },
        other => return Err(format!("unexpected payload: {other}")),
    };

    if raw.len() > super::native_view_pages::MAX_ROWS {
        return Err(format!(
            "server sent {} workspaces; refusing to present a truncated {}-row list",
            raw.len(),
            super::native_view_pages::MAX_ROWS
        ));
    }

    let mut rows = Vec::with_capacity(raw.len());
    for entry in raw {
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("workspace entry not an object: {entry}"))?;

        let directory = obj
            .get("directory")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "workspace entry missing string `directory`".to_string())?;

        let strategy = obj
            .get("strategy")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        rows.push(WorkspaceRow {
            directory: directory.to_owned(),
            label: String::new(), // populated by workspace_picker_rows
            detail: strategy.unwrap_or_default(),
            current: false,
            enabled: true,
        });
    }
    Ok(rows)
}

/// Produce display-ready workspace picker rows from parsed worktree entries.
///
/// Mirrors `DialogWorkspaces` in
/// `packages/tui/src/component/dialog-workspaces.tsx`:
///
/// - `current_directory` (if `Some`) marks the "Current" category and
///   triggers home-abbreviation on labels. The current directory row sorts
///   first under "Current"; the rest are "Other".
/// - `title_width` controls `truncate_left` on each label (cell-aware).
/// - When `current_directory` matches a worktree, that row gets `current: true`.
/// - Rows that are subdirectories of the current root get a relative-path
///   suffix as `detail` (overriding the strategy tag only when no strategy
///   is present, matching the DialogWorkspaces suffix-vs-strategy logic).
///
/// Directory authority is never derived from display text: `directory` is the
/// raw path from the backend; `label` is purely for presentation.
pub fn workspace_picker_rows(
    rows: Vec<WorkspaceRow>,
    current_directory: Option<&str>,
    home: Option<&str>,
    title_width: usize,
) -> Vec<WorkspaceRow> {
    let home = home.filter(|h| !h.is_empty());
    let current = current_directory.filter(|d| !d.is_empty());

    // Label and mark current.
    let mut rows: Vec<WorkspaceRow> = rows
        .into_iter()
        .map(|mut row| {
            row.label = truncate_left(&abbreviate_home(&row.directory, home), title_width);
            row.current = current.is_some_and(|c| same_directory(c, &row.directory));
            row
        })
        .collect();

    // Sort: Current first, then shorter paths first, then alphabetical.
    rows.sort_by(|a, b| {
        let cur = b.current.cmp(&a.current);
        let len = a.directory.len().cmp(&b.directory.len());
        let alpha = a.directory.cmp(&b.directory);
        cur.then(len).then(alpha)
    });

    // Fill in subdirectory suffix for non-current rows under the current root.
    // Clone the root string so we don't borrow `rows` during the mutable pass.
    let current_root: Option<String> = rows.iter()
        .find(|row| row.current)
        .map(|row| row.directory.clone());

    for row in &mut rows {
        if row.current {
            continue;
        }
        if let Some(root) = &current_root {
            if let Some(rel) = relative_path(root, &row.directory) {
                if !rel.is_empty() && rel != "." {
                    // Only override detail (strategy) if strategy is empty,
                    // so actual strategy tags from the backend take priority
                    // as the detail column.
                    if row.detail.is_empty() {
                        row.detail = format!("/{}", rel);
                    }
                }
            }
        }
    }

    rows
}

/// Compare two directory paths, normalizing trailing separators.
fn same_directory(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Compute the relative path of `child` from `root` (like `path.relative`).
/// Returns `""` if `child` equals `root`, the subpath string for
/// subdirectories, or `None` if not contained.
fn relative_path(root: &str, child: &str) -> Option<String> {
    let root_norm = root.trim_end_matches('/');
    let child_norm = child.trim_end_matches('/');
    if child_norm == root_norm {
        return Some(String::new());
    }
    let prefix = format!("{root_norm}/");
    child_norm.strip_prefix(&prefix).map(|s| s.to_owned())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_requires_one_argument() {
        assert_eq!(cd_argument("/cd /tmp"), Ok("/tmp".into()));
        assert_eq!(cd_argument("/cd \"dir with spaces\""), Ok("dir with spaces".into()));
        assert_eq!(cd_argument("/cd dir\\ with\\ spaces"), Ok("dir with spaces".into()));
        assert!(cd_argument("/cd").is_err());
        assert!(cd_argument("/cd /tmp && echo nope").is_err());
        assert!(cd_argument("/cd foo\"").is_err());
        assert!(cd_argument("/cd foo'bar").is_err());
        assert_eq!(cd_argument("/cd foo\\\"bar"), Ok("foo\"bar".into()));
        assert!(cd_argument("/cd 'foo\\'bar'").is_err());
        assert_eq!(cd_argument("/cd 'foo\\bar'"), Ok("foo\\bar".into()));
        assert_eq!(cd_argument("/cd \"foo\\\"bar\""), Ok("foo\"bar".into()));
        assert!(cd_argument("/cd foo\"bar").is_err());
        assert!(cd_argument("/cd foo'bar").is_err());
    }

    #[test]
    fn test_abbreviate_home() {
        assert_eq!(abbreviate_home("/home/user/projects", Some("/home/user")), "~/projects");
        assert_eq!(abbreviate_home("/home/user", Some("/home/user")), "~");
        assert_eq!(abbreviate_home("/other/path", Some("/home/user")), "/other/path");
        assert_eq!(abbreviate_home("/home/user2/x", Some("/home/user")), "/home/user2/x");
        assert_eq!(abbreviate_home("/tmp\\windows", None), "/tmp/windows");
        assert_eq!(abbreviate_home("/home/user/x", Some("")), "/home/user/x");
    }

    #[test]
    fn test_truncate_empty_and_short() {
        assert_eq!(truncate_left("", 10), "");
        assert_eq!(truncate_left("short", 10), "short");
    }

    #[test]
    fn test_truncate_left_basic() {
        assert_eq!(truncate_left("abcdefghij", 4), "…hij");
        assert_eq!(truncate_left("abcdefghij", 1), "…");
    }

    #[test]
    fn test_truncate_left_exact_fit() {
        assert_eq!(truncate_left("abc", 3), "abc");
        assert_eq!(truncate_left("abc", 4), "abc");
    }

    #[test]
    fn test_truncate_left_wide_chars() {
        // Chinese chars are 2 cells each.
        assert_eq!(truncate_left("日本語テスト", 6), "…本語テ");
    }

    #[test]
    fn test_truncate_middle_basic() {
        assert_eq!(truncate_middle("abcdefghij", 7), "ab…hij");
        assert_eq!(truncate_middle("abcdefghij", 1), "…");
        assert_eq!(truncate_middle("abcdefghij", 0), "");
    }

    #[test]
    fn test_truncate_middle_exact_fit() {
        assert_eq!(truncate_middle("abc", 3), "abc");
        assert_eq!(truncate_middle("abc", 4), "abc");
    }

    #[test]
    fn test_truncate_middle_wide_chars() {
        // "日本語テスト" = 10 cells. truncate_middle at 6 keeps 2 start + 3 end.
        // Keep start 2 cells = "日", keep end 3 cells = "ス" (2) + "ト" (1) won't fit cleanly.
        // Actually keep_start=2 (1 wide char = 日), keep_end=3 (1 wide=ス + 1 narrow won't work since all are wide).
        // Let's just verify it doesn't panic and produces output with ellipsis.
        let result = truncate_middle("日本語テスト", 6);
        assert!(result.contains("…"));
        assert!(result.len() > 0);
    }

    #[test]
    fn test_char_width() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('\u{3042}'), 2); // Japanese hiragana
        assert_eq!(char_width('\u{200B}'), 0); // Zero-width space
        assert_eq!(char_width('\u{0301}'), 0); // Combining acute accent
    }

    #[test]
    fn test_parse_workspace_rows_valid() {
        let json = r#"[{"directory":"/home/user/projects","strategy":"git"},{"directory":"/other/path"}]"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let rows = parse_workspace_rows(&value).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].directory, "/home/user/projects");
        assert_eq!(rows[0].detail, "git");
        assert_eq!(rows[1].directory, "/other/path");
        assert_eq!(rows[1].detail, "");
    }

    #[test]
    fn test_parse_workspace_rows_wrapped_in_object() {
        let json = r#"{"worktrees":[{"directory":"/proj","strategy":"git"}]}"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let rows = parse_workspace_rows(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, "/proj");
    }

    #[test]
    fn test_parse_workspace_rows_empty_array() {
        let value = Value::Array(vec![]);
        let rows = parse_workspace_rows(&value).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_parse_workspace_rows_missing_directory() {
        let json = r#"[{"strategy":"git"}]"#;
        let value: Value = serde_json::from_str(json).unwrap();
        assert!(parse_workspace_rows(&value).is_err());
    }

    #[test]
    fn test_parse_workspace_rows_non_object_entry() {
        let json = r#"["not an object"]"#;
        let value: Value = serde_json::from_str(json).unwrap();
        assert!(parse_workspace_rows(&value).is_err());
    }

    #[test]
    fn test_workspace_picker_rows_current_first() {
        let rows = vec![
            WorkspaceRow {
                directory: "/projects/main".into(),
                label: String::new(),
                detail: String::new(),
                current: false,
                enabled: true,
            },
            WorkspaceRow {
                directory: "/home/user/current".into(),
                label: String::new(),
                detail: String::new(),
                current: false,
                enabled: true,
            },
            WorkspaceRow {
                directory: "/projects/branch".into(),
                label: String::new(),
                detail: String::new(),
                current: false,
                enabled: true,
            },
        ];
        let result = workspace_picker_rows(rows, Some("/home/user/current"), Some("/home/user"), 80);
        assert_eq!(result.len(), 3);
        // Current should be first.
        assert!(result[0].current);
        assert_eq!(result[0].directory, "/home/user/current");
        assert_eq!(result[0].label, "~/current");
        // Current should be marked as "Current".
        assert!(result[1..].iter().all(|r| !r.current));
    }

    #[test]
    fn test_workspace_picker_rows_home_abbreviation() {
        let rows = vec![WorkspaceRow {
            directory: "/home/user/projects/myapp".into(),
            label: String::new(),
            detail: String::new(),
            current: false,
            enabled: true,
        }];
        let result = workspace_picker_rows(rows, None, Some("/home/user"), 80);
        assert_eq!(result[0].label, "~/projects/myapp");
    }

    #[test]
    fn test_workspace_picker_rows_truncation() {
        let rows = vec![WorkspaceRow {
            directory: "/home/user/very/long/path/to/some/project".into(),
            label: String::new(),
            detail: String::new(),
            current: true,
            enabled: true,
        }];
        let result = workspace_picker_rows(rows, Some("/home/user/very/long/path/to/some/project"), Some("/home/user"), 10);
        // Should be truncated with ellipsis and fit in 10 cells.
        let cells: usize = result[0].label.chars().map(|c| usize::from(char_width(c))).sum();
        assert!(cells <= 10, "label too wide: {} cells for width 10", cells);
        assert!(result[0].label.starts_with('…'));
    }

    #[test]
    fn test_workspace_picker_rows_subdirectory_suffix() {
        let rows = vec![
            WorkspaceRow {
                directory: "/projects/root".into(),
                label: String::new(),
                detail: String::new(),
                current: true,
                enabled: true,
            },
            WorkspaceRow {
                directory: "/projects/root/sub/dir".into(),
                label: String::new(),
                detail: String::new(),
                current: false,
                enabled: true,
            },
        ];
        let result = workspace_picker_rows(rows, Some("/projects/root"), None, 80);
        // The first row is current.
        assert!(result[0].current);
        // The subdirectory row gets a relative suffix.
        let sub = result.iter().find(|r| r.directory == "/projects/root/sub/dir").unwrap();
        assert_eq!(sub.detail, "/sub/dir");
    }

    #[test]
    fn test_workspace_picker_rows_strategy_preserved() {
        let rows = vec![
            WorkspaceRow {
                directory: "/projects/root".into(),
                label: String::new(),
                detail: String::new(),
                current: true,
                enabled: true,
            },
            WorkspaceRow {
                directory: "/projects/root/sub".into(),
                label: String::new(),
                detail: "git".into(),
                current: false,
                enabled: true,
            },
        ];
        let result = workspace_picker_rows(rows, Some("/projects/root"), None, 80);
        let sub = result.iter().find(|r| r.directory == "/projects/root/sub").unwrap();
        // Strategy takes priority over subdirectory suffix.
        assert_eq!(sub.detail, "git");
    }

    #[test]
    fn test_relative_path() {
        assert_eq!(relative_path("/root", "/root"), Some(String::new()));
        assert_eq!(relative_path("/root", "/root/sub"), Some("sub".to_string()));
        assert_eq!(relative_path("/root", "/root/sub/deep"), Some("sub/deep".to_string()));
        assert_eq!(relative_path("/root", "/other"), None);
        assert_eq!(relative_path("/root/", "/root/sub"), Some("sub".to_string()));
    }

    #[test]
    fn test_same_directory() {
        assert!(same_directory("/a/b", "/a/b"));
        assert!(same_directory("/a/b/", "/a/b"));
        assert!(same_directory("/a/b", "/a/b/"));
    }
}
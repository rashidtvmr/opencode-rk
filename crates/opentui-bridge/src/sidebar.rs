#![forbid(unsafe_code)]
//! Sidebar panel state (mirrors `packages/tui/src/feature-plugins/sidebar/`, TS checkout a0d9b6c).
//!
//! TS panels: context.tsx:13-47 (tokens/percent/cost), lsp.tsx:7-47 (id+root rows,
//! connected=success else error dot, `lsp.tsx:30-37`), mcp.tsx:7-79 (name rows with
//! status badge via Switch `mcp.tsx:61-70`, dot color `mcp.tsx:20-27`, collapsed
//! summary `(on active, bad errors)` `mcp.tsx:38-43`), files.tsx:14-52 (Modified
//! Files diff list `+additions`/`-deletions`), todo.tsx:8-31 (TodoItem rows, hidden
//! when all completed `todo.tsx:12`), footer.tsx:9-80 (cwd/branch path + version).
//! Collapse/open + `length > 2` gating (`files.tsx:22`, `lsp.tsx:15`, `mcp.tsx:32`,
//! `todo.tsx:17`) is view logic, out of scope here.
//! Divergence note: checkout is a0d9b6c, not pinned 95daf90.

/// Fail-closed row bound (TS lists unbounded).
pub const MAX_ROWS: usize = 512;

/// Sidebar panels (one per `sidebar/*.tsx` plugin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Files,
    Context,
    Lsp,
    Mcp,
    Todo,
}

/// Modified-file row (`files.tsx:31-47`: path + +/- counts; dirty/diag summary).
/// Divergence: TS diff item is `{file, additions, deletions}` only
/// (`tui.ts:449-453`); `dirty`/`diagnostics` kept from prior bridge revision,
/// unmapped to TS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileRow {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub dirty: bool,
    pub diagnostics: u32,
}

/// MCP server row (`mcp.tsx:48-73`: name + status badge; enabled = connected).
/// `enabled` kept for back-compat; `status` refines it (None = legacy path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRow {
    pub name: String,
    pub enabled: bool,
    pub status: Option<McpStatus>,
    pub error: Option<String>,
}

/// MCP status union (`mcp.tsx:20-27` dot colors + `mcp.tsx:61-70` badge Switch;
/// `McpStatus*` object types `types.gen.ts:2380-2407`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpStatus {
    #[default]
    Connected,
    Disabled,
    Failed,
    NeedsAuth,
    NeedsClientRegistration,
}

/// Fail-closed MCP error bound (TS `error?: string` unbounded, `tui.ts:442`).
pub const MAX_MCP_ERROR: usize = 512;

impl McpRow {
    /// Bounded constructor; None when `error` exceeds [`MAX_MCP_ERROR`].
    #[must_use]
    pub fn new(name: &str, enabled: bool, status: Option<McpStatus>, error: Option<&str>) -> Option<Self> {
        let error = error.map(str::to_string);
        if let Some(e) = &error {
            if e.len() > MAX_MCP_ERROR {
                return None;
            }
        }
        Some(Self { name: name.to_string(), enabled, status, error })
    }

    /// Connected check (`mcp.tsx:11`: `status === "connected"` counts active;
    /// legacy rows without status fall back to `enabled`).
    #[must_use]
    pub fn active(&self) -> bool {
        match &self.status {
            Some(s) => *s == McpStatus::Connected,
            None => self.enabled,
        }
    }
}

/// Todo row (`todo.tsx:26` via TodoItem status/content; done = completed).
/// `done` kept for back-compat; `status` refines it (None = legacy path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoRow {
    pub text: String,
    pub done: bool,
    pub status: Option<TodoStatus>,
}

/// Todo status (`session-todo.ts:10`: pending, in_progress, completed, cancelled;
/// `todo-item.tsx:16-19`: completed=check, in_progress=bullet, else blank).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl TodoRow {
    /// Done check (`todo.tsx:12`: `status !== "completed"` hides panel;
    /// legacy rows without status fall back to `done`).
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done || matches!(self.status, Some(TodoStatus::Completed))
    }
}

/// LSP server row (`lsp.tsx:27-43`: id + root rows, connected=success dot else
/// error `lsp.tsx:30-37`; `TuiSidebarLspItem` picks id/root/status `tui.ts:445`).
/// Global `off` flag (`lsp.tsx:11`, config-gated empty message) folds into
/// [`LspStatus::Off`]; no list-level struct stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRow {
    pub id: String,
    pub root: String,
    pub status: LspStatus,
}

/// LSP status. TS wire status is connected|error (`types.gen.ts:2367-2372`);
/// Connecting/Off cover transitional and config-disabled display states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LspStatus {
    Connected,
    #[default]
    Connecting,
    Error,
    Off,
}

/// Fail-closed LSP bounds (TS plain strings, unbounded).
pub const MAX_LSP_ID: usize = 128;
pub const MAX_LSP_ROOT: usize = 1024;

impl LspRow {
    /// Bounded constructor; None when `id`/`root` exceed bounds.
    #[must_use]
    pub fn new(id: &str, root: &str, status: LspStatus) -> Option<Self> {
        if id.len() > MAX_LSP_ID || root.len() > MAX_LSP_ROOT {
            return None;
        }
        Some(Self { id: id.to_string(), root: root.to_string(), status })
    }

    /// Success-dot check (`lsp.tsx:33`: connected=success else error).
    #[must_use]
    pub fn healthy(&self) -> bool {
        self.status == LspStatus::Connected
    }
}

/// Context panel summary (`context.tsx:19-44`: summed tokens, optional model
/// context percent, session cost). Standalone (f64 cost is not Eq, so it stays
/// out of [`SidebarRow`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ContextUsage {
    pub tokens: u64,
    pub percent: Option<u8>,
    pub cost: f64,
}

impl ContextUsage {
    #[must_use]
    pub fn display(&self) -> String {
        format!("{} tokens, {}% used, ${:.2} spent", self.tokens, self.percent.unwrap_or(0), self.cost)
    }
}

/// Sidebar footer (`footer.tsx:19-30` dir + branch-gated suffix, `footer.tsx:76`
/// app version).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarFooter {
    pub dir: String,
    pub branch: Option<String>,
    pub version: Option<String>,
}

/// Fail-closed footer dir bound (TS plain string, unbounded).
pub const MAX_FOOTER_DIR: usize = 1024;

impl SidebarFooter {
    /// Bounded constructor; None when `dir` exceeds [`MAX_FOOTER_DIR`].
    #[must_use]
    pub fn new(dir: &str, branch: Option<&str>, version: Option<&str>) -> Option<Self> {
        if dir.len() > MAX_FOOTER_DIR {
            return None;
        }
        Some(Self {
            dir: dir.to_string(),
            branch: branch.map(str::to_string),
            version: version.map(str::to_string),
        })
    }
}

/// Heterogeneous sidebar row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
    File(FileRow),
    Lsp(LspRow),
    Mcp(McpRow),
    Todo(TodoRow),
}

/// Active panel + bounded rows + clamped cursor (selected row highlight).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarState {
    pub active: Panel,
    pub rows: Vec<SidebarRow>,
    pub selected: usize,
}

impl SidebarState {
    #[must_use]
    pub const fn new(active: Panel) -> Self {
        Self { active, rows: Vec::new(), selected: 0 }
    }

    /// Switch panel; resets cursor.
    pub fn set_active(&mut self, panel: Panel) {
        self.active = panel;
        self.rows.clear();
        self.selected = 0;
    }

    /// Push row; false when full (fail-closed).
    pub fn push_row(&mut self, row: SidebarRow) -> bool {
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(row);
        self.clamp();
        true
    }

    /// Move cursor down; clamps at last row.
    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(self.rows.len() - 1);
    }

    /// Move cursor up; clamps at 0.
    pub fn select_prev(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = self.selected.saturating_sub(1).min(self.rows.len() - 1);
    }

    #[must_use]
    pub fn selected_row(&self) -> Option<&SidebarRow> {
        self.rows.get(self.selected)
    }

    fn clamp(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.rows.len() - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(p: &str) -> SidebarRow {
        SidebarRow::File(FileRow { path: p.to_string(), additions: 0, deletions: 0, dirty: true, diagnostics: 0 })
    }

    #[test]
    fn empty_nav_stays_zero() {
        let mut s = SidebarState::new(Panel::Files);
        s.select_next();
        s.select_prev();
        assert_eq!(s.selected, 0);
        assert_eq!(s.selected_row(), None);
    }

    #[test]
    fn next_prev_clamp() {
        let mut s = SidebarState::new(Panel::Todo);
        s.push_row(SidebarRow::Todo(TodoRow { text: "a".into(), done: false, status: None }));
        s.push_row(SidebarRow::Todo(TodoRow { text: "b".into(), done: true, status: None }));
        s.select_next();
        assert_eq!(s.selected, 1);
        s.select_next();
        assert_eq!(s.selected, 1);
        s.select_prev();
        assert_eq!(s.selected, 0);
        s.select_prev();
        assert_eq!(s.selected, 0);
        assert!(matches!(s.selected_row(), Some(SidebarRow::Todo(_))));
    }

    #[test]
    fn bound_rejects_overfill() {
        let mut s = SidebarState::new(Panel::Mcp);
        for i in 0..MAX_ROWS {
            assert!(s.push_row(SidebarRow::Mcp(McpRow {
                name: i.to_string(),
                enabled: i % 2 == 0,
                status: None,
                error: None
            })));
        }
        assert_eq!(s.rows.len(), MAX_ROWS);
        assert!(!s.push_row(SidebarRow::Mcp(McpRow { name: "x".into(), enabled: true, status: None, error: None })));
    }

    #[test]
    fn set_active_resets() {
        let mut s = SidebarState::new(Panel::Files);
        s.push_row(file("a"));
        s.select_next();
        s.set_active(Panel::Lsp);
        assert_eq!(s.active, Panel::Lsp);
        assert!(s.rows.is_empty());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn row_shapes() {
        let f = FileRow {
            path: "x".into(),
            additions: 3,
            deletions: 1,
            dirty: false,
            diagnostics: 2
        };
        let m = McpRow { name: "srv".into(), enabled: true, status: Some(McpStatus::Connected), error: None };
        let t = TodoRow { text: "do".into(), done: false, status: Some(TodoStatus::Pending) };
        assert!(!f.dirty && f.diagnostics == 2);
        assert!(m.active() && !t.is_done());
        assert_eq!(Panel::default(), Panel::Files);
    }

    #[test]
    fn context_usage_display() {
        let c = ContextUsage { tokens: 1234, percent: Some(42), cost: 0.99 };
        assert_eq!(c.display(), "1234 tokens, 42% used, $0.99 spent");
        let missing = ContextUsage { tokens: 0, percent: None, cost: 0.0 };
        assert_eq!(missing.display(), "0 tokens, 0% used, $0.00 spent");
    }

    #[test]
    fn file_row_diff_counts() {
        let f = FileRow { path: "a".into(), additions: 5, deletions: 2, ..Default::default() };
        assert_eq!((f.additions, f.deletions), (5, 2));
    }

    #[test]
    fn lsp_bounds_rejected() {
        assert!(LspRow::new(&"i".repeat(MAX_LSP_ID + 1), "r", LspStatus::Connected).is_none());
        assert!(LspRow::new("i", &"r".repeat(MAX_LSP_ROOT + 1), LspStatus::Connected).is_none());
        let ok = LspRow::new("ts", "/repo", LspStatus::Connected).unwrap();
        assert!(ok.healthy());
        assert!(!LspRow::new("ts", "/repo", LspStatus::Error).unwrap().healthy());
    }

    #[test]
    fn mcp_status_active_and_error_bound() {
        let on = McpRow::new("s", false, Some(McpStatus::Connected), Some("boom")).unwrap();
        assert!(on.active());
        assert_eq!(on.error.as_deref(), Some("boom"));
        assert!(!McpRow::new("s", true, Some(McpStatus::Failed), None).unwrap().active());
        // Legacy rows without status fall back to enabled.
        assert!(McpRow::new("s", true, None, None).unwrap().active());
        assert!(McpRow::new("s", false, None, None).unwrap().error.is_none());
        assert!(McpRow::new("s", true, None, Some(&"e".repeat(MAX_MCP_ERROR + 1))).is_none());
    }

    #[test]
    fn todo_status_mapping() {
        let mk = |s: Option<TodoStatus>| TodoRow { text: "t".into(), done: false, status: s };
        assert!(!mk(Some(TodoStatus::InProgress)).is_done());
        assert!(!mk(Some(TodoStatus::Cancelled)).is_done());
        assert!(mk(Some(TodoStatus::Completed)).is_done());
        // Legacy done flag still honored.
        assert!(TodoRow { text: "t".into(), done: true, status: None }.is_done());
    }

    #[test]
    fn footer_bound_and_lsp_row_in_state() {
        assert!(SidebarFooter::new(&"d".repeat(MAX_FOOTER_DIR + 1), None, None).is_none());
        let f = SidebarFooter::new("/repo", Some("main"), Some("1.0")).unwrap();
        assert_eq!(f.branch.as_deref(), Some("main"));
        let mut s = SidebarState::new(Panel::Lsp);
        assert!(s.push_row(SidebarRow::Lsp(LspRow::new("ts", "/r", LspStatus::Off).unwrap())));
        assert!(matches!(s.selected_row(), Some(SidebarRow::Lsp(_))));
    }
}

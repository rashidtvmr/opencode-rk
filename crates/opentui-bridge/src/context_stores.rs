#![forbid(unsafe_code)]
//! Plain-state mirrors of TUI Solid contexts (no reactivity here; Solid owns that TS-side).
//!
//! TS checkout /home/rashid/projects/opencode @ a0d9b6c (NOT pinned 95daf90).
//! Evidence:
//! - permission.tsx:5 (PermissionMode auto|normal), :11-13 (auto-init from args.auto),
//!   :18-20 (set), :21-23 (toggle)
//! - clipboard.tsx:5-8 (ClipboardService optional read/write), :12-14 (injectable value)
//! - theme.tsx:32-35 (ThemeSource discover), :52-61 (discoverThemes dir scan),
//!   :209-220 (pin/free/apply), :293-298 (set)
//! - thinking.ts:4 (show|hide), :12-17 (bold-title split), :24-27 (show->hide->show)
//! - prompt.tsx:9-16 (PromptRef get/set holder)
//! - event.ts:12-20 (subscribe filters "sync"), :22-30 (on type filter)
//! - exit.tsx:3 (Exit=(reason?:unknown)=>void)
//! - project.tsx:22-36 (workspace store), :89-112 (current/set/list/get/sync)
//! - directory.ts:7-16 (directory memo: project path or cwd, abbreviateHome, :branch)
//!
//! Reuses crate::context_ui::{PermissionGate,PermissionState,ExitCode} and
//! crate::context_kv::{tildefy,Project}; nothing redefined here.
//! NOT wired in lib.rs (scope forbids touching it).
//!
//! Divergence: ClipboardService holds an injectable in-memory value only; no OS
//! clipboard calls (TS clipboard.tsx delegates to ../clipboard platform code).

use crate::context_kv::tildefy;
use crate::context_ui::{ExitCode, PermissionGate, PermissionState};

/// Permission store: gate mirrors permission.tsx mode; decision tracks last reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionStore {
    pub gate: PermissionGate,
    pub mode: PermissionState,
}

impl PermissionStore {
    /// TS permission.tsx:11-13: `mode: args.auto ? "auto" : "normal"`.
    #[must_use]
    pub fn new(auto: bool) -> Self {
        Self {
            gate: if auto {
                PermissionGate::Auto
            } else {
                PermissionGate::Normal
            },
            mode: PermissionState::Ask,
        }
    }
    /// TS permission.tsx:18-20.
    pub fn set(&mut self, gate: PermissionGate) {
        self.gate = gate;
    }
    /// TS permission.tsx:21-23.
    pub fn toggle(&mut self) {
        self.gate = match self.gate {
            PermissionGate::Auto => PermissionGate::Normal,
            PermissionGate::Normal => PermissionGate::Auto,
        };
    }
}

/// Injectable clipboard value (clipboard.tsx:12-14). Bounded 64 KiB, no OS calls.
pub const MAX_CLIPBOARD_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardService {
    pub read: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardError {
    TooLarge,
}

impl ClipboardService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn read(&self) -> Option<&str> {
        self.read.as_deref()
    }
    pub fn write(&mut self, s: &str) -> Result<(), ClipboardError> {
        if s.len() > MAX_CLIPBOARD_BYTES {
            return Err(ClipboardError::TooLarge);
        }
        self.read = Some(s.to_string());
        Ok(())
    }
}

/// Theme apply state: source name bounded 64 chars, pinned via pin/unpin.
pub const MAX_THEME_SOURCE_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeApply {
    pub source: String,
    pub pinned: bool,
}

impl ThemeApply {
    #[must_use]
    pub fn new(source: &str) -> Self {
        Self {
            source: truncate_chars(source, MAX_THEME_SOURCE_LEN),
            pinned: false,
        }
    }
    /// TS theme.tsx:209-213 pin.
    pub fn pin(&mut self) {
        self.pinned = true;
    }
    /// TS theme.tsx:216-220 free.
    pub fn unpin(&mut self) {
        self.pinned = false;
    }
    /// TS theme.tsx:52-61 discoverThemes: bounded passthrough of names.
    #[must_use]
    pub fn discover_names(names: &[&str]) -> Vec<String> {
        names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(|n| truncate_chars(n, MAX_THEME_SOURCE_LEN))
            .collect()
    }
}

/// Thinking toggle (thinking.ts:4,24-27 show->hide->show).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingMode {
    Show,
    Hide,
}

impl ThinkingMode {
    #[must_use]
    pub fn next(mode: Self) -> Self {
        match mode {
            Self::Show => Self::Hide,
            Self::Hide => Self::Show,
        }
    }
}

/// Bold-title split per thinking.ts:12-17. Returns the disclosure title when
/// thinking, else empty; strips a leading `**title**` block when present.
#[must_use]
pub fn reasoning_summary(thinking: bool, title: &str) -> String {
    if !thinking {
        return String::new();
    }
    let content = title.trim();
    if let Some(rest) = content.strip_prefix("**") {
        if let Some(end) = rest.find("**") {
            let (head, tail) = (&rest[..end], &rest[end + 2..]);
            if !head.contains('\n') && (tail.is_empty() || tail.starts_with("\r\n") || tail.starts_with('\n')) {
                return head.trim().to_string();
            }
        }
    }
    content.to_string()
}

/// Prompt ref holder (prompt.tsx:9-16). Bounded 16 KiB.
pub const MAX_PROMPT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptRef {
    pub current: Option<String>,
}

impl PromptRef {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn get(&self) -> Option<&str> {
        self.current.as_deref()
    }
    /// Overlong values are dropped fail-closed, previous kept.
    pub fn set(&mut self, value: Option<&str>) -> bool {
        match value {
            None => {
                self.current = None;
                true
            }
            Some(s) if s.len() > MAX_PROMPT_BYTES => false,
            Some(s) => {
                self.current = Some(s.to_string());
                true
            }
        }
    }
}

/// Exit hook (exit.tsx:3). Never calls process::exit; returns code for caller.
pub const MAX_EXIT_REASON_LEN: usize = 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExitHook {
    pub reason: Option<String>,
}

impl ExitHook {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_reason(&mut self, reason: Option<&str>) {
        self.reason = match reason {
            None => None,
            Some(r) if r.len() > MAX_EXIT_REASON_LEN => None,
            Some(r) => Some(r.to_string()),
        };
    }
    #[must_use]
    pub fn exit(&self) -> ExitCode {
        match &self.reason {
            None => ExitCode::Success,
            Some(_) => ExitCode::Failure,
        }
    }
}

/// Project store: workspace list/current (project.tsx:89-112). Bounded 128.
pub const MAX_PROJECTS: usize = 128;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectStore {
    pub projects: Vec<String>,
    pub current: Option<String>,
}

impl ProjectStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, id: &str) -> bool {
        let id = id.trim();
        if id.is_empty() || self.projects.iter().any(|p| p == id) {
            return false;
        }
        if self.projects.len() >= MAX_PROJECTS {
            return false;
        }
        self.projects.push(id.to_string());
        true
    }
    /// TS project.tsx:98-100 list.
    #[must_use]
    pub fn list(&self) -> &[String] {
        &self.projects
    }
    /// TS project.tsx:101-103 get.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&str> {
        self.projects.iter().find(|p| p.as_str() == id).map(String::as_str)
    }
    /// TS project.tsx:93-97 set; unknown ids rejected fail-closed.
    pub fn set(&mut self, id: Option<&str>) -> bool {
        match id {
            None => {
                self.current = None;
                true
            }
            Some(want) if self.projects.iter().any(|p| p == want) => {
                self.current = Some(want.to_string());
                true
            }
            Some(_) => false,
        }
    }
}

/// Directory memo (directory.ts:7-16): dir or cwd, abbreviateHome, :branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryMemo {
    pub dir: String,
    pub cwd: String,
    pub home: String,
    pub branch: Option<String>,
}

impl DirectoryMemo {
    #[must_use]
    pub fn resolve(&self) -> String {
        let base = if self.dir.is_empty() { &self.cwd } else { &self.dir };
        let short = tildefy(base, &self.home);
        match &self.branch {
            Some(b) if !b.is_empty() => format!("{short}:{b}"),
            _ => short,
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        s.chars().take(max).collect()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_auto_init_set_toggle() {
        let mut p = PermissionStore::new(true);
        assert_eq!(p.gate, PermissionGate::Auto);
        assert_eq!(p.mode, PermissionState::Ask);
        p.toggle();
        assert_eq!(p.gate, PermissionGate::Normal);
        p.set(PermissionGate::Auto);
        assert_eq!(p.gate, PermissionGate::Auto);
        assert_eq!(PermissionStore::new(false).gate, PermissionGate::Normal);
    }

    #[test]
    fn clipboard_write_read_bound() {
        let mut c = ClipboardService::new();
        assert_eq!(c.read(), None);
        c.write("hi").unwrap();
        assert_eq!(c.read(), Some("hi"));
        assert_eq!(c.write(&"x".repeat(MAX_CLIPBOARD_BYTES + 1)), Err(ClipboardError::TooLarge));
        assert_eq!(c.read(), Some("hi"));
    }

    #[test]
    fn theme_pin_unpin_discover() {
        let mut t = ThemeApply::new("opencode");
        assert!(!t.pinned);
        t.pin();
        assert!(t.pinned);
        t.unpin();
        assert!(!t.pinned);
        let names = ThemeApply::discover_names(&[" a ", "", "b"]);
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(ThemeApply::new(&"n".repeat(100)).source.len(), 64);
    }

    #[test]
    fn thinking_cycle() {
        assert_eq!(ThinkingMode::next(ThinkingMode::Show), ThinkingMode::Hide);
        assert_eq!(ThinkingMode::next(ThinkingMode::Hide), ThinkingMode::Show);
    }

    #[test]
    fn summary_bold_split() {
        assert_eq!(reasoning_summary(false, "**T**\n\nb"), "");
        assert_eq!(reasoning_summary(true, "**Inspecting PR**\n\nbody"), "Inspecting PR");
        assert_eq!(reasoning_summary(true, "plain"), "plain");
        assert_eq!(reasoning_summary(true, "**streaming title**"), "streaming title");
    }

    #[test]
    fn prompt_get_set_bound() {
        let mut r = PromptRef::new();
        assert_eq!(r.get(), None);
        assert!(r.set(Some("hello")));
        assert_eq!(r.get(), Some("hello"));
        assert!(!r.set(Some(&"x".repeat(MAX_PROMPT_BYTES + 1))));
        assert_eq!(r.get(), Some("hello"));
        assert!(r.set(None));
        assert_eq!(r.get(), None);
    }

    #[test]
    fn exit_codes() {
        let mut h = ExitHook::new();
        assert_eq!(h.exit(), ExitCode::Success);
        h.set_reason(Some("boom"));
        assert_eq!(h.exit(), ExitCode::Failure);
        h.set_reason(Some(&"x".repeat(MAX_EXIT_REASON_LEN + 1)));
        assert_eq!(h.reason, None);
    }

    #[test]
    fn project_list_get_set() {
        let mut s = ProjectStore::new();
        assert!(s.add("w1"));
        assert!(!s.add("w1"));
        assert_eq!(s.list(), &["w1".to_string()]);
        assert_eq!(s.get("w1"), Some("w1"));
        assert_eq!(s.get("nope"), None);
        assert!(s.set(Some("w1")));
        assert_eq!(s.current.as_deref(), Some("w1"));
        assert!(!s.set(Some("nope")));
        assert!(s.set(None));
        assert_eq!(s.current, None);
    }

    #[test]
    fn directory_resolve() {
        let m = DirectoryMemo {
            dir: "/home/u/repo".into(),
            cwd: "/home/u".into(),
            home: "/home/u".into(),
            branch: Some("main".into()),
        };
        assert_eq!(m.resolve(), "~/repo:main");
        let m2 = DirectoryMemo {
            dir: String::new(),
            cwd: "/etc".into(),
            home: "/home/u".into(),
            branch: None,
        };
        assert_eq!(m2.resolve(), "/etc");
    }
}

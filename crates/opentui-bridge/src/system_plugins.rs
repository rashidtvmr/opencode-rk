#![forbid(unsafe_code)]
//! Builtin system plugin surface (mirrors TS checkout a0d9b6c, NOT pinned 95daf90).
//! - `packages/tui/src/feature-plugins/system/notifications.ts:9-27` notify kinds +
//!   sessionErrorMessage ("Session aborted"/"Model stopped responding"/"Session error")
//! - `packages/tui/src/feature-plugins/system/which-key.tsx:10-22,126-142`
//!   (key,label) entries from active keymap keys
//! - `packages/tui/src/feature-plugins/home/tips-view.tsx:71,164-283`
//!   NO_MODELS_TIP + static TIPS subset, rotating selection
//! - `packages/tui/src/feature-plugins/home/footer.tsx:64-82`
//!   left (directory/mcp) + right (version) footer row
//! - ids from `packages/tui/src/feature-plugins/builtins.ts:21-35`
//!   (+ `home/footer.tsx:8`, `home/tips.tsx:7`, `system/plugins.tsx:9`).
//! Slot wiring reuses `super::plugin_slots::{SlotRegistry, SlotName}`; not redefined here.

use super::plugin_slots::{SlotName, SlotRegistry};

/// Builtin plugin ids (`builtins.ts:21-35` + per-file `id` consts).
pub const ID_HOME_FOOTER: &str = "internal:home-footer";
pub const ID_HOME_TIPS: &str = "internal:home-tips";
pub const ID_NOTIFICATIONS: &str = "internal:notifications";
pub const ID_PLUGIN_MANAGER: &str = "internal:plugin-manager";
pub const ID_WHICH_KEY: &str = "which-key";
pub const SYSTEM_PLUGIN_IDS: &[&str] =
    &[ID_HOME_FOOTER, ID_HOME_TIPS, ID_NOTIFICATIONS, ID_PLUGIN_MANAGER, ID_WHICH_KEY];

/// Max notification message bytes (mirrors toast bound; TS plain string).
pub const MAX_NOTIFICATION: usize = 1024;
/// Max queued notifications (fail-closed; TS dedups via live `Set`s).
pub const MAX_NOTIFICATIONS: usize = 32;
/// Max which-key bindings (TS unbounded active-key list; Rust bounded).
pub const MAX_BINDINGS: usize = 128;
/// Max home-footer side bytes (TS plain strings).
pub const MAX_FOOTER_SIDE: usize = 256;

/// Notification kind (mirrors `notify()` sounds/messages, notifications.ts:9-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Question,
    Permission,
    Done,
    SubagentDone,
    Error,
    Aborted,
    Unresponsive,
}

impl NotificationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Permission => "permission",
            Self::Done => "done",
            Self::SubagentDone => "subagent_done",
            Self::Error => "error",
            Self::Aborted => "aborted",
            Self::Unresponsive => "unresponsive",
        }
    }
}

/// Validated notification (message bounded 1024, nonempty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub message: String,
    pub kind: NotificationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    Empty,
    TooLong,
    Full,
}

impl core::fmt::Display for NotificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "notification empty"),
            Self::TooLong => write!(f, "notification too long"),
            Self::Full => write!(f, "notification queue full"),
        }
    }
}

impl std::error::Error for NotificationError {}

impl Notification {
    pub fn new(message: &str, kind: NotificationKind) -> Result<Self, NotificationError> {
        if message.is_empty() {
            return Err(NotificationError::Empty);
        }
        if message.len() > MAX_NOTIFICATION {
            return Err(NotificationError::TooLong);
        }
        Ok(Self { message: message.to_string(), kind })
    }

    /// Map a session error name to kind+message
    /// (mirrors `sessionErrorMessage`, notifications.ts:20-27).
    #[must_use]
    pub fn session_error(name: Option<&str>, sse_timeout: bool) -> Self {
        if name == Some("MessageAbortedError") {
            return Self { message: "Session aborted".to_string(), kind: NotificationKind::Aborted };
        }
        if sse_timeout {
            return Self { message: "Model stopped responding".to_string(), kind: NotificationKind::Unresponsive };
        }
        Self { message: "Session error".to_string(), kind: NotificationKind::Error }
    }
}

/// Bounded FIFO queue (fail-closed `Full`; TS dedups, never grows unbounded).
#[derive(Debug, Default, Clone)]
pub struct NotificationQueue {
    items: Vec<Notification>,
}

impl NotificationQueue {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, n: Notification) -> Result<(), NotificationError> {
        if self.items.len() >= MAX_NOTIFICATIONS {
            return Err(NotificationError::Full);
        }
        self.items.push(n);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Notification> {
        if self.items.is_empty() { None } else { Some(self.items.remove(0)) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Which-key binding table (mirrors `activeKeyEntry`, which-key.tsx:126-142).
#[derive(Debug, Default, Clone)]
pub struct WhichKey {
    bindings: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhichKeyError {
    Empty,
    Full,
}

impl core::fmt::Display for WhichKeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty key or label"),
            Self::Full => write!(f, "binding table full"),
        }
    }
}

impl std::error::Error for WhichKeyError {}

impl WhichKey {
    #[must_use]
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    pub fn add(&mut self, key: &str, label: &str) -> Result<(), WhichKeyError> {
        if key.is_empty() || label.is_empty() {
            return Err(WhichKeyError::Empty);
        }
        if self.bindings.len() >= MAX_BINDINGS {
            return Err(WhichKeyError::Full);
        }
        self.bindings.push((key.to_string(), label.to_string()));
        Ok(())
    }

    #[must_use]
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.bindings.iter().find(|(k, _)| k == key).map(|(_, l)| l.as_str())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Builtin static tips (plain-string subset of `TIPS`, tips-view.tsx:164-283).
pub const BUILTIN_TIPS: &[&str] = &[
    "Run /connect to add an AI provider and start coding",
    "Type @ followed by a filename to fuzzy search and attach files",
    "Start a message with ! to run shell commands (e.g., !ls -la)",
    "Use /undo to revert the last message and file changes",
    "Use /redo to restore previously undone messages and file changes",
    "Run /share to create a public opencode.ai link",
    "Drag and drop images or PDFs into the terminal as context",
    "Run /init to auto-generate project rules based on your codebase",
    "Run /compact to summarize long sessions near context limits",
    "Run /connect to add API keys for 75+ supported LLM providers",
    "Switch to Plan agent for suggestions without making changes",
    "Use /review to review uncommitted changes, branches, or PRs",
    "Use /rename to rename the current session",
];

/// Rotating tip selector (TS picks one random offset; Rust rotates index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    index: usize,
}

impl Tip {
    #[must_use]
    pub const fn new() -> Self {
        Self { index: 0 }
    }

    #[must_use]
    pub fn next(&mut self) -> &'static str {
        let tip = BUILTIN_TIPS[self.index % BUILTIN_TIPS.len()];
        self.index = (self.index + 1) % BUILTIN_TIPS.len();
        tip
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.index
    }
}

impl Default for Tip {
    fn default() -> Self {
        Self::new()
    }
}

/// Home footer row (mirrors `View`, footer.tsx:64-82: left + spacer + version).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeFooter {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterError {
    TooLong,
}

impl core::fmt::Display for FooterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "footer side too long")
    }
}

impl std::error::Error for FooterError {}

impl HomeFooter {
    pub fn new(left: &str, right: &str) -> Result<Self, FooterError> {
        if left.len() > MAX_FOOTER_SIDE || right.len() > MAX_FOOTER_SIDE {
            return Err(FooterError::TooLong);
        }
        Ok(Self { left: left.to_string(), right: right.to_string() })
    }

    #[must_use]
    pub fn render(&self) -> String {
        if self.left.is_empty() {
            return self.right.clone();
        }
        if self.right.is_empty() {
            return self.left.clone();
        }
        format!("{} | {}", self.left, self.right)
    }

    /// Host slot this plugin fills (`home_footer`, footer.tsx:88).
    #[must_use]
    pub const fn slot() -> SlotName {
        SlotName::HomeFooter
    }
}

/// Register the home-footer slot owner id (reuses shared `SlotRegistry`).
pub fn register_home_footer(registry: &mut SlotRegistry) -> Result<usize, super::plugin_slots::SlotError> {
    registry.register(ID_HOME_FOOTER, SlotName::HomeFooter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_bounds() {
        assert_eq!(
            Notification::new("", NotificationKind::Done).unwrap_err(),
            NotificationError::Empty
        );
        assert_eq!(
            Notification::new(&"x".repeat(MAX_NOTIFICATION + 1), NotificationKind::Done).unwrap_err(),
            NotificationError::TooLong
        );
        let n = Notification::new("Session done", NotificationKind::Done).unwrap();
        assert_eq!(n.kind.as_str(), "done");
    }

    #[test]
    fn session_error_mapping() {
        assert_eq!(Notification::session_error(Some("MessageAbortedError"), false).message, "Session aborted");
        assert_eq!(Notification::session_error(Some("Other"), true).message, "Model stopped responding");
        let e = Notification::session_error(None, false);
        assert_eq!((e.message.as_str(), e.kind), ("Session error", NotificationKind::Error));
    }

    #[test]
    fn queue_bounded_fail_closed() {
        let mut q = NotificationQueue::new();
        for i in 0..MAX_NOTIFICATIONS {
            q.push(Notification::new(&format!("m{i}"), NotificationKind::Question).unwrap()).unwrap();
        }
        assert_eq!(
            q.push(Notification::new("x", NotificationKind::Done).unwrap()).unwrap_err(),
            NotificationError::Full
        );
        assert_eq!(q.len(), MAX_NOTIFICATIONS);
        assert_eq!(q.pop().unwrap().message, "m0");
        assert!(q.pop().is_some());
    }

    #[test]
    fn whichkey_add_lookup_bound() {
        let mut w = WhichKey::new();
        assert!(w.is_empty());
        w.add("ctrl+p", "Show key bindings").unwrap();
        assert_eq!(w.lookup("ctrl+p"), Some("Show key bindings"));
        assert_eq!(w.lookup("ctrl+z"), None);
        assert_eq!(w.add("", "x").unwrap_err(), WhichKeyError::Empty);
        for i in 1..MAX_BINDINGS {
            w.add(&format!("k{i}"), "l").unwrap();
        }
        assert_eq!(w.len(), MAX_BINDINGS);
        assert_eq!(w.add("kX", "l").unwrap_err(), WhichKeyError::Full);
    }

    #[test]
    fn tip_rotates_and_wraps() {
        let mut t = Tip::new();
        let first = t.next();
        assert_eq!(first, BUILTIN_TIPS[0]);
        for _ in 1..BUILTIN_TIPS.len() {
            t.next();
        }
        assert_eq!(t.position(), 0);
        assert_eq!(t.next(), BUILTIN_TIPS[0]);
    }

    #[test]
    fn footer_bound_render_slot() {
        let f = HomeFooter::new("~/proj", "v1.0").unwrap();
        assert_eq!(f.render(), "~/proj | v1.0");
        assert_eq!(HomeFooter::new("", "v1.0").unwrap().render(), "v1.0");
        assert_eq!(
            HomeFooter::new(&"x".repeat(MAX_FOOTER_SIDE + 1), "").unwrap_err(),
            FooterError::TooLong
        );
        assert_eq!(HomeFooter::slot(), SlotName::HomeFooter);
        let mut r = SlotRegistry::new();
        assert!(register_home_footer(&mut r).is_ok());
        assert_eq!(r.get(SlotName::HomeFooter).len(), 1);
    }

    #[test]
    fn system_ids_known() {
        assert!(SYSTEM_PLUGIN_IDS.contains(&ID_WHICH_KEY));
        assert!(SYSTEM_PLUGIN_IDS.contains(&ID_NOTIFICATIONS));
        assert_eq!(SYSTEM_PLUGIN_IDS.len(), 5);
    }
}

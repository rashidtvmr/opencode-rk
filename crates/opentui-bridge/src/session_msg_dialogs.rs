#![forbid(unsafe_code)]
//! Per-message dialogs (mirrors `packages/tui/src/routes/session/dialog-message.tsx`,
//! `dialog-timeline.tsx`, `dialog-fork-from-timeline.tsx`; TS checkout a0d9b6c).
//!
//! Evidence (all line refs TS):
//! - `dialog-message.tsx:24-55` Revert (`value "session.revert"`, `sdk session.revert`
//!   + prompt rebuild from non-synthetic text + file parts, `dialog.clear()`).
//! - `dialog-message.tsx:56-75` Copy (`value "message.copy"`, non-synthetic text
//!   parts joined, `clipboard.write`, `dialog.clear()`).
//! - `dialog-message.tsx:76-105` Fork (`value "session.fork"`,
//!   `sdk session.fork({sessionID,messageID})`, prompt rebuild, `route.navigate`
//!   to forked id, `dialog.clear()`). No edit/retry actions exist in source.
//! - `dialog-timeline.tsx:25-30` user-role only, first non-synthetic non-ignored
//!   text part; `:32` title newlines flattened; `:42` reversed (newest first);
//!   `:35-39` select replaces dialog with `DialogMessage`; `onMove` jumps.
//! - `dialog-fork-from-timeline.tsx:24-35` leading "Full session" entry
//!   (`value undefined`, `fork({sessionID})` only); `:37-42` same user filter;
//!   `:48-51` `fork({sessionID,messageID})`; `:53-62` prompt rebuild;
//!   `:63-67` navigate to fork.
//! Divergence note: `routes_state.rs` `SessionDialog` covers open/close state for
//! these dialogs; this module covers their inner contracts (actions, list, fork
//! request). No `mode` param evidenced in TS fork call; `None` message_id IS the
//! full-session mode (`:24-35`).

/// Max id chars (matches `routes_state.rs:25` MAX_ID).
pub const MAX_ID: usize = 256;
/// Max timeline title chars (matches `dialog.rs` MAX_DIALOG_TITLE scale).
pub const MAX_TITLE: usize = 256;
/// Max timeline entries (matches `dialog.rs` MAX_OPTIONS bound).
pub const MAX_ENTRIES: usize = 64;

/// Every per-message action evidenced in `dialog-message.tsx` (exactly 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageAction {
    Revert,
    Copy,
    Fork,
}

impl MessageAction {
    /// All evidenced actions, in dialog order (`dialog-message.tsx:24,57,77`).
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::Revert, Self::Copy, Self::Fork]
    }

    /// Option `value` string (`dialog-message.tsx:27,58,78`).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Revert => "session.revert",
            Self::Copy => "message.copy",
            Self::Fork => "session.fork",
        }
    }

    /// Option `title` string (`dialog-message.tsx:26,57,77`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Revert => "Revert",
            Self::Copy => "Copy",
            Self::Fork => "Fork",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|a| a.id() == id)
    }
}

/// One timeline row (`dialog-timeline.tsx:31-34`, `dialog-fork-from-timeline.tsx:43-46`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    pub message_id: String,
    pub title: String,
}

impl TimelineEntry {
    /// Fail-closed: id non-empty bounded; title newlines flattened
    /// (`dialog-timeline.tsx:32` `replace(/\n/g," ")`), bounded.
    #[must_use]
    pub fn new(message_id: &str, title: &str) -> Option<Self> {
        if message_id.is_empty() || message_id.chars().count() > MAX_ID {
            return None;
        }
        let flat: String = title.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
        if flat.chars().count() > MAX_TITLE {
            return None;
        }
        Some(Self { message_id: message_id.to_string(), title: flat })
    }
}

/// Newest-first user-message list (`dialog-timeline.tsx:22-44`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimelineList {
    pub entries: Vec<TimelineEntry>,
}

impl TimelineList {
    #[must_use]
    pub fn new(mut entries: Vec<TimelineEntry>) -> Self {
        entries.truncate(MAX_ENTRIES);
        entries.reverse();
        Self { entries }
    }

    /// Clamped jump (`onMove(option.value)` `:46`): index past end clamps to
    /// last entry; empty list yields `None`.
    #[must_use]
    pub fn jump_to(&self, index: usize) -> Option<&TimelineEntry> {
        if self.entries.is_empty() {
            return None;
        }
        Some(&self.entries[index.min(self.entries.len() - 1)])
    }

    #[must_use]
    pub fn find(&self, message_id: &str) -> Option<&TimelineEntry> {
        self.entries.iter().find(|e| e.message_id == message_id)
    }
}

/// Fork request (`dialog-fork-from-timeline.tsx:28,48-51`).
/// `message_id: None` = "Full session" mode (`:24-35`); `Some` = fork from
/// that message. No other mode param evidenced in TS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkRequest {
    pub message_id: Option<String>,
}

impl ForkRequest {
    /// Fail-closed constructor.
    #[must_use]
    pub fn new(message_id: Option<&str>) -> Option<Self> {
        match message_id {
            None => Some(Self { message_id: None }),
            Some(id) if !id.is_empty() && id.chars().count() <= MAX_ID => {
                Some(Self { message_id: Some(id.to_string()) })
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn is_full_session(&self) -> bool {
        self.message_id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_table_matches_ts() {
        assert_eq!(MessageAction::Revert.id(), "session.revert");
        assert_eq!(MessageAction::Copy.id(), "message.copy");
        assert_eq!(MessageAction::Fork.id(), "session.fork");
        assert_eq!(MessageAction::Revert.label(), "Revert");
        assert_eq!(MessageAction::Copy.label(), "Copy");
        assert_eq!(MessageAction::Fork.label(), "Fork");
        assert_eq!(MessageAction::all().len(), 3);
    }

    #[test]
    fn action_from_id_roundtrip() {
        for a in MessageAction::all() {
            assert_eq!(MessageAction::from_id(a.id()), Some(a));
        }
        assert_eq!(MessageAction::from_id("message.edit"), None);
        assert_eq!(MessageAction::from_id("message.retry"), None);
    }

    #[test]
    fn entry_flattens_newlines_and_bounds() {
        let e = TimelineEntry::new("m1", "line1\nline2").unwrap();
        assert_eq!(e.title, "line1 line2");
        assert!(TimelineEntry::new("", "t").is_none());
        assert!(TimelineEntry::new("m1", &"t".repeat(MAX_TITLE + 1)).is_none());
        assert!(TimelineEntry::new(&"x".repeat(MAX_ID + 1), "t").is_none());
    }

    #[test]
    fn list_reverses_and_jumps_clamped() {
        let mk = |id: &str| TimelineEntry::new(id, id).unwrap();
        let list = TimelineList::new(vec![mk("m1"), mk("m2"), mk("m3")]);
        assert_eq!(list.entries[0].message_id, "m3");
        assert_eq!(list.jump_to(0).unwrap().message_id, "m3");
        assert_eq!(list.jump_to(99).unwrap().message_id, "m1");
        assert_eq!(list.find("m2").unwrap().title, "m2");
        assert!(list.find("zzz").is_none());
    }

    #[test]
    fn list_empty_and_truncated() {
        assert!(TimelineList::default().jump_to(0).is_none());
        let mk = |i: usize| TimelineEntry::new(&format!("m{i}"), "t").unwrap();
        let list = TimelineList::new((0..MAX_ENTRIES + 10).map(mk).collect());
        assert_eq!(list.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn fork_full_session_vs_message() {
        let full = ForkRequest::new(None).unwrap();
        assert!(full.is_full_session());
        let at = ForkRequest::new(Some("m1")).unwrap();
        assert!(!at.is_full_session());
        assert_eq!(at.message_id.as_deref(), Some("m1"));
        assert!(ForkRequest::new(Some("")).is_none());
        assert!(ForkRequest::new(Some(&"x".repeat(MAX_ID + 1))).is_none());
    }
}

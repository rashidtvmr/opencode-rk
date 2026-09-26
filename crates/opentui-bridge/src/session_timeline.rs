#![forbid(unsafe_code)]
//! Session timeline dialog (mirrors
//! `packages/tui/src/routes/session/dialog-timeline.tsx:10` `DialogTimeline`
//! user-message list + `ui/dialog-select.tsx` cursor/`onMove`).
//!
//! Divergences:
//! - T-erasure: TS option `value` is message id string; Rust stores `id`.
//! - TS title is newline-flattened text part, footer is locale time; Rust
//!   stores pre-flattened `label`, no timestamps.
//! - TS reverse-chronological + fuzzy filter + replace-with-DialogMessage are
//!   host concerns, not modeled.

/// Max chars for `TimelineEntry.id` (message id).
pub const MAX_ID: usize = 64;
/// Max chars for `TimelineEntry.label` (flattened title).
pub const MAX_LABEL: usize = 256;
/// Max entries in one `TimelineDialog` (fail-closed bound).
pub const MAX_ENTRIES: usize = 200;

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// One timeline row: TS `DialogSelectOption<string>` (`title`+`value`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

impl TimelineEntry {
    pub fn new(id: &str, label: &str, selected: bool) -> Self {
        Self {
            id: trunc(id, MAX_ID),
            label: trunc(label, MAX_LABEL),
            selected,
        }
    }
}

/// Cursor over timeline rows; `cursor` indexes insertion order.
#[derive(Debug, Clone, Default)]
pub struct TimelineDialog {
    pub entries: Vec<TimelineEntry>,
    pub cursor: usize,
}

impl TimelineDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: TimelineEntry) -> bool {
        if self.entries.len() >= MAX_ENTRIES {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.entries.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor as isize + delta).rem_euclid(len as isize);
        self.cursor = next as usize;
    }

    pub fn selected(&self) -> Option<&TimelineEntry> {
        self.entries.get(self.cursor)
    }

    pub fn fork_selected(&self) -> Option<String> {
        self.selected().map(|e| e.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> TimelineEntry {
        TimelineEntry::new(id, "label", false)
    }

    #[test]
    fn push_cap() {
        let mut d = TimelineDialog::new();
        for i in 0..MAX_ENTRIES {
            assert!(d.push(entry(&format!("m{i}"))));
        }
        assert!(!d.push(entry("overflow")));
        assert_eq!(d.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn cursor_wraps_forward() {
        let mut d = TimelineDialog::new();
        d.push(entry("a"));
        d.push(entry("b"));
        d.move_cursor(2);
        assert_eq!(d.cursor, 0);
        d.move_cursor(1);
        assert_eq!(d.cursor, 1);
    }

    #[test]
    fn cursor_wraps_backward() {
        let mut d = TimelineDialog::new();
        d.push(entry("a"));
        d.push(entry("b"));
        d.move_cursor(-1);
        assert_eq!(d.cursor, 1);
        assert_eq!(d.selected().unwrap().id, "b");
    }

    #[test]
    fn selected_none_when_empty() {
        let d = TimelineDialog::new();
        assert!(d.selected().is_none());
        assert!(d.fork_selected().is_none());
    }

    #[test]
    fn fork_returns_id_clone() {
        let mut d = TimelineDialog::new();
        d.push(TimelineEntry::new("msg1", "hello", true));
        d.push(entry("msg2"));
        assert_eq!(d.fork_selected(), Some("msg1".to_string()));
        d.move_cursor(1);
        assert_eq!(d.fork_selected(), Some("msg2".to_string()));
    }

    #[test]
    fn overlong_truncates() {
        let e = TimelineEntry::new(&"i".repeat(100), &"l".repeat(300), false);
        assert_eq!(e.id.chars().count(), MAX_ID);
        assert_eq!(e.label.chars().count(), MAX_LABEL);
    }
}

#![forbid(unsafe_code)]
//! Dialog host pairing id stack with timeline dialog.
//!
//! Mirrors `packages/tui/src/ui/dialog.tsx` stack plus
//! `routes/session/dialog-timeline.tsx:10` `DialogTimeline` selection.
//! Opening id [`TIMELINE_ID`] raises the timeline flag; popping it lowers it.

use crate::dialog_stack::DialogStack;
use crate::timeline_dialog::TimelineDialog;

/// Stack id that syncs [`TimelineDialog::open`].
pub const TIMELINE_ID: &str = "timeline";

/// Owns a [`DialogStack`] plus its timeline dialog state.
#[derive(Debug, Clone, Default)]
pub struct DialogHost {
    pub stack: DialogStack,
    pub timeline: TimelineDialog,
}

impl DialogHost {
    /// Empty host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push id; raises timeline flag when id is [`TIMELINE_ID`].
    pub fn open(&mut self, id: &str) -> bool {
        let ok = self.stack.open(id);
        if ok && id == TIMELINE_ID {
            self.timeline.open_dialog();
        }
        ok
    }

    /// Pop top id; lowers timeline flag when it was [`TIMELINE_ID`].
    pub fn close_top(&mut self) -> Option<String> {
        let popped = self.stack.close();
        if popped.as_deref() == Some(TIMELINE_ID) {
            self.timeline.close_dialog();
        }
        popped
    }

    /// Peek top id.
    pub fn top(&self) -> Option<&str> {
        self.stack.top()
    }

    /// Timeline cursor selection.
    pub fn timeline_selected(&self) -> Option<&str> {
        self.timeline.selected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_top_close_roundtrip() {
        let mut h = DialogHost::new();
        assert!(h.open("a"));
        assert_eq!(h.top(), Some("a"));
        assert_eq!(h.close_top(), Some("a".to_string()));
        assert_eq!(h.top(), None);
    }

    #[test]
    fn open_timeline_raises_flag() {
        let mut h = DialogHost::new();
        assert!(h.open(TIMELINE_ID));
        assert!(h.timeline.open);
        assert_eq!(h.top(), Some(TIMELINE_ID));
    }

    #[test]
    fn close_timeline_lowers_flag() {
        let mut h = DialogHost::new();
        h.open("a");
        h.open(TIMELINE_ID);
        assert_eq!(h.close_top(), Some(TIMELINE_ID.to_string()));
        assert!(!h.timeline.open);
        assert_eq!(h.top(), Some("a"));
    }

    #[test]
    fn close_empty_none() {
        let mut h = DialogHost::new();
        assert_eq!(h.close_top(), None);
        assert_eq!(h.timeline_selected(), None);
    }

    #[test]
    fn timeline_selected_delegates() {
        let mut h = DialogHost::new();
        h.timeline.set_entries(vec!["m0".into(), "m1".into()]);
        h.timeline.move_cursor(1);
        assert_eq!(h.timeline_selected(), Some("m1"));
    }

    #[test]
    fn open_rejects_long_id() {
        let mut h = DialogHost::new();
        let long = "x".repeat(65);
        assert!(!h.open(&long));
        assert_eq!(h.top(), None);
        assert!(!h.timeline.open);
    }
}

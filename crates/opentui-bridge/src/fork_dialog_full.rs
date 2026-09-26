#![forbid(unsafe_code)]
//! Full-session fork pick list (mirrors
//! `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12`
//! `DialogForkFromTimeline` full-session entry + per-message `onSelect` fork
//! picks). Single-message confirm gate lives in `crate::session_fork_dialog`.

/// Max chars per pick id.
pub const MAX_ID: usize = 64;
/// Max picks retained.
pub const MAX_PICKS: usize = 64;

/// One forkable timeline message pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkPick {
    pub message_id: String,
    pub index: usize,
}

/// Pick-then-confirm list for fork-from-timeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkDialogFull {
    pub picks: Vec<ForkPick>,
    pub confirmed: Option<usize>,
}

impl ForkDialogFull {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append pick; false on blank id or when at cap.
    pub fn add_pick(&mut self, id: &str) -> bool {
        if id.trim().is_empty() || self.picks.len() >= MAX_PICKS {
            return false;
        }
        let index = self.picks.len();
        self.picks.push(ForkPick {
            message_id: id.chars().take(MAX_ID).collect(),
            index,
        });
        true
    }

    /// Confirm pick by index; false when out of bounds.
    pub fn confirm(&mut self, idx: usize) -> bool {
        if idx >= self.picks.len() {
            return false;
        }
        self.confirmed = Some(idx);
        true
    }

    /// Confirmed pick's message id, if any.
    pub fn confirmed_id(&self) -> Option<&str> {
        self.confirmed
            .and_then(|i| self.picks.get(i))
            .map(|p| p.message_id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_ok_indexed() {
        let mut d = ForkDialogFull::new();
        assert!(d.add_pick("msg1"));
        assert_eq!(d.picks.len(), 1);
        assert_eq!(d.picks[0].index, 0);
        assert_eq!(d.picks[0].message_id, "msg1");
    }

    #[test]
    fn add_blank_rejected() {
        let mut d = ForkDialogFull::new();
        assert!(!d.add_pick("   "));
        assert!(d.picks.is_empty());
    }

    #[test]
    fn add_truncates_id() {
        let mut d = ForkDialogFull::new();
        assert!(d.add_pick(&"m".repeat(100)));
        assert_eq!(d.picks[0].message_id.chars().count(), MAX_ID);
    }

    #[test]
    fn add_at_cap_rejected() {
        let mut d = ForkDialogFull::new();
        for i in 0..MAX_PICKS {
            assert!(d.add_pick(&format!("m{i}")));
        }
        assert!(!d.add_pick("extra"));
        assert_eq!(d.picks.len(), MAX_PICKS);
    }

    #[test]
    fn confirm_bounds() {
        let mut d = ForkDialogFull::new();
        assert!(!d.confirm(0));
        d.add_pick("a");
        assert!(d.confirm(0));
        assert!(!d.confirm(1));
    }

    #[test]
    fn confirmed_none_before() {
        let mut d = ForkDialogFull::new();
        d.add_pick("a");
        assert_eq!(d.confirmed_id(), None);
    }

    #[test]
    fn overwrite_confirm() {
        let mut d = ForkDialogFull::new();
        d.add_pick("a");
        d.add_pick("b");
        assert!(d.confirm(0));
        assert_eq!(d.confirmed_id(), Some("a"));
        assert!(d.confirm(1));
        assert_eq!(d.confirmed_id(), Some("b"));
    }
}

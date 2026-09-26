#![forbid(unsafe_code)]
//! Pinned scrollback core over ScrollbackSharedFull.
//!
//! Claim: auto-follow wrapper. push re-pins, unpin holds offset, repin snaps to bottom.
//! Evidence: crates/opentui-bridge/src/scrollback_shared_full.rs:13-48 (store, push re-pin, visible window).
//! Boundary: pin flag only; scrolling itself lives in ScrollbackSharedFull::scroll.

use crate::scrollback_shared_full::ScrollbackSharedFull;

/// Auto-follow core: `pinned` true means view tracks newest lines.
#[derive(Debug, Clone, Default)]
pub struct ScrollbackCore {
    pub store: ScrollbackSharedFull,
    pub pinned: bool,
}

impl ScrollbackCore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: ScrollbackSharedFull::new(),
            pinned: true,
        }
    }

    pub fn push(&mut self, line: &str) {
        self.store.push(line);
        self.pinned = true;
    }

    pub fn unpin(&mut self) {
        self.pinned = false;
    }

    pub fn repin(&mut self) {
        self.pinned = true;
        self.store.offset = 0;
    }

    #[must_use]
    pub fn view(&self, h: usize) -> Vec<String> {
        self.store.visible(h).to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_pinned_empty() {
        let c = ScrollbackCore::new();
        assert!(c.pinned);
        assert!(c.view(4).is_empty());
    }

    #[test]
    fn push_sets_pinned_and_visible() {
        let mut c = ScrollbackCore::new();
        c.unpin();
        c.push("a");
        assert!(c.pinned);
        assert_eq!(c.view(4), vec!["a".to_owned()]);
    }

    #[test]
    fn unpin_holds_flag() {
        let mut c = ScrollbackCore::new();
        c.push("a");
        c.unpin();
        assert!(!c.pinned);
    }

    #[test]
    fn repin_snaps_offset_to_bottom() {
        let mut c = ScrollbackCore::new();
        for i in 0..10 {
            c.push(format!("l{i}").as_str());
        }
        c.unpin();
        c.store.scroll(3, 4);
        assert_eq!(c.store.offset, 3);
        c.repin();
        assert!(c.pinned);
        assert_eq!(c.store.offset, 0);
    }

    #[test]
    fn view_clones_visible_window() {
        let mut c = ScrollbackCore::new();
        for i in 0..6 {
            c.push(format!("l{i}").as_str());
        }
        assert_eq!(c.view(2), vec!["l4".to_owned(), "l5".to_owned()]);
        c.store.scroll(2, 2);
        assert_eq!(c.view(2), vec!["l2".to_owned(), "l3".to_owned()]);
    }

    #[test]
    fn push_after_unpin_repins() {
        let mut c = ScrollbackCore::new();
        for i in 0..6 {
            c.push(format!("l{i}").as_str());
        }
        c.unpin();
        c.store.scroll(2, 2);
        c.push("new");
        assert!(c.pinned);
        assert!(c.view(2).contains(&"new".to_owned()));
    }
}

#![forbid(unsafe_code)]
//! Menu keyboard nav over [`FocusRing`]. Pure, bounded, no IO.
//!
//! Paint side: [`crate::widget_paint::menu_calls`] renders the rows;
//! this tracks which row is focused. Empty nav is fail-closed (`None`).

use crate::input::{FocusRing, MAX_FOCUS};

/// Bounded menu cursor with ids `1..=count`. Up/down wrap around.
#[derive(Debug, Clone, Default)]
pub struct MenuNav {
    pub ring: FocusRing,
    pub count: usize,
}

impl MenuNav {
    /// Build nav for `count` rows, capped at 64. Empty when `count` is 0.
    #[must_use]
    pub fn new(count: usize) -> Self {
        let n = count.min(MAX_FOCUS);
        let mut ring = FocusRing::new();
        for id in 1..=n as u32 {
            let _ = ring.push(id);
        }
        Self { ring, count: n }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Move focus down with wrap. `None` when empty.
    pub fn down(&mut self) -> Option<u32> {
        self.ring.next().ok()
    }

    /// Move focus up with wrap. `None` when empty.
    pub fn up(&mut self) -> Option<u32> {
        self.ring.prev().ok()
    }

    /// Confirm the current row. `None` when empty.
    #[must_use]
    pub fn select(&self) -> Option<u32> {
        self.ring.current()
    }

    /// Currently focused row id, if any.
    #[must_use]
    pub fn selected(&self) -> Option<u32> {
        self.ring.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_fail_closed() {
        let mut nav = MenuNav::new(0);
        assert!(nav.is_empty());
        assert_eq!(nav.selected(), None);
        assert_eq!(nav.down(), None);
        assert_eq!(nav.up(), None);
        assert_eq!(nav.select(), None);
    }

    #[test]
    fn down_wraps() {
        let mut nav = MenuNav::new(3);
        assert_eq!(nav.down(), Some(2));
        assert_eq!(nav.down(), Some(3));
        assert_eq!(nav.down(), Some(1));
    }

    #[test]
    fn up_wraps() {
        let mut nav = MenuNav::new(3);
        assert_eq!(nav.up(), Some(3));
        assert_eq!(nav.up(), Some(2));
    }

    #[test]
    fn select_tracks_focus() {
        let mut nav = MenuNav::new(2);
        assert_eq!(nav.select(), Some(1));
        nav.down();
        assert_eq!(nav.select(), Some(2));
        assert_eq!(nav.selected(), Some(2));
    }

    #[test]
    fn count_capped_at_64() {
        let nav = MenuNav::new(999);
        assert_eq!(nav.count, 64);
        assert_eq!(nav.len(), 64);
        assert_eq!(nav.ring.len(), 64);
    }

    #[test]
    fn single_item_stays() {
        let mut nav = MenuNav::new(1);
        assert_eq!(nav.down(), Some(1));
        assert_eq!(nav.up(), Some(1));
        assert_eq!(nav.select(), Some(1));
    }
}

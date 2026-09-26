#![forbid(unsafe_code)]
//! Pinned-bottom scrollback view over [`ScrollbackSharedFull`].
//!
//! Thin view: [`ScrollbackSharedFull`] owns lines/offset, this owns the
//! window height and paints the visible window via
//! [`crate::transcript_paint::paint_lines`] with role `assistant`.
//! ponytail: plain-text paint only; add styling when theme spans land.

use crate::scrollback_shared_full::ScrollbackSharedFull;
use crate::transcript_paint::paint_lines;

/// Scrollback view: shared store plus fixed window height.
#[derive(Debug, Clone, Default)]
pub struct ScrollbackView {
    pub store: ScrollbackSharedFull,
    pub height: usize,
}

impl ScrollbackView {
    #[must_use]
    pub fn new(height: usize) -> Self {
        Self {
            store: ScrollbackSharedFull::new(),
            height,
        }
    }

    pub fn push(&mut self, line: &str) {
        self.store.push(line);
    }

    pub fn scroll(&mut self, delta: isize) {
        self.store.scroll(delta, self.height);
    }

    #[must_use]
    pub fn render(&self, width: usize) -> Vec<String> {
        let entries: Vec<(String, String)> = self
            .store
            .visible(self.height)
            .iter()
            .map(|l| ("assistant".to_string(), l.clone()))
            .collect();
        paint_lines(&entries, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(n: usize, h: usize) -> ScrollbackView {
        let mut v = ScrollbackView::new(h);
        for i in 0..n {
            v.push(&format!("l{i}"));
        }
        v
    }

    #[test]
    fn render_paints_newest_with_prefix() {
        let v = view(3, 2);
        assert_eq!(v.render(80), vec!["assistant> l1", "assistant> l2"]);
    }

    #[test]
    fn render_clips_width_charsafe() {
        let mut v = ScrollbackView::new(2);
        v.push("éééééé");
        let out = v.render(12);
        assert_eq!(out.len(), 1);
        assert!(out[0].chars().count() <= 12);
    }

    #[test]
    fn scroll_moves_window() {
        let mut v = view(10, 3);
        v.scroll(2);
        assert_eq!(
            v.render(80),
            vec!["assistant> l5", "assistant> l6", "assistant> l7"]
        );
    }

    #[test]
    fn scroll_clamps_both_ends() {
        let mut v = view(5, 2);
        v.scroll(isize::MAX);
        assert_eq!(v.render(80), vec!["assistant> l0", "assistant> l1"]);
        v.scroll(isize::MIN);
        assert_eq!(v.render(80), vec!["assistant> l3", "assistant> l4"]);
    }

    #[test]
    fn push_repins_to_bottom() {
        let mut v = view(5, 2);
        v.scroll(2);
        v.push("new");
        assert_eq!(v.render(80), vec!["assistant> l4", "assistant> new"]);
    }

    #[test]
    fn width_zero_empty() {
        let v = view(3, 2);
        assert!(v.render(0).is_empty());
    }

    #[test]
    fn empty_store_renders_empty() {
        let v = ScrollbackView::new(4);
        assert!(v.render(80).is_empty());
    }
}

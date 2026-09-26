#![forbid(unsafe_code)]
//! Surface viewport over retained scrollback rows.
//!
//! TS truth: `scrollback.surface.ts` retained-surface machinery commits
//! rows then renders a tail window; this type models the tail window
//! (`height`, `offset` from bottom) dependency-free.

/// Max viewport height (rows).
pub const MAX_HEIGHT: usize = 200;

/// Viewport onto a row buffer. `offset` counts rows scrolled up from tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceView {
    pub offset: usize,
    pub height: usize,
}

impl SurfaceView {
    #[must_use]
    pub fn new(height: usize) -> Self {
        Self {
            offset: 0,
            height: height.min(MAX_HEIGHT),
        }
    }

    #[must_use]
    pub fn visible(&self, rows: &[String]) -> Vec<String> {
        if self.height == 0 || rows.is_empty() {
            return Vec::new();
        }
        let max_offset = rows.len().saturating_sub(self.height.min(rows.len()));
        let end = rows.len().saturating_sub(self.offset.min(max_offset));
        let start = end.saturating_sub(self.height);
        rows[start..end].to_vec()
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_add(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("row-{i}")).collect()
    }

    #[test]
    fn window_tail() {
        let v = SurfaceView::new(3);
        assert_eq!(v.visible(&rows(5)), vec!["row-2", "row-3", "row-4"]);
    }

    #[test]
    fn offset_shifts() {
        let mut v = SurfaceView::new(2);
        v.scroll_up(1);
        assert_eq!(v.visible(&rows(5)), vec!["row-2", "row-3"]);
    }

    #[test]
    fn up_saturates() {
        let mut v = SurfaceView::new(2);
        v.scroll_up(usize::MAX);
        let got = v.visible(&rows(3));
        assert_eq!(got, vec!["row-0", "row-1"]);
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn down_saturates() {
        let mut v = SurfaceView::new(2);
        v.scroll_up(3);
        v.scroll_down(10);
        assert_eq!(v.offset, 0);
        assert_eq!(v.visible(&rows(4)), vec!["row-2", "row-3"]);
    }

    #[test]
    fn reset_zero() {
        let mut v = SurfaceView::new(2);
        v.scroll_up(2);
        v.reset();
        assert_eq!(v.offset, 0);
        assert_eq!(v.visible(&rows(3)), vec!["row-1", "row-2"]);
    }

    #[test]
    fn height_caps_at_200() {
        assert_eq!(SurfaceView::new(500).height, MAX_HEIGHT);
    }
}

#![forbid(unsafe_code)]
//! Full scrollback ring: bounded lines with scroll offset and window view.
//!
//! `offset == 0` pins the view to the newest lines. Positive scroll moves
//! toward older lines. `push` evicts the oldest line and re-pins to bottom.

/// Max retained lines. Older lines evict first.
pub const MAX_LINES: usize = 500;
/// Max chars retained per line.
pub const LINE_CAP_CHARS: usize = 2 * 1024;

/// Bounded scrollback store with a scroll offset and window view.
#[derive(Debug, Clone, Default)]
pub struct ScrollbackSharedFull {
    pub lines: Vec<String>,
    pub offset: usize,
}

impl ScrollbackSharedFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, line: &str) {
        if self.lines.len() >= MAX_LINES {
            self.lines.remove(0);
        }
        self.lines.push(truncate(line));
        self.offset = 0;
    }

    pub fn scroll(&mut self, delta: isize, height: usize) {
        let max = self.max_offset(height);
        if delta >= 0 {
            self.offset = self.offset.saturating_add(delta as usize).min(max);
        } else {
            self.offset = self.offset.saturating_sub(delta.unsigned_abs()).min(max);
        }
    }

    #[must_use]
    pub fn visible(&self, height: usize) -> &[String] {
        let eff = self.offset.min(self.max_offset(height));
        let end = self.lines.len().saturating_sub(eff);
        let start = end.saturating_sub(height);
        &self.lines[start..end]
    }

    fn max_offset(&self, height: usize) -> usize {
        self.lines.len().saturating_sub(height)
    }
}

fn truncate(s: &str) -> String {
    if s.len() <= LINE_CAP_CHARS {
        return s.to_owned();
    }
    if s.chars().count() <= LINE_CAP_CHARS {
        return s.to_owned();
    }
    s.chars().take(LINE_CAP_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_and_resets_offset() {
        let mut s = ScrollbackSharedFull::new();
        for i in 0..MAX_LINES + 3 {
            s.push(&format!("l{i}"));
        }
        assert_eq!(s.lines.len(), MAX_LINES);
        assert_eq!(s.lines[0], "l3");
        s.scroll(2, 4);
        assert_eq!(s.offset, 2);
        s.push("new");
        assert_eq!(s.offset, 0);
        assert_eq!(s.lines.last().unwrap(), "new");
    }

    #[test]
    fn scroll_clamps_top() {
        let mut s = ScrollbackSharedFull::new();
        for i in 0..10 {
            s.push(&format!("l{i}"));
        }
        s.scroll(isize::MAX, 4);
        assert_eq!(s.offset, 6);
    }

    #[test]
    fn scroll_clamps_bottom() {
        let mut s = ScrollbackSharedFull::new();
        for i in 0..10 {
            s.push(&format!("l{i}"));
        }
        s.scroll(3, 4);
        s.scroll(isize::MIN, 4);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn visible_window_follows_offset() {
        let mut s = ScrollbackSharedFull::new();
        for i in 0..10 {
            s.push(&format!("l{i}"));
        }
        let v: Vec<&str> = s.visible(3).iter().map(String::as_str).collect();
        assert_eq!(v, ["l7", "l8", "l9"]);
        s.scroll(2, 3);
        let v: Vec<&str> = s.visible(3).iter().map(String::as_str).collect();
        assert_eq!(v, ["l5", "l6", "l7"]);
    }

    #[test]
    fn truncates_to_2kib_chars() {
        let mut s = ScrollbackSharedFull::new();
        s.push(&"x".repeat(3000));
        assert_eq!(s.lines[0].chars().count(), LINE_CAP_CHARS);
        s.push(&"é".repeat(3000));
        assert_eq!(s.lines[1].chars().count(), LINE_CAP_CHARS);
        s.push("short");
        assert_eq!(s.lines[2], "short");
    }

    #[test]
    fn visible_edge_heights() {
        let mut s = ScrollbackSharedFull::new();
        s.push("a");
        s.push("b");
        assert_eq!(s.visible(99).len(), 2);
        assert!(s.visible(0).is_empty());
        assert!(ScrollbackSharedFull::new().visible(4).is_empty());
    }
}

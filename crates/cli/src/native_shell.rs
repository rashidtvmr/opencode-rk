#![forbid(unsafe_code)]
//! Native shell paint state (opentui buffers).
//!
//! Pure state only: capped lines for shell paint buffers. No rendering,
//! no IO, no threads; caller paints `snapshot()` into the real buffer.

use std::collections::VecDeque;

/// Max chars retained per line.
pub const MAX_LINE: usize = 1024;
/// Max lines retained per buffer.
pub const MAX_LINES: usize = 500;

/// One paintable shell line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellLine {
    pub text: String,
    pub bold: bool,
}

impl ShellLine {
    #[must_use]
    pub fn new(text: impl Into<String>, bold: bool) -> Self {
        let raw: String = text.into();
        let text = if raw.chars().count() > MAX_LINE {
            raw.chars().take(MAX_LINE).collect()
        } else {
            raw
        };
        Self { text, bold }
    }
}

/// Bounded shell buffer with scrollback offset.
#[derive(Clone, Debug, Default)]
pub struct ShellBuffer {
    lines: VecDeque<ShellLine>,
    /// Rows scrolled up from latest (0 = pinned to bottom).
    scroll: usize,
}

impl ShellBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a line; evicts oldest when full.
    pub fn push_line(&mut self, text: impl Into<String>, bold: bool) {
        if self.lines.len() >= MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(ShellLine::new(text, bold));
        self.clamp_scroll();
    }

    /// Visible window: last `n` lines offset by scroll.
    #[must_use]
    pub fn snapshot(&self, n: usize) -> Vec<&ShellLine> {
        let len = self.lines.len();
        let scrolled = self.scroll.min(len);
        let end = len - scrolled;
        let start = end.saturating_sub(n);
        self.lines.iter().skip(start).take(end - start).collect()
    }

    /// Scroll up (away from latest) by `delta` rows.
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_add(delta).min(self.lines.len());
    }

    /// Scroll down (toward latest) by `delta` rows.
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
    }

    /// Clear all lines and reset scroll.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    #[must_use]
    pub fn lines(&self) -> &VecDeque<ShellLine> {
        &self.lines
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.min(self.lines.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_evicts_oldest_when_full() {
        let mut buf = ShellBuffer::new();
        for i in 0..(MAX_LINES + 10) {
            buf.push_line(format!("line{i}"), false);
        }
        assert_eq!(buf.len(), MAX_LINES);
        assert_eq!(buf.lines()[0].text, "line10");
        assert_eq!(buf.lines()[MAX_LINES - 1].text, format!("line{}", MAX_LINES + 9));
    }

    #[test]
    fn line_text_capped_at_max_line() {
        let long = "x".repeat(MAX_LINE + 100);
        let line = ShellLine::new(long, true);
        assert_eq!(line.text.chars().count(), MAX_LINE);
        assert!(line.bold);
        let mut buf = ShellBuffer::new();
        buf.push_line("y".repeat(MAX_LINE + 1), false);
        assert_eq!(buf.lines()[0].text.chars().count(), MAX_LINE);
    }

    #[test]
    fn empty_snapshot_and_window() {
        let buf = ShellBuffer::new();
        assert!(buf.is_empty());
        assert!(buf.snapshot(10).is_empty());
        assert!(buf.snapshot(0).is_empty());
        let mut buf = ShellBuffer::new();
        buf.push_line("a", false);
        buf.push_line("b", true);
        let snap: Vec<&str> = buf.snapshot(10).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(snap, vec!["a", "b"]);
        let last: Vec<&str> = buf.snapshot(1).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(last, vec!["b"]);
        assert!(buf.snapshot(1)[0].bold);
    }

    #[test]
    fn scroll_offsets_snapshot_window() {
        let mut buf = ShellBuffer::new();
        for i in 0..5 {
            buf.push_line(format!("l{i}"), false);
        }
        buf.scroll_up(2);
        assert_eq!(buf.scroll(), 2);
        let snap: Vec<&str> = buf.snapshot(2).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(snap, vec!["l1", "l2"]);
        buf.scroll_down(10);
        assert_eq!(buf.scroll(), 0);
    }
}

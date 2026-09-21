#![forbid(unsafe_code)]
//! Native shell paint state (opentui buffers).
//!
//! Pure state only: capped lines for shell paint buffers. No rendering,
//! no IO, no threads; caller paints `snapshot()` into the real buffer.

use std::collections::VecDeque;

use crate::native_timeline::{TimelineBuilder, TimelinePage};

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

/// Page snapshots: one bounded [`ShellBuffer`] per paint region
/// (transcript, composer, sidebar). Caller sizes each window from the
/// region height (`ShellLayout` rect) so pages never overlap.
#[derive(Clone, Debug, Default)]
pub struct ShellPages {
    transcript: ShellBuffer,
    composer: ShellBuffer,
    sidebar: ShellBuffer,
}

impl ShellPages {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_transcript(&mut self, text: impl Into<String>, bold: bool) {
        self.transcript.push_line(text, bold);
    }

    /// Paint the timeline sidebar window into the transcript buffer.
    ///
    /// Calls [`TimelineBuilder::render_page`] with the caller's `page`,
    /// `height` (region rows) and `width` (chars per row), then pushes each
    /// bounded row into the transcript buffer. Rows are already bounded by
    /// `render_page` (`MAX_PAGE` rows, wrapped at `width`); the buffer caps
    /// them again at [`MAX_LINES`] lines / [`MAX_LINE`] chars. Returns the
    /// number of rows painted.
    pub fn paint_timeline(
        &mut self,
        builder: &TimelineBuilder,
        page: &TimelinePage,
        height: usize,
        width: usize,
    ) -> usize {
        let rows = builder.render_page(page, height, width);
        let n = rows.len();
        for row in rows {
            self.push_transcript(row, false);
        }
        n
    }

    pub fn push_composer(&mut self, text: impl Into<String>, bold: bool) {
        self.composer.push_line(text, bold);
    }

    pub fn push_sidebar(&mut self, text: impl Into<String>, bold: bool) {
        self.sidebar.push_line(text, bold);
    }

    #[must_use]
    pub fn transcript_window(&self, height: usize) -> Vec<&ShellLine> {
        self.transcript.snapshot(height)
    }

    #[must_use]
    pub fn composer_window(&self, height: usize) -> Vec<&ShellLine> {
        self.composer.snapshot(height)
    }

    #[must_use]
    pub fn sidebar_window(&self, height: usize) -> Vec<&ShellLine> {
        self.sidebar.snapshot(height)
    }

    pub fn scroll_transcript_up(&mut self, delta: usize) {
        self.transcript.scroll_up(delta);
    }

    pub fn scroll_transcript_down(&mut self, delta: usize) {
        self.transcript.scroll_down(delta);
    }

    pub fn scroll_composer_up(&mut self, delta: usize) {
        self.composer.scroll_up(delta);
    }

    pub fn scroll_composer_down(&mut self, delta: usize) {
        self.composer.scroll_down(delta);
    }

    pub fn scroll_sidebar_up(&mut self, delta: usize) {
        self.sidebar.scroll_up(delta);
    }

    pub fn scroll_sidebar_down(&mut self, delta: usize) {
        self.sidebar.scroll_down(delta);
    }

    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
    }

    pub fn clear_composer(&mut self) {
        self.composer.clear();
    }

    pub fn clear_sidebar(&mut self) {
        self.sidebar.clear();
    }

    #[must_use]
    pub fn transcript_len(&self) -> usize {
        self.transcript.len()
    }

    #[must_use]
    pub fn composer_len(&self) -> usize {
        self.composer.len()
    }

    #[must_use]
    pub fn sidebar_len(&self) -> usize {
        self.sidebar.len()
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

    #[test]
    fn pages_route_to_own_region() {
        let mut pages = ShellPages::new();
        pages.push_transcript("t1", false);
        pages.push_composer("c1", true);
        pages.push_sidebar("s1", false);
        let t: Vec<&str> = pages.transcript_window(10).iter().map(|l| l.text.as_str()).collect();
        let c: Vec<&str> = pages.composer_window(10).iter().map(|l| l.text.as_str()).collect();
        let s: Vec<&str> = pages.sidebar_window(10).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(t, vec!["t1"]);
        assert_eq!(c, vec!["c1"]);
        assert_eq!(s, vec!["s1"]);
    }

    #[test]
    fn page_windows_clamp_to_height() {
        let mut pages = ShellPages::new();
        for i in 0..5 {
            pages.push_transcript(format!("l{i}"), false);
        }
        let win: Vec<&str> =
            pages.transcript_window(2).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(win, vec!["l3", "l4"]);
        assert!(pages.transcript_window(0).is_empty());
        assert!(pages.sidebar_window(10).is_empty());
    }

    #[test]
    fn page_scroll_is_per_region() {
        let mut pages = ShellPages::new();
        for i in 0..5 {
            pages.push_transcript(format!("t{i}"), false);
            pages.push_sidebar(format!("s{i}"), false);
        }
        pages.scroll_transcript_up(2);
        let t: Vec<&str> =
            pages.transcript_window(2).iter().map(|l| l.text.as_str()).collect();
        let s: Vec<&str> = pages.sidebar_window(2).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(t, vec!["t1", "t2"]);
        assert_eq!(s, vec!["s3", "s4"]);
    }

    #[test]
    fn page_buffers_respect_bounds() {
        let mut pages = ShellPages::new();
        for i in 0..(MAX_LINES + 10) {
            pages.push_transcript(format!("line{i}"), false);
        }
        assert_eq!(pages.transcript_len(), MAX_LINES);
        assert_eq!(pages.transcript_window(1)[0].text, format!("line{}", MAX_LINES + 9));
        pages.push_composer("z".repeat(MAX_LINE + 5), false);
        assert_eq!(pages.composer_window(1)[0].text.chars().count(), MAX_LINE);
    }

    #[test]
    fn page_clear_resets_region() {
        let mut pages = ShellPages::new();
        pages.push_transcript("t", false);
        pages.scroll_transcript_up(1);
        pages.clear_transcript();
        assert!(pages.transcript_window(10).is_empty());
        assert_eq!(pages.transcript_len(), 0);
    }
}

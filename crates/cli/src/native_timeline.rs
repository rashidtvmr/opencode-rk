#![forbid(unsafe_code)]
//! Timeline view model: stream accumulators + tool states → bounded,
//! chronologically sorted timeline items for virtualized paint.
//!
//! Pure-state transform. No I/O. std only.

use std::collections::VecDeque;

/// Max lines retained in the virtualized activity page window.
pub const MAX_PAGE: usize = 200;

/// Max items retained in the timeline window.
pub const MAX_ITEMS: usize = 512;
/// Max bytes in one preview string (char-boundary safe).
pub const MAX_PREVIEW_BYTES: usize = 160;

/// Classification of a timeline entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Message,
    Tool,
    Reasoning,
}

/// One entry in the rendered timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineItem {
    pub kind: ItemKind,
    pub stream_id: u64,
    pub preview: String,
}

/// Lifecycle of one tool call shown in the timeline.
/// Mirrors `native_transcript::ToolState` for standalone compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolState {
    Approval,
    Running,
    Failed,
    Completed,
}

/// Per-stream input to the builder.
pub struct StreamState {
    pub stream_id: u64,
    pub text: String,
    pub tool_state: Option<ToolState>,
    pub is_reasoning: bool,
}

/// Accumulates stream states into a bounded timeline view model.
pub struct TimelineBuilder {
    items: VecDeque<TimelineItem>,
}

impl TimelineBuilder {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    /// Build a bounded timeline from stream states.
    ///
    /// Each stream produces at most one `TimelineItem`. Streams with tool
    /// states are classified as `Tool`; streams flagged as reasoning as
    /// `Reasoning`; all others as `Message`. Items are sorted by
    /// `stream_id` for chronological ordering. Preview is bounded to
    /// `MAX_PREVIEW_BYTES` on a UTF-8 char boundary. The deque is
    /// capped at `MAX_ITEMS` (oldest evicted first).
    pub fn from_streams<I>(streams: I) -> Self
    where
        I: IntoIterator<Item = StreamState>,
    {
        let mut builder = Self::new();

        let mut states: Vec<StreamState> = streams.into_iter().collect();
        states.sort_by_key(|s| s.stream_id);

        // Group consecutive tool states per stream: keep only the last
        // entry per stream_id (already adjacent after sort).
        let mut deduped: Vec<&StreamState> = Vec::new();
        for state in &states {
            match deduped.last() {
                Some(last) if last.stream_id == state.stream_id => {
                    *deduped.last_mut().unwrap() = state;
                }
                _ => deduped.push(state),
            }
        }

        for state in &deduped {
            let kind = classify(state);
            let preview = bound_preview(&state.text);
            builder.push_back(TimelineItem {
                kind,
                stream_id: state.stream_id,
                preview,
            });
        }

        builder
    }

    /// Push a new item, evicting the oldest if at capacity.
    pub fn push_back(&mut self, item: TimelineItem) {
        if self.items.len() >= MAX_ITEMS {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    /// The newest-window render slice for virtualized paint.
    /// Returns at most `limit` items from the front of the deque.
    pub fn render_slice(&self, limit: usize) -> VecDeque<&TimelineItem> {
        self.items.iter().take(limit).collect()
    }

    /// Snapshot the full bounded window.
    pub fn snapshot(&self) -> VecDeque<TimelineItem> {
        self.items.clone()
    }

    /// Scrollback-stable marker: does the item at `stream_id` exist?
    pub fn marker_stable(&self, stream_id: u64) -> bool {
        self.items.iter().any(|i| i.stream_id == stream_id)
    }

    /// Pinned-to-bottom window: last `height` items offset by `page` scroll.
    /// `height` hard-capped to `MAX_PAGE`. A scrolled-up view is frozen at
    /// the anchor recorded when leaving the pinned state, so pushes do not
    /// move it; [`TimelinePage::reset`] (or scrolling to `0`) re-pins to
    /// bottom. Empty when `height == 0` or no items. Mirrors
    /// `native_transcript::Transcript::page`.
    pub fn page(&self, page: &TimelinePage, height: usize) -> Vec<&TimelineItem> {
        let height = height.min(MAX_PAGE);
        if height == 0 {
            return Vec::new();
        }
        let len = self.items.len();
        let frozen = page.anchor.min(len);
        let end = if page.scroll == 0 {
            len
        } else {
            frozen.saturating_sub(page.scroll.min(frozen))
        };
        let start = end.saturating_sub(height);
        self.items.iter().skip(start).take(end - start).collect()
    }

    /// [`page`](Self::page) rendered to wrapped rows at `width` chars.
    pub fn render_page(&self, page: &TimelinePage, height: usize, width: usize) -> Vec<String> {
        let owned: Vec<TimelineItem> = self.page(page, height).into_iter().cloned().collect();
        render_lines(&owned, width)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Pinned-to-bottom scroll state for [`TimelineBuilder::page`].
///
/// `scroll == 0` follows the latest item; scrolling up freezes the view so
/// arriving streams do not move it. The freeze is positional: leaving the
/// pinned state records `anchor` (absolute end index at that moment) and
/// [`TimelineBuilder::page`] renders `anchor - scroll`, so pushes that grow
/// the deque do not shift the visible window. Pure state: caller supplies
/// lengths. Mirrors `native_transcript::TranscriptPage`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimelinePage {
    scroll: usize,
    anchor: usize,
}

impl TimelinePage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up (away from latest) by `delta`, clamped to `len`.
    /// Leaving the pinned state records the absolute end index so the
    /// frozen view stays stable across pushes.
    pub fn scroll_up(&mut self, delta: usize, len: usize) {
        if self.scroll == 0 {
            self.anchor = len;
        }
        let cap = self.anchor.min(len).max(self.scroll);
        self.scroll = self.scroll.saturating_add(delta).min(cap);
    }

    /// Scroll down (toward latest) by `delta`. Reaching `0` re-pins.
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
        if self.scroll == 0 {
            self.anchor = 0;
        }
    }

    /// Re-pin to the latest item.
    pub fn reset(&mut self) {
        self.scroll = 0;
        self.anchor = 0;
    }

    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }
}

/// Label prefix for one timeline item kind (sidebar/activity paint).
fn kind_label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Message => "message",
        ItemKind::Tool => "tool",
        ItemKind::Reasoning => "reasoning",
    }
}

/// Render timeline items to wrapped rows, at most `width` chars per row.
///
/// Splits on `\n`, word-wraps (oversize words chunked), `width == 0`
/// disables wrapping (one row per source line, kind-prefixed).
/// Pure/bounded: input capped to `MAX_PAGE` items, output capped to
/// `MAX_PAGE` rows (tail kept). Mirrors `native_transcript::render_lines`.
#[must_use]
pub fn render_lines(items: &[TimelineItem], width: usize) -> Vec<String> {
    fn push_wrapped(out: &mut Vec<String>, paragraph: &str, width: usize) {
        if out.len() >= MAX_PAGE {
            return;
        }
        if paragraph.chars().count() <= width {
            out.push(paragraph.to_owned());
            return;
        }
        let mut line = String::new();
        let mut line_len = 0usize;
        for word in paragraph.split_whitespace() {
            if out.len() >= MAX_PAGE {
                return;
            }
            let wlen = word.chars().count();
            if wlen > width {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    line_len = 0;
                    if out.len() >= MAX_PAGE {
                        return;
                    }
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(width) {
                    out.push(chunk.iter().collect());
                    if out.len() >= MAX_PAGE {
                        return;
                    }
                }
                continue;
            }
            if line.is_empty() {
                line.push_str(word);
                line_len = wlen;
            } else if line_len + 1 + wlen <= width {
                line.push(' ');
                line.push_str(word);
                line_len += 1 + wlen;
            } else {
                out.push(std::mem::take(&mut line));
                if out.len() >= MAX_PAGE {
                    line.push_str(word);
                    line_len = wlen;
                    return;
                }
                line.push_str(word);
                line_len = wlen;
            }
        }
        if !line.is_empty() && out.len() < MAX_PAGE {
            out.push(line);
        }
    }
    let items = if items.len() > MAX_PAGE {
        &items[items.len() - MAX_PAGE..]
    } else {
        items
    };
    let mut out = Vec::new();
    for item in items {
        let base = format!("{}: {}", kind_label(item.kind), item.preview);
        if width == 0 {
            for row in base.split('\n') {
                if out.len() >= MAX_PAGE {
                    break;
                }
                out.push(row.to_owned());
            }
        } else {
            for para in base.split('\n') {
                push_wrapped(&mut out, para, width);
                if out.len() >= MAX_PAGE {
                    break;
                }
            }
        }
        if out.len() >= MAX_PAGE {
            break;
        }
    }
    out
}

/// Classify a stream into an `ItemKind`.
fn classify(state: &StreamState) -> ItemKind {
    if state.tool_state.is_some() {
        return ItemKind::Tool;
    }
    if state.is_reasoning {
        return ItemKind::Reasoning;
    }
    ItemKind::Message
}

/// Truncate text to `MAX_PREVIEW_BYTES` on a UTF-8 char boundary.
fn bound_preview(text: &str) -> String {
    if text.len() <= MAX_PREVIEW_BYTES {
        return text.to_string();
    }
    let mut end = MAX_PREVIEW_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T01: message + tool + reasoning classified correctly.
    #[test]
    fn t01_classify_message_tool_reasoning() {
        let builder = TimelineBuilder::from_streams(vec![
            StreamState {
                stream_id: 1,
                text: "hello".into(),
                tool_state: None,
                is_reasoning: false,
            },
            StreamState {
                stream_id: 2,
                text: "tool output".into(),
                tool_state: Some(ToolState::Running),
                is_reasoning: false,
            },
            StreamState {
                stream_id: 3,
                text: "thinking...".into(),
                tool_state: None,
                is_reasoning: true,
            },
        ]);
        let snap = builder.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].kind, ItemKind::Message);
        assert_eq!(snap[0].stream_id, 1);
        assert_eq!(snap[1].kind, ItemKind::Tool);
        assert_eq!(snap[1].stream_id, 2);
        assert_eq!(snap[2].kind, ItemKind::Reasoning);
        assert_eq!(snap[2].stream_id, 3);
    }

    /// T02: bounded window at MAX_ITEMS, oldest evicted.
    #[test]
    fn t02_bounded_window_evicts_oldest() {
        let streams: Vec<StreamState> = (0..=MAX_ITEMS)
            .map(|i| StreamState {
                stream_id: i as u64,
                text: format!("item {i}"),
                tool_state: None,
                is_reasoning: false,
            })
            .collect();
        let builder = TimelineBuilder::from_streams(streams);
        assert_eq!(builder.len(), MAX_ITEMS);
        let snap = builder.snapshot();
        // Oldest (stream_id 0) evicted.
        assert_eq!(snap[0].stream_id, 1);
        assert_eq!(snap[snap.len() - 1].stream_id, MAX_ITEMS as u64);
    }

    /// T03: preview truncation is char-boundary safe.
    #[test]
    fn t03_preview_truncation_char_boundary() {
        // 4-byte UTF-8 char ('管理体系' repeated).
        let long = "管理体系".repeat(80);
        assert!(long.len() > MAX_PREVIEW_BYTES);
        let builder = TimelineBuilder::from_streams(vec![StreamState {
            stream_id: 1,
            text: long.clone(),
            tool_state: None,
            is_reasoning: false,
        }]);
        let snap = builder.snapshot();
        let preview = &snap[0].preview;
        assert!(preview.len() <= MAX_PREVIEW_BYTES);
        // Must be valid UTF-8.
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
        // Must not panic — already validated above.
        // First 160 bytes of original that are char-aligned.
        let mut expected_end = MAX_PREVIEW_BYTES;
        while !long.is_char_boundary(expected_end) {
            expected_end -= 1;
        }
        assert_eq!(preview, &long[..expected_end]);
    }

    /// T04: tool grouping — consecutive tool states per stream produce one item.
    #[test]
    fn t04_tool_grouping_per_stream() {
        // Multiple tool state transitions on the same stream → single Tool item.
        let builder = TimelineBuilder::from_streams(vec![
            StreamState {
                stream_id: 5,
                text: "approved".into(),
                tool_state: Some(ToolState::Approval),
                is_reasoning: false,
            },
            StreamState {
                stream_id: 5,
                text: "running".into(),
                tool_state: Some(ToolState::Running),
                is_reasoning: false,
            },
            StreamState {
                stream_id: 5,
                text: "done".into(),
                tool_state: Some(ToolState::Completed),
                is_reasoning: false,
            },
        ]);
        let snap = builder.snapshot();
        // Deduplication: same stream_id → last one wins via push_back.
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].stream_id, 5);
        assert_eq!(snap[0].kind, ItemKind::Tool);
        assert_eq!(snap[0].preview, "done");
    }

    /// T05: stable under out-of-order stream_ids (sorted internally).
    #[test]
    fn t05_stable_under_out_of_order() {
        let builder = TimelineBuilder::from_streams(vec![
            StreamState {
                stream_id: 10,
                text: "late".into(),
                tool_state: None,
                is_reasoning: false,
            },
            StreamState {
                stream_id: 2,
                text: "early".into(),
                tool_state: None,
                is_reasoning: false,
            },
            StreamState {
                stream_id: 7,
                text: "mid".into(),
                tool_state: None,
                is_reasoning: false,
            },
        ]);
        let snap = builder.snapshot();
        assert_eq!(snap[0].stream_id, 2);
        assert_eq!(snap[1].stream_id, 7);
        assert_eq!(snap[2].stream_id, 10);
    }

    /// T06: empty input → empty output.
    #[test]
    fn t06_empty_input_empty_output() {
        let builder = TimelineBuilder::from_streams(vec![]);
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert!(builder.snapshot().is_empty());
        assert!(builder.render_slice(10).is_empty());
        assert!(!builder.marker_stable(0));
    }

    /// T07: activity page pins to bottom; scrolled-up view stable across pushes.
    #[test]
    fn t07_page_pins_to_bottom_and_scrolls() {
        let mut builder = TimelineBuilder::new();
        for i in 0..5 {
            builder.push_back(TimelineItem {
                kind: ItemKind::Message,
                stream_id: i,
                preview: format!("line {i}"),
            });
        }
        let vis: Vec<u64> = builder
            .page(&TimelinePage::new(), 2)
            .iter()
            .map(|l| l.stream_id)
            .collect();
        assert_eq!(vis, vec![3, 4]);
        let mut page = TimelinePage::new();
        page.scroll_up(2, builder.len());
        assert_eq!(page.scroll(), 2);
        let vis: Vec<u64> = builder
            .page(&page, 2)
            .iter()
            .map(|l| l.stream_id)
            .collect();
        assert_eq!(vis, vec![1, 2]);
        // New arrivals must not move a scrolled-up view.
        builder.push_back(TimelineItem {
            kind: ItemKind::Message,
            stream_id: 5,
            preview: "line 5".into(),
        });
        let vis: Vec<u64> = builder
            .page(&page, 2)
            .iter()
            .map(|l| l.stream_id)
            .collect();
        assert_eq!(vis, vec![1, 2]);
        page.scroll_down(10);
        assert_eq!(page.scroll(), 0);
    }

    /// T08: page height capped and empty safe.
    #[test]
    fn t08_page_height_capped_and_empty_safe() {
        let builder = TimelineBuilder::new();
        let page = TimelinePage::new();
        assert!(builder.page(&page, 10).is_empty());
        assert!(builder.page(&page, 0).is_empty());
        let mut builder = TimelineBuilder::new();
        for i in 0..(MAX_PAGE + 50) {
            builder.push_back(TimelineItem {
                kind: ItemKind::Message,
                stream_id: i as u64,
                preview: format!("l{i}"),
            });
        }
        assert_eq!(builder.page(&TimelinePage::new(), 10_000).len(), MAX_PAGE);
    }

    /// T09: scroll clamps to len.
    #[test]
    fn t09_scroll_clamps_to_len() {
        let mut p = TimelinePage::new();
        p.scroll_up(100, 3);
        assert_eq!(p.scroll(), 3);
        p.scroll_up(10, 3);
        assert_eq!(p.scroll(), 3);
        p.reset();
        assert_eq!(p.scroll(), 0);
    }

    /// T10: render lines wraps and bounds.
    #[test]
    fn t10_render_lines_wraps_and_bounds() {
        let items = vec![TimelineItem {
            kind: ItemKind::Message,
            stream_id: 0,
            preview: "hello world foo".into(),
        }];
        let rows = render_lines(&items, 5);
        assert!(rows.join("|").contains("hello"));
        for r in &rows {
            assert!(r.chars().count() <= 5, "row overflow: {r:?}");
        }
        assert_eq!(
            render_lines(&items, 0),
            vec!["message: hello world foo".to_owned()]
        );
        let items = vec![TimelineItem {
            kind: ItemKind::Tool,
            stream_id: 1,
            preview: "a\nb".into(),
        }];
        assert_eq!(
            render_lines(&items, 0),
            vec!["tool: a".to_owned(), "b".to_owned()]
        );
        let big: Vec<TimelineItem> = (0..500)
            .map(|i| TimelineItem {
                kind: ItemKind::Message,
                stream_id: i,
                preview: "x".repeat(100),
            })
            .collect();
        assert!(render_lines(&big, 10).len() <= MAX_PAGE);
    }

    /// T11: render page end to end.
    #[test]
    fn t11_render_page_end_to_end() {
        let builder = TimelineBuilder::from_streams(vec![StreamState {
            stream_id: 1,
            text: "alpha beta gamma".into(),
            tool_state: None,
            is_reasoning: false,
        }]);
        let rows = builder.render_page(&TimelinePage::new(), 10, 5);
        assert!(!rows.is_empty());
        for r in &rows {
            assert!(r.chars().count() <= 5, "row overflow: {r:?}");
        }
    }
}

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

/// Compaction usage is the request's own token accounting. It is **not** the
/// resulting context size, which only appears on the next assistant step.
pub const COMPACTION_USAGE_SEMANTICS: &str =
    "usage of the compaction request, not the resulting context size (shown on the next assistant step)";

/// Compaction lifecycle from `routes/session/index.tsx` (`CompactionMessage`,
/// `CompactionQueued`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

/// Token counters attached to a compaction request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompactionTokens {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub reasoning: u64,
}

impl CompactionTokens {
    /// `input + cache.read + cache.write`, `output + reasoning`.
    #[must_use]
    pub fn request_usage(&self) -> (u64, u64) {
        (
            self.input
                .saturating_add(self.cache_read)
                .saturating_add(self.cache_write),
            self.output.saturating_add(self.reasoning),
        )
    }
}

/// Pure compaction row state: header label, streaming summary, failure text,
/// cancellation and request usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionState {
    status: CompactionStatus,
    cancelled: bool,
    provider_native: bool,
    summary: String,
    error: String,
    tokens: Option<CompactionTokens>,
}

impl Default for CompactionState {
    fn default() -> Self {
        Self::queued()
    }
}

impl CompactionState {
    #[must_use]
    pub fn queued() -> Self {
        Self {
            status: CompactionStatus::Queued,
            cancelled: false,
            provider_native: false,
            summary: String::new(),
            error: String::new(),
            tokens: None,
        }
    }

    #[must_use]
    pub fn running() -> Self {
        Self {
            status: CompactionStatus::Running,
            ..Self::queued()
        }
    }

    #[must_use]
    pub fn completed(summary: impl Into<String>, provider_native: bool) -> Self {
        Self {
            status: CompactionStatus::Completed,
            cancelled: false,
            provider_native,
            summary: summary.into(),
            error: String::new(),
            tokens: None,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: CompactionStatus::Failed,
            cancelled: false,
            provider_native: false,
            summary: String::new(),
            error: message.into(),
            tokens: None,
        }
    }

    /// Queued → running. Repeated or out-of-order transitions are rejected.
    pub fn start(&mut self) -> bool {
        if self.status != CompactionStatus::Queued {
            return false;
        }
        self.status = CompactionStatus::Running;
        true
    }

    /// Running → completed, carrying the final summary and provider-native
    /// checkpoint flag.
    pub fn complete(&mut self, summary: impl Into<String>, provider_native: bool) -> bool {
        if self.status != CompactionStatus::Running {
            return false;
        }
        self.status = CompactionStatus::Completed;
        self.cancelled = false;
        self.provider_native = provider_native;
        self.summary = summary.into();
        self.error.clear();
        true
    }

    /// Running → failed with the error message shown in the body.
    pub fn fail(&mut self, message: impl Into<String>) -> bool {
        if self.status != CompactionStatus::Running {
            return false;
        }
        self.status = CompactionStatus::Failed;
        self.cancelled = false;
        self.provider_native = false;
        self.error = message.into();
        true
    }

    /// Running → failed with `error.type === "aborted"`: no icon, empty body,
    /// and a `· cancelled` suffix.
    pub fn cancel(&mut self) -> bool {
        if self.status != CompactionStatus::Running {
            return false;
        }
        self.status = CompactionStatus::Failed;
        self.cancelled = true;
        self.provider_native = false;
        self.error.clear();
        true
    }

    pub fn set_tokens(&mut self, tokens: CompactionTokens) {
        self.tokens = Some(tokens);
    }

    /// Replace the streaming summary (partial text is rendered as it arrives).
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = summary.into();
    }

    #[must_use]
    pub fn status(&self) -> CompactionStatus {
        self.status
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    #[must_use]
    pub fn provider_native(&self) -> bool {
        self.provider_native
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }

    #[must_use]
    pub fn tokens(&self) -> Option<CompactionTokens> {
        self.tokens
    }

    /// Header text after the lifecycle icon.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.status == CompactionStatus::Queued {
            return "Compaction queued";
        }
        if self.status == CompactionStatus::Completed && self.provider_native {
            "Provider compaction"
        } else {
            "Compaction"
        }
    }

    /// Leading glyph: `◇` queued, `✗` failed, spinner supplied by the caller
    /// while running, none for completed or cancelled.
    #[must_use]
    pub fn icon<'a>(&self, running_glyph: Option<&'a str>) -> Option<&'a str> {
        match self.status {
            CompactionStatus::Queued => Some("◇"),
            CompactionStatus::Running => running_glyph.filter(|glyph| !glyph.is_empty()),
            CompactionStatus::Failed if !self.cancelled => Some("✗"),
            CompactionStatus::Completed | CompactionStatus::Failed => None,
        }
    }

    /// Request usage shown only after running, with cache reads/writes folded
    /// into input and reasoning into output.
    #[must_use]
    pub fn usage(&self) -> Option<(u64, u64)> {
        if self.status == CompactionStatus::Running {
            return None;
        }
        let (input, output) = self.tokens?.request_usage();
        if input.saturating_add(output) == 0 {
            return None;
        }
        Some((input, output))
    }

    #[must_use]
    pub fn usage_label(&self) -> Option<String> {
        let (input, output) = self.usage()?;
        Some(format!(
            "{} in · {} out",
            compact_number(input),
            compact_number(output)
        ))
    }

    /// Rendered body: streaming summary until failure, the error message for
    /// an ordinary failure, and nothing when cancelled.
    #[must_use]
    pub fn content(&self) -> &str {
        match self.status {
            CompactionStatus::Failed if self.cancelled => "",
            CompactionStatus::Failed => self.error.trim(),
            CompactionStatus::Queued | CompactionStatus::Running | CompactionStatus::Completed => {
                self.summary.trim()
            }
        }
    }

    /// Centered top-rule header, matching the `border={["top"]}` /
    /// `flexGrow={1}` segments on both sides of the label group.
    #[must_use]
    pub fn header(&self, running_glyph: Option<&str>, width: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let mut text = String::new();
        if let Some(icon) = self.icon(running_glyph) {
            text.push_str(icon);
            text.push(' ');
        }
        text.push_str(self.label());
        if self.cancelled {
            text.push_str(" · cancelled");
        }
        if let Some(usage) = self.usage_label() {
            text.push_str(" · ");
            text.push_str(&usage);
        }
        center_rule(&text, width)
    }

    /// Header plus optional indented summary/error body, bounded to
    /// [`MAX_PAGE`] rows.
    #[must_use]
    pub fn render(&self, running_glyph: Option<&str>, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        let mut rows = vec![self.header(running_glyph, width)];
        let content = self.content();
        if content.is_empty() {
            return rows;
        }
        rows.push(String::new());
        let indent = if width > 3 { 3 } else { 0 };
        let wrap_width = width.saturating_sub(indent).max(1);
        let mut body = Vec::new();
        for line in content.split('\n') {
            wrap_line(line, wrap_width, &mut body);
            if body.len() >= MAX_PAGE {
                break;
            }
        }
        for line in body.into_iter().take(MAX_PAGE.saturating_sub(rows.len())) {
            if indent == 0 {
                rows.push(line);
            } else {
                rows.push(format!("{}{line}", " ".repeat(indent)));
            }
        }
        rows
    }
}

fn center_rule(text: &str, width: usize) -> String {
    let inner = format!(" {text} ");
    let count = inner.chars().count();
    if width <= count {
        return inner.chars().take(width).collect();
    }
    let fill = width - count;
    let left = fill / 2;
    let right = fill - left;
    format!("{}{inner}{}", "─".repeat(left), "─".repeat(right))
}

fn wrap_line(line: &str, width: usize, out: &mut Vec<String>) {
    if width == 0 {
        return;
    }
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let mut current = String::new();
    let mut current_len = 0usize;
    for word in line.split_whitespace() {
        if out.len() >= MAX_PAGE {
            return;
        }
        let word_len = word.chars().count();
        if word_len > width {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_len = 0;
            }
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(width) {
                if out.len() >= MAX_PAGE {
                    return;
                }
                out.push(chunk.iter().collect());
            }
            continue;
        }
        if current.is_empty() {
            current.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = word_len;
        }
    }
    if !current.is_empty() && out.len() < MAX_PAGE {
        out.push(current);
    }
}

/// `Locale.number` used by the compaction usage line.
fn compact_number(value: u64) -> String {
    if value >= 999_950 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
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
        let vis: Vec<u64> = builder.page(&page, 2).iter().map(|l| l.stream_id).collect();
        assert_eq!(vis, vec![1, 2]);
        // New arrivals must not move a scrolled-up view.
        builder.push_back(TimelineItem {
            kind: ItemKind::Message,
            stream_id: 5,
            preview: "line 5".into(),
        });
        let vis: Vec<u64> = builder.page(&page, 2).iter().map(|l| l.stream_id).collect();
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

    #[test]
    fn t12_compaction_transitions_queued_running_completed() {
        let mut state = CompactionState::queued();
        assert_eq!(state.status(), CompactionStatus::Queued);
        assert!(state.start());
        assert_eq!(state.status(), CompactionStatus::Running);
        assert!(!state.start(), "running cannot restart");
        assert!(state.complete("summary text", true));
        assert_eq!(state.status(), CompactionStatus::Completed);
        assert!(state.provider_native());
        assert_eq!(state.label(), "Provider compaction");
        assert_eq!(state.content(), "summary text");
        assert!(!state.complete("again", false), "completed is terminal");
        assert!(!state.fail("late error"));
        assert_eq!(state.status(), CompactionStatus::Completed);
    }

    #[test]
    fn t13_compaction_failed_and_cancelled_states() {
        let mut failed = CompactionState::running();
        assert!(failed.fail("provider exploded"));
        assert_eq!(failed.status(), CompactionStatus::Failed);
        assert!(!failed.cancelled());
        assert_eq!(failed.content(), "provider exploded");
        let header = failed.header(None, 40);
        assert!(header.contains("✗ Compaction"), "header={header:?}");

        let mut cancelled = CompactionState::running();
        assert!(cancelled.cancel());
        assert_eq!(cancelled.status(), CompactionStatus::Failed);
        assert!(cancelled.cancelled());
        assert_eq!(cancelled.content(), "", "aborted body is empty");
        let header = cancelled.header(None, 40);
        assert!(
            header.contains("Compaction · cancelled"),
            "header={header:?}"
        );
        assert!(!header.contains('✗'));
        assert!(!cancelled.cancel(), "cancelled state is terminal");
    }

    #[test]
    fn t14_compaction_queued_row_and_header_geometry() {
        let queued = CompactionState::queued();
        assert_eq!(queued.label(), "Compaction queued");
        let header = queued.header(Some("⠋"), 40);
        assert_eq!(header.chars().count(), 40);
        assert!(header.starts_with('─') && header.ends_with('─'));
        assert!(header.contains("◇ Compaction queued"));
        assert!(
            !header.contains('⠋'),
            "queued shows a diamond, not a spinner"
        );

        let running = CompactionState::running();
        let header = running.header(Some("⠋"), 40);
        assert_eq!(header.chars().count(), 40);
        assert!(header.contains("⠋ Compaction"), "header={header:?}");
        let off = running.header(Some("⋯"), 40);
        assert!(off.contains("⋯ Compaction"));
    }

    #[test]
    fn t15_compaction_usage_uses_request_tokens_not_context_size() {
        let mut state = CompactionState::running();
        state.set_tokens(CompactionTokens {
            input: 1_000,
            cache_read: 500,
            cache_write: 0,
            output: 2_000,
            reasoning: 0,
        });
        assert_eq!(state.usage(), None, "usage is hidden while running");
        assert!(!state.header(Some("⠋"), 60).contains(" in · "));
        state.complete("done", false);
        assert_eq!(state.usage(), Some((1_500, 2_000)));
        assert_eq!(state.usage_label().as_deref(), Some("1.5K in · 2.0K out"));
        let header = state.header(None, 80);
        assert!(header.contains("· 1.5K in · 2.0K out"), "header={header:?}");
        assert!(!header.contains("context"));
        assert!(COMPACTION_USAGE_SEMANTICS.contains("not"));

        let mut zero = CompactionState::completed("done", false);
        zero.set_tokens(CompactionTokens::default());
        assert_eq!(zero.usage(), None, "all-zero counters are omitted");
    }

    #[test]
    fn t16_compaction_streaming_summary_and_bounded_render() {
        let mut state = CompactionState::running();
        state.set_summary("partial summary");
        assert_eq!(state.content(), "partial summary");
        let rows = state.render(Some("⠋"), 30);
        assert!(
            rows.iter().any(|row| row.starts_with("   partial")),
            "{rows:?}"
        );
        state.set_summary("final streaming summary");
        assert_eq!(state.content(), "final streaming summary");
        assert!(state.complete("final streaming summary", false));
        for row in state.render(None, 16) {
            assert!(row.chars().count() <= 16, "overflow: {row:?}");
        }

        let mut failed = CompactionState::running();
        failed.fail("failure body");
        let rows = failed.render(None, 40);
        assert!(rows.iter().any(|row| row.trim() == "failure body"));
        let cancelled = {
            let mut state = CompactionState::running();
            state.cancel();
            state
        };
        assert_eq!(
            cancelled.render(None, 40).len(),
            1,
            "no body when cancelled"
        );
    }

    #[test]
    fn t17_compaction_provider_native_only_labels_completed_rows() {
        let running = CompactionState::running();
        assert_eq!(running.label(), "Compaction");
        let completed = CompactionState::completed("summary", true);
        assert_eq!(completed.label(), "Provider compaction");
        let fallback = CompactionState::completed("summary", false);
        assert_eq!(fallback.label(), "Compaction");
        let failed = CompactionState::failed("boom");
        assert_eq!(failed.label(), "Compaction");
    }
}

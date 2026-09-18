#![forbid(unsafe_code)]
//! Timeline view model: stream accumulators + tool states → bounded,
//! chronologically sorted timeline items for virtualized paint.
//!
//! Pure-state transform. No I/O. std only.

use std::collections::VecDeque;

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

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
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
}

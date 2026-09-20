#![forbid(unsafe_code)]
//! Native transcript types: streaming token deltas, tool states, hostile
//! output sanitization, bounded virtualized window, scrollback-stable marker.
//!
//! std only. All retained output bounded.

/// Max bytes retained per stream's assembled text.
pub const MAX_STREAM_BYTES: usize = 64 * 1024;
/// Max pending out-of-order deltas per stream.
pub const MAX_PENDING_DELTAS: usize = 64;
/// Max bytes in one delta's text.
pub const MAX_DELTA_BYTES: usize = 8 * 1024;
/// Max scrollback lines retained.
pub const MAX_SCROLLBACK: usize = 1_000;
/// Max lines per virtualized window.
pub const MAX_WINDOW: usize = 200;
/// Max bytes kept from sanitized output.
pub const MAX_SANITIZED_BYTES: usize = 64 * 1024;

/// One partial token chunk on a stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenDelta {
    pub stream_id: u64,
    pub seq: u64,
    pub text: String,
}

impl TokenDelta {
    pub fn new(stream_id: u64, seq: u64, text: impl Into<String>) -> Self {
        let mut t: String = text.into();
        if t.len() > MAX_DELTA_BYTES {
            let mut end = MAX_DELTA_BYTES;
            while !t.is_char_boundary(end) {
                end -= 1;
            }
            t.truncate(end);
        }
        Self { stream_id, seq, text: t }
    }
}

/// Lifecycle of one tool call shown in the transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolState {
    Approval,
    Running,
    Failed,
    Completed,
}

impl ToolState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::Completed)
    }

    /// Allowed transitions; terminal states have none.
    pub fn can_transition(self, next: Self) -> bool {
        match (self, next) {
            (Self::Approval, Self::Running | Self::Approval) => true,
            (Self::Running, Self::Failed | Self::Completed) => true,
            (Self::Running, Self::Running) => true,
            _ => false,
        }
    }
}

/// Accumulates deltas for one stream; out-of-order partials converge.
#[derive(Clone, Debug, Default)]
pub struct StreamAccumulator {
    stream_id: u64,
    next_seq: u64,
    pending: Vec<(u64, String)>,
    text: String,
    dropped_bytes: u64,
}

impl StreamAccumulator {
    pub fn new(stream_id: u64) -> Self {
        Self { stream_id, next_seq: 0, pending: Vec::new(), text: String::new(), dropped_bytes: 0 }
    }

    /// Push a delta. Wrong stream ignored (returns false). Duplicates/stale
    /// ignored. Over-cap pending ignored. Returns true if new data buffered.
    pub fn push(&mut self, delta: &TokenDelta) -> bool {
        if delta.stream_id != self.stream_id {
            return false;
        }
        if delta.seq < self.next_seq {
            return false;
        }
        if delta.seq == self.next_seq {
            self.append(&delta.text);
            self.next_seq += 1;
            self.drain_pending();
            return true;
        }
        if self.pending.iter().any(|(s, _)| *s == delta.seq) {
            return false;
        }
        if self.pending.len() >= MAX_PENDING_DELTAS {
            return false;
        }
        self.pending.push((delta.seq, delta.text.clone()));
        true
    }

    fn append(&mut self, chunk: &str) {
        let room = MAX_STREAM_BYTES.saturating_sub(self.text.len());
        if room == 0 {
            self.dropped_bytes += chunk.len() as u64;
            return;
        }
        if chunk.len() <= room {
            self.text.push_str(chunk);
        } else {
            let mut end = room;
            while !chunk.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&chunk[..end]);
            self.dropped_bytes += (chunk.len() - end) as u64;
        }
    }

    fn drain_pending(&mut self) {
        loop {
            let pos = self.pending.iter().position(|(s, _)| *s == self.next_seq);
            match pos {
                Some(i) => {
                    let (_, chunk) = self.pending.swap_remove(i);
                    self.append(&chunk);
                    self.next_seq += 1;
                }
                None => break,
            }
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes
    }
}

/// Strip ANSI CSI/SGR, OSC (incl. hostile OSC-8 hyperlinks), lone ESC/C1, and
/// C0 controls except `\n` and `\t`. Terminator-less OSC is swallowed to end
/// of input (fail closed). Output truncated to `MAX_SANITIZED_BYTES`.
pub fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(input.len().min(MAX_SANITIZED_BYTES));
    let mut i = 0;
    while i < bytes.len() && out.len() < MAX_SANITIZED_BYTES {
        let b = bytes[i];
        if b == 0x1b {
            // ESC-prefixed sequence.
            if i + 1 >= bytes.len() {
                i += 1;
                continue;
            }
            let n = bytes[i + 1];
            match n {
                b'[' => {
                    // CSI: ESC [ params intermediates final(@..~).
                    i += 2;
                    while i < bytes.len() {
                        let c = bytes[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&c) {
                            break;
                        }
                    }
                }
                b']' => {
                    // OSC: ESC ] ... (BEL | ESC \ | end-of-input).
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                b'P' | b'X' | b'^' | b'_' => {
                    // DCS/SOS/PM/APC: swallow to ST or BEL.
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                b'(' | b')' | b'#' => {
                    i += 3.min(bytes.len() - i);
                }
                _ => {
                    // Lone ESC + single char: drop both.
                    i += 2;
                }
            }
            continue;
        }
        if (b == 0x9b) || (b == 0xc2 && i + 1 < bytes.len() && bytes[i + 1] == 0x9b) {
            // C1 CSI (single byte or UTF-8 C2 9B).
            i += if b == 0x9b { 1 } else { 2 };
            while i < bytes.len() {
                let c = bytes[i];
                i += 1;
                if (0x40..=0x7e).contains(&c) {
                    break;
                }
            }
            continue;
        }
        if (b == 0x9d) || (b == 0xc2 && i + 1 < bytes.len() && bytes[i + 1] == 0x9d) {
            // C1 OSC.
            i += if b == 0x9d { 1 } else { 2 };
            while i < bytes.len() {
                if bytes[i] == 0x07 {
                    i += 1;
                    break;
                }
                if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b < 0x20 && b != b'\n' && b != b'\t' {
            // Other C0 controls (incl. BEL, \r): drop.
            i += 1;
            continue;
        }
        if b == 0x7f {
            i += 1;
            continue;
        }
        // Copy one UTF-8 scalar.
        let s = &input[i..];
        match s.chars().next() {
            Some(ch) => {
                let len = ch.len_utf8();
                if out.len() + len > MAX_SANITIZED_BYTES {
                    break;
                }
                out.extend_from_slice(&bytes[i..i + len]);
                i += len;
            }
            None => break,
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// One transcript line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptLine {
    pub id: u64,
    pub text: String,
}

/// Bounded scrollback with a stable marker surviving truncation.
#[derive(Clone, Debug, Default)]
pub struct Transcript {
    lines: Vec<TranscriptLine>,
    next_id: u64,
    dropped: u64,
    marker: Option<u64>,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push sanitized text; returns assigned line id.
    pub fn push(&mut self, text: &str) -> u64 {
        let clean = strip_ansi(text);
        let kept = if clean.len() > MAX_DELTA_BYTES {
            let mut end = MAX_DELTA_BYTES;
            while !clean.is_char_boundary(end) {
                end -= 1;
            }
            clean[..end].to_string()
        } else {
            clean
        };
        let id = self.next_id;
        self.next_id += 1;
        self.lines.push(TranscriptLine { id, text: kept });
        if self.lines.len() > MAX_SCROLLBACK {
            let overflow = self.lines.len() - MAX_SCROLLBACK;
            self.lines.drain(..overflow);
            self.dropped += overflow as u64;
        }
        id
    }

    /// Pin a scrollback-stable marker to a line id. Unknown ids rejected.
    pub fn set_marker(&mut self, id: u64) -> bool {
        if self.lines.iter().any(|l| l.id == id) {
            self.marker = Some(id);
            true
        } else {
            false
        }
    }

    pub fn marker(&self) -> Option<u64> {
        self.marker
    }

    /// Marker id still present in retained scrollback.
    pub fn marker_stable(&self) -> bool {
        match self.marker {
            Some(id) => self.lines.iter().any(|l| l.id == id),
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Virtualized window: `offset` lines from top of retained scrollback,
    /// at most `limit` lines, hard-capped to `MAX_WINDOW`.
    pub fn window(&self, offset: usize, limit: usize) -> &[TranscriptLine] {
        window_slice(&self.lines, offset, limit)
    }

    /// Pinned-to-bottom page: the last `height` retained lines, shifted up
    /// by `page` scroll offset. `height` hard-capped to `MAX_WINDOW`.
    /// A scrolled-up view is stable across pushes; [`TranscriptPage::reset`]
    /// re-pins to the bottom. Empty when `height == 0` or no lines.
    pub fn page(&self, page: &TranscriptPage, height: usize) -> &[TranscriptLine] {
        let height = height.min(MAX_WINDOW);
        if height == 0 {
            return &[];
        }
        let len = self.lines.len();
        let end = len.saturating_sub(page.scroll.min(len));
        let start = end.saturating_sub(height);
        &self.lines[start..end]
    }

    /// [`page`](Self::page) rendered to wrapped rows at `width` chars.
    pub fn render_page(&self, page: &TranscriptPage, height: usize, width: usize) -> Vec<String> {
        render_lines(self.page(page, height), width)
    }
}

/// Pinned-to-bottom scroll state for [`Transcript::page`].
///
/// `scroll == 0` follows the latest line; scrolling up freezes the view so
/// arriving tokens do not move it. Pure state: caller supplies lengths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TranscriptPage {
    scroll: usize,
}

impl TranscriptPage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up (away from latest) by `delta`, clamped to `len`.
    pub fn scroll_up(&mut self, delta: usize, len: usize) {
        self.scroll = self.scroll.saturating_add(delta).min(len);
    }

    /// Scroll down (toward latest) by `delta`.
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
    }

    /// Re-pin to the latest line.
    pub fn reset(&mut self) {
        self.scroll = 0;
    }

    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }
}

/// Render transcript lines to wrapped rows, at most `width` chars per row.
///
/// Splits on `\n`, word-wraps (oversize words chunked), `width == 0`
/// disables wrapping (one row per source line). Pure/bounded: input capped
/// to `MAX_WINDOW` lines, output capped to `MAX_WINDOW` rows (tail kept).
#[must_use]
pub fn render_lines(lines: &[TranscriptLine], width: usize) -> Vec<String> {
    fn push_wrapped(out: &mut Vec<String>, paragraph: &str, width: usize) {
        if out.len() >= MAX_WINDOW {
            return;
        }
        if paragraph.chars().count() <= width {
            out.push(paragraph.to_owned());
            return;
        }
        let mut line = String::new();
        let mut line_len = 0usize;
        for word in paragraph.split_whitespace() {
            if out.len() >= MAX_WINDOW {
                return;
            }
            let wlen = word.chars().count();
            if wlen > width {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    line_len = 0;
                    if out.len() >= MAX_WINDOW {
                        return;
                    }
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(width) {
                    out.push(chunk.iter().collect());
                    if out.len() >= MAX_WINDOW {
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
                if out.len() >= MAX_WINDOW {
                    line.push_str(word);
                    line_len = wlen;
                    return;
                }
                line.push_str(word);
                line_len = wlen;
            }
        }
        if !line.is_empty() && out.len() < MAX_WINDOW {
            out.push(line);
        }
    }
    let lines = if lines.len() > MAX_WINDOW { &lines[lines.len() - MAX_WINDOW..] } else { lines };
    let mut out = Vec::new();
    for l in lines {
        if width == 0 {
            for row in l.text.split('\n') {
                if out.len() >= MAX_WINDOW {
                    break;
                }
                out.push(row.to_owned());
            }
        } else {
            for para in l.text.split('\n') {
                push_wrapped(&mut out, para, width);
                if out.len() >= MAX_WINDOW {
                    break;
                }
            }
        }
        if out.len() >= MAX_WINDOW {
            break;
        }
    }
    out
}

/// Slice a virtualized window with offset/limit caps.
pub fn window_slice<T>(items: &[T], offset: usize, limit: usize) -> &[T] {
    let limit = limit.min(MAX_WINDOW);
    if limit == 0 || offset >= items.len() {
        return &[];
    }
    let end = (offset + limit).min(items.len());
    &items[offset..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partials_converge_in_order() {
        let mut acc = StreamAccumulator::new(7);
        assert!(acc.push(&TokenDelta::new(7, 0, "hel")));
        assert!(acc.push(&TokenDelta::new(7, 1, "lo ")));
        assert!(acc.push(&TokenDelta::new(7, 2, "world")));
        assert_eq!(acc.text(), "hello world");
        assert_eq!(acc.next_seq(), 3);
    }

    #[test]
    fn partials_converge_out_of_order_with_duplicate() {
        let mut acc = StreamAccumulator::new(1);
        assert!(acc.push(&TokenDelta::new(1, 2, "c")));
        assert!(acc.push(&TokenDelta::new(1, 0, "a")));
        assert_eq!(acc.text(), "a");
        // Duplicate of buffered seq ignored.
        assert!(!acc.push(&TokenDelta::new(1, 2, "c")));
        // Stale seq ignored.
        assert!(!acc.push(&TokenDelta::new(1, 0, "a")));
        assert!(acc.push(&TokenDelta::new(1, 1, "b")));
        assert_eq!(acc.text(), "abc");
        // Wrong stream ignored.
        assert!(!acc.push(&TokenDelta::new(99, 3, "x")));
    }

    #[test]
    fn hostile_hyperlink_neutralized_but_text_kept() {
        let evil = "\x1b]8;;http://evil.example/\x1b\\click me\x1b]8;;\x1b\\";
        let clean = strip_ansi(evil);
        assert_eq!(clean, "click me");
        assert!(!clean.contains('\x1b'));
        assert!(!clean.contains("evil.example"));
    }

    #[test]
    fn hostile_escapes_stripped() {
        let cases = [
            ("\x1b[31mred\x1b[0m", "red"),
            ("\x1b[2J\x1b[Hhome", "home"),
            ("a\x1bb", "a"),
            ("bell\x07gone", "bellgone"),
            ("bad\x1b]8;;http://x", "bad"),
            ("\u{9b}31mgreen\u{9b}0m", "green"),
            ("keep\nnewline\ttab", "keep\nnewline\ttab"),
        ];
        for (input, expected) in cases {
            assert_eq!(strip_ansi(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn window_bounded_and_clamped() {
        let items: Vec<u32> = (0..500).collect();
        let w = window_slice(&items, 0, 10_000);
        assert_eq!(w.len(), MAX_WINDOW);
        assert!(window_slice::<u32>(&items, 600, 10).is_empty());
        assert!(window_slice::<u32>(&items, 0, 0).is_empty());
        let w2 = window_slice(&items, 490, 50);
        assert_eq!(w2.len(), 10);
        assert_eq!(w2[0], 490);
    }

    #[test]
    fn transcript_marker_survives_bounded_scrollback() {
        let mut t = Transcript::new();
        for i in 0..MAX_SCROLLBACK {
            t.push(&format!("line {i}"));
        }
        let pinned = t.push("pinned");
        assert!(t.set_marker(pinned));
        assert!(t.marker_stable());
        assert_eq!(t.len(), MAX_SCROLLBACK);
        // Push enough to evict the marker; stability must flip, not panic.
        for i in 0..MAX_SCROLLBACK {
            t.push(&format!("new {i}"));
        }
        assert_eq!(t.len(), MAX_SCROLLBACK);
        assert!(!t.marker_stable());
        assert!(!t.set_marker(999_999_999));
    }

    #[test]
    fn tool_state_terminals_and_transitions() {
        assert!(ToolState::Failed.is_terminal());
        assert!(ToolState::Completed.is_terminal());
        assert!(!ToolState::Approval.is_terminal());
        assert!(!ToolState::Running.is_terminal());
        assert!(ToolState::Approval.can_transition(ToolState::Running));
        assert!(ToolState::Running.can_transition(ToolState::Completed));
        assert!(!ToolState::Completed.can_transition(ToolState::Running));
        assert!(!ToolState::Approval.can_transition(ToolState::Completed));
    }

    #[test]
    fn page_pins_to_bottom_and_scrolls() {
        let mut t = Transcript::new();
        for i in 0..5 {
            t.push(&format!("line {i}"));
        }
        let vis: Vec<u64> = t.page(&TranscriptPage::new(), 2).iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![3, 4]);
        let mut page = TranscriptPage::new();
        page.scroll_up(2, t.len());
        assert_eq!(page.scroll(), 2);
        let vis: Vec<u64> = t.page(&page, 2).iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![1, 2]);
        // New arrivals must not move a scrolled-up view.
        t.push("line 5");
        let vis: Vec<u64> = t.page(&page, 2).iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![1, 2]);
        page.scroll_down(10);
        assert_eq!(page.scroll(), 0);
    }

    #[test]
    fn page_height_capped_and_empty_safe() {
        let t = Transcript::new();
        let page = TranscriptPage::new();
        assert!(t.page(&page, 10).is_empty());
        assert!(t.page(&page, 0).is_empty());
        let mut t = Transcript::new();
        for i in 0..(MAX_WINDOW + 50) {
            t.push(&format!("l{i}"));
        }
        assert_eq!(t.page(&TranscriptPage::new(), 10_000).len(), MAX_WINDOW);
    }

    #[test]
    fn scroll_clamps_to_len() {
        let mut p = TranscriptPage::new();
        p.scroll_up(100, 3);
        assert_eq!(p.scroll(), 3);
        p.scroll_up(10, 3);
        assert_eq!(p.scroll(), 3);
        p.reset();
        assert_eq!(p.scroll(), 0);
    }

    #[test]
    fn render_lines_wraps_and_bounds() {
        let lines = vec![TranscriptLine { id: 0, text: "hello world foo".into() }];
        let rows = render_lines(&lines, 5);
        assert!(rows.join("|").contains("hello"));
        for r in &rows {
            assert!(r.chars().count() <= 5, "row overflow: {r:?}");
        }
        assert_eq!(render_lines(&lines, 0), vec!["hello world foo".to_owned()]);
        let lines = vec![TranscriptLine { id: 1, text: "a\nb".into() }];
        assert_eq!(render_lines(&lines, 0), vec!["a".to_owned(), "b".to_owned()]);
        let big: Vec<TranscriptLine> =
            (0..500).map(|i| TranscriptLine { id: i, text: "x".repeat(100) }).collect();
        assert!(render_lines(&big, 10).len() <= MAX_WINDOW);
    }

    #[test]
    fn render_page_end_to_end() {
        let mut t = Transcript::new();
        t.push("alpha beta gamma");
        let rows = t.render_page(&TranscriptPage::new(), 10, 5);
        assert!(!rows.is_empty());
        for r in &rows {
            assert!(r.chars().count() <= 5, "row overflow: {r:?}");
        }
    }
}

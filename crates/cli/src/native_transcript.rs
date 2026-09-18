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
}

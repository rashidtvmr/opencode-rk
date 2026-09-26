#![forbid(unsafe_code)]
//! Prompt composer state (mirrors `packages/tui/src/component/prompt/index.tsx`,
//! `packages/tui/src/prompt/display.ts`, `.../prompt/autocomplete.tsx`).
//!
//! TS checkout at /home/rashid/projects/opencode commit a0d9b6c (NOT the
//! pinned 95daf90; paths/lines cited below are against a0d9b6c).
//! Evidence: index.tsx:927-957 (submit gating: `submitting` reentry guard,
//! `props.disabled`, workspace/move creating, `auto()?.visible`, empty input);
//! index.tsx:1393-1417 (onPaste: normalize CRLF/CR, empty paste falls through
//! to clipboard read, `preventDefault` then manual insert, never submits);
//! index.tsx:1180-1218 (`pasteInputText`: CRLF/CR normalize, filepath/URL
//! probe, `>=3 lines or >150 chars` summary threshold, plain `insertText`);
//! index.tsx:1221-1267 (`pasteAttachment`: `[Image N]`/`[PDF N]` virtual text
//! + file part); autocomplete.tsx:692-700 (`/` at offset 0 reopens slash
//! list), :676-689 (hide on whitespace/arg complete), :448-519 (slash list +
//! fuzzysort then frecency-weighted rank); display.ts:1-10 (grapheme
//! `promptOffsetWidth`; newline counts 1).
//!
//! Scope: buffer + cursor + attachment count + slash hint + busy gate + paste
//! ingest as pure data. No I/O, no auto-submit: the paste path only inserts
//! text. `slash_open` is a UI hint; the TS submit block on autocomplete
//! visibility (index.tsx:956) stays with the caller via [`Composer::slash_open`].

/// Byte bound for the composer buffer (fail-closed: truncate, never grow past).
pub const MAX_VALUE: usize = 65536;
/// Max attachment labels (mirrors file-part fan-out; hard reject past this).
pub const MAX_ATTACHMENTS: usize = 16;

/// Outcome of [`Composer::paste_ingest`]. Data-only: no variant submits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Accepted,
    Truncated,
    Rejected,
}

/// Minimal composer: value, char-offset cursor, bounded attachments,
// slash/autocomplete hint, submit-busy gate.
#[derive(Debug, Clone, Default)]
pub struct Composer {
    value: String,
    cursor: usize,
    attachments: Vec<String>,
    slash_open: bool,
    busy: bool,
}

impl Composer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Cursor as a char offset (emoji-safe; never a byte index).
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn attachments(&self) -> &[String] {
        &self.attachments
    }

    #[must_use]
    pub fn slash_open(&self) -> bool {
        self.slash_open
    }

    #[must_use]
    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// Submit gate: non-empty buffer while not busy (index.tsx:935,957).
    #[must_use]
    pub fn can_submit(&self) -> bool {
        !self.busy && !self.value.is_empty()
    }

    /// Byte index for the char-offset cursor (always a char boundary).
    fn byte_idx(&self) -> usize {
        self.value
            .char_indices()
            .nth(self.cursor)
            .map_or(self.value.len(), |(i, _)| i)
    }

    /// Largest `<= max` byte bound that is a char boundary of `s`.
    fn floor_boundary(s: &str, max: usize) -> usize {
        let mut end = max.min(s.len());
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        end
    }

    /// Recompute the `/` trigger: leading `/` with no whitespace before the
    /// cursor (autocomplete.tsx:696).
    fn refresh_trigger(&mut self) {
        let prefix: String = self.value.chars().take(self.cursor).collect();
        self.slash_open = prefix.starts_with('/') && !prefix.chars().any(char::is_whitespace);
    }

    /// Insert typed text at the cursor, truncated at [`MAX_VALUE`] on a char
    /// boundary. Returns false when nothing fit.
    pub fn insert(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let avail = MAX_VALUE.saturating_sub(self.value.len());
        if avail == 0 {
            return false;
        }
        let end = Self::floor_boundary(text, avail);
        if end == 0 {
            return false;
        }
        let chunk = &text[..end];
        let idx = self.byte_idx();
        self.value.insert_str(idx, chunk);
        self.cursor += chunk.chars().count();
        self.refresh_trigger();
        true
    }

    /// Delete the char before the cursor. Returns false at offset 0.
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let end = self.byte_idx();
        let start = self.value[..end]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        self.value.drain(start..end);
        self.cursor -= 1;
        self.refresh_trigger();
        true
    }

    /// Attach a label (`[Image N]`/`[PDF N]` style virtual text stays with the
    /// caller). False when [`MAX_ATTACHMENTS`] is reached.
    pub fn attach(&mut self, label: &str) -> bool {
        if self.attachments.len() >= MAX_ATTACHMENTS {
            return false;
        }
        self.attachments.push(label.to_string());
        true
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
        self.attachments.clear();
        self.slash_open = false;
    }

    /// Ingest pasted text as data only: normalize CRLF/CR to LF
    /// (index.tsx:1181,1402), reject blank pastes (index.tsx:1407 falls
    /// through to the clipboard-image path), truncate to `max_paste` bytes and
    /// [`MAX_VALUE`] on char boundaries. Never submits, never touches `busy`.
    pub fn paste_ingest(&mut self, text: &str, max_paste: usize) -> PasteOutcome {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if normalized.trim().is_empty() {
            return PasteOutcome::Rejected;
        }
        let mut truncated = false;
        let mut chunk = normalized.as_str();
        let paste_end = Self::floor_boundary(chunk, max_paste);
        if paste_end < chunk.len() {
            truncated = true;
            chunk = &chunk[..paste_end];
        }
        let avail = MAX_VALUE.saturating_sub(self.value.len());
        let fit_end = Self::floor_boundary(chunk, avail);
        if fit_end < chunk.len() {
            truncated = true;
            chunk = &chunk[..fit_end];
        }
        if chunk.is_empty() {
            return PasteOutcome::Rejected;
        }
        let idx = self.byte_idx();
        self.value.insert_str(idx, chunk);
        self.cursor += chunk.chars().count();
        self.refresh_trigger();
        if truncated {
            PasteOutcome::Truncated
        } else {
            PasteOutcome::Accepted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_emoji_keeps_char_cursor() {
        let mut c = Composer::new();
        assert!(c.insert("a😀b"));
        assert_eq!(c.value(), "a😀b");
        assert_eq!(c.cursor(), 3);
        assert!(c.value.is_char_boundary(c.byte_idx()));
    }

    #[test]
    fn backspace_removes_whole_emoji() {
        let mut c = Composer::new();
        c.insert("a😀");
        assert!(c.backspace());
        assert_eq!(c.value(), "a");
        assert_eq!(c.cursor(), 1);
        assert!(!c.backspace() || c.cursor() == 0);
    }

    #[test]
    fn insert_respects_max_value() {
        let mut c = Composer::new();
        assert!(c.insert(&"x".repeat(MAX_VALUE)));
        assert_eq!(c.value().len(), MAX_VALUE);
        assert!(!c.insert("more"));
        assert!(c.can_submit());
    }

    #[test]
    fn paste_accepted_under_bound() {
        let mut c = Composer::new();
        assert_eq!(c.paste_ingest("hello\nworld", 1024), PasteOutcome::Accepted);
        assert_eq!(c.value(), "hello\nworld");
        assert!(!c.busy());
    }

    #[test]
    fn paste_truncates_at_char_boundary() {
        let mut c = Composer::new();
        // "ab"=2 bytes, emoji=4 bytes; cap 3 lands mid-emoji -> floor to "ab".
        assert_eq!(c.paste_ingest("ab😀cd", 3), PasteOutcome::Truncated);
        assert_eq!(c.value(), "ab");
    }

    #[test]
    fn paste_rejected_when_blank_or_noroom() {
        let mut c = Composer::new();
        assert_eq!(c.paste_ingest("   \n  ", 1024), PasteOutcome::Rejected);
        assert_eq!(c.paste_ingest("", 1024), PasteOutcome::Rejected);
        assert!(c.value().is_empty());
    }

    #[test]
    fn bracketed_paste_never_executes() {
        let mut c = Composer::new();
        let evil = "\x1b[200~/quit\nrm -rf /\x1b[201~";
        assert_eq!(c.paste_ingest(evil, 1024), PasteOutcome::Accepted);
        assert!(c.value().contains(evil));
        assert!(!c.busy());
        // Data only: still gated as a normal non-empty buffer, nothing ran.
        assert!(c.can_submit());
        assert!(c.value().starts_with("\x1b[200~"));
    }

    #[test]
    fn busy_blocks_submit() {
        let mut c = Composer::new();
        assert!(!c.can_submit());
        c.insert("hi");
        assert!(c.can_submit());
        c.set_busy(true);
        assert!(!c.can_submit());
        c.set_busy(false);
        assert!(c.can_submit());
    }

    #[test]
    fn attachments_bounded_at_16() {
        let mut c = Composer::new();
        for i in 0..MAX_ATTACHMENTS {
            assert!(c.attach(&format!("[Image {}]", i + 1)));
        }
        assert!(!c.attach("[Image 17]"));
        assert_eq!(c.attachments().len(), MAX_ATTACHMENTS);
    }

    #[test]
    fn slash_trigger_opens_and_closes() {
        let mut c = Composer::new();
        c.insert("/skills");
        assert!(c.slash_open());
        c.insert(" ");
        assert!(!c.slash_open());
        c.clear();
        c.insert("@file");
        assert!(!c.slash_open());
    }
}

// --- BRIDGE-067 additive: submit gate, paste branch, summary, counters ---
// Mirrors index.tsx:954-969 (disabled/creating/auto gates),
// :1184-1210 (paste branch + summary), :1221-1231 (counters). 
/// Submit gates beyond [`Composer::can_submit`] (index.tsx:954-956).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubmitGate {
    pub disabled: bool,
    pub creating: bool,
    pub auto_visible: bool,
}

impl Composer {
    /// Extends [`Composer::can_submit`]: busy/empty plus gate flags.
    #[must_use]
    pub fn can_submit_gated(&self, gate: &SubmitGate) -> bool {
        self.can_submit() && !gate.disabled && !gate.creating && !gate.auto_visible
    }
}

/// Paste branch (index.tsx:1184-1200): URL bypass, SVG text, binary attach, plain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteBranch {
    Url,
    Svg { name: String },
    Binary,
    Text,
}

fn paste_basename(s: &str) -> String {
    let b = s.rsplit(['/', '\\']).next().unwrap_or(s);
    if b.is_empty() { s.to_string() } else { b.to_string() }
}

/// Classify a mime type, URL, or filepath into a [`PasteBranch`].
/// Data-only: never reads or executes the target.
#[must_use]
pub fn classify_paste(mime_or_url: &str) -> PasteBranch {
    let s = mime_or_url.trim();
    if s.starts_with("http://") || s.starts_with("https://") {
        return PasteBranch::Url;
    }
    let lower = s.to_ascii_lowercase();
    if s == "image/svg+xml" {
        return PasteBranch::Svg { name: "image".to_string() };
    }
    if lower.ends_with(".svg") {
        return PasteBranch::Svg { name: paste_basename(s) };
    }
    if lower.starts_with("image/")
        || lower == "application/pdf"
        || lower == "application/octet-stream"
        || lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".pdf")
    {
        return PasteBranch::Binary;
    }
    PasteBranch::Text
}

/// Summary threshold (index.tsx:1203-1208): `>=3 lines or >150 chars`.
pub const SUMMARY_LINES: usize = 3;
pub const SUMMARY_CHARS: usize = 150;

/// Summarize pasted text as `[Pasted ~N lines]`, or `None` when short.
/// Line count mirrors TS: newlines in trimmed LF-normalized text, plus one.
#[must_use]
pub fn summarize_pasted(text: &str) -> Option<String> {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let t = norm.trim();
    if t.is_empty() {
        return None;
    }
    let lines = t.bytes().filter(|&b| b == b'\n').count() + 1;
    if lines >= SUMMARY_LINES || t.chars().count() > SUMMARY_CHARS {
        Some(format!("[Pasted ~{lines} lines]"))
    } else {
        None
    }
}

/// Next attach label from current per-type count (index.tsx:1224-1230).
#[must_use]
pub fn next_image_n(count: u32) -> String {
    format!("[Image {}]", count.saturating_add(1))
}

/// Next PDF label from current PDF count (index.tsx:1230).
#[must_use]
pub fn next_pdf_n(count: u32) -> String {
    format!("[PDF {}]", count.saturating_add(1))
}

#[cfg(test)]
mod bridge067_tests {
    use super::*;

    #[test]
    fn gated_blocks_each_flag() {
        let mut c = Composer::new();
        c.insert("hi");
        assert!(c.can_submit_gated(&SubmitGate::default()));
        assert!(!c.can_submit_gated(&SubmitGate { disabled: true, ..Default::default() }));
        assert!(!c.can_submit_gated(&SubmitGate { creating: true, ..Default::default() }));
        assert!(!c.can_submit_gated(&SubmitGate { auto_visible: true, ..Default::default() }));
    }

    #[test]
    fn gated_still_needs_nonempty_and_idle() {
        let c = Composer::new();
        assert!(!c.can_submit_gated(&SubmitGate::default()));
        let mut b = Composer::new();
        b.insert("x");
        b.set_busy(true);
        assert!(!b.can_submit_gated(&SubmitGate::default()));
    }

    #[test]
    fn classify_url() {
        assert_eq!(classify_paste("https://a.b/c"), PasteBranch::Url);
        assert_eq!(classify_paste("http://a.b"), PasteBranch::Url);
    }

    #[test]
    fn classify_svg_mime_and_path() {
        assert_eq!(classify_paste("image/svg+xml"), PasteBranch::Svg { name: "image".to_string() });
        assert_eq!(
            classify_paste("/tmp/icon.svg"),
            PasteBranch::Svg { name: "icon.svg".to_string() }
        );
    }

    #[test]
    fn classify_binary() {
        assert_eq!(classify_paste("image/png"), PasteBranch::Binary);
        assert_eq!(classify_paste("application/pdf"), PasteBranch::Binary);
        assert_eq!(classify_paste("photo.jpg"), PasteBranch::Binary);
    }

    #[test]
    fn classify_text_fallback() {
        assert_eq!(classify_paste("hello world"), PasteBranch::Text);
        assert_eq!(classify_paste("text/plain"), PasteBranch::Text);
    }

    #[test]
    fn summarize_short_is_none() {
        assert_eq!(summarize_pasted("hi"), None);
        assert_eq!(summarize_pasted("a\nb"), None);
        assert_eq!(summarize_pasted("   "), None);
    }

    #[test]
    fn summarize_three_lines() {
        assert_eq!(summarize_pasted("a\nb\nc"), Some("[Pasted ~3 lines]".to_string()));
    }

    #[test]
    fn summarize_long_single_line() {
        let long = "x".repeat(SUMMARY_CHARS + 1);
        assert_eq!(summarize_pasted(&long), Some("[Pasted ~1 lines]".to_string()));
    }

    #[test]
    fn counters_label_next() {
        assert_eq!(next_image_n(0), "[Image 1]");
        assert_eq!(next_image_n(2), "[Image 3]");
        assert_eq!(next_pdf_n(0), "[PDF 1]");
        assert_eq!(next_pdf_n(4), "[PDF 5]");
    }
}

#![forbid(unsafe_code)]
//! App-scope transcript: bounded role-tagged entries.
//!
//! Pure state only: no rendering, no IO. Distinct from
//! `native_transcript.rs` (stream deltas / scrollback window).

use std::collections::VecDeque;

/// Max entries retained; oldest evicted on overflow.
pub const MAX_ENTRIES: usize = 1000;
/// Max bytes per entry text; oversize push rejected.
pub const MAX_TEXT_BYTES: usize = 4096;

/// Speaker role for one transcript entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AppRole {
    User = 0,
    Assistant = 1,
    System = 2,
}

/// One role-tagged transcript entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTranscriptEntry {
    pub role: AppRole,
    pub text: String,
}

/// Rejection reason for [`AppTranscript::push`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTranscriptError {
    Oversize { len: usize, max: usize },
}

impl std::fmt::Display for AppTranscriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Oversize { len, max } => write!(f, "entry text {len} bytes exceeds cap {max}"),
        }
    }
}

impl std::error::Error for AppTranscriptError {}

/// Bounded app transcript; oldest entries evicted past [`MAX_ENTRIES`].
#[derive(Clone, Debug, Default)]
pub struct AppTranscript {
    entries: VecDeque<AppTranscriptEntry>,
}

impl AppTranscript {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: VecDeque::new() }
    }

    /// Push one entry; rejects text over [`MAX_TEXT_BYTES`], evicts oldest past cap.
    pub fn push(&mut self, role: AppRole, text: impl Into<String>) -> Result<(), AppTranscriptError> {
        let text = text.into();
        if text.len() > MAX_TEXT_BYTES {
            return Err(AppTranscriptError::Oversize { len: text.len(), max: MAX_TEXT_BYTES });
        }
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(AppTranscriptEntry { role, text });
        Ok(())
    }

    /// Last `n` entries, oldest-first.
    pub fn last_n(&self, n: usize) -> impl Iterator<Item = &AppTranscriptEntry> {
        let skip = self.entries.len().saturating_sub(n);
        self.entries.iter().skip(skip)
    }

    /// Render last `n` entries as `"ROLE: text"` rows, word-wrapped at `width` chars.
    ///
    /// Oldest-first. Pure/bounded: no IO, output capped by stored entries.
    /// `width == 0` disables wrapping (one row per entry).
    #[must_use]
    pub fn render_window(&self, n: usize, width: usize) -> Vec<String> {
        fn label(role: AppRole) -> &'static str {
            match role {
                AppRole::User => "USER",
                AppRole::Assistant => "ASSISTANT",
                AppRole::System => "SYSTEM",
            }
        }
        fn push_wrapped(out: &mut Vec<String>, paragraph: &str, width: usize) {
            if paragraph.chars().count() <= width {
                out.push(paragraph.to_owned());
                return;
            }
            let mut line = String::new();
            let mut line_len = 0usize;
            for word in paragraph.split_whitespace() {
                let wlen = word.chars().count();
                if wlen > width {
                    if !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                        line_len = 0;
                    }
                    let chars: Vec<char> = word.chars().collect();
                    for chunk in chars.chunks(width) {
                        out.push(chunk.iter().collect());
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
                    line.push_str(word);
                    line_len = wlen;
                }
            }
            if !line.is_empty() {
                out.push(line);
            }
        }
        let mut out = Vec::new();
        for e in self.last_n(n) {
            let full = format!("{}: {}", label(e.role), e.text);
            if width == 0 {
                out.push(full);
                continue;
            }
            for para in full.split('\n') {
                push_wrapped(&mut out, para, width);
            }
        }
        out
    }

    #[must_use]
    pub fn entries(&self) -> &VecDeque<AppTranscriptEntry> {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_past_cap() {
        let mut t = AppTranscript::new();
        for i in 0..(MAX_ENTRIES + 10) {
            t.push(AppRole::User, format!("m{i}")).unwrap();
        }
        assert_eq!(t.len(), MAX_ENTRIES);
        assert_eq!(t.entries()[0].text, "m10");
        assert_eq!(t.last_n(2).map(|e| e.text.as_str()).collect::<Vec<_>>(), ["m1008", "m1009"]);
    }

    #[test]
    fn rejects_oversize_text() {
        let mut t = AppTranscript::new();
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            t.push(AppRole::Assistant, big),
            Err(AppTranscriptError::Oversize { len: MAX_TEXT_BYTES + 1, max: MAX_TEXT_BYTES })
        );
        assert!(t.is_empty());
        t.push(AppRole::System, "x".repeat(MAX_TEXT_BYTES)).unwrap();
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn render_window_wraps_at_width() {
        let mut t = AppTranscript::new();
        t.push(AppRole::User, "hello world foo").unwrap();
        let rows = t.render_window(1, 10);
        assert_eq!(rows, vec!["USER:".to_owned(), "hello".to_owned(), "world foo".to_owned()]);
        for r in &rows {
            assert!(r.chars().count() <= 10, "row overflow: {r:?}");
        }
    }

    #[test]
    fn render_window_empty() {
        let t = AppTranscript::new();
        assert!(t.render_window(5, 20).is_empty());
        assert!(t.render_window(0, 20).is_empty());
    }

    #[test]
    fn render_window_caps_to_last_n_oldest_first() {
        let mut t = AppTranscript::new();
        t.push(AppRole::User, "one").unwrap();
        t.push(AppRole::Assistant, "two").unwrap();
        t.push(AppRole::System, "three").unwrap();
        let rows = t.render_window(2, 0);
        assert_eq!(rows, vec!["ASSISTANT: two".to_owned(), "SYSTEM: three".to_owned()]);
        assert!(t.render_window(0, 0).is_empty());
    }
}

#![forbid(unsafe_code)]
//! App-scope composer draft state.
//!
//! Pure state only: no rendering, no IO. Minimal sibling of
//! `native_composer.rs` (which owns `Composer`/`EditBuffer`); type names
//! here are intentionally distinct (`AppComposer`) to avoid collision.

use std::fmt;

/// Max draft bytes retained.
pub const MAX_DRAFT: usize = 8192;

/// App composer failures. A denied call leaves state untouched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppComposerError {
    DraftTooLong { bytes: usize, limit: usize },
}

impl fmt::Display for AppComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DraftTooLong { bytes, limit } => {
                write!(f, "draft of {bytes} bytes exceeds {limit} byte budget")
            }
        }
    }
}

impl std::error::Error for AppComposerError {}

/// App-scope single-line draft with a char-boundary cursor.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppComposer {
    draft: String,
    cursor: usize,
}

impl AppComposer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.draft.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.draft.is_empty()
    }

    #[must_use]
    pub fn is_cursor_on_boundary(&self) -> bool {
        self.cursor <= self.draft.len() && self.draft.is_char_boundary(self.cursor)
    }

    /// Snap `pos` into `[0, len]` on a char boundary (floor).
    fn clamp(pos: usize, text: &str) -> usize {
        let mut p = pos.min(text.len());
        while !text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    /// Move cursor; out-of-bounds clamps, mid-code-point snaps down.
    pub fn move_cursor(&mut self, pos: usize) {
        self.cursor = Self::clamp(pos, &self.draft);
        debug_assert!(self.is_cursor_on_boundary());
    }

    /// Insert one scalar at the cursor.
    pub fn insert(&mut self, c: char) -> Result<(), AppComposerError> {
        let after = self.draft.len() + c.len_utf8();
        if after > MAX_DRAFT {
            return Err(AppComposerError::DraftTooLong {
                bytes: after,
                limit: MAX_DRAFT,
            });
        }
        debug_assert!(self.is_cursor_on_boundary());
        self.draft.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        debug_assert!(self.is_cursor_on_boundary());
        Ok(())
    }

    /// Delete the scalar before the cursor. False at buffer start.
    pub fn backspace(&mut self) -> bool {
        let width = match self.draft[..self.cursor].chars().next_back() {
            Some(c) => c.len_utf8(),
            None => return false,
        };
        self.cursor -= width;
        self.draft.drain(self.cursor..self.cursor + width);
        debug_assert!(self.is_cursor_on_boundary());
        true
    }

    pub fn clear(&mut self) {
        self.draft.clear();
        self.cursor = 0;
    }

    /// Status-line view with `|` at the cursor, capped at `max_chars` chars.
    ///
    /// `max_chars` counts every char including `|` and `…`(s). Pure, never
    /// mutates. Char-boundary safe: cursor snaps down, output built from
    /// chars only. Truncation keeps cursor visible, `…` marks cut side(s).
    /// `max_chars == 0` yields `""`. Out-of-bounds cursor omits the marker.
    // ponytail: char budget, not terminal columns (CJK/emoji width 2
    // counts 1); upgrade to unicode-width when TUI needs real columns.
    #[must_use]
    pub fn display_with_cursor(&self, max_chars: usize) -> String {
        if max_chars == 0 {
            return String::new();
        }
        let on_boundary =
            self.cursor <= self.draft.len() && self.draft.is_char_boundary(self.cursor);
        let cursor = Self::clamp(self.cursor, &self.draft);
        let chars: Vec<char> = self.draft.chars().collect();
        let n = chars.len();
        let c_idx = self.draft[..cursor].chars().count();
        if !on_boundary {
            if n <= max_chars {
                return self.draft.clone();
            }
            if max_chars == 1 {
                return "…".to_string();
            }
            let mut s: String = chars[..max_chars - 1].iter().collect();
            s.push('…');
            return s;
        }
        if n + 1 <= max_chars {
            let mut s = String::with_capacity(self.draft.len() + 1);
            s.extend(chars[..c_idx].iter());
            s.push('|');
            s.extend(chars[c_idx..].iter());
            return s;
        }
        if max_chars == 1 {
            return "…".to_string();
        }
        if max_chars == 2 {
            return if c_idx == 0 {
                "|…".to_string()
            } else {
                "…|".to_string()
            };
        }
        // Head fits: marker + head + suffix ellipsis.
        if c_idx + 2 <= max_chars {
            let take = max_chars - 2;
            let mut s = String::new();
            s.extend(chars[..c_idx].iter());
            s.push('|');
            s.extend(chars[c_idx..take].iter());
            s.push('…');
            return s;
        }
        // Tail fits: prefix ellipsis + tail + marker.
        if (n - c_idx) + 2 <= max_chars {
            let take = max_chars - 2;
            let start = n - take;
            let mut s = String::from("…");
            s.extend(chars[start..c_idx].iter());
            s.push('|');
            s.extend(chars[c_idx..].iter());
            return s;
        }
        // Mid: both sides cut, window centered on cursor.
        let win = max_chars - 3;
        let start = c_idx.saturating_sub(win / 2).min(c_idx).min(n.saturating_sub(win));
        let end = (start + win).min(n);
        let mut s = String::from("…");
        for (k, ch) in chars.iter().skip(start).take(end - start).enumerate() {
            if start + k == c_idx {
                s.push('|');
            }
            s.push(*ch);
        }
        if c_idx == end {
            s.push('|');
        }
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oob_cursor_clamps() {
        let mut c = AppComposer::new();
        c.insert('a').unwrap();
        c.insert('b').unwrap();
        c.move_cursor(99);
        assert_eq!(c.cursor(), 2);
        c.move_cursor(usize::MAX);
        assert_eq!(c.cursor(), 2);
        assert!(c.is_cursor_on_boundary());
    }

    #[test]
    fn multibyte_never_splits() {
        let mut c = AppComposer::new();
        c.insert('🦀').unwrap();
        assert!(c.is_cursor_on_boundary());
        // Mid-code-point snaps down to 0.
        c.move_cursor(2);
        assert_eq!(c.cursor(), 0);
        assert!(c.is_cursor_on_boundary());
        // Backspace from end removes whole scalar.
        c.move_cursor(usize::MAX);
        assert!(c.backspace());
        assert_eq!(c.draft(), "");
        assert_eq!(c.cursor(), 0);
    }

    #[test]
    fn cap_rejected_without_mutation() {
        let mut c = AppComposer::new();
        for _ in 0..MAX_DRAFT {
            c.insert('x').unwrap();
        }
        assert_eq!(
            c.insert('y'),
            Err(AppComposerError::DraftTooLong {
                bytes: MAX_DRAFT + 1,
                limit: MAX_DRAFT
            })
        );
        assert_eq!(c.len_bytes(), MAX_DRAFT);
    }

    #[test]
    fn clear_resets() {
        let mut c = AppComposer::new();
        c.insert('a').unwrap();
        c.clear();
        assert_eq!(c.draft(), "");
        assert_eq!(c.cursor(), 0);
        assert!(!c.backspace());
    }

    #[test]
    fn display_marks_cursor() {
        let mut c = AppComposer::new();
        for ch in "abc".chars() {
            c.insert(ch).unwrap();
        }
        c.move_cursor(1);
        assert_eq!(c.display_with_cursor(99), "a|bc");
        c.move_cursor(usize::MAX);
        assert_eq!(c.display_with_cursor(99), "abc|");
        c.move_cursor(0);
        assert_eq!(c.display_with_cursor(99), "|abc");
    }

    #[test]
    fn display_multibyte_safe() {
        let mut c = AppComposer::new();
        for ch in "a🦀b".chars() {
            c.insert(ch).unwrap();
        }
        c.move_cursor(usize::MAX);
        c.backspace(); // remove 'b'
        c.move_cursor(1); // after 'a', before '🦀'
        assert_eq!(c.display_with_cursor(99), "a|🦀");
        // Forced mid-code-point cursor never panics, snaps down.
        c.cursor = 2;
        assert_eq!(c.display_with_cursor(99), "a🦀");
    }

    #[test]
    fn display_truncates_with_ellipsis() {
        let mut c = AppComposer::new();
        for ch in "abcdef".chars() {
            c.insert(ch).unwrap();
        }
        c.move_cursor(0);
        assert_eq!(c.display_with_cursor(4), "|ab…");
        c.move_cursor(usize::MAX);
        assert_eq!(c.display_with_cursor(4), "…ef|");
        c.move_cursor(3);
        assert_eq!(c.display_with_cursor(4), "…|d…");
        assert_eq!(c.display_with_cursor(1), "…");
        assert_eq!(c.display_with_cursor(0), "");
        // Every output respects the char budget.
        for max in 1..10 {
            for pos in 0..8 {
                c.move_cursor(pos);
                assert!(c.display_with_cursor(max).chars().count() <= max);
            }
        }
        // Out-of-bounds cursor omits the marker instead of panicking.
        c.cursor = 99;
        assert_eq!(c.display_with_cursor(99), "abcdef");
    }
}

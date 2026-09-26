#![forbid(unsafe_code)]
//! Offline chat helpers: banner, empty hint, char-safe clip, cap check.
//!
//! Evidence: `crates/cli/src/tui_entry.rs:480` (offline title),
//! `:496` (`EMPTY_HINT`), `:572` (char-safe clip), `:728-731`
//! (`MAX_NATIVE_TRANSCRIPT` drain cap).
//!
//! ponytail: offline-only slice; live snapshot wiring stays in tui_entry.

/// Banner when no live snapshot bound (tui_entry.rs:480 shape).
pub const OFFLINE_TITLE: &str = "OpenCode RK -- offline";
/// Body placeholder for empty transcript (tui_entry.rs:496).
pub const EMPTY_HINT: &str = "Start typing to send a turn.";
/// Cap on retained transcript lines (tui_entry.rs:728).
pub const MAX_OFFLINE_TRANSCRIPT: usize = 500;

/// Unicode-safe clip to `max` chars (tui_entry.rs:572).
#[must_use]
pub fn clip_line(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// True when `len` lines still fit the 500 cap (tui_entry.rs:729).
#[must_use]
pub fn capped_push(len: usize) -> bool {
    len < MAX_OFFLINE_TRANSCRIPT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_match_truth() {
        assert!(OFFLINE_TITLE.contains("offline"));
        assert_eq!(EMPTY_HINT, "Start typing to send a turn.");
    }

    #[test]
    fn clip_ascii() {
        assert_eq!(clip_line("abcdef", 3), "abc");
        assert_eq!(clip_line("ab", 5), "ab");
    }

    #[test]
    fn clip_unicode_char_safe() {
        assert_eq!(clip_line("héllo🍕world", 6), "héllo🍕");
    }

    #[test]
    fn clip_zero_empty() {
        assert_eq!(clip_line("abc", 0), "");
        assert_eq!(clip_line("", 3), "");
    }

    #[test]
    fn cap_boundary() {
        assert!(capped_push(499));
        assert!(!capped_push(500));
        assert!(!capped_push(600));
    }
}

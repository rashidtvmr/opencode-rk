#![forbid(unsafe_code)]
//! Session id8 + status line (TS truth `tui_entry.rs:502-506`).
//!
//! `ponytail:` space-joined line per spec; bullet variant when caller needs it.

/// First 8 chars of `id`, char-safe.
#[must_use]
pub fn id8(id: &str) -> String {
    id.chars().take(8).collect()
}

/// `"session <id8> <state> <n> msgs"`, capped at 256 chars.
#[must_use]
pub fn session_line(id: &str, state: &str, n: usize) -> String {
    format!("session {} {state} {n} msgs", id8(id))
        .chars()
        .take(256)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_to_8() {
        assert_eq!(id8("ses_abcdef123456"), "ses_abcd");
    }

    #[test]
    fn short_passthrough() {
        assert_eq!(id8("abc"), "abc");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(id8(""), "");
    }

    #[test]
    fn line_format() {
        assert_eq!(
            session_line("ses_abcdef123", "idle", 3),
            "session ses_abcd idle 3 msgs"
        );
    }

    #[test]
    fn line_caps_256() {
        let s = session_line("ses_abcdef123", &"x".repeat(400), 1);
        assert!(s.chars().count() <= 256);
    }
}

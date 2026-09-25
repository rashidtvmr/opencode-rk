#![forbid(unsafe_code)]
//! Reply transcript lines: `you:`, `assistant:`, `offline:` prefixes.
//!
//! TS truth (`crates/cli/src/tui_entry.rs:717,722,726`): transcript pushes
//! `format!("you: {text}")`, `format!("assistant: {reply}")`, and literal
//! `"offline: turn not executed"`. Caps are char-counts, std-only.
//!
//! `ponytail:` no ellipsis on clip; add when caller needs it.

pub const YOU_CAP_CHARS: usize = 4096;
pub const ASSISTANT_CAP_CHARS: usize = 65536;
pub const OFFLINE_LINE: &str = "offline: turn not executed";

/// `you: <text>` clipped to 4KiB chars.
#[must_use]
pub fn you_line(text: &str) -> String {
    format!("you: {}", clip(text, YOU_CAP_CHARS))
}

/// `assistant: <text>` clipped to 64KiB chars.
#[must_use]
pub fn assistant_line(text: &str) -> String {
    format!("assistant: {}", clip(text, ASSISTANT_CAP_CHARS))
}

/// Offline placeholder when no live daemon bound.
#[must_use]
pub fn offline_line() -> &'static str {
    OFFLINE_LINE
}

fn clip(text: &str, cap: usize) -> String {
    text.chars().take(cap).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn you_prefix() {
        assert_eq!(you_line("hi"), "you: hi");
    }

    #[test]
    fn you_caps_4k_chars() {
        let t = "a".repeat(YOU_CAP_CHARS + 10);
        assert_eq!(you_line(&t).chars().count(), "you: ".len() + YOU_CAP_CHARS);
    }

    #[test]
    fn assistant_prefix() {
        assert_eq!(assistant_line("ok"), "assistant: ok");
    }

    #[test]
    fn assistant_caps_64k_chars() {
        let t = "b".repeat(ASSISTANT_CAP_CHARS + 10);
        let line = assistant_line(&t);
        assert_eq!(
            line.chars().count(),
            "assistant: ".len() + ASSISTANT_CAP_CHARS
        );
    }

    #[test]
    fn offline_exact() {
        assert_eq!(offline_line(), "offline: turn not executed");
        assert_eq!(offline_line(), OFFLINE_LINE);
    }
}

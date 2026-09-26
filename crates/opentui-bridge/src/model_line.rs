#![forbid(unsafe_code)]

//! Model status row for run views (std-only).
//!
//! TS truth: `tui_entry.rs:341` status bar row `model: {model} (...)`.

/// `model: <m> idle|busy`, char-capped at 256.
#[must_use]
pub fn model_status(model: &str, busy: bool) -> String {
    let state = if busy { "busy" } else { "idle" };
    clip(&format!("model: {model} {state}"), 256)
}

/// Static composer/palette key hints.
#[must_use]
pub fn status_hints() -> &'static str {
    "Enter send | Backspace edit | Ctrl+P commands"
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_row() {
        assert_eq!(model_status("gpt", false), "model: gpt idle");
    }

    #[test]
    fn busy_row() {
        assert_eq!(model_status("gpt", true), "model: gpt busy");
    }

    #[test]
    fn caps_at_256_chars() {
        let m = "x".repeat(300);
        let out = model_status(&m, false);
        assert_eq!(out.chars().count(), 256);
        assert!(out.starts_with("model: "));
    }

    #[test]
    fn unicode_cap_is_char_safe() {
        let m = "e".repeat(250) + &"🦀".repeat(10);
        let out = model_status(&m, true);
        assert_eq!(out.chars().count(), 256);
    }

    #[test]
    fn hints_exact() {
        assert_eq!(
            status_hints(),
            "Enter send | Backspace edit | Ctrl+P commands"
        );
    }
}

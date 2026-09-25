#![forbid(unsafe_code)]
//! Full toast UI slot: message + level, one-line render.
//!
//! TS truth (`packages/tui/src/ui/toast.tsx`): single current toast with
//! `message` + `variant`. This slot stores both; view never mutates.
//!
//! `ponytail:` char-count width, `level: msg` shape; add when caller needs it.

/// Toast slot; `msg` capped 512 chars, `level` one of info|warn|error.
pub struct ToastUi {
    msg: String,
    level: String,
}

impl ToastUi {
    /// Empty slot, level `info`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            msg: String::new(),
            level: "info".to_string(),
        }
    }

    /// Store `msg` (512-char cap) and normalized `level` (16-char cap).
    pub fn show(&mut self, msg: &str, level: &str) {
        let lvl = level.chars().take(16).collect::<String>();
        self.level = match lvl.as_str() {
            "info" | "warn" | "error" => lvl,
            _ => "info".to_string(),
        };
        self.msg = msg.chars().take(512).collect();
    }

    /// Current level.
    #[must_use]
    pub fn level_of(&self) -> &str {
        &self.level
    }

    /// `level: msg` clipped to `width` chars, char-safe.
    #[must_use]
    pub fn line(&self, width: usize) -> String {
        format!("{}: {}", self.level, self.msg)
            .chars()
            .take(width)
            .collect()
    }
}

impl Default for ToastUi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_info_empty() {
        let t = ToastUi::new();
        assert_eq!(t.level_of(), "info");
        assert_eq!(t.line(20), "info: ".to_string());
    }

    #[test]
    fn show_warn_and_error() {
        let mut t = ToastUi::new();
        t.show("disk low", "warn");
        assert_eq!(t.level_of(), "warn");
        assert_eq!(t.line(99), "warn: disk low".to_string());
        t.show("boom", "error");
        assert_eq!(t.line(99), "error: boom".to_string());
    }

    #[test]
    fn unknown_level_falls_back_info() {
        let mut t = ToastUi::new();
        t.show("hi", "success");
        assert_eq!(t.level_of(), "info");
    }

    #[test]
    fn msg_capped_at_512_chars() {
        let mut t = ToastUi::new();
        let long = "a".repeat(600);
        t.show(&long, "info");
        assert_eq!(t.line(9999).chars().count(), 6 + 512);
    }

    #[test]
    fn line_clips_char_safe() {
        let mut t = ToastUi::new();
        t.show("e\u{301}clair", "info");
        let out = t.line(7);
        assert_eq!(out.chars().count(), 7);
        assert!(out.starts_with("info: "));
    }
}

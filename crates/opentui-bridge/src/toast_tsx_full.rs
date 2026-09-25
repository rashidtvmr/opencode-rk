#![forbid(unsafe_code)]
//! TSX toast slot: message + numeric level, one-line render.
//!
//! TS truth (`packages/tui/src/ui/toast.tsx`): single current toast with
//! `message` + `variant` (info|success|warning|error). This slot stores
//! `msg` (256-char cap) + `level: u8` (0 info, 1 success, 2 warning, 3 error,
//! passthrough otherwise); view never mutates.
//!
//! `ponytail:` numeric level, `level: msg` shape; add when caller needs it.

/// Toast slot; `msg` capped at 256 chars.
pub struct ToastTsx {
    msg: String,
    level: u8,
}

impl ToastTsx {
    /// Empty slot, level 0 (info).
    #[must_use]
    pub fn new() -> Self {
        Self {
            msg: String::new(),
            level: 0,
        }
    }

    /// Store `msg` (256-char cap) and `level`.
    pub fn show(&mut self, msg: &str, level: u8) {
        self.msg = msg.chars().take(256).collect();
        self.level = level;
    }

    /// Reset to empty info toast.
    pub fn clear(&mut self) {
        self.msg.clear();
        self.level = 0;
    }

    /// `level: msg` clipped to 256 chars, char-safe.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{}: {}", self.level, self.msg)
            .chars()
            .take(256)
            .collect()
    }
}

impl Default for ToastTsx {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty_info() {
        let t = ToastTsx::new();
        assert_eq!(t.line(), "0: ");
    }

    #[test]
    fn show_stores_level_and_caps_msg() {
        let mut t = ToastTsx::new();
        t.show(&"a".repeat(300), 3);
        assert_eq!(t.line().chars().count(), 256);
        assert!(t.line().starts_with("3: "));
    }

    #[test]
    fn clear_resets() {
        let mut t = ToastTsx::new();
        t.show("boom", 2);
        t.clear();
        assert_eq!(t.line(), "0: ");
    }
}

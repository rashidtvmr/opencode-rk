#![forbid(unsafe_code)]
//! TUI runtime flags (color/kitty/mouse/title).
//!
//! Rust mirror of `packages/tui/src/runtime.tsx` (`abbreviateHome`) and
//! `packages/tui/src/context/runtime.tsx` (provider/default value pattern):
//! explicit defaults plus bounded setters, no ambient state.

/// Max title length in chars.
pub const TITLE_CAP: usize = 128;
/// Default window title.
pub const DEFAULT_TITLE: &str = "OpenCode RK";
/// Max accepted kitty graphics level.
pub const MAX_KITTY_LEVEL: u8 = 3;

/// Runtime display flags for the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiRuntime {
    pub color: bool,
    pub kitty: bool,
    pub mouse: bool,
    pub title: String,
}

impl TuiRuntime {
    /// Defaults: color on, kitty/mouse off, title [`DEFAULT_TITLE`].
    pub fn defaults() -> Self {
        Self {
            color: true,
            kitty: false,
            mouse: false,
            title: DEFAULT_TITLE.to_string(),
        }
    }

    /// Enable kitty graphics iff `level <= 3`; returns whether enabled.
    pub fn enable_kitty(&mut self, level: u8) -> bool {
        if level <= MAX_KITTY_LEVEL {
            self.kitty = true;
            true
        } else {
            false
        }
    }

    /// Set title, truncating to [`TITLE_CAP`] chars.
    pub fn set_title(&mut self, title: String) {
        if title.chars().count() > TITLE_CAP {
            self.title = title.chars().take(TITLE_CAP).collect();
        } else {
            self.title = title;
        }
    }

    /// One-line human-readable snapshot.
    pub fn summary(&self) -> String {
        format!(
            "color={} kitty={} mouse={} title={:?}",
            self.color, self.kitty, self.mouse, self.title
        )
    }
}

impl Default for TuiRuntime {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let rt = TuiRuntime::defaults();
        assert!(rt.color);
        assert!(!rt.kitty);
        assert!(!rt.mouse);
        assert_eq!(rt.title, "OpenCode RK");
    }

    #[test]
    fn kitty_gate_accepts_level_3() {
        let mut rt = TuiRuntime::defaults();
        assert!(rt.enable_kitty(3));
        assert!(rt.kitty);
    }

    #[test]
    fn kitty_over_returns_false() {
        let mut rt = TuiRuntime::defaults();
        assert!(!rt.enable_kitty(4));
        assert!(!rt.kitty);
    }

    #[test]
    fn title_truncates_at_cap() {
        let mut rt = TuiRuntime::defaults();
        rt.set_title("x".repeat(200));
        assert_eq!(rt.title.chars().count(), TITLE_CAP);
    }

    #[test]
    fn summary_non_empty() {
        let rt = TuiRuntime::defaults();
        let s = rt.summary();
        assert!(!s.is_empty());
        assert!(s.contains("OpenCode RK"));
    }
}

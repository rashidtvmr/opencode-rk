#![forbid(unsafe_code)]
//! Resolved TUI top-level config (mirrors `packages/tui/src/config/index.tsx`).
//!
//! TS checkout /home/rashid/projects/opencode @ a0d9b6c (NOT pinned 95daf90):
//! - `index.tsx:21` LeaderTimeoutDefault 2000; `index.tsx:22-24` leader > 0.
//! - `index.tsx:65,115` mouse optional, default true.
//! - `index.tsx:55` theme free string; default "opencode" per
//!   `context/theme.tsx:121` (`config.theme ?? kv.get("theme", "opencode")`).
//! - `index.tsx:62-63` scroll_speed (>= 0.001) + scroll_acceleration{enabled}.
//! - animations is NOT in `Info`; session KV `animations_enabled` defaults true
//!   (`routes/session/index.tsx:260`, `app.tsx:898`, `spinner.tsx:17`).
//! Reuses `crate::scroll_accel::ScrollConfig` and
//! `crate::keymap::LEADER_TIMEOUT_DEFAULT_MS`; nothing redefined.

use crate::keymap::LEADER_TIMEOUT_DEFAULT_MS;
use crate::scroll_accel::{ScrollConfig, is_valid_speed};

/// Default theme name (`context/theme.tsx:121`).
pub const THEME_DEFAULT: &str = "opencode";
/// Max bytes for a theme name (fail-closed bound; TS leaves it unbounded).
pub const MAX_THEME_LEN: usize = 64;

/// Resolved TUI config; see `resolve()` (`index.tsx:88-117`) for defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct TuiConfig {
    pub mouse: bool,
    pub scroll: ScrollConfig,
    pub theme_name: String,
    pub leader_timeout_ms: u64,
    pub animations: bool,
}

/// Fail-closed config errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    EmptyTheme,
    ThemeTooLong,
    BadThemeChar,
    ZeroTimeout,
    BadScrollSpeed,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyTheme => f.write_str("empty theme name"),
            Self::ThemeTooLong => f.write_str("theme name too long"),
            Self::BadThemeChar => f.write_str("bad char in theme name"),
            Self::ZeroTimeout => f.write_str("leader timeout must be > 0"),
            Self::BadScrollSpeed => f.write_str("bad scroll speed"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl TuiConfig {
    /// TS `resolve()` defaults: mouse true, theme "opencode", leader 2000,
    /// animations on, scroll unset (falls back to default speed).
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            mouse: true,
            scroll: ScrollConfig::default(),
            theme_name: THEME_DEFAULT.to_string(),
            leader_timeout_ms: LEADER_TIMEOUT_DEFAULT_MS,
            animations: true,
        }
    }

    /// Fail-closed: theme non-empty, <= 64 bytes, no control chars;
    /// leader timeout > 0 (`index.tsx:22`); scroll speed finite + > 0 if set.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.theme_name.is_empty() {
            return Err(ConfigError::EmptyTheme);
        }
        if self.theme_name.len() > MAX_THEME_LEN {
            return Err(ConfigError::ThemeTooLong);
        }
        if self.theme_name.chars().any(|c| c.is_control()) {
            return Err(ConfigError::BadThemeChar);
        }
        if self.leader_timeout_ms == 0 {
            return Err(ConfigError::ZeroTimeout);
        }
        if let Some(v) = self.scroll.speed {
            if !is_valid_speed(v) {
                return Err(ConfigError::BadScrollSpeed);
            }
        }
        Ok(())
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_ts_resolve() {
        let c = TuiConfig::defaults();
        assert!(c.mouse); // index.tsx:115
        assert_eq!(c.theme_name, "opencode"); // theme.tsx:121
        assert_eq!(c.leader_timeout_ms, 2000); // index.tsx:21,114
        assert!(c.animations); // session/index.tsx:260
        assert_eq!(c.scroll, ScrollConfig::default());
        assert_eq!(TuiConfig::default(), c);
    }

    #[test]
    fn defaults_validate_ok() {
        assert_eq!(TuiConfig::defaults().validate(), Ok(()));
    }

    #[test]
    fn empty_theme_errs() {
        let mut c = TuiConfig::defaults();
        c.theme_name.clear();
        assert_eq!(c.validate(), Err(ConfigError::EmptyTheme));
    }

    #[test]
    fn long_or_control_theme_errs() {
        let mut c = TuiConfig::defaults();
        c.theme_name = "x".repeat(MAX_THEME_LEN + 1);
        assert_eq!(c.validate(), Err(ConfigError::ThemeTooLong));
        c.theme_name = "bad\nname".to_string();
        assert_eq!(c.validate(), Err(ConfigError::BadThemeChar));
    }

    #[test]
    fn zero_timeout_errs() {
        let mut c = TuiConfig::defaults();
        c.leader_timeout_ms = 0;
        assert_eq!(c.validate(), Err(ConfigError::ZeroTimeout));
    }

    #[test]
    fn bad_scroll_speed_errs() {
        for bad in [0.0, -1.5, f64::NAN, f64::INFINITY] {
            let mut c = TuiConfig::defaults();
            c.scroll.speed = Some(bad);
            assert_eq!(c.validate(), Err(ConfigError::BadScrollSpeed), "{bad:?}");
        }
        let mut c = TuiConfig::defaults();
        c.scroll.speed = Some(2.5);
        assert_eq!(c.validate(), Ok(()));
    }
}

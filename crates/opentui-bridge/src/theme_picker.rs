//! Cyclic picker over bundled theme assets. ponytail: index only; persistence later.
#![forbid(unsafe_code)]

use crate::theme_assets::{is_known, ASSET_NAMES};
use crate::theme_engine::ThemeEngine;

pub struct ThemePicker {
    pub engine: ThemeEngine,
    pub index: usize,
}

impl ThemePicker {
    fn index_of(n: &str) -> usize {
        ASSET_NAMES.iter().position(|&a| a == n).unwrap_or(0)
    }
    pub fn new() -> Self {
        let e = ThemeEngine::default();
        Self {
            index: Self::index_of(e.name()),
            engine: e,
        }
    }
    pub fn with_engine(engine: ThemeEngine) -> Self {
        Self {
            index: Self::index_of(engine.name()),
            engine,
        }
    }
    pub fn current(&self) -> &str {
        ASSET_NAMES[self.index]
    }
    pub fn next(&mut self) -> &str {
        if !self.engine.locked() {
            self.index = (self.index + 1) % ASSET_NAMES.len();
            let _ = self.engine.apply(ASSET_NAMES[self.index]);
        }
        self.current()
    }
    pub fn prev(&mut self) -> &str {
        if !self.engine.locked() {
            self.index = (self.index + ASSET_NAMES.len() - 1) % ASSET_NAMES.len();
            let _ = self.engine.apply(ASSET_NAMES[self.index]);
        }
        self.current()
    }
    pub fn apply_known(&mut self, name: &str) -> bool {
        if !is_known(name) || self.engine.locked() {
            return false;
        }
        if self.engine.apply(name) {
            self.index = Self::index_of(name);
            return true;
        }
        false
    }
}

impl Default for ThemePicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_current_is_opencode() {
        let p = ThemePicker::new();
        assert_eq!(p.current(), "opencode");
        assert_eq!(p.engine.name(), "opencode");
    }
    #[test]
    fn next_follows_asset_order() {
        let mut p = ThemePicker::new();
        let pos = ASSET_NAMES.iter().position(|&n| n == "opencode").unwrap();
        let want = ASSET_NAMES[(pos + 1) % ASSET_NAMES.len()];
        assert_eq!(p.next(), want);
        assert_eq!(p.engine.name(), want);
    }
    #[test]
    fn prev_wraps_from_first() {
        let e = ThemeEngine::new("aura", crate::theme_engine::ThemeMode::Dark);
        let mut p = ThemePicker::with_engine(e);
        assert_eq!(p.current(), "aura");
        assert_eq!(p.prev(), ASSET_NAMES[ASSET_NAMES.len() - 1]);
    }
    #[test]
    fn full_cycle_returns_to_start() {
        let mut p = ThemePicker::new();
        for _ in 0..ASSET_NAMES.len() {
            p.next();
        }
        assert_eq!(p.current(), "opencode");
    }
    #[test]
    fn apply_known_updates() {
        let mut p = ThemePicker::new();
        assert!(p.apply_known("dracula"));
        assert_eq!(p.current(), "dracula");
        assert_eq!(p.engine.name(), "dracula");
    }
    #[test]
    fn apply_unknown_false() {
        let mut p = ThemePicker::new();
        assert!(!p.apply_known("nope"));
        assert!(!p.apply_known(""));
        assert_eq!(p.current(), "opencode");
    }
    #[test]
    fn locked_blocks_all() {
        let mut p = ThemePicker::new();
        p.engine.toggle_lock();
        assert!(!p.apply_known("dracula"));
        let b = p.current().to_string();
        assert_eq!(p.next(), b);
        assert_eq!(p.prev(), b);
        assert_eq!(p.engine.name(), "opencode");
    }
}

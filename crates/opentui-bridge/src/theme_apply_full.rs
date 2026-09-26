#![forbid(unsafe_code)]
//! Apply gate for theme changes (FIX-53).
//!
//! Symptom: theme never applies; typo names accepted silently.
//! Rule: gate only. Pin blocks apply; free restores; set validates elsewhere.
//! pin(): lock changes. free_pin(): unlock. apply(): needs can_apply().
//! set(): rename/palette write, must check can_apply() first.
//! Overlap (5): context_theme::ThemeCtx, theme_engine::ThemeEngine,
//! ctx_theme_full::CtxTheme, theme_engine_full::ThemeFlow,
//! theme_picker::ThemePicker. None consulted here; gate decides only.
//! Collision: cli native_theme.rs emits fixed 8-slot BoundedColorMap;
//! this gate carries no colors, only Dark/Light + pin. No dup enum use.

/// Display mode. Dark default; Light opt-in. No System/Locked here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Mode name. Exact "dark" | "light"; typo input never reaches here.
#[must_use]
pub const fn mode_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    }
}

/// Pin gate. Pinned blocks apply fail-closed; free restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApplyGate {
    pub pinned: bool,
}

impl ApplyGate {
    /// Lock changes.
    pub fn pin(&mut self) {
        self.pinned = true;
    }
    /// Unlock changes.
    pub fn free_pin(&mut self) {
        self.pinned = false;
    }
    /// True only when unpinned.
    #[must_use]
    pub const fn can_apply(&self) -> bool {
        !self.pinned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_unpinned_applies() {
        assert!(ApplyGate::default().can_apply());
    }
    #[test]
    fn pin_blocks_apply() {
        let mut g = ApplyGate::default();
        g.pin();
        assert!(g.pinned && !g.can_apply());
    }
    #[test]
    fn free_pin_restores_apply() {
        let mut g = ApplyGate { pinned: true };
        g.free_pin();
        assert!(!g.pinned && g.can_apply());
    }
    #[test]
    fn mode_names_exact() {
        assert_eq!(mode_name(ThemeMode::Dark), "dark");
        assert_eq!(mode_name(ThemeMode::Light), "light");
    }
}

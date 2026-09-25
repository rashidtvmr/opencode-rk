#![forbid(unsafe_code)]
//! Theme mode engine mirroring `theme.tsx` mode/lock/apply/palette.
//!
//! TS ref: `theme.tsx` exposes a theme context with a display mode
//! (`dark` | `light` | `system`), a lock flag that blocks mode changes
//! fail-closed, an `apply` entry point, and a palette name accessor.

/// Maximum palette name length in chars.
pub const MAX_PALETTE_LEN: usize = 64;
/// Maximum entries returned by [`discover_themes`].
pub const MAX_DISCOVER: usize = 256;
/// Default palette name.
pub const DEFAULT_PALETTE: &str = "default";

/// Display mode mirroring `theme.tsx` (`dark` | `light` | `system`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Theme context: mode + fail-closed lock + capped palette name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCtx {
    mode: ThemeMode,
    locked: bool,
    palette: String,
}

impl Default for ThemeCtx {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            locked: false,
            palette: DEFAULT_PALETTE.to_string(),
        }
    }
}

impl ThemeCtx {
    #[must_use]
    pub fn new(mode: ThemeMode) -> Self {
        Self {
            mode,
            locked: false,
            palette: DEFAULT_PALETTE.to_string(),
        }
    }

    #[must_use]
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    #[must_use]
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Current palette name.
    #[must_use]
    pub fn palette_name(&self) -> &str {
        &self.palette
    }

    /// Set palette name, truncated to [`MAX_PALETTE_LEN`] chars.
    pub fn set_palette(&mut self, name: &str) {
        let capped: String = name.chars().take(MAX_PALETTE_LEN).collect();
        self.palette = if capped.is_empty() {
            DEFAULT_PALETTE.to_string()
        } else {
            capped
        };
    }

    /// Apply a mode; returns `false` fail-closed while locked.
    pub fn apply(&mut self, mode: ThemeMode) -> bool {
        if self.locked {
            return false;
        }
        self.mode = mode;
        true
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }
}

/// Pass-through of known theme names, capped at [`MAX_DISCOVER`].
#[must_use]
pub fn discover_themes(known: &[String]) -> Vec<String> {
    known.iter().take(MAX_DISCOVER).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_switches_mode_when_unlocked() {
        let mut ctx = ThemeCtx::new(ThemeMode::Dark);
        assert!(ctx.apply(ThemeMode::Light));
        assert_eq!(ctx.mode(), ThemeMode::Light);
    }

    #[test]
    fn locked_apply_blocks_fail_closed() {
        let mut ctx = ThemeCtx::new(ThemeMode::Dark);
        ctx.lock();
        assert!(!ctx.apply(ThemeMode::Light));
        assert_eq!(ctx.mode(), ThemeMode::Dark);
    }

    #[test]
    fn palette_defaults_to_default() {
        assert_eq!(ThemeCtx::default().palette_name(), DEFAULT_PALETTE);
    }

    #[test]
    fn discover_caps_at_256() {
        let known: Vec<String> = (0..300).map(|i| format!("t{i}")).collect();
        let out = discover_themes(&known);
        assert_eq!(out.len(), MAX_DISCOVER);
        assert_eq!(out[0], "t0");
    }

    #[test]
    fn lock_unlock_cycle_restores_apply() {
        let mut ctx = ThemeCtx::default();
        ctx.lock();
        assert!(ctx.locked());
        assert!(!ctx.apply(ThemeMode::System));
        ctx.unlock();
        assert!(!ctx.locked());
        assert!(ctx.apply(ThemeMode::System));
        assert_eq!(ctx.mode(), ThemeMode::System);
    }

    #[test]
    fn set_palette_truncates_to_64_chars() {
        let mut ctx = ThemeCtx::default();
        ctx.set_palette(&"x".repeat(100));
        assert_eq!(ctx.palette_name().chars().count(), MAX_PALETTE_LEN);
    }
}

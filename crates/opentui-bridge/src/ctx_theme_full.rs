#![forbid(unsafe_code)]
//! Theme name context mirroring `theme.tsx` palette name.

/// Maximum theme name length in chars.
pub const MAX_THEME_LEN: usize = 64;
/// Default theme name.
pub const DEFAULT_THEME: &str = "default";

/// Minimal theme name holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxTheme {
    name: String,
}

impl Default for CtxTheme {
    fn default() -> Self {
        Self {
            name: DEFAULT_THEME.to_string(),
        }
    }
}

impl CtxTheme {
    /// Set name, truncated to 64 chars; empty falls back to default.
    pub fn set(&mut self, name: &str) {
        let capped: String = name.chars().take(MAX_THEME_LEN).collect();
        self.name = if capped.is_empty() {
            DEFAULT_THEME.to_string()
        } else {
            capped
        };
    }

    /// Current name.
    #[must_use]
    pub fn name_of(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_default() {
        assert_eq!(CtxTheme::default().name_of(), DEFAULT_THEME);
    }

    #[test]
    fn set_roundtrips_name() {
        let mut c = CtxTheme::default();
        c.set("solarized");
        assert_eq!(c.name_of(), "solarized");
    }

    #[test]
    fn set_truncates_to_64() {
        let mut c = CtxTheme::default();
        c.set(&"x".repeat(100));
        assert_eq!(c.name_of().chars().count(), MAX_THEME_LEN);
    }
}

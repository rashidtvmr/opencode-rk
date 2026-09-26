#![forbid(unsafe_code)]
//! Full default theme index (mirrors `packages/tui/src/theme/index.ts` DEFAULT_THEMES).
//! ponytail: index over bundled names only, no JSON. Upgrade when defs load.

use crate::theme_assets::ASSET_NAMES;

/// Number of bundled default themes.
#[must_use]
pub fn theme_count() -> usize {
    ASSET_NAMES.len()
}

/// Name at index, or empty when out of bounds.
#[must_use]
pub fn theme_name(i: usize) -> &'static str {
    ASSET_NAMES.get(i).copied().unwrap_or("")
}

/// True when `name` is a bundled default theme.
#[must_use]
pub fn has_theme(name: &str) -> bool {
    ASSET_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_is_33() {
        assert_eq!(theme_count(), 33);
    }
    #[test]
    fn name_roundtrip() {
        assert_eq!(theme_name(0), "aura");
        assert_eq!(theme_name(32), "zenburn");
        assert_eq!(theme_name(33), "");
    }
    #[test]
    fn has_known_unknown() {
        assert!(has_theme("opencode"));
        assert!(has_theme("one-dark"));
        assert!(!has_theme("nope"));
        assert!(!has_theme(""));
    }
    #[test]
    fn index_matches_has() {
        for i in 0..theme_count() {
            assert!(has_theme(theme_name(i)));
        }
    }
}

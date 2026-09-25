#![forbid(unsafe_code)]
//! Full 33-theme picker catalog: names only, no colors.
//! Divergence: approximations-vs-copies - `theme.rs` defaults are
//! hand-picked dark approximations, NOT copies of any `assets/*.json`;
//! this catalog lists picker names (mirrors `theme_assets::ASSET_NAMES`)
//! without inventing color values (33 JSON files unvendored).
//! ponytail: name list only. Upgrade when JSON vending accepted.

/// All 33 picker theme names, sorted.
pub const THEME_NAMES: [&str; 33] = [
    "aura",
    "ayu",
    "carbonfox",
    "catppuccin",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "cobalt2",
    "cursor",
    "dracula",
    "everforest",
    "flexoki",
    "github",
    "gruvbox",
    "kanagawa",
    "lucent-orng",
    "material",
    "matrix",
    "mercury",
    "monokai",
    "nightowl",
    "nord",
    "one-dark",
    "opencode",
    "orng",
    "osaka-jade",
    "palenight",
    "rosepine",
    "solarized",
    "synthwave84",
    "tokyonight",
    "vercel",
    "vesper",
    "zenburn",
];

/// Number of catalog themes.
#[must_use]
pub fn theme_count() -> usize {
    THEME_NAMES.len()
}

/// Name at index, fail-closed to first on out-of-range.
#[must_use]
pub fn theme_name(i: usize) -> &'static str {
    THEME_NAMES.get(i).copied().unwrap_or(THEME_NAMES[0])
}

/// True when `name` is in the catalog.
#[must_use]
pub fn has_theme(name: &str) -> bool {
    THEME_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_is_33() {
        assert_eq!(theme_count(), 33);
    }
    #[test]
    fn fail_closed_first() {
        assert_eq!(theme_name(0), "aura");
        assert_eq!(theme_name(99), "aura");
    }
    #[test]
    fn has_known_unknown() {
        assert!(has_theme("opencode") && has_theme("dracula"));
        assert!(!has_theme("nope") && !has_theme(""));
    }
}

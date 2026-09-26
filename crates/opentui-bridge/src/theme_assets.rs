#![forbid(unsafe_code)]
//! Built-in theme asset names (mirrors `packages/tui/src/theme/assets/`).
//! ponytail: name list only, no JSON parsing. Upgrade when asset loading accepted.

/// Basenames (no `.json`) of all bundled `assets/*.json` themes, sorted.
pub const ASSET_NAMES: [&str; 33] = [
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

/// True when `name` is a bundled asset basename.
pub fn is_known(name: &str) -> bool {
    ASSET_NAMES.contains(&name)
}

/// `[name, "opencode"]` for known names, else `["opencode", "opencode"]`.
pub fn fallback_chain(name: &str) -> [&'static str; 2] {
    match ASSET_NAMES.iter().find(|&&n| n == name) {
        Some(n) => [*n, "opencode"],
        None => ["opencode", "opencode"],
    }
}

/// Number of bundled assets.
pub fn asset_count() -> usize {
    ASSET_NAMES.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_is_33() {
        assert_eq!(asset_count(), 33);
        assert_eq!(ASSET_NAMES.len(), 33);
    }
    #[test]
    fn known_opencode() {
        assert!(is_known("opencode"));
    }
    #[test]
    fn known_tokyonight_dracula() {
        assert!(is_known("tokyonight"));
        assert!(is_known("dracula"));
    }
    #[test]
    fn unknown_false() {
        assert!(!is_known("nope"));
        assert!(!is_known(""));
    }
    #[test]
    fn fallback_known() {
        assert_eq!(fallback_chain("dracula"), ["dracula", "opencode"]);
    }
    #[test]
    fn fallback_unknown() {
        assert_eq!(fallback_chain("nope"), ["opencode", "opencode"]);
    }
}

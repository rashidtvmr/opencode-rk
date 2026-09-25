#![forbid(unsafe_code)]
//! Run theme pick (mirrors `cli/cmd/run/theme.ts` pick/fallback @ a0d9b6c).
//!
//! TS `resolveRunTheme` probes palette, picks dark/light, falls back.
//! ponytail: name+mode only, no palette. Upgrade when full RunTheme port lands.

/// Max theme name length (bytes/chars).
pub const MAX_THEME_NAME_LEN: usize = 64;

/// Minimal run theme selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTheme {
    pub name: String,
    pub dark: bool,
}

/// Pick theme; empty/blank name defaults to `"opencode"`, truncates to 64.
#[must_use]
pub fn pick_theme(name: &str, dark: bool) -> RunTheme {
    let t = name.trim();
    let t = if t.is_empty() { "opencode" } else { t };
    let name: String = t.chars().take(MAX_THEME_NAME_LEN).collect();
    RunTheme { name, dark }
}

/// Label as `"name (dark|light)"`.
#[must_use]
pub fn theme_label(t: &RunTheme) -> String {
    format!("{} ({})", t.name, if t.dark { "dark" } else { "light" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_ok() {
        let t = pick_theme("tokyo", true);
        assert_eq!(t.name, "tokyo");
        assert!(t.dark);
    }

    #[test]
    fn empty_defaults() {
        assert_eq!(pick_theme("", true).name, "opencode");
        assert_eq!(pick_theme("   ", false).name, "opencode");
    }

    #[test]
    fn label_dark() {
        let t = pick_theme("tokyo", true);
        assert_eq!(theme_label(&t), "tokyo (dark)");
    }

    #[test]
    fn label_light() {
        let t = pick_theme("tokyo", false);
        assert_eq!(theme_label(&t), "tokyo (light)");
    }

    #[test]
    fn truncates_64() {
        let long = "x".repeat(100);
        let t = pick_theme(&long, true);
        assert_eq!(t.name.len(), 64);
    }
}

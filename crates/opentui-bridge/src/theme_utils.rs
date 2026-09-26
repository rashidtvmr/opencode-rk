#![forbid(unsafe_code)]
//! Theme slot + pack helpers over `theme.rs` / `theme_registry.rs`.
//!
//! ponytail: free fns, no new types. Upgrade when JSON pack loading lands.

use crate::theme::Theme;
use crate::theme_registry::ThemeRegistry;

/// Max names `pack_options` returns (registry caps at 64; 256 is headroom).
pub const MAX_PACK_OPTIONS: usize = 256;

/// Canonical slot name when known (`Theme::names`), else `None`.
#[must_use]
pub fn theme_slot(name: &str) -> Option<String> {
    if Theme::names().contains(&name) {
        Some(name.to_string())
    } else {
        None
    }
}

/// Sorted theme names from a registry, capped at 256.
#[must_use]
pub fn pack_options(registry: &ThemeRegistry) -> Vec<String> {
    let mut out: Vec<String> = registry.list().iter().map(|s| s.to_string()).collect();
    out.sort();
    out.truncate(MAX_PACK_OPTIONS);
    out
}

/// 6 border glyphs `[tl, tr, bl, br, h, v]` for a style name.
/// Unknown styles fall back to `single`.
#[must_use]
pub fn border_chars(style: &str) -> [char; 6] {
    match style {
        "double" => [
            '\u{2554}', '\u{2557}', '\u{255a}', '\u{255d}', '\u{2550}', '\u{2551}',
        ],
        "rounded" => [
            '\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2500}', '\u{2502}',
        ],
        "empty" => [' '; 6],
        _ => [
            '\u{250c}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{2500}', '\u{2502}',
        ],
    }
}

/// One-step static fallback for a slot; unknown slots fall back to `text`.
#[must_use]
pub fn selected_fallback(slot: &str) -> &'static str {
    match slot {
        "selected_list_item_text" | "text_muted" => "text",
        "background_panel" | "background_element" | "background_menu" => "background",
        "border_active" | "border_subtle" => "border",
        "secondary" | "accent" | "info" => "primary",
        "text" | "primary" | "background" | "border" => "text",
        _ if Theme::names().contains(&slot) => "text",
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    #[test]
    fn known_slot_some() {
        assert_eq!(theme_slot("primary"), Some("primary".to_string()));
        assert_eq!(theme_slot("text_muted"), Some("text_muted".to_string()));
    }

    #[test]
    fn unknown_slot_none() {
        assert_eq!(theme_slot("nope"), None);
        assert_eq!(theme_slot(""), None);
    }

    #[test]
    fn pack_sorted_capped() {
        let mut r = ThemeRegistry::new();
        let t = Theme::default_opencode();
        r.add("zebra", t);
        r.add("apple", t);
        r.add("mango", t);
        let opts = pack_options(&r);
        assert_eq!(
            opts,
            vec![
                "apple".to_string(),
                "mango".to_string(),
                "zebra".to_string()
            ]
        );
        assert!(opts.len() <= MAX_PACK_OPTIONS);
        assert!(pack_options(&ThemeRegistry::new()).is_empty());
    }

    #[test]
    fn border_single_rounded() {
        assert_eq!(
            border_chars("single"),
            ['\u{250c}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{2500}', '\u{2502}']
        );
        assert_eq!(
            border_chars("rounded"),
            ['\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2500}', '\u{2502}']
        );
        assert_eq!(border_chars("double")[4], '\u{2550}');
        assert_eq!(border_chars("empty"), [' '; 6]);
    }

    #[test]
    fn fallback_nonempty() {
        for s in [
            "selected_list_item_text",
            "border_active",
            "background_panel",
            "nope",
            "",
        ] {
            assert!(!selected_fallback(s).is_empty(), "{s}");
        }
        assert_eq!(selected_fallback("text"), "text");
    }

    #[test]
    fn registry_roundtrip_unused_rgba() {
        let _ = Rgba::rgb(1, 2, 3);
        let mut r = ThemeRegistry::new();
        r.add("only", Theme::default_opencode());
        assert_eq!(pack_options(&r), vec!["only".to_string()]);
    }
}

#![forbid(unsafe_code)]
//! Theme registry + color math (mirrors `packages/tui/src/theme/index.ts` @ a0d9b6c).
//!
//! Reuses `crate::theme::{Theme, SyntaxPalette}` and `crate::color::Rgba`
//! (incl. `Rgba::from_ansi256`, whose table math matches TS `ansiToRgba`).
//! Integrator: add `pub mod theme_registry;` to `lib.rs` (not touched here).
//! Divergence: no Solid `subscribeThemes` listeners (stays TS-side); no
//! bundled `assets/*.json` defaults, plugin/system layers, defs/ref chains,
//! or `{dark,light}` variants - string specs only (`#hex`, `transparent`,
//! `none`, decimal `0`-`255`). `generate_gray_scale` uses canonical extreme
//! backgrounds (black/white, hitting the TS `lum<10` / `lum>245` branches);
//! arbitrary-bg ratio path stays TS-side. `muted_text_color` takes bg
//! luminance `u8` and infers dark/light at midpoint 128 (TS takes `isDark`
//! from terminal mode).

use crate::color::Rgba;
use crate::theme::Theme;

/// Registry bound (TS maps are unbounded; cap keeps native side predictable).
pub const MAX_THEMES: usize = 64;

/// Parse a TS `ColorValue` string form: `#rrggbb`/`#rgb` (TS index.ts:248),
/// `transparent`/`none` (TS :246), decimal ANSI code (TS :260-262).
/// Digits-only routes to ANSI (TS numbers); other strings to hex.
#[must_use]
pub fn resolve_color(spec: &str) -> Option<Rgba> {
    if spec == "transparent" || spec == "none" {
        return Some(Rgba::transparent());
    }
    if !spec.is_empty() && spec.bytes().all(|b| b.is_ascii_digit()) {
        return spec.parse::<u8>().ok().map(ansi_to_rgba);
    }
    Rgba::from_hex(spec)
}

/// TS `ansiToRgba` (index.ts:301-344). Delegates to `color.rs`
/// `ansi256_index_to_rgb`: 0-15 table, cube `x==0?0:x*40+55`
/// (0/95/135/175/215/255), grays `(code-232)*10+8`, fallback black.
#[must_use]
pub fn ansi_to_rgba(code: u8) -> Rgba {
    Rgba::from_ansi256(code)
}

/// TS `tint` (index.ts:346-351): per-lane lerp, rounded, opaque out.
#[must_use]
pub fn tint(base: Rgba, overlay: Rgba, alpha: f32) -> Rgba {
    let lerp = |b: u8, o: u8| {
        ((f32::from(b) + (f32::from(o) - f32::from(b)) * alpha).round() as i32).clamp(0, 255) as u8
    };
    Rgba::rgb(lerp(base.r, overlay.r), lerp(base.g, overlay.g), lerp(base.b, overlay.b))
}

/// TS `generateGrayScale` extreme-bg branches (index.ts:489-517), slots 1-12.
/// Dark (black bg, lum 0 < 10): `floor(factor*0.4*255)`; light (white bg,
/// lum 255 > 245): `floor(255-factor*0.4*255)`. Index 0 == TS key 1.
#[must_use]
pub fn generate_gray_scale(dark: bool) -> [Rgba; 12] {
    let mut out = [Rgba::rgb(0, 0, 0); 12];
    for (i, slot) in out.iter_mut().enumerate() {
        let v = (((i as f32 + 1.0) / 12.0 * 0.4 * 255.0).floor()) as u8;
        *slot = if dark { Rgba::rgb(v, v, v) } else { Rgba::rgb(255 - v, 255 - v, 255 - v) };
    }
    out
}

/// TS `generateMutedTextColor` (index.ts:525-554): lum<10 -> 180 (`#b4b4b4`),
/// lum>245 -> 75 (`#4b4b4b`); mid ranges scaled, dark/light split at 128.
#[must_use]
pub fn muted_text_color(bg_lum: u8) -> Rgba {
    let lum = f32::from(bg_lum);
    let v = if bg_lum < 10 {
        180
    } else if bg_lum > 245 {
        75
    } else if bg_lum < 128 {
        (160.0 + lum * 0.3).floor().min(200.0) as u8
    } else {
        (100.0 - (255.0 - lum) * 0.2).floor().max(60.0) as u8
    };
    Rgba::rgb(v, v, v)
}

/// TS `selectedForeground` (index.ts:95-111): explicit value wins; transparent
/// bg -> black/white contrast (`lum>0.5` on 0-1 floats == `>127.5` on u8);
/// else background.
#[must_use]
pub fn selected_foreground(theme: &Theme, bg: Option<Rgba>) -> Rgba {
    if theme.has_selected_list_item_text {
        return theme.selected_list_item_text;
    }
    if theme.background.a == 0 {
        let t = bg.unwrap_or(theme.primary);
        let lum = 0.299 * f32::from(t.r) + 0.587 * f32::from(t.g) + 0.114 * f32::from(t.b);
        if lum > 127.5 {
            return Rgba::rgb(0, 0, 0);
        }
        return Rgba::rgb(255, 255, 255);
    }
    theme.background
}

/// Bounded named-theme store (TS `addTheme`/`hasTheme`/`allThemes`/:215-228,
/// `upsertTheme`/:229-239 minus listeners). Empty = `new()`.
#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry {
    entries: Vec<(String, Theme)>,
}

impl ThemeRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Insert; false if name empty, present (TS `addTheme`), or full.
    pub fn add(&mut self, name: &str, theme: Theme) -> bool {
        if name.is_empty() || self.has(name) || self.entries.len() >= MAX_THEMES {
            return false;
        }
        self.entries.push((name.to_string(), theme));
        true
    }

    /// Copy of stored theme (TS `allThemes()[name]`).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Theme> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, t)| *t)
    }

    /// TS `hasTheme` (empty name -> false).
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        !name.is_empty() && self.entries.iter().any(|(n, _)| n == name)
    }

    /// Insert or overwrite (TS `upsertTheme` minus plugin/custom split).
    /// False if name empty, or full when name is new.
    pub fn set_custom(&mut self, name: &str, theme: Theme) -> bool {
        if name.is_empty() {
            return false;
        }
        if let Some(slot) = self.entries.iter_mut().find(|(n, _)| n == name) {
            slot.1 = theme;
            return true;
        }
        if self.entries.len() >= MAX_THEMES {
            return false;
        }
        self.entries.push((name.to_string(), theme));
        true
    }

    /// Names in insertion order.
    #[must_use]
    pub fn list(&self) -> Vec<&str> {
        self.entries.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no themes stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_hex_literals() {
        assert_eq!(resolve_color("#ff0000"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(resolve_color("#00ff00"), Some(Rgba::rgb(0, 255, 0)));
        assert_eq!(resolve_color("#0000ff"), Some(Rgba::rgb(0, 0, 255)));
        assert_eq!(resolve_color("#c0c0c0"), Some(Rgba::rgb(192, 192, 192)));
        assert_eq!(resolve_color("#800000"), Some(Rgba::rgb(128, 0, 0)));
    }

    #[test]
    fn resolve_transparent_forms() {
        assert_eq!(resolve_color("transparent"), Some(Rgba::transparent()));
        assert_eq!(resolve_color("none"), Some(Rgba::transparent()));
    }

    #[test]
    fn resolve_ansi_numbers() {
        assert_eq!(resolve_color("9"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(resolve_color("0"), Some(Rgba::rgb(0, 0, 0)));
        assert_eq!(resolve_color("bogus!!!"), None);
    }

    #[test]
    fn ansi_table_0_15() {
        assert_eq!(ansi_to_rgba(0), Rgba::rgb(0, 0, 0));
        assert_eq!(ansi_to_rgba(1), Rgba::rgb(0x80, 0, 0));
        assert_eq!(ansi_to_rgba(7), Rgba::rgb(0xc0, 0xc0, 0xc0));
        assert_eq!(ansi_to_rgba(9), Rgba::rgb(255, 0, 0));
        assert_eq!(ansi_to_rgba(15), Rgba::rgb(255, 255, 255));
    }

    #[test]
    fn ansi_cube_16_231() {
        assert_eq!(ansi_to_rgba(16), Rgba::rgb(0, 0, 0));
        assert_eq!(ansi_to_rgba(21), Rgba::rgb(0, 0, 255));
        assert_eq!(ansi_to_rgba(46), Rgba::rgb(0, 255, 0));
        assert_eq!(ansi_to_rgba(196), Rgba::rgb(255, 0, 0));
        assert_eq!(ansi_to_rgba(231), Rgba::rgb(255, 255, 255));
    }

    #[test]
    fn ansi_grays_232_255() {
        assert_eq!(ansi_to_rgba(232), Rgba::rgb(8, 8, 8));
        assert_eq!(ansi_to_rgba(244), Rgba::rgb(128, 128, 128));
        assert_eq!(ansi_to_rgba(255), Rgba::rgb(238, 238, 238));
    }

    #[test]
    fn tint_midpoint_rounds() {
        let b = Rgba::rgb(0, 0, 0);
        let w = Rgba::rgb(255, 255, 255);
        assert_eq!(tint(b, w, 0.5), Rgba::rgb(128, 128, 128));
        assert_eq!(tint(b, w, 0.0), b);
        assert_eq!(tint(b, w, 1.0), w);
    }

    #[test]
    fn gray_scale_extremes() {
        let dark = generate_gray_scale(true);
        assert_eq!(dark[0], Rgba::rgb(8, 8, 8));
        assert_eq!(dark[11], Rgba::rgb(102, 102, 102));
        let light = generate_gray_scale(false);
        assert_eq!(light[0], Rgba::rgb(247, 247, 247));
        assert_eq!(light[11], Rgba::rgb(153, 153, 153));
    }

    #[test]
    fn muted_extremes() {
        assert_eq!(muted_text_color(0), Rgba::rgb(180, 180, 180));
        assert_eq!(muted_text_color(255), Rgba::rgb(75, 75, 75));
    }

    #[test]
    fn registry_crud() {
        let mut r = ThemeRegistry::new();
        let t = Theme::default_opencode();
        assert!(r.is_empty());
        assert!(r.add("opencode", t));
        assert!(!r.add("opencode", t));
        assert!(!r.add("", t));
        assert!(r.has("opencode"));
        assert!(!r.has(""));
        assert_eq!(r.get("opencode"), Some(t));
        assert_eq!(r.get("missing"), None);
        let mut t2 = t;
        t2.text = Rgba::rgb(1, 2, 3);
        assert!(r.set_custom("opencode", t2));
        assert_eq!(r.get("opencode"), Some(t2));
        assert!(r.set_custom("extra", t));
        assert_eq!(r.list(), vec!["opencode", "extra"]);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn registry_bounded_64() {
        let mut r = ThemeRegistry::new();
        let t = Theme::default_opencode();
        for i in 0..MAX_THEMES {
            assert!(r.add(&format!("t{i}"), t), "{i}");
        }
        assert!(!r.add("overflow", t));
        assert!(!r.set_custom("new-name", t));
        assert!(r.set_custom("t0", t));
    }

    #[test]
    fn selected_foreground_contrast() {
        let mut t = Theme::default_opencode();
        t.has_selected_list_item_text = false;
        t.background = Rgba::transparent();
        assert_eq!(selected_foreground(&t, None), Rgba::rgb(0, 0, 0)); // cyan primary lum 174.9
        assert_eq!(
            selected_foreground(&t, Some(Rgba::rgb(0, 0, 0))),
            Rgba::rgb(255, 255, 255)
        );
        let full = Theme::default_opencode();
        assert_eq!(selected_foreground(&full, None), full.selected_list_item_text);
    }
}

#![forbid(unsafe_code)]
//! Focused-state style overrides (BRIDGE-GAP-12).
//!
//! Homes homeless fields from `renderables.rs`: `focusedTextColor`/`textColor`
//! (`ui/dialog-select.tsx:580,582`), `focusedBackgroundColor`
//! (`ui/dialog-select.tsx:580`), `height` (`ui/dialog-export-options.tsx:108`).
//! `renderables.rs` keeps `Focusable { focusable, focused }` + `InputStyle`
//! base colors; this module owns the focused-override resolution.

use crate::color::Rgba;

/// Focused-state overrides (`dialog-select.tsx:580,582`,
/// `dialog-export-options.tsx:108`). `None` = keep base.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusedStyle {
    pub focused_text_color: Option<Rgba>,
    pub focused_background_color: Option<Rgba>,
    pub height: Option<u16>,
}

impl FocusedStyle {
    /// Resolve `(text, bg)`: unfocused = base passthrough; focused = override
    /// per lane, `None` falls back to base.
    #[must_use]
    pub const fn apply(&self, base_text: Rgba, base_bg: Rgba, focused: bool) -> (Rgba, Rgba) {
        if !focused {
            return (base_text, base_bg);
        }
        let text = match self.focused_text_color {
            Some(c) => c,
            None => base_text,
        };
        let bg = match self.focused_background_color {
            Some(c) => c,
            None => base_bg,
        };
        (text, bg)
    }

    /// Explicit `height` or caller default (`None` = default).
    #[must_use]
    pub const fn effective_height(&self, default: u16) -> u16 {
        match self.height {
            Some(h) => h,
            None => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_TEXT: Rgba = Rgba::rgb(1, 2, 3);
    const BASE_BG: Rgba = Rgba::rgb(4, 5, 6);

    #[test]
    fn unfocused_passthrough() {
        let s = FocusedStyle {
            focused_text_color: Some(Rgba::rgb(9, 9, 9)),
            focused_background_color: Some(Rgba::rgb(8, 8, 8)),
            height: Some(3),
        };
        assert_eq!(s.apply(BASE_TEXT, BASE_BG, false), (BASE_TEXT, BASE_BG));
    }

    #[test]
    fn focused_override() {
        let s = FocusedStyle {
            focused_text_color: Some(Rgba::rgb(9, 9, 9)),
            focused_background_color: Some(Rgba::rgb(8, 8, 8)),
            height: None,
        };
        assert_eq!(
            s.apply(BASE_TEXT, BASE_BG, true),
            (Rgba::rgb(9, 9, 9), Rgba::rgb(8, 8, 8))
        );
    }

    #[test]
    fn partial_none_falls_back_to_base() {
        let s = FocusedStyle {
            focused_text_color: None,
            focused_background_color: Some(Rgba::rgb(8, 8, 8)),
            ..Default::default()
        };
        assert_eq!(
            s.apply(BASE_TEXT, BASE_BG, true),
            (BASE_TEXT, Rgba::rgb(8, 8, 8))
        );
    }

    #[test]
    fn height_none_uses_default() {
        assert_eq!(FocusedStyle::default().effective_height(5), 5);
        let s = FocusedStyle {
            height: Some(3),
            ..Default::default()
        };
        assert_eq!(s.effective_height(5), 3);
    }
}

#![forbid(unsafe_code)]
//! Small semantic color bridge for transcript regions.
//!
//! Region defaults follow the bridge theme surface. Known theme names get
//! stable palettes; unknown names deliberately use the mode defaults.

use crate::color::Rgba;
use crate::theme::Theme;

/// Colors used by the transcript, composer, status line, and selected item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionColors {
    pub transcript_bg: Rgba,
    pub composer_bg: Rgba,
    pub status_fg: Rgba,
    pub selected_bg: Rgba,
}

struct Palette {
    transcript_bg: Rgba,
    composer_bg: Rgba,
    status_fg: Rgba,
    selected_bg: Option<Rgba>,
}

fn defaults(is_dark: bool) -> RegionColors {
    if is_dark {
        let theme = Theme::default_opencode();
        RegionColors {
            transcript_bg: theme.background,
            composer_bg: theme.background_panel,
            status_fg: theme.text_muted,
            // selectedListItemText compatibility fallback is background.
            selected_bg: theme.background,
        }
    } else {
        RegionColors {
            transcript_bg: Rgba::rgb(0xf5, 0xf5, 0xf7),
            composer_bg: Rgba::rgb(0xff, 0xff, 0xff),
            status_fg: Rgba::rgb(0x4a, 0x4a, 0x4a),
            selected_bg: Rgba::rgb(0xf5, 0xf5, 0xf7),
        }
    }
}

fn from_palette(palette: Palette) -> RegionColors {
    RegionColors {
        transcript_bg: palette.transcript_bg,
        composer_bg: palette.composer_bg,
        status_fg: palette.status_fg,
        selected_bg: palette.selected_bg.unwrap_or(palette.transcript_bg),
    }
}

fn palette(theme_name: &str, is_dark: bool) -> Option<RegionColors> {
    let name = theme_name.trim().to_ascii_lowercase();
    let selected_bg = if is_dark { None } else { None };
    let base = match name.as_str() {
        "opencode" | "default" => return Some(defaults(is_dark)),
        "tokyonight" | "tokyo-night" => Palette {
            transcript_bg: Rgba::rgb(0x1a, 0x1b, 0x26),
            composer_bg: Rgba::rgb(0x24, 0x28, 0x3b),
            status_fg: Rgba::rgb(0xa9, 0xb1, 0xd6),
            selected_bg,
        },
        "gruvbox" => Palette {
            transcript_bg: Rgba::rgb(0x28, 0x28, 0x28),
            composer_bg: Rgba::rgb(0x3c, 0x38, 0x36),
            status_fg: Rgba::rgb(0xa8, 0x99, 0x84),
            selected_bg,
        },
        "catppuccin" | "catppuccin-mocha" => Palette {
            transcript_bg: Rgba::rgb(0x1e, 0x1e, 0x2e),
            composer_bg: Rgba::rgb(0x31, 0x32, 0x44),
            status_fg: Rgba::rgb(0xa6, 0xad, 0xc8),
            selected_bg,
        },
        "nord" => Palette {
            transcript_bg: Rgba::rgb(0x2e, 0x34, 0x40),
            composer_bg: Rgba::rgb(0x3b, 0x42, 0x52),
            status_fg: Rgba::rgb(0xd8, 0xde, 0xe9),
            selected_bg,
        },
        "dracula" => Palette {
            transcript_bg: Rgba::rgb(0x28, 0x2a, 0x36),
            composer_bg: Rgba::rgb(0x44, 0x47, 0x5a),
            status_fg: Rgba::rgb(0xf8, 0xf8, 0xf2),
            selected_bg,
        },
        "solarized" => Palette {
            transcript_bg: Rgba::rgb(0x00, 0x2b, 0x36),
            composer_bg: Rgba::rgb(0x07, 0x36, 0x42),
            status_fg: Rgba::rgb(0x93, 0xa1, 0xa1),
            selected_bg,
        },
        "everforest" => Palette {
            transcript_bg: Rgba::rgb(0x2d, 0x35, 0x3b),
            composer_bg: Rgba::rgb(0x34, 0x3f, 0x44),
            status_fg: Rgba::rgb(0xd3, 0xc6, 0xaa),
            selected_bg,
        },
        "rose-pine" | "rosepine" => Palette {
            transcript_bg: Rgba::rgb(0x19, 0x17, 0x24),
            composer_bg: Rgba::rgb(0x1f, 0x1d, 0x2e),
            status_fg: Rgba::rgb(0x90, 0x8c, 0xaa),
            selected_bg,
        },
        "ayu" => Palette {
            transcript_bg: Rgba::rgb(0x0b, 0x0e, 0x14),
            composer_bg: Rgba::rgb(0x11, 0x15, 0x1c),
            status_fg: Rgba::rgb(0xb3, 0xb1, 0xad),
            selected_bg,
        },
        _ => return None,
    };
    let _ = is_dark;
    Some(from_palette(base))
}

/// Resolve semantic region colors, falling back to safe mode defaults.
#[must_use]
pub fn paint_for(theme_name: &str, is_dark: bool) -> RegionColors {
    palette(theme_name, is_dark).unwrap_or_else(|| defaults(is_dark))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_and_light_regions_differ() {
        let dark = paint_for("opencode", true);
        let light = paint_for("opencode", false);
        assert_ne!(dark.transcript_bg, light.transcript_bg);
        assert_ne!(dark.composer_bg, light.composer_bg);
        assert_ne!(dark.status_fg, light.status_fg);
    }

    #[test]
    fn unknown_theme_uses_mode_fallback() {
        assert_eq!(
            paint_for("missing-theme", true),
            paint_for("opencode", true)
        );
        assert_eq!(
            paint_for("missing-theme", false),
            paint_for("opencode", false)
        );
    }

    #[test]
    fn selected_background_falls_back_to_transcript_background() {
        let colors = paint_for("tokyonight", true);
        assert_eq!(colors.selected_bg, colors.transcript_bg);
    }

    #[test]
    fn known_theme_keeps_named_palette() {
        assert_eq!(
            paint_for("tokyonight", true).transcript_bg,
            Rgba::rgb(0x1a, 0x1b, 0x26)
        );
        assert_eq!(
            paint_for("gruvbox", true).composer_bg,
            Rgba::rgb(0x3c, 0x38, 0x36)
        );
    }
}

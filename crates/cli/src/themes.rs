#![forbid(unsafe_code)]
//! App-scope theme selector (pure state).
//!
//! Thin name-only layer over `native_theme`: picks which builtin (or custom
//! file) the app wants. No colors, no registry, no IO here; caller resolves
//! [`AppTheme::registry_name`] against the real `ThemeRegistry`.

/// App-level theme names. Distinct from `native_theme::ThemeDef`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
    HighContrast,
}

impl AppTheme {
    /// Semantic palette slots for [`AppTheme::palette`].
    pub const NORMAL: u16 = 0;
    /// Accent slot.
    pub const ACCENT: u16 = 1;
    /// Dim slot.
    pub const DIM: u16 = 2;

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::HighContrast => "high-contrast",
        }
    }

    #[must_use]
    pub const fn registry_name(self) -> &'static str {
        match self {
            Self::Dark => "opencode-dark",
            Self::Light => "opencode-light",
            Self::HighContrast => "opencode-high-contrast",
        }
    }

    /// Fixed sRGB fg colors per slot on this theme's bg. Each `[u16; 4]` is
    /// `[r, g, b, a]` 0-255 (opaque). No `native_theme` dep; `const`-friendly.
    #[must_use]
    pub const fn palette(self) -> [(u16, [u16; 4]); 3] {
        match self {
            Self::Dark => [
                (Self::NORMAL, [0xE6, 0xE6, 0xE6, 0xFF]),
                (Self::ACCENT, [0x6C, 0x9E, 0xEE, 0xFF]),
                (Self::DIM, [0x7F, 0x84, 0x90, 0xFF]),
            ],
            Self::Light => [
                (Self::NORMAL, [0x1A, 0x1B, 0x1E, 0xFF]),
                (Self::ACCENT, [0x4A, 0x6F, 0xC4, 0xFF]),
                (Self::DIM, [0x9C, 0xA0, 0xAB, 0xFF]),
            ],
            Self::HighContrast => [
                (Self::NORMAL, [0xFF, 0xFF, 0xFF, 0xFF]),
                (Self::ACCENT, [0xFF, 0xFF, 0x00, 0xFF]),
                (Self::DIM, [0xC8, 0xC8, 0xC8, 0xFF]),
            ],
        }
    }
}

/// Chosen theme plus optional custom theme file. `None` = builtin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppThemeChoice {
    pub name: AppTheme,
    pub custom_path: Option<String>,
}

impl AppThemeChoice {
    #[must_use]
    pub const fn new(name: AppTheme) -> Self {
        Self {
            name,
            custom_path: None,
        }
    }

    #[must_use]
    pub fn with_custom_path(name: AppTheme, path: String) -> Self {
        Self {
            name,
            custom_path: Some(path),
        }
    }

    #[must_use]
    pub const fn pick(&self) -> AppTheme {
        self.name
    }

    #[must_use]
    pub fn custom_path(&self) -> Option<&str> {
        self.custom_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dark_builtin() {
        let c = AppThemeChoice::default();
        assert_eq!(c.pick(), AppTheme::Dark);
        assert_eq!(c.custom_path(), None);
        assert_eq!(AppTheme::Dark.registry_name(), "opencode-dark");
    }

    #[test]
    fn custom_path_preserved() {
        let c = AppThemeChoice::with_custom_path(AppTheme::Light, "/t/custom.json".into());
        assert_eq!(c.pick(), AppTheme::Light);
        assert_eq!(c.custom_path(), Some("/t/custom.json"));
    }

    #[test]
    fn names_distinct() {
        assert_eq!(AppTheme::Dark.name(), "dark");
        assert_eq!(AppTheme::HighContrast.registry_name(), "opencode-high-contrast");
    }

    fn slot(p: [(u16, [u16; 4]); 3], id: u16) -> [u16; 4] {
        p.iter().find(|(k, _)| *k == id).map(|(_, v)| *v).unwrap()
    }

    #[test]
    fn palette_variants_differ() {
        let d = AppTheme::Dark.palette();
        let l = AppTheme::Light.palette();
        let h = AppTheme::HighContrast.palette();
        assert_ne!(d, l);
        assert_ne!(d, h);
        assert_ne!(l, h);
    }

    #[test]
    fn palette_accent_distinct() {
        for t in [AppTheme::Dark, AppTheme::Light, AppTheme::HighContrast] {
            let p = t.palette();
            assert_eq!(p.len(), 3);
            assert_eq!([p[0].0, p[1].0, p[2].0], [AppTheme::NORMAL, AppTheme::ACCENT, AppTheme::DIM]);
            let (n, a, d) = (slot(p, 0), slot(p, 1), slot(p, 2));
            assert_ne!(a, n);
            assert_ne!(a, d);
            for c in [n, a, d] {
                assert_eq!(c[3], 0xFF);
            }
        }
    }
}

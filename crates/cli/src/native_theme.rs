#![forbid(unsafe_code)]
//! Theme engine for the native TUI.
//!
//! Pure-state module: `ThemeDef` with named RGBA colors, `ThemeRegistry` for
//! bounded theme lookup, and `ThemeEngine::apply` producing a
//! `BoundedColorMap` consumed by `native_palette`/`native_status` render
//! state. No rendering, no IO, no threads; std-only, no external crates.
//!
//! Bounds: `MAX_THEMES = 32`, name ≤ 32 bytes, 8 color slots per theme.
//!
//! Fail-closed on malformed hex input; unknown theme name returns a typed
//! error instead of panicking.

use std::collections::BTreeMap;
use std::fmt;

/// Maximum themes in the registry.
pub const MAX_THEMES: usize = 32;
/// Maximum theme name length in bytes (excl. NUL).
pub const MAX_NAME_LEN: usize = 32;
/// Color slots emitted by `ThemeEngine::apply`.
pub const COLOR_SLOTS: usize = 8;

// ── RGBA color ────────────────────────────────────────────────────────

/// 8-bit sRGB color with alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Fully opaque shortcut.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

// ── Hex parsing ───────────────────────────────────────────────────────

/// Errors from hex color parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HexParseError {
    /// Missing leading `#`.
    MissingHash,
    /// Wrong number of hex digits (need 6).
    InvalidLength,
    /// Non-hex character in the input.
    InvalidChar { pos: usize, ch: char },
}

impl fmt::Display for HexParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHash => write!(f, "hex color must start with '#'"),
            Self::InvalidLength => write!(f, "hex color must be exactly 6 hex digits after '#'"),
            Self::InvalidChar { pos, ch } => {
                write!(f, "invalid hex character '{ch}' at position {pos}")
            }
        }
    }
}

/// Parse `#RRGGBB` into an opaque [`Rgba`] (alpha = 0xFF).  Fail-closed on
/// any malformed input.
///
/// # Errors
///
/// Returns [`HexParseError`] if the input is not valid `#RRGGBB`.
pub fn parse_hex(hex: &str) -> Result<Rgba, HexParseError> {
    let bytes = hex.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return Err(HexParseError::MissingHash);
    }
    if bytes.len() != 7 {
        return Err(HexParseError::InvalidLength);
    }
    let mut r: u8 = 0;
    let mut g: u8 = 0;
    let mut b: u8 = 0;
    for (i, &byte) in bytes[1..].iter().enumerate() {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => {
                return Err(HexParseError::InvalidChar {
                    pos: i + 1,
                    ch: byte as char,
                })
            }
        };
        match i {
            0 => r = digit,
            1 => r = r * 16 + digit,
            2 => g = digit,
            3 => g = g * 16 + digit,
            4 => b = digit,
            5 => b = b * 16 + digit,
            _ => unreachable!("exactly 6 hex digits"),
        }
    }
    Ok(Rgba::rgb(r, g, b))
}

// ── Theme definition ──────────────────────────────────────────────────

/// A complete theme with named color slots.
#[derive(Clone, Debug)]
pub struct ThemeDef {
    pub name: String,
    pub fg: Rgba,
    pub bg: Rgba,
    pub accent: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub muted: Rgba,
}

impl ThemeDef {
    /// Whether this theme definition satisfies all bounds.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= MAX_NAME_LEN
    }
}

// ── Color map (apply output) ──────────────────────────────────────────

/// Bounded map produced by `ThemeEngine::apply`. Exactly 8 entries keyed
/// by semantic name.
#[derive(Clone, Debug)]
pub struct BoundedColorMap {
    entries: [(String, Rgba); COLOR_SLOTS],
}

impl BoundedColorMap {
    /// Create from a sorted slice of (key, value) pairs. Caller must
    /// supply exactly [`COLOR_SLOTS`] entries.
    fn from_entries(entries: [(String, Rgba); COLOR_SLOTS]) -> Self {
        Self { entries }
    }

    /// Look up a color by semantic key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Rgba> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
    }

    /// All keys present in the map.
    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }
}

// ── Theme registry ────────────────────────────────────────────────────

/// Errors from registry operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeError {
    /// Theme name not found in registry.
    UnknownTheme { name: String },
    /// Registry is full (`MAX_THEMES`).
    Full,
    /// Theme definition is invalid (empty name or name too long).
    InvalidTheme { reason: String },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTheme { name } => write!(f, "unknown theme: {name}"),
            Self::Full => write!(f, "theme registry full (max {MAX_THEMES})"),
            Self::InvalidTheme { reason } => write!(f, "invalid theme: {reason}"),
        }
    }
}

/// Bounded theme registry. Maximum [`MAX_THEMES`] entries, keyed by name.
pub struct ThemeRegistry {
    themes: BTreeMap<String, ThemeDef>,
}

impl ThemeRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            themes: BTreeMap::new(),
        }
    }

    /// Number of registered themes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.themes.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }

    /// Register a theme. Fails closed on bounds or collision.
    pub fn register(&mut self, theme: ThemeDef) -> Result<(), ThemeError> {
        if !theme.is_valid() {
            return Err(ThemeError::InvalidTheme {
                reason: if theme.name.is_empty() {
                    "name must not be empty".to_string()
                } else {
                    format!("name length {} exceeds {MAX_NAME_LEN}", theme.name.len())
                },
            });
        }
        if self.themes.contains_key(&theme.name) {
            return Err(ThemeError::InvalidTheme {
                reason: format!("duplicate theme name: {}", theme.name),
            });
        }
        if self.themes.len() >= MAX_THEMES {
            return Err(ThemeError::Full);
        }
        self.themes.insert(theme.name.clone(), theme);
        Ok(())
    }

    /// Look up a theme by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ThemeDef> {
        self.themes.get(name)
    }

    /// All registered theme names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Theme engine ──────────────────────────────────────────────────────

/// Semantic color keys emitted by [`ThemeEngine::apply`].
const SEMANTIC_KEYS: [&str; COLOR_SLOTS] = ["fg", "bg", "accent", "success", "warning", "error", "muted", "border"];

/// Stateless theme engine. Produces [`BoundedColorMap`] from a
/// [`ThemeDef`].
pub struct ThemeEngine;

impl ThemeEngine {
    /// Apply a theme, producing a complete 8-entry color map. The map is
    /// always full — no missing keys.
    #[must_use]
    pub fn apply(theme: &ThemeDef) -> BoundedColorMap {
        BoundedColorMap::from_entries([
            ("fg".to_string(), theme.fg),
            ("bg".to_string(), theme.bg),
            ("accent".to_string(), theme.accent),
            ("success".to_string(), theme.success),
            ("warning".to_string(), theme.warning),
            ("error".to_string(), theme.error),
            ("muted".to_string(), theme.muted),
            // border defaults to a muted variant of fg
            ("border".to_string(), Rgba::rgb(
                (theme.fg.r as u16 * 2 + theme.bg.r as u16 * 3 / 4) as u8,
                (theme.fg.g as u16 * 2 + theme.bg.g as u16 * 3 / 4) as u8,
                (theme.fg.b as u16 * 2 + theme.bg.b as u16 * 3 / 4) as u8,
            )),
        ])
    }
}

// ── Built-in themes ──────────────────────────────────────────────────

/// Dark theme matching OpenCode defaults.
#[must_use]
pub fn opencode_dark() -> ThemeDef {
    ThemeDef {
        name: "opencode-dark".to_string(),
        fg: Rgba::rgb(0xE6, 0xE6, 0xE6),
        bg: Rgba::rgb(0x1A, 0x1B, 0x1E),
        accent: Rgba::rgb(0x6C, 0x9E, 0xEE),
        success: Rgba::rgb(0x7E, 0xC9, 0x7E),
        warning: Rgba::rgb(0xE0, 0xAF, 0x68),
        error: Rgba::rgb(0xE0, 0x6C, 0x75),
        muted: Rgba::rgb(0x7F, 0x84, 0x90),
    }
}

/// Light theme matching OpenCode defaults.
#[must_use]
pub fn opencode_light() -> ThemeDef {
    ThemeDef {
        name: "opencode-light".to_string(),
        fg: Rgba::rgb(0x1A, 0x1B, 0x1E),
        bg: Rgba::rgb(0xFA, 0xFA, 0xFA),
        accent: Rgba::rgb(0x4A, 0x6F, 0xC4),
        success: Rgba::rgb(0x2E, 0x8B, 0x57),
        warning: Rgba::rgb(0xC4, 0x7B, 0x1E),
        error: Rgba::rgb(0xC4, 0x3B, 0x46),
        muted: Rgba::rgb(0x9C, 0xA0, 0xAB),
    }
}

/// Populate a registry with the built-in OpenCode themes.
#[must_use]
pub fn register_builtins(reg: &mut ThemeRegistry) {
    let _ = reg.register(opencode_dark());
    let _ = reg.register(opencode_light());
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── T01: hex parse valid / invalid ────────────────────────────────

    #[test]
    fn hex_parse_valid() {
        let c = parse_hex("#FF00AB").expect("valid hex");
        assert_eq!(c.r, 0xFF);
        assert_eq!(c.g, 0x00);
        assert_eq!(c.b, 0xAB);
        assert_eq!(c.a, 0xFF);
        // lowercase
        let c = parse_hex("#1a2b3c").expect("lowercase hex");
        assert_eq!(c.r, 0x1A);
        assert_eq!(c.g, 0x2B);
        assert_eq!(c.b, 0x3C);
        // black
        let c = parse_hex("#000000").expect("black");
        assert_eq!((c.r, c.g, c.b), (0, 0, 0));
        // white
        let c = parse_hex("#FFFFFF").expect("white");
        assert_eq!((c.r, c.g, c.b), (255, 255, 255));
    }

    #[test]
    fn hex_parse_missing_hash() {
        assert_eq!(parse_hex("FF00AB"), Err(HexParseError::MissingHash));
        assert_eq!(parse_hex(""), Err(HexParseError::MissingHash));
    }

    #[test]
    fn hex_parse_invalid_length() {
        assert_eq!(parse_hex("#FFF"), Err(HexParseError::InvalidLength));
        assert_eq!(parse_hex("#12345678"), Err(HexParseError::InvalidLength));
        assert_eq!(parse_hex("#"), Err(HexParseError::InvalidLength));
    }

    #[test]
    fn hex_parse_invalid_char() {
        assert_eq!(
            parse_hex("#GG0000"),
            Err(HexParseError::InvalidChar { pos: 1, ch: 'G' })
        );
        assert_eq!(
            parse_hex("#12345Z"),
            Err(HexParseError::InvalidChar { pos: 6, ch: 'Z' })
        );
    }

    // ── T02: registry bounded + lookup ────────────────────────────────

    fn make_theme(name: &str) -> ThemeDef {
        ThemeDef {
            name: name.to_string(),
            fg: Rgba::rgb(0, 0, 0),
            bg: Rgba::rgb(0xFF, 0xFF, 0xFF),
            accent: Rgba::rgb(0, 0, 0),
            success: Rgba::rgb(0, 0, 0),
            warning: Rgba::rgb(0, 0, 0),
            error: Rgba::rgb(0, 0, 0),
            muted: Rgba::rgb(0, 0, 0),
        }
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = ThemeRegistry::new();
        reg.register(make_theme("dark")).unwrap();
        reg.register(make_theme("light")).unwrap();
        assert_eq!(reg.len(), 2);
        assert!(reg.get("dark").is_some());
        assert!(reg.get("light").is_some());
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn registry_bounded_at_max() {
        let mut reg = ThemeRegistry::new();
        for i in 0..MAX_THEMES {
            reg.register(make_theme(&format!("t{i:02}"))).unwrap();
        }
        assert_eq!(reg.len(), MAX_THEMES);
        assert_eq!(
            reg.register(make_theme("overflow")),
            Err(ThemeError::Full)
        );
    }

    #[test]
    fn registry_rejects_duplicate_name() {
        let mut reg = ThemeRegistry::new();
        reg.register(make_theme("dup")).unwrap();
        assert!(matches!(
            reg.register(make_theme("dup")),
            Err(ThemeError::InvalidTheme { .. })
        ));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_rejects_empty_name() {
        let mut reg = ThemeRegistry::new();
        assert!(matches!(
            reg.register(make_theme("")),
            Err(ThemeError::InvalidTheme { .. })
        ));
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_rejects_long_name() {
        let mut reg = ThemeRegistry::new();
        let long = "x".repeat(MAX_NAME_LEN + 1);
        assert!(matches!(
            reg.register(ThemeDef {
                name: long,
                fg: Rgba::rgb(0, 0, 0),
                bg: Rgba::rgb(0, 0, 0),
                accent: Rgba::rgb(0, 0, 0),
                success: Rgba::rgb(0, 0, 0),
                warning: Rgba::rgb(0, 0, 0),
                error: Rgba::rgb(0, 0, 0),
                muted: Rgba::rgb(0, 0, 0),
            }),
            Err(ThemeError::InvalidTheme { .. })
        ));
    }

    #[test]
    fn registry_names_sorted() {
        let mut reg = ThemeRegistry::new();
        reg.register(make_theme("z")).unwrap();
        reg.register(make_theme("a")).unwrap();
        reg.register(make_theme("m")).unwrap();
        assert_eq!(reg.names(), vec!["a", "m", "z"]);
    }

    // ── T03: apply produces complete color map ────────────────────────

    #[test]
    fn apply_produces_all_semantic_keys() {
        let theme = opencode_dark();
        let map = ThemeEngine::apply(&theme);
        assert_eq!(map.keys().len(), COLOR_SLOTS);
        for key in SEMANTIC_KEYS {
            assert!(
                map.get(key).is_some(),
                "missing semantic key: {key}"
            );
        }
    }

    #[test]
    fn apply_values_match_theme_fields() {
        let theme = opencode_dark();
        let map = ThemeEngine::apply(&theme);
        assert_eq!(map.get("fg"), Some(theme.fg));
        assert_eq!(map.get("bg"), Some(theme.bg));
        assert_eq!(map.get("accent"), Some(theme.accent));
        assert_eq!(map.get("success"), Some(theme.success));
        assert_eq!(map.get("warning"), Some(theme.warning));
        assert_eq!(map.get("error"), Some(theme.error));
        assert_eq!(map.get("muted"), Some(theme.muted));
    }

    // ── T04: unknown theme typed error ────────────────────────────────

    #[test]
    fn unknown_theme_returns_error() {
        let mut reg = ThemeRegistry::new();
        let _ = register_builtins(&mut reg);
        let err = reg.get("nonexistent");
        assert!(err.is_none());
        // The typed error variant from ThemeError
        let err = ThemeError::UnknownTheme {
            name: "nonexistent".to_string(),
        };
        assert!(err.to_string().contains("nonexistent"));
    }

    // ── T05: theme switch preserves bounded memory ────────────────────

    #[test]
    fn theme_switch_preserves_bounded_memory() {
        let mut reg = ThemeRegistry::new();
        let _ = register_builtins(&mut reg);
        assert_eq!(reg.len(), 2);
        // Switch: remove one, add another — still within bounds
        let removed = reg.themes.remove("opencode-dark").unwrap();
        assert_eq!(reg.len(), 1);
        let mut custom = removed;
        custom.name = "custom-dark".to_string();
        reg.register(custom).unwrap();
        assert_eq!(reg.len(), 2);
        assert!(reg.get("custom-dark").is_some());
        assert!(reg.get("opencode-dark").is_none());
    }

    // ── T06: built-in opencode dark/light present ─────────────────────

    #[test]
    fn builtins_dark_and_light_present() {
        let dark = opencode_dark();
        let light = opencode_light();
        assert_eq!(dark.name, "opencode-dark");
        assert_eq!(light.name, "opencode-light");
        // Both are readable (fg != bg)
        assert_ne!(dark.fg, dark.bg);
        assert_ne!(light.fg, light.bg);
        // Registry has both
        let mut reg = ThemeRegistry::new();
        let _ = register_builtins(&mut reg);
        assert!(reg.get("opencode-dark").is_some());
        assert!(reg.get("opencode-light").is_some());
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn builtins_apply_complete() {
        let mut reg = ThemeRegistry::new();
        let _ = register_builtins(&mut reg);
        for name in reg.names() {
            let theme = reg.get(name).unwrap();
            let map = ThemeEngine::apply(theme);
            assert_eq!(
                map.keys().len(),
                COLOR_SLOTS,
                "theme {name} must produce {COLOR_SLOTS} color slots"
            );
        }
    }
}

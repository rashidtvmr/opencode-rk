#![forbid(unsafe_code)]
//! Runtime theme engine (new): upstream-parity palette resolution shared with
//! the native painter. Sibling of (not a rewrite of) the frozen legacy
//! `native_theme` module; its contracts remain untouched.
//!
//! Upstream evidence (pinned `sources/upstream.lock.json` pin, read-only
//! checkout at HEAD 95daf90670b7c039c436c85537da5fbfe2205b41):
//!
//! - `packages/tui/src/theme/assets/opencode.json`: `defs` map (darkStep1..
//!   darkStep12, lightStep1..lightStep12, darkSecondary, darkAccent, darkRed,
//!   darkOrange, darkGreen, darkCyan, darkYellow + light* counterparts) and a
//!   50-key `theme` table. The 10 core roles map: background=`darkStep1`/
//!   `lightStep1`, backgroundPanel=Step2, backgroundElement=Step3,
//!   border=Step7, primary=Step9, text=Step12, textMuted=Step11,
//!   error=`darkRed`/`lightRed`, warning=`darkOrange`/`lightOrange`,
//!   success=`darkGreen`/`lightGreen`.
//! - `packages/tui/src/theme/index.ts`: `resolveTheme(theme, mode)` resolves
//!   each `ColorValue` = hex string | reference name | `{dark,light}` variant;
//!   references walk `defs` then `theme.theme` and throw when missing; the
//!   `chain` parameter is the drift guard this module bounds with depth 32 +
//!   visited membership. `terminalMode(colors)` = luminance
//!   `0.299r + 0.587g + 0.114b > 0.5` (unit colors) → light, else dark.
//! - `packages/tui/src/config/index.tsx` line 63: `theme: Schema.optional(
//!   Schema.String)` — configuration persists only the theme NAME. No mode
//!   field exists in the persisted schema, so no mode field is invented here;
//!   mode is runtime (terminal probe / theme.switch_mode action per
//!   `packages/tui/src/config/keybind.ts`).
//!
//! Overlay alphas verified from upstream source by the lane package:
//! `packages/tui/src/ui/dialog.tsx` dims the full viewport with black
//! alpha 150, and the narrow `session/index.tsx` sidebar overlay dims with
//! black alpha 70. Both are exposed here as constants.
//!
//! Guarantees of this module: pure functions allocate only bounded tables;
//! the loader reads ONLY explicitly supplied paths (no env, no user-DB, no
//! secret/log sources) with a 256 KiB per-file cap, strict JSON, and typed
//! failure for every unusable config. Missing config paths are not an error —
//! they resolve to the shipped upstream default theme, honestly named.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

// ── Bounds ────────────────────────────────────────────────────────────

/// Palette slots (upstream core role set).
pub const ROLE_COUNT: usize = 10;
/// Maximum keys accepted in `defs` or `theme` tables.
pub const MAX_MAP_KEYS: usize = 128;
/// Maximum key/alias-name length in bytes.
pub const MAX_KEY_BYTES: usize = 64;
/// Maximum hex token bytes (`#RRGGBBAA` = 9).
pub const MAX_COLOR_TOKEN_BYTES: usize = 9;
/// Alias recursion bound (upstream chain drift guard, bounded for O(1)).
pub const MAX_ALIAS_DEPTH: usize = 32;
/// Per-file cap for runtime config JSON reads.
pub const MAX_CONFIG_JSON_BYTES: usize = 256 * 1024;
/// Full-viewport dialog overlay alpha (`packages/tui/src/ui/dialog.tsx`).
pub const OVERLAY_DIALOG_ALPHA: u8 = 150;
/// Narrow sidebar overlay alpha (`packages/tui/src/session/index.tsx`).
pub const OVERLAY_SIDEBAR_ALPHA: u8 = 70;

// ── Mode ──────────────────────────────────────────────────────────────

/// Theme mode. The persisted upstream schema carries only a theme NAME;
/// the mode is a runtime decision (terminal probe or theme.switch_mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    /// Upstream variant key in `{dark, light}` ColorValues.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Upstream `terminalMode` luminance rule on an RGB terminal probe
/// (colors scaled to 0..255 upstream compares in 0..1; the caller must
/// probe real terminal colors — when none exist upstream resolves no
/// system theme, so without a probe the product default stays Dark).
#[must_use]
pub fn terminal_mode(r: u8, g: u8, b: u8) -> ThemeMode {
    let lum = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
    if lum / 255.0 > 0.5 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

// ── Source-over compositing ───────────────────────────────────────────

/// Straight-alpha source-over compositing with round-to-nearest, matching
/// the float expression `(co*ao + cb*ab*(1-ao)) / out_a` rounded by
/// `Math.round` upstream. Fully integer arithmetic (no float rounding
/// drift). Alpha: `ao + ab*(1-ao)`; when the base is opaque the result is
/// always opaque (255), which is the native cell contract.
///
/// Dimming a known region before FFI uses an opaque base plus a black
/// overlay with the region's alpha, so every terminal cell color is a
/// deterministic opaque RGB — measured native RGBA alpha blending is NOT
/// asserted.
#[must_use]
pub fn source_over(base: [u8; 4], overlay: [u8; 4]) -> [u8; 4] {
    let ao = u32::from(overlay[3]);
    let ab = u32::from(base[3]);
    let out_a_num = ao * 255 + ab * (255 - ao);
    if out_a_num == 0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    out[3] = ((out_a_num + 127) / 255) as u8;
    for i in 0..3 {
        let num = u32::from(overlay[i]) * ao * 255 + u32::from(base[i]) * ab * (255 - ao);
        // round-to-nearest integer trick: floor((2*num + den) / (2*den));
        // max operand 33_227_775 fits u32 comfortably.
        let den = out_a_num;
        out[i] = ((num * 2 + den) / (2 * den)) as u8;
    }
    out
}

/// Convenience dim: composite `base` under a black overlay (upstream dim
/// overlay is exactly `rgba(0,0,0,alpha)`), producing an opaque result.
#[must_use]
pub fn dim(base: [u8; 4], alpha: u8) -> [u8; 4] {
    source_over(base, [0, 0, 0, alpha])
}

// ── Hex parsing (4 upstream forms) ────────────────────────────────────

/// Hex color parse errors, fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HexError {
    /// No leading `#`.
    MissingHash,
    /// Digit count not in {3, 4, 6, 8}.
    BadLength,
    /// Non-hex byte at position (0-based within the token).
    BadDigit { pos: usize },
}

impl fmt::Display for HexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHash => write!(f, "hex color must start with '#'"),
            Self::BadLength => write!(f, "hex color must have 3, 4, 6, or 8 digits"),
            Self::BadDigit { pos } => write!(f, "invalid hex digit at position {pos}"),
        }
    }
}

/// Parse `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` into `[r,g,b,a]`.
/// Short digits double per CSS semantics (`#abc` = 0xAA,0xBB,0xCC);
/// 3-digit form is opaque.
#[must_use]
pub fn parse_color(token: &str) -> Result<[u8; 4], HexError> {
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return Err(HexError::MissingHash);
    }
    match bytes.len() - 1 {
        3 | 4 | 6 | 8 => {}
        _ => return Err(HexError::BadLength),
    }
    let digits = bytes.len() - 1;
    let short = digits == 3 || digits == 4;
    let byte_digit = |pos: usize| -> Result<u8, HexError> {
        let at = 1 + pos;
        match bytes.get(at) {
            Some(b'0'..=b'9') => Ok(bytes[at] - b'0'),
            Some(b'a'..=b'f') => Ok(bytes[at] - b'a' + 10),
            Some(b'A'..=b'F') => Ok(bytes[at] - b'A' + 10),
            _ => Err(HexError::BadDigit { pos: at }),
        }
    };
    let mut out = [0u8; 4];
    let mut cursor = 0usize;
    for step in 0..4 {
        if step == 3 && matches!(digits, 3 | 6) {
            out[3] = 0xFF;
            break;
        }
        out[step] = if short {
            let d = byte_digit(cursor)?;
            cursor += 1;
            d * 17
        } else {
            let hi = byte_digit(cursor)?;
            cursor += 1;
            let lo = byte_digit(cursor)?;
            cursor += 1;
            hi * 16 + lo
        };
    }
    Ok(out)
}

// ── Palette slots ─────────────────────────────────────────────────────

/// The 10 core palette slots, keyed by exact upstream role names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Background,
    BackgroundPanel,
    BackgroundElement,
    Text,
    TextMuted,
    Primary,
    Border,
    Error,
    Warning,
    Success,
}

impl Slot {
    /// All slots in variant-declaration order (== array index).
    pub const ALL: [Slot; ROLE_COUNT] = [
        Self::Background,
        Self::BackgroundPanel,
        Self::BackgroundElement,
        Self::Text,
        Self::TextMuted,
        Self::Primary,
        Self::Border,
        Self::Error,
        Self::Warning,
        Self::Success,
    ];

    /// Exact upstream theme key for this slot.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::BackgroundPanel => "backgroundPanel",
            Self::BackgroundElement => "backgroundElement",
            Self::Text => "text",
            Self::TextMuted => "textMuted",
            Self::Primary => "primary",
            Self::Border => "border",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Success => "success",
        }
    }

    /// Parse an upstream role name into a slot (exact keys only).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|slot| slot.name() == name)
    }
}

/// Resolved palette: one opaque `[r,g,b,a]` per slot, indexed by `Slot as usize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    slots: [[u8; 4]; ROLE_COUNT],
}

impl Palette {
    /// Builtin dark palette: exact `packages/tui/src/theme/assets/opencode.json`
    /// `defs` values referenced by the dark variant of each core role.
    #[must_use]
    pub fn opencode_dark() -> Self {
        Self::from_table([
            0x0a0a0a, 0x141414, 0x1e1e1e, 0xeeeeee, 0x808080, 0xfab283, 0x484848, 0xe06c75,
            0xf5a742, 0x7fd88f,
        ])
    }

    /// Builtin light palette: exact light `defs` values.
    #[must_use]
    pub fn opencode_light() -> Self {
        Self::from_table([
            0xffffff, 0xfafafa, 0xf5f5f5, 0x1a1a1a, 0x8a8a8a, 0x3b7dd8, 0xb8b8b8, 0xd1383d,
            0xd68c27, 0x3d9a57,
        ])
    }

    /// Build from `0xRRGGBB` per slot in [`Slot::ALL`] order (opaque alpha).
    #[must_use]
    pub fn from_table(table: [u32; ROLE_COUNT]) -> Self {
        let mut slots = [[0u8; 4]; ROLE_COUNT];
        for (index, value) in table.iter().enumerate() {
            slots[index] = [(value >> 16) as u8, (value >> 8) as u8, *value as u8, 0xFF];
        }
        Self { slots }
    }

    /// Color for a slot.
    #[must_use]
    pub fn get(&self, slot: Slot) -> [u8; 4] {
        self.slots[slot as usize]
    }

    /// Overwrite a slot (painter override path; bounded to palette size).
    pub fn set(&mut self, slot: Slot, color: [u8; 4]) {
        self.slots[slot as usize] = color;
    }

    /// Per-palette tuple view for the painter.
    #[must_use]
    pub fn table(&self) -> [[u8; 4]; ROLE_COUNT] {
        self.slots
    }
}

// ── JSON resolution (upstream resolveColor, bounded) ──────────────────

/// Palette resolution errors, fail-closed; every variant names the failing
/// role so the operator can find the offending theme file key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteError {
    /// `defs`/`theme` is not a JSON object (`context` names which).
    NotAnObject { context: &'static str },
    /// `defs`/`theme` exceeded [`MAX_MAP_KEYS`] (`context` names which).
    TooManyKeys { context: &'static str },
    /// Key/alias name longer than [`MAX_KEY_BYTES`].
    KeyTooLong { key: String },
    /// A required core role is missing from `theme` (`slot.name()`).
    MissingRole { role: String },
    /// Role value is not hex/alias/variant.
    BadColor { role: String },
    /// Alias target absent from both `defs` and `theme`.
    AliasNotFound { role: String, name: String },
    /// Alias cycle detected through visited membership.
    AliasCycle { role: String },
    /// Alias chain deeper than [`MAX_ALIAS_DEPTH`].
    AliasTooDeep { role: String },
    /// `{dark,light}` variant lacks the selected side.
    VariantMissing { role: String, side: &'static str },
    /// V2 semantic theme document is malformed or contains a bounded
    /// reference/alias failure.
    V2Invalid { message: String },
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject { context } => write!(f, "theme {context} is not a JSON object"),
            Self::TooManyKeys { context } => {
                write!(f, "theme {context} exceeds {MAX_MAP_KEYS} keys")
            }
            Self::KeyTooLong { key } => {
                write!(f, "theme key exceeds {MAX_KEY_BYTES} bytes: {key}")
            }
            Self::MissingRole { role } => write!(f, "theme role {role} is required but missing"),
            Self::BadColor { role } => write!(f, "theme role {role} is not a color value"),
            Self::AliasNotFound { role, name } => {
                write!(f, "theme role {role} references unknown name {name:?}")
            }
            Self::AliasCycle { role } => write!(f, "theme role {role} is part of an alias cycle"),
            Self::AliasTooDeep { role } => {
                write!(f, "theme role {role} alias chain exceeds {MAX_ALIAS_DEPTH}")
            }
            Self::VariantMissing { role, side } => {
                write!(f, "theme role {role} variant is missing its {side} color")
            }
            Self::V2Invalid { message } => write!(f, "invalid V2 theme: {message}"),
        }
    }
}

/// Bounded ergonomic view over a parsed theme asset: upstream
/// `resolveColor(defs ?? theme.theme)` semantics with explicit depth and
/// visited bounds replacing the unbounded TS `chain`.
#[derive(Clone, Copy)]
struct View<'v> {
    defs: Option<&'v Map<String, Value>>,
    theme: &'v Map<String, Value>,
    mode: ThemeMode,
}

impl<'v> View<'v> {
    /// Resolve one ColorValue (`hex` | ref | variant) for `role`.
    fn resolve(
        &self,
        role: &'v str,
        value: &'v Value,
        chain: &mut Vec<&'v str>,
    ) -> Result<[u8; 4], PaletteError> {
        match value {
            Value::String(token) => self.resolve_token(role, token, chain),
            Value::Object(variant) => {
                let side = self.mode.key();
                let picked = variant
                    .get(side)
                    .ok_or_else(|| PaletteError::VariantMissing {
                        role: role.to_owned(),
                        side,
                    })?;
                self.resolve(role, picked, chain)
            }
            _ => Err(PaletteError::BadColor {
                role: role.to_owned(),
            }),
        }
    }

    #[allow(clippy::too_many_lines)] // nothing needs legacy code references
    fn resolve_token(
        &self,
        role: &'v str,
        token: &'v str,
        chain: &mut Vec<&'v str>,
    ) -> Result<[u8; 4], PaletteError> {
        if matches!(token, "transparent" | "none") {
            return Ok([0, 0, 0, 0]);
        }
        if token.starts_with('#') {
            if token.len() > MAX_COLOR_TOKEN_BYTES {
                return Err(PaletteError::BadColor {
                    role: role.to_owned(),
                });
            }
            return parse_color(token).map_err(|_| PaletteError::BadColor {
                role: role.to_owned(),
            });
        }
        if token.len() > MAX_KEY_BYTES {
            return Err(PaletteError::BadColor {
                role: role.to_owned(),
            });
        }
        if chain.iter().any(|seen| *seen == token) {
            return Err(PaletteError::AliasCycle {
                role: role.to_owned(),
            });
        }
        if chain.len() >= MAX_ALIAS_DEPTH {
            return Err(PaletteError::AliasTooDeep {
                role: role.to_owned(),
            });
        }
        chain.push(token);
        let next = self
            .defs
            .and_then(|defs| defs.get(token))
            .or_else(|| self.theme.get(token))
            .ok_or_else(|| PaletteError::AliasNotFound {
                role: role.to_owned(),
                name: token.to_owned(),
            })?;
        self.resolve(role, next, chain)
    }
}

/// Check one table of role/def names against key bounds.
fn check_keys(table: &Map<String, Value>, context: &'static str) -> Result<(), PaletteError> {
    if table.len() > MAX_MAP_KEYS {
        return Err(PaletteError::TooManyKeys { context });
    }
    table.keys().try_for_each(|key| {
        if key.len() > MAX_KEY_BYTES {
            Err(PaletteError::KeyTooLong { key: key.clone() })
        } else {
            Ok(())
        }
    })
}

/// Resolve a palette from `defs` + `theme` JSON values (may be `Value::Null`
/// defs) for one mode. All 10 core roles must resolve; extra upstream roles
/// (diff*, markdown*, syntax*, …) are tolerated and skipped — the palette
/// surface stays exactly [`ROLE_COUNT`].
///
/// # Errors
///
/// [`PaletteError`] on every malformed, unknown-reference, unbounded or
/// missing-role input; never falls back silently.
pub fn palette_from_json<'v>(
    defs: &'v Value,
    theme: &'v Value,
    mode: ThemeMode,
) -> Result<Palette, PaletteError> {
    let defs_table = match defs {
        Value::Null => None,
        Value::Object(defs) => {
            check_keys(defs, "defs")?;
            // defs values are typed ReferenceName | hex upstream: strings only.
            defs.values().try_for_each(|value| {
                if value.is_string() {
                    Ok(())
                } else {
                    Err(PaletteError::BadColor {
                        role: "defs".to_owned(),
                    })
                }
            })?;
            Some(defs)
        }
        _ => return Err(PaletteError::NotAnObject { context: "defs" }),
    };
    let theme_table = match theme {
        Value::Object(theme) => theme,
        _ => return Err(PaletteError::NotAnObject { context: "theme" }),
    };
    check_keys(theme_table, "theme")?;
    let view = View {
        defs: defs_table,
        theme: theme_table,
        mode,
    };
    let mut out = Palette::from_table([0; ROLE_COUNT]);
    for &slot in Slot::ALL.iter() {
        let role = slot.name();
        let value = theme_table
            .get(role)
            .ok_or_else(|| PaletteError::MissingRole {
                role: role.to_owned(),
            })?;
        out.set(
            slot,
            // Capacity: role names come from Slot::name() with 'static.
            view.resolve(slot.name(), value, &mut Vec::new())?,
        );
    }
    Ok(out)
}

/// Resolve from a whole upstream theme asset object
/// (`{"$schema":…, "defs":{…}, "theme":{…}}`).
///
/// # Errors
///
/// [`PaletteError`] when the asset shape or any core role is unusable.
pub fn palette_from_asset(asset: &Value, mode: ThemeMode) -> Result<Palette, PaletteError> {
    let object = asset.as_object().ok_or(PaletteError::NotAnObject {
        context: "theme asset",
    })?;
    if object.contains_key("base") {
        return v2_theme_from_asset(asset, mode).map(|theme| theme.palette);
    }
    let defs = object.get("defs").cloned().unwrap_or(Value::Null);
    let theme = object
        .get("theme")
        .ok_or(PaletteError::NotAnObject { context: "theme" })?;
    palette_from_json(&defs, theme, mode)
}

const V2_STEPS: [u16; 9] = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const V2_STATES: [&str; 5] = ["disabled", "pressed", "focused", "selected", "hovered"];
const V2_MAX_HUES: usize = 128;
const V2_DEFAULT_ASSET: &str = include_str!("../assets/opencode-v2.json");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HueSource {
    pub hue: String,
    pub step: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HueColor {
    pub rgba: [u8; 4],
    pub provenance: Option<HueSource>,
}

impl HueColor {
    fn literal(rgba: [u8; 4]) -> Self {
        Self { rgba, provenance: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownPalette {
    /// Resolved upstream `markdown.code` foreground.
    pub code: HueColor,
    /// Resolved upstream `background.base` for inline raw Markdown.
    pub inline_background: HueColor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticPalette {
    pub background_base: HueColor,
    pub background_raised_base: HueColor,
    pub background_raised_high: HueColor,
    pub text_base: HueColor,
    pub text_muted: HueColor,
    pub text_feedback_error: HueColor,
    pub text_feedback_warning: HueColor,
    pub text_feedback_success: HueColor,
    /// V2 tab status hues: interactive.200 for running and accent.200 for attention.
    pub tab_interactive: HueColor,
    pub tab_attention: HueColor,
    pub background_action_primary: V2StatefulColor,
    pub markdown: MarkdownPalette,
    pub categorical: Vec<String>,
    hues: HashMap<String, Vec<HueColor>>,
}

impl SemanticPalette {
    #[must_use]
    pub fn source<'a>(&self, color: &'a HueColor) -> Option<&'a HueSource> {
        color.provenance.as_ref()
    }

    #[must_use]
    pub fn increase(&self, color: &HueColor, amount: f64) -> HueColor {
        self.shift(color, amount)
    }

    #[must_use]
    pub fn decrease(&self, color: &HueColor, amount: f64) -> HueColor {
        self.shift(color, -amount)
    }

    /// Upstream default amount (`amount = 1`) for [`Self::increase`].
    #[must_use]
    pub fn increase_default(&self, color: &HueColor) -> HueColor {
        self.increase(color, 1.0)
    }

    /// Upstream default amount (`amount = 1`) for [`Self::decrease`].
    #[must_use]
    pub fn decrease_default(&self, color: &HueColor) -> HueColor {
        self.decrease(color, 1.0)
    }

    fn shift(&self, color: &HueColor, amount: f64) -> HueColor {
        let Some(source) = &color.provenance else { return color.clone() };
        let Some(scale) = self.hues.get(&source.hue) else { return color.clone() };
        let position = V2_STEPS.iter().position(|step| *step == source.step).unwrap_or(0);
        let delta = if amount.is_finite() { amount.trunc() } else { 0.0 };
        let delta = delta.clamp(-8.0, 8.0) as i32;
        let target = (position as i32 + delta).clamp(0, 8) as usize;
        scale[target].clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2StatefulColor {
    pub base: HueColor,
    pub states: HashMap<String, HueColor>,
}

impl V2StatefulColor {
    #[must_use]
    pub fn state(&self, states: &HashMap<String, bool>) -> &HueColor {
        V2_STATES
            .iter()
            .find(|name| states.get(**name).copied().unwrap_or(false))
            .and_then(|name| self.states.get(*name))
            .unwrap_or(&self.base)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Theme {
    pub selected_mode: ThemeMode,
    pub palette: Palette,
    pub semantic: SemanticPalette,
}

/// Resolve one V2 theme document through the existing palette boundary.
pub fn v2_theme_from_asset(asset: &Value, mode: ThemeMode) -> Result<V2Theme, PaletteError> {
    let invalid = |message: String| PaletteError::V2Invalid { message };
    let root = asset.as_object().ok_or_else(|| invalid("document is not an object".into()))?;
    let modes = ["light", "dark"].into_iter().filter(|name| root.contains_key(*name)).collect::<Vec<_>>();
    if modes.is_empty() { return Err(invalid("document provides no modes".into())); }
    if root.len() > MAX_MAP_KEYS { return Err(PaletteError::TooManyKeys { context: "v2 document" }); }
    let requested = mode.key();
    let selected = if root.contains_key(requested) { requested } else { modes[0] };
    let base = root.get("base").and_then(Value::as_object).cloned().unwrap_or_default();
    let selected_obj = root.get(selected).and_then(Value::as_object).ok_or_else(|| invalid("selected mode is not an object".into()))?;
    let mut merged = v2_merge(Value::Object(base.clone()), Value::Object(selected_obj.clone()));
    v2_apply_contextual_actions(&mut merged, &Value::Object(base), &Value::Object(selected_obj.clone()));
    let mut resolver = V2Resolver::new(merged);
    resolver.compile_hues()?;
    let palette = resolver.palette()?;
    let semantic = resolver.semantic()?;
    Ok(V2Theme { selected_mode: if selected == "dark" { ThemeMode::Dark } else { ThemeMode::Light }, palette, semantic })
}

/// Action variants rebuilt with upstream `contextualActions` precedence.
const V2_ACTION_VARIANTS: [&str; 3] = ["primary", "secondary", "destructive"];
/// Keys rebuilt per action variant by upstream `contextualActions`.
const V2_ACTION_KEYS: [&str; 6] = ["base", "$disabled", "$pressed", "$focused", "$selected", "$hovered"];

/// Upstream `contextualActions` value chain
/// (`packages/theme/src/tui/resolve.ts`): `over[key] ?? over.base ??
/// base[key] ?? base.base`.
fn v2_state_pick(over: Option<&Value>, base: Option<&Value>, key: &str) -> Option<Value> {
    over.and_then(|value| value.get(key))
        .cloned()
        .or_else(|| over.and_then(|value| value.get("base")).cloned())
        .or_else(|| base.and_then(|value| value.get(key)).cloned())
        .or_else(|| base.and_then(|value| value.get("base")).cloned())
}

/// Rebuild `text.action` / `background.action` variant state keys with the
/// upstream `contextualActions` precedence instead of the ordinary deep merge:
/// a mode-level `base` beats a shared-`base` state key
/// (`surface[key] ?? surface.base ?? base[key] ?? base.base`). Variants defined
/// on neither layer are left untouched.
fn v2_apply_contextual_actions(merged: &mut Value, base: &Value, over: &Value) {
    let Some(root) = merged.as_object_mut() else { return };
    for surface in ["text", "background"] {
        let base_action = base.get(surface).and_then(|value| value.get("action"));
        let over_action = over.get(surface).and_then(|value| value.get("action"));
        if base_action.is_none() && over_action.is_none() {
            continue;
        }
        let surface_entry = root
            .entry(surface.to_owned())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(surface_map) = surface_entry.as_object_mut() else { continue };
        let action_entry = surface_map
            .entry("action".to_owned())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(actions) = action_entry.as_object_mut() else { continue };
        for variant in V2_ACTION_VARIANTS {
            let base_variant = base_action.and_then(|value| value.get(variant));
            let over_variant = over_action.and_then(|value| value.get(variant));
            if base_variant.is_none() && over_variant.is_none() {
                continue;
            }
            let mut entry = serde_json::Map::new();
            for key in V2_ACTION_KEYS {
                if let Some(value) = v2_state_pick(over_variant, base_variant, key) {
                    entry.insert(key.to_owned(), value);
                }
            }
            actions.insert(variant.to_owned(), Value::Object(entry));
        }
    }
}

fn v2_merge(base: Value, over: Value) -> Value {
    match (base, over) {
        (Value::Object(mut left), Value::Object(right)) => {
            for (key, value) in right { let previous = left.remove(&key).unwrap_or(Value::Null); left.insert(key, v2_merge(previous, value)); }
            Value::Object(left)
        }
        (_, value) => value,
    }
}

struct V2Resolver {
    mode: Value,
    hues: HashMap<String, Vec<HueColor>>,
}

impl V2Resolver {
    fn new(mode: Value) -> Self { Self { mode, hues: HashMap::new() } }

    fn compile_hues(&mut self) -> Result<(), PaletteError> {
        let map = self.mode.get("hue").and_then(Value::as_object).ok_or_else(|| PaletteError::V2Invalid { message: "mode hue map missing".into() })?;
        if map.len() > V2_MAX_HUES { return Err(PaletteError::TooManyKeys { context: "v2 hues" }); }
        let names = map.keys().cloned().collect::<Vec<_>>();
        for name in names { self.resolve_hue(&name, &mut Vec::new())?; }
        Ok(())
    }

    fn resolve_hue(&mut self, name: &str, stack: &mut Vec<String>) -> Result<Vec<HueColor>, PaletteError> {
        if let Some(hue) = self.hues.get(name) { return Ok(hue.clone()); }
        if stack.len() >= MAX_ALIAS_DEPTH { return Err(PaletteError::V2Invalid { message: "hue alias depth exceeded".into() }); }
        if stack.iter().any(|seen| seen == name) { return Err(PaletteError::V2Invalid { message: format!("hue alias cycle at {name}") }); }
        let raw = self.mode.get("hue").and_then(|h| h.get(name)).ok_or_else(|| PaletteError::V2Invalid { message: format!("missing hue {name}") })?.clone();
        stack.push(name.to_owned());
        let values = if let Some(alias) = raw.as_str() {
            let target = alias.strip_prefix("$hue.").ok_or_else(|| PaletteError::V2Invalid { message: "invalid hue alias".into() })?;
            self.resolve_hue(target, stack)?.into_iter().map(|color| HueColor { rgba: color.rgba, provenance: Some(HueSource { hue: name.to_owned(), step: color.provenance.map_or(100, |p| p.step) }) }).collect()
        } else {
            let object = raw.as_object().ok_or_else(|| PaletteError::V2Invalid { message: format!("hue {name} is not an object") })?;
            if object.len() != V2_STEPS.len() { return Err(PaletteError::V2Invalid { message: format!("hue {name} must contain nine steps") }); }
            let mut values = Vec::with_capacity(V2_STEPS.len());
            for step in V2_STEPS {
                let token = object.get(&step.to_string()).and_then(Value::as_str).ok_or_else(|| PaletteError::V2Invalid { message: format!("invalid hue {name}.{step}") })?;
                let rgba = parse_color(token).map_err(|_| PaletteError::V2Invalid { message: format!("invalid hue color {name}.{step}") })?;
                values.push(HueColor { rgba, provenance: Some(HueSource { hue: name.to_owned(), step }) });
            }
            values
        };
        stack.pop();
        self.hues.insert(name.to_owned(), values.clone());
        Ok(values)
    }

    fn resolve_color(&self, value: &Value, context: &Value, depth: usize) -> Result<HueColor, PaletteError> {
        if depth > MAX_ALIAS_DEPTH { return Err(PaletteError::V2Invalid { message: "token reference depth exceeded".into() }); }
        let token = value.as_str().ok_or_else(|| PaletteError::V2Invalid { message: "token is not a string".into() })?;
        if token == "transparent" { return Ok(HueColor::literal([0, 0, 0, 0])); }
        if token.starts_with('#') { return parse_color(token).map(HueColor::literal).map_err(|_| PaletteError::V2Invalid { message: "invalid hex color".into() }); }
        let target = token.strip_prefix('$').ok_or_else(|| PaletteError::V2Invalid { message: format!("invalid color token {token}") })?;
        if let Some(rest) = target.strip_prefix("hue.") {
            if let Some((name, step)) = rest.rsplit_once('.') {
                let step: u16 = step.parse().map_err(|_| PaletteError::V2Invalid { message: "invalid hue step".into() })?;
                if let Some(index) = V2_STEPS.iter().position(|candidate| *candidate == step) {
                    return self.hues.get(name).and_then(|scale| scale.get(index)).cloned().ok_or_else(|| PaletteError::V2Invalid { message: format!("missing hue reference {target}") });
                }
            }
        }
        let mut current = context;
        for segment in target.split('.') { current = current.get(segment).ok_or_else(|| PaletteError::V2Invalid { message: format!("missing reference {target}") })?; }
        self.resolve_color(current, context, depth + 1)
    }

    fn palette(&self) -> Result<Palette, PaletteError> {
        let role = |path: &[&str]| -> Result<HueColor, PaletteError> {
            let mut value = &self.mode;
            for segment in path { value = value.get(*segment).ok_or_else(|| PaletteError::V2Invalid { message: format!("missing semantic role {}", path.join(".")) })?; }
            self.resolve_color(value, &self.mode, 0)
        };
        let mut palette = Palette::from_table([0; ROLE_COUNT]);
        for (slot, path) in [
            (Slot::Background, vec!["background", "base"]),
            (Slot::BackgroundPanel, vec!["background", "raised", "base"]),
            (Slot::BackgroundElement, vec!["background", "raised", "high"]),
            (Slot::Text, vec!["text", "base"]), (Slot::TextMuted, vec!["text", "muted"]),
            (Slot::Border, vec!["border", "base"]), (Slot::Error, vec!["text", "feedback", "error", "base"]),
            (Slot::Warning, vec!["text", "feedback", "warning", "base"]), (Slot::Success, vec!["text", "feedback", "success", "base"]),
        ] { palette.set(slot, role(&path)?.rgba); }
        palette.set(Slot::Primary, role(&["background", "action", "primary", "$focused"])?.rgba);
        Ok(palette)
    }

    fn semantic(&self) -> Result<SemanticPalette, PaletteError> {
        let get = |path: &[&str]| -> Result<HueColor, PaletteError> {
            let mut value = &self.mode;
            for segment in path { value = value.get(*segment).ok_or_else(|| PaletteError::V2Invalid { message: format!("missing semantic role {}", path.join(".")) })?; }
            self.resolve_color(value, &self.mode, 0)
        };
        let action = self.mode.get("background").and_then(|v| v.get("action")).and_then(|v| v.get("primary")).ok_or_else(|| PaletteError::V2Invalid { message: "missing primary action".into() })?;
        let base = action.get("base").ok_or_else(|| PaletteError::V2Invalid { message: "missing primary action base".into() })?;
        let mut states = HashMap::new();
        for state in V2_STATES {
            let key = format!("${state}");
            let value = action.get(&key).unwrap_or(base);
            states.insert(state.to_owned(), self.resolve_color(value, &self.mode, 0)?);
        }
        let categorical = self.mode.get("categorical").and_then(Value::as_array).ok_or_else(|| PaletteError::V2Invalid { message: "categorical list missing".into() })?.iter().map(|v| v.as_str().map(str::to_owned).ok_or_else(|| PaletteError::V2Invalid { message: "invalid categorical hue".into() })).collect::<Result<Vec<_>, _>>()?;
        Ok(SemanticPalette {
            background_base: get(&["background", "base"])?, background_raised_base: get(&["background", "raised", "base"])?, background_raised_high: get(&["background", "raised", "high"])?,
            text_base: get(&["text", "base"])?, text_muted: get(&["text", "muted"])?,
            text_feedback_error: get(&["text", "feedback", "error", "base"])?, text_feedback_warning: get(&["text", "feedback", "warning", "base"])?, text_feedback_success: get(&["text", "feedback", "success", "base"])?,
            tab_interactive: self.resolve_color(&Value::String("$hue.interactive.200".into()), &self.mode, 0)?,
            tab_attention: self.resolve_color(&Value::String("$hue.accent.200".into()), &self.mode, 0)?,
            background_action_primary: V2StatefulColor { base: self.resolve_color(base, &self.mode, 0)?, states },
            markdown: MarkdownPalette { code: get(&["markdown", "code"])?, inline_background: get(&["background", "base"])? },
            categorical, hues: self.hues.clone(),
        })
    }
}

// ── Runtime theme + runtime config resolution ─────────────────────────

/// Config-shape errors for the runtime config `theme` key. The pinned
/// upstream schema (`packages/tui/src/config/index.tsx`) persists only an
/// optional string theme name; no mode field exists and none is invented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigThemeError {
    /// Config root is not a JSON object.
    RootNotObject,
    /// `theme` present but not a string.
    ThemeWrongType,
    /// `theme` name longer than [`MAX_KEY_BYTES`].
    ThemeTooLong,
}

impl fmt::Display for ConfigThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotObject => write!(f, "runtime config is not a JSON object"),
            Self::ThemeWrongType => write!(f, "config key 'theme' is not a string"),
            Self::ThemeTooLong => {
                write!(f, "config key 'theme' exceeds {MAX_KEY_BYTES} bytes")
            }
        }
    }
}

/// Extract the optional theme NAME from a parsed runtime config.
///
/// # Errors
///
/// [`ConfigThemeError`] on non-object roots, non-string `theme`, or
/// unbounded names; absent `theme` (or explicit null) is `Ok(None)`.
pub fn config_theme_name(config: &Value) -> Result<Option<String>, ConfigThemeError> {
    let object = config.as_object().ok_or(ConfigThemeError::RootNotObject)?;
    match object.get("theme") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => {
            if name.len() > MAX_KEY_BYTES {
                Err(ConfigThemeError::ThemeTooLong)
            } else {
                Ok(Some(name.clone()))
            }
        }
        Some(_) => Err(ConfigThemeError::ThemeWrongType),
    }
}

/// Runtime-theme load errors. Loading reads ONLY explicitly supplied paths —
/// no environment probing, no user-database reads, no secret or log files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Existing config file larger than [`MAX_CONFIG_JSON_BYTES`] bytes.
    TooLarge,
    /// Existing config file not valid UTF-8 / not valid JSON.
    InvalidJson,
    /// Config parsed but the `theme` key is unusably shaped.
    Config(ConfigThemeError),
    /// Config names a theme that is not a builtin and no asset was supplied.
    UnknownTheme { name: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => {
                write!(f, "runtime config exceeds {MAX_CONFIG_JSON_BYTES} bytes")
            }
            Self::InvalidJson => write!(f, "runtime config is not valid UTF-8 JSON"),
            Self::Config(error) => write!(f, "{error}"),
            Self::UnknownTheme { name } => {
                write!(
                    f,
                    "unknown non-builtin theme {name:?}; no asset source supplied"
                )
            }
        }
    }
}

/// Resolve the runtime theme from explicit config paths.
///
/// Semantics (lazy-last-good, fail-closed on real errors): missing or
/// unreadable paths are skipped; the first readable config that parses and
/// carries a usable `theme` name decides (currently only the builtin
/// `opencode` theme is natively available); a config that exists but is
/// unusable marks the FIRST typed error, returned when no path yields a
/// good theme. When no path contains a config at all the shipped upstream
/// default is returned — nothing is written or persisted by this loader.
///
/// # Errors
///
/// [`LoadError`] for the first bad-but-present config when none resolves.
pub fn load_runtime_theme(
    config_paths: &[&Path],
    mode: ThemeMode,
) -> Result<RuntimeTheme, LoadError> {
    let mut first_bad: Option<LoadError> = None;
    for path in config_paths {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => continue, // missing/unreadable: skip, not an error
        };
        if bytes.len() > MAX_CONFIG_JSON_BYTES {
            first_bad.get_or_insert(LoadError::TooLarge);
            continue;
        }
        let config = match std::str::from_utf8(&bytes)
            .map_err(|_| ())
            .and_then(|text| serde_json::from_str::<Value>(text).map_err(|_| ()))
        {
            Ok(config) => config,
            Err(_) => {
                first_bad.get_or_insert(LoadError::InvalidJson);
                continue;
            }
        };
        match config_theme_name(&config) {
            Ok(None) => return Ok(RuntimeTheme::builtin_default(mode)),
            Ok(Some(name)) => {
                return RuntimeTheme::named(&name, mode)
                    .ok_or_else(|| LoadError::UnknownTheme { name });
            }
            Err(error) => {
                first_bad.get_or_insert(LoadError::Config(error));
                continue;
            }
        }
    }
    match first_bad {
        Some(error) => Err(error),
        None => Ok(RuntimeTheme::builtin_default(mode)),
    }
}

/// Resolved runtime theme: upstream theme NAME, active runtime mode, and
/// the palette resolved for that mode. Pure state; persistence of any
/// selection is owned by callers, never by this module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTheme {
    /// Theme name (config `theme` value or the shipped `opencode` default).
    pub name: String,
    /// Active mode: a runtime decision, not a persisted config field.
    pub mode: ThemeMode,
    /// Palette resolved from the theme definitions for [`Self::mode`].
    pub palette: Palette,
    /// Semantic V2 token view when loaded from a V2 theme document.
    pub semantic: Option<SemanticPalette>,
}

impl RuntimeTheme {
    /// Shipped upstream default theme (`packages/tui/src/theme/assets/
    /// opencode.json`) resolved in `mode`.
    #[must_use]
    pub fn builtin_default(mode: ThemeMode) -> Self {
        let v2 = serde_json::from_str::<Value>(V2_DEFAULT_ASSET)
            .ok()
            .and_then(|asset| v2_theme_from_asset(&asset, mode).ok());
        let palette = v2.as_ref().map_or_else(
            || match mode { ThemeMode::Dark => Palette::opencode_dark(), ThemeMode::Light => Palette::opencode_light() },
            |theme| theme.palette,
        );
        Self {
            name: "opencode".to_owned(),
            mode: v2.as_ref().map_or(mode, |theme| theme.selected_mode),
            palette,
            semantic: v2.map(|theme| theme.semantic),
        }
    }

    /// Named builtin theme. Only the shipped default exists natively;
    /// library/custom themes enter through [`Self::from_asset`].
    #[must_use]
    pub fn named(name: &str, mode: ThemeMode) -> Option<Self> {
        (name == "opencode").then(|| Self::builtin_default(mode))
    }

    /// Build a theme from one parsed upstream theme-asset object.
    ///
    /// # Errors
    ///
    /// [`PaletteError`] when the asset shape or any core role is unusable
    /// for `mode`.
    pub fn from_asset(name: &str, asset: &Value, mode: ThemeMode) -> Result<Self, PaletteError> {
        if asset.as_object().is_some_and(|object| object.contains_key("base")) {
            let resolved = v2_theme_from_asset(asset, mode)?;
            return Ok(Self { name: name.to_owned(), mode: resolved.selected_mode, palette: resolved.palette, semantic: Some(resolved.semantic) });
        }
        Ok(Self {
            name: name.to_owned(),
            mode,
            palette: palette_from_asset(asset, mode)?,
            semantic: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hex(token: &str) -> [u8; 4] {
        parse_color(token).expect("test hex token")
    }

    // ── source-over compositing (upstream dialog alpha 150 / sidebar 70)

    #[test]
    fn source_over_dim_alpha150_yields_4_on_base_10() {
        let out = source_over([10, 10, 10, 255], [0, 0, 0, OVERLAY_DIALOG_ALPHA]);
        assert_eq!(out, [4, 4, 4, 255]);
        let dimmed = dim([10, 10, 10, 255], OVERLAY_DIALOG_ALPHA);
        assert_eq!(dimmed, out);
    }

    #[test]
    fn source_over_sidebar_alpha70_yields_7_on_base_10() {
        let out = source_over([10, 10, 10, 255], [0, 0, 0, OVERLAY_SIDEBAR_ALPHA]);
        assert_eq!(out, [7, 7, 7, 255]);
    }

    #[test]
    fn source_over_opaque_overlay_full_replace() {
        let out = source_over([1, 2, 3, 255], [9, 9, 9, 255]);
        assert_eq!(out, [9, 9, 9, 255]);
    }

    #[test]
    fn source_over_transparent_overlay_passthrough() {
        let out = source_over([5, 5, 5, 255], [7, 7, 7, 0]);
        assert_eq!(out, [5, 5, 5, 255]);
    }

    #[test]
    fn source_over_semi_both_alpha_composites() {
        // base black 128, overlay red 128: out_a = 0.5 + 0.5*0.5 = 0.75 → 192;
        // r = 255*0.5/0.75 → 170.25 → 170 (Math.round parity).
        let out = source_over([0, 0, 0, 128], [255, 0, 0, 128]);
        assert_eq!(out, [170, 0, 0, 192]);
    }

    #[test]
    fn source_over_transparent_base_transparent_stack() {
        let out = source_over([5, 5, 5, 0], [7, 7, 7, 0]);
        assert_eq!(out, [0, 0, 0, 0]);
    }

    // ── hex forms (upstream ColorValue hex strings)

    #[test]
    fn hex_rgb_short_doubles() {
        assert_eq!(hex("#fab"), [255, 170, 187, 255]);
    }

    #[test]
    fn hex_rgba_short_doubles_alpha() {
        assert_eq!(hex("#fab1"), [255, 170, 187, 17]);
    }

    #[test]
    fn hex_rrggbb_opaque() {
        assert_eq!(hex("#fa0055"), [250, 0, 85, 255]);
    }

    #[test]
    fn hex_rrggbbaa_alpha() {
        assert_eq!(hex("#fa005540"), [250, 0, 85, 64]);
    }

    #[test]
    fn hex_errors_are_typed() {
        assert_eq!(parse_color("123456"), Err(HexError::MissingHash));
        assert_eq!(parse_color(""), Err(HexError::MissingHash));
        assert_eq!(parse_color("#12345"), Err(HexError::BadLength));
        assert_eq!(parse_color("#112233445"), Err(HexError::BadLength));
        assert_eq!(parse_color("#GG0000"), Err(HexError::BadDigit { pos: 1 }));
        assert_eq!(parse_color("#12345G"), Err(HexError::BadDigit { pos: 6 }));
    }

    // ── builtins (exact opencode.json defs values)

    #[test]
    fn builtin_dark_matches_upstream_asset_values() {
        let palette = Palette::opencode_dark();
        assert_eq!(palette.get(Slot::Background), hex("#0a0a0a"));
        assert_eq!(palette.get(Slot::BackgroundPanel), hex("#141414"));
        assert_eq!(palette.get(Slot::BackgroundElement), hex("#1e1e1e"));
        assert_eq!(palette.get(Slot::Text), hex("#eeeeee"));
        assert_eq!(palette.get(Slot::TextMuted), hex("#808080"));
        assert_eq!(palette.get(Slot::Primary), hex("#fab283"));
        assert_eq!(palette.get(Slot::Border), hex("#484848"));
        assert_eq!(palette.get(Slot::Error), hex("#e06c75"));
        assert_eq!(palette.get(Slot::Warning), hex("#f5a742"));
        assert_eq!(palette.get(Slot::Success), hex("#7fd88f"));
        assert!(palette.table().iter().all(|c| c[3] == 0xFF));
    }

    #[test]
    fn builtin_light_matches_upstream_asset_values() {
        let palette = Palette::opencode_light();
        assert_eq!(palette.get(Slot::Background), hex("#ffffff"));
        assert_eq!(palette.get(Slot::BackgroundPanel), hex("#fafafa"));
        assert_eq!(palette.get(Slot::BackgroundElement), hex("#f5f5f5"));
        assert_eq!(palette.get(Slot::Text), hex("#1a1a1a"));
        assert_eq!(palette.get(Slot::TextMuted), hex("#8a8a8a"));
        assert_eq!(palette.get(Slot::Primary), hex("#3b7dd8"));
        assert_eq!(palette.get(Slot::Border), hex("#b8b8b8"));
        assert_eq!(palette.get(Slot::Error), hex("#d1383d"));
        assert_eq!(palette.get(Slot::Warning), hex("#d68c27"));
        assert_eq!(palette.get(Slot::Success), hex("#3d9a57"));
        assert_ne!(Palette::opencode_light(), Palette::opencode_dark());
    }

    #[test]
    fn slot_names_roundtrip_exact_upstream_keys() {
        for slot in Slot::ALL {
            assert_eq!(Slot::parse(slot.name()), Some(slot));
        }
        assert_eq!(Slot::parse("textMuted"), Some(Slot::TextMuted));
        assert_eq!(Slot::parse("backgroundPanel"), Some(Slot::BackgroundPanel));
        assert_eq!(Slot::parse("diffAdded"), None);
        assert_eq!(Slot::parse("text_muted"), None); // exact upstream keys only
    }

    // ── JSON resolution (upstream resolveColor semantics, bounded)

    fn base_theme() -> Map<String, Value> {
        let mut map = Map::new();
        for slot in Slot::ALL {
            map.insert(slot.name().to_owned(), json!("#101010"));
        }
        map
    }

    #[test]
    fn asset_shape_resolves_to_builtin_for_both_modes() {
        // Fixture mirrors the real opencode.json shape: defs + variant theme
        // with one extra non-core role ("secondary"), which must be tolerated.
        let asset = json!({
            "$schema": "https://opencode.ai/theme.json",
            "defs": {
                "darkStep1": "#0a0a0a", "darkStep2": "#141414",
                "darkStep3": "#1e1e1e", "darkStep7": "#484848",
                "darkStep9": "#fab283", "darkStep11": "#808080",
                "darkStep12": "#eeeeee", "darkRed": "#e06c75",
                "darkOrange": "#f5a742", "darkGreen": "#7fd88f",
                "darkSecondary": "#5c9cf5",
                "lightStep1": "#ffffff", "lightStep2": "#fafafa",
                "lightStep3": "#f5f5f5", "lightStep7": "#b8b8b8",
                "lightStep9": "#3b7dd8", "lightStep11": "#8a8a8a",
                "lightStep12": "#1a1a1a", "lightRed": "#d1383d",
                "lightOrange": "#d68c27", "lightGreen": "#3d9a57",
                "lightSecondary": "#7b5bb6"
            },
            "theme": {
                "primary":       { "dark": "darkStep9",  "light": "lightStep9" },
                "error":         { "dark": "darkRed",    "light": "lightRed" },
                "warning":       { "dark": "darkOrange", "light": "lightOrange" },
                "success":       { "dark": "darkGreen",  "light": "lightGreen" },
                "info":          { "dark": "darkStep11", "light": "lightStep11" },
                "text":          { "dark": "darkStep12", "light": "lightStep12" },
                "textMuted":     { "dark": "darkStep11", "light": "lightStep11" },
                "background":    { "dark": "darkStep1",  "light": "lightStep1" },
                "backgroundPanel":    { "dark": "darkStep2", "light": "lightStep2" },
                "backgroundElement":  { "dark": "darkStep3", "light": "lightStep3" },
                "border":        { "dark": "darkStep7",  "light": "lightStep7" },
                "secondary":     { "dark": "darkSecondary", "light": "lightSecondary" }
            }
        });
        let dark = palette_from_asset(&asset, ThemeMode::Dark).expect("dark resolves");
        assert_eq!(dark, Palette::opencode_dark());
        let light = palette_from_asset(&asset, ThemeMode::Light).expect("light resolves");
        assert_eq!(light, Palette::opencode_light());
    }

    #[test]
    fn alias_walks_defs_then_role_variant() {
        let defs = json!({ "e1": "e2" });
        let mut theme = base_theme();
        theme.insert(
            "background".to_owned(),
            json!({ "dark": "#112233", "light": "#445566" }),
        );
        theme.insert("e2".to_owned(), json!("background"));
        theme.insert("text".to_owned(), json!("e1"));
        let dark = palette_from_json(&defs, &Value::Object(theme.clone()), ThemeMode::Dark)
            .expect("dark alias chain");
        assert_eq!(dark.get(Slot::Text), [0x11, 0x22, 0x33, 255]);
        let light = palette_from_json(&defs, &Value::Object(theme), ThemeMode::Light)
            .expect("light alias chain");
        assert_eq!(light.get(Slot::Text), [0x44, 0x55, 0x66, 255]);
    }

    #[test]
    fn alias_cycle_is_detected() {
        let mut theme = base_theme();
        theme.insert("background".to_owned(), json!("backgroundPanel"));
        theme.insert("backgroundPanel".to_owned(), json!("background"));
        let error = palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark)
            .expect_err("cycle must fail closed");
        assert_eq!(
            error,
            PaletteError::AliasCycle {
                role: "background".to_owned()
            }
        );
    }

    #[test]
    fn alias_chain_is_bounded_at_depth_32() {
        let mut defs = Map::new();
        for i in 0..40 {
            defs.insert(format!("chain{i}"), json!(format!("chain{}", i + 1)));
        }
        let mut theme = base_theme();
        theme.insert("text".to_owned(), json!("chain0"));
        let error = palette_from_json(&Value::Object(defs), &Value::Object(theme), ThemeMode::Dark)
            .expect_err("deep chain must fail closed");
        assert_eq!(
            error,
            PaletteError::AliasTooDeep {
                role: "text".to_owned()
            }
        );
    }

    #[test]
    fn alias_to_unknown_name_fails_closed() {
        let mut theme = base_theme();
        theme.insert("text".to_owned(), json!("nowhere"));
        let error = palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark)
            .expect_err("unknown alias must fail closed");
        assert_eq!(
            error,
            PaletteError::AliasNotFound {
                role: "text".to_owned(),
                name: "nowhere".to_owned()
            }
        );
    }

    #[test]
    fn oversized_hex_token_is_bad_color() {
        let mut theme = base_theme();
        theme.insert("text".to_owned(), json!("#112233445"));
        let error = palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark)
            .expect_err("over-long token must fail closed");
        assert_eq!(
            error,
            PaletteError::BadColor {
                role: "text".to_owned()
            }
        );
    }

    #[test]
    fn variant_missing_selected_side_fails_closed() {
        let mut theme = base_theme();
        theme.insert("background".to_owned(), json!({ "dark": "#000000" }));
        assert!(
            palette_from_json(&Value::Null, &Value::Object(theme.clone()), ThemeMode::Dark).is_ok()
        );
        let error = palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Light)
            .expect_err("missing light side");
        assert_eq!(
            error,
            PaletteError::VariantMissing {
                role: "background".to_owned(),
                side: "light"
            }
        );
    }

    #[test]
    fn non_color_role_value_fails_closed() {
        let mut theme = base_theme();
        theme.insert("text".to_owned(), json!(5));
        assert!(matches!(
            palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark),
            Err(PaletteError::BadColor { .. })
        ));
    }

    #[test]
    fn missing_core_role_fails_closed() {
        let mut theme = base_theme();
        theme.remove("primary");
        assert!(matches!(
            palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark),
            Err(PaletteError::MissingRole { .. })
        ));
    }

    #[test]
    fn too_many_theme_keys_fails_closed() {
        let mut theme = Map::new();
        for i in 0..(MAX_MAP_KEYS + 2) {
            theme.insert(format!("role{i}"), json!("#101010"));
        }
        assert_eq!(
            palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark),
            Err(PaletteError::TooManyKeys { context: "theme" })
        );
    }

    #[test]
    fn oversized_key_fails_closed() {
        let mut theme = base_theme();
        theme.insert("x".repeat(MAX_KEY_BYTES + 1), json!("#101010"));
        assert!(matches!(
            palette_from_json(&Value::Null, &Value::Object(theme), ThemeMode::Dark),
            Err(PaletteError::KeyTooLong { .. })
        ));
    }

    #[test]
    fn non_string_defs_value_fails_closed() {
        let defs = json!({ "broken": 5 });
        let error = palette_from_json(&defs, &Value::Object(base_theme()), ThemeMode::Dark)
            .expect_err("defs values are strings upstream");
        assert_eq!(
            error,
            PaletteError::BadColor {
                role: "defs".to_owned()
            }
        );
    }

    // ── runtime mode selection (upstream terminalMode)

    #[test]
    fn terminal_mode_luma_boundaries() {
        assert_eq!(terminal_mode(10, 10, 10), ThemeMode::Dark);
        assert_eq!(terminal_mode(230, 230, 230), ThemeMode::Light);
        // 128: lum/255 = 0.5019… > 0.5 → light; 127 → 0.4980… → dark.
        assert_eq!(terminal_mode(128, 128, 128), ThemeMode::Light);
        assert_eq!(terminal_mode(127, 127, 127), ThemeMode::Dark);
    }

    // ── config key + loader

    #[test]
    fn config_theme_name_shapes() {
        assert_eq!(config_theme_name(&json!({})), Ok(None));
        assert_eq!(
            config_theme_name(&json!({ "theme": "ayu" })),
            Ok(Some("ayu".to_owned()))
        );
        assert_eq!(config_theme_name(&json!({ "theme": null })), Ok(None));
        assert_eq!(
            config_theme_name(&json!({ "theme": 5 })),
            Err(ConfigThemeError::ThemeWrongType)
        );
        assert_eq!(
            config_theme_name(&json!({ "theme": "x".repeat(MAX_KEY_BYTES + 1) })),
            Err(ConfigThemeError::ThemeTooLong)
        );
        assert_eq!(
            config_theme_name(&json!("not-an-object")),
            Err(ConfigThemeError::RootNotObject)
        );
    }

    fn scratch_dir(tag: usize) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oc2-native-theme-runtime-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir); // leftover from an old run
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn loader_missing_paths_default_to_shipped_theme() {
        let missing = scratch_dir(1).join("absent.json");
        let theme = load_runtime_theme(&[&missing], ThemeMode::Dark).expect("default theme");
        assert_eq!(theme, RuntimeTheme::builtin_default(ThemeMode::Dark));
        assert_eq!(theme.name, "opencode");
    }

    #[test]
    fn loader_reads_explicit_config_theme_name() {
        let dir = scratch_dir(2);
        let path = dir.join("tui.json");
        std::fs::write(&path, r#"{"theme":"opencode"}"#).expect("write config");
        let theme = load_runtime_theme(&[&path], ThemeMode::Light).expect("valid config");
        assert_eq!(theme, RuntimeTheme::builtin_default(ThemeMode::Light));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn loader_unknown_theme_name_fails_closed() {
        let dir = scratch_dir(3);
        let path = dir.join("tui.json");
        std::fs::write(&path, r#"{"theme":"ayu"}"#).expect("write config");
        assert_eq!(
            load_runtime_theme(&[&path], ThemeMode::Dark),
            Err(LoadError::UnknownTheme {
                name: "ayu".to_owned()
            })
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn loader_invalid_json_is_typed_error() {
        let dir = scratch_dir(4);
        let path = dir.join("tui.json");
        std::fs::write(&path, "{oops").expect("write config");
        assert_eq!(
            load_runtime_theme(&[&path], ThemeMode::Dark),
            Err(LoadError::InvalidJson)
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn loader_good_path_after_bad_path_wins() {
        let dir = scratch_dir(5);
        let bad = dir.join("bad.json");
        let good = dir.join("good.json");
        std::fs::write(&bad, "{oops").expect("write bad config");
        std::fs::write(&good, r#"{"theme":"opencode"}"#).expect("write good config");
        let theme = load_runtime_theme(&[&bad, &good], ThemeMode::Dark);
        assert_eq!(theme, Ok(RuntimeTheme::builtin_default(ThemeMode::Dark)));
        let _ = std::fs::remove_file(&bad);
        let _ = std::fs::remove_file(&good);
        let _ = std::fs::remove_dir(&dir);
    }

// ── genuine-V2 semantic fixtures (M02 F1–F6 + V3, additive; the frozen legacy
// tests above stay byte-identical to canonical). Oracle authority: pinned
// upstream 450e7b92412c780a7b5a75226bb8d49aaca24e22 — schema.ts ActionState
// order and StatefulColorDefinition keys; resolve.ts statefulColor/state(),
// contextualActions, compileHueSteps; shipped asset
// packages/tui/src/theme/assets/v2/opencode.json. The frozen helper `hex` above
// is reused (M02's identical helper is not redeclared).

fn dark_semantic() -> SemanticPalette {
    RuntimeTheme::builtin_default(ThemeMode::Dark)
        .semantic
        .expect("shipped V2 asset attaches semantic once $focused key is fixed")
}

fn hue_entry(semantic: &SemanticPalette, hue: &str, step: u16) -> HueColor {
    let index = (step / 100 - 1) as usize;
    semantic.hues.get(hue).expect("hue scale present")[index].clone()
}

fn state_on(keys: &[&str]) -> HashMap<String, bool> {
    keys.iter().map(|key| ((*key).to_owned(), true)).collect()
}

// ── F1: semantic != None and genuine-V2 document resolves end-to-end ──────

#[test]
fn f1_builtin_default_dark_has_semantic_projection() {
    let semantic = dark_semantic();
    // V17 prerequisite: dark background.raised.base = neutral.700 = #141414.
    assert_eq!(
        semantic.background_raised_base,
        HueColor {
            rgba: hex("#141414"),
            provenance: Some(HueSource {
                hue: "neutral".to_owned(),
                step: 700
            })
        }
    );
}

#[test]
fn f1_shipped_v2_asset_resolves_via_from_asset() {
    let asset: Value =
        serde_json::from_str(include_str!("../assets/opencode-v2.json"))
            .expect("shipped asset parses");
    let dark = RuntimeTheme::from_asset("opencode-v2", &asset, ThemeMode::Dark)
        .expect("genuine V2 document must resolve ($focused key)");
    assert!(dark.semantic.is_some(), "semantic must not be silently None");
    assert_eq!(dark.palette.get(Slot::Warning), hex("#f5a742"));
    assert_eq!(dark.palette.get(Slot::Primary), hex("#fab283"));
    let light = RuntimeTheme::from_asset("opencode-v2", &asset, ThemeMode::Light)
        .expect("light mode resolves");
    assert!(light.semantic.is_some());
    assert_eq!(light.palette.get(Slot::Warning), hex("#d68c27"));
    assert_eq!(light.palette.get(Slot::Primary), hex("#3b7dd8"));
}

// ── F2: alias identity (V4/V12/V13/V14/V15) ────────────────────────────────

#[test]
fn f2_alias_identity_distinct_provenance_at_equal_value() {
    let semantic = dark_semantic();
    let neutral = hue_entry(&semantic, "neutral", 700);
    let gray = hue_entry(&semantic, "gray", 700);
    assert_eq!(neutral.rgba, hex("#141414"));
    assert_eq!(gray.rgba, neutral.rgba, "alias copies are value-equal");
    // Identity oracle: value-equal but distinct provenance (upstream WeakMap
    // registers each scale's own entries; alias clones are separate keys).
    assert_eq!(
        semantic.source(&neutral),
        Some(&HueSource {
            hue: "neutral".to_owned(),
            step: 700
        })
    );
    assert_eq!(
        semantic.source(&gray),
        Some(&HueSource {
            hue: "gray".to_owned(),
            step: 700
        })
    );
}

#[test]
fn f2_alias_shift_stays_in_alias_scale() {
    let semantic = dark_semantic();
    let neutral = hue_entry(&semantic, "neutral", 700);
    let gray = hue_entry(&semantic, "gray", 700);
    // V13: result carries the ALIAS name, never "gray".
    assert_eq!(
        semantic.decrease(&neutral, 1.0).provenance,
        Some(HueSource {
            hue: "neutral".to_owned(),
            step: 600
        })
    );
    // V15: gray.700 never conflated with neutral.700.
    assert_eq!(
        semantic.decrease(&gray, 1.0).provenance,
        Some(HueSource {
            hue: "gray".to_owned(),
            step: 600
        })
    );
}

#[test]
fn f2_untracked_literal_passthrough() {
    let semantic = dark_semantic();
    // V4/V14: value-equal hand-made literal has NO provenance -> identity map
    // miss -> returned unchanged, source() = None.
    let lit = HueColor {
        rgba: hex("#141414"),
        provenance: None,
    };
    assert_eq!(semantic.decrease(&lit, 1.0), lit);
    assert_eq!(semantic.increase(&lit, 5.0), lit);
    assert_eq!(semantic.source(&lit), None);
}

// ── F3: shift amount arithmetic (V5–V11, V16, V7–V9) ───────────────────────

#[test]
fn f3_shift_truncates_toward_zero_and_sign_doubles() {
    let semantic = dark_semantic();
    let n700 = hue_entry(&semantic, "neutral", 700);
    let n600 = hue_entry(&semantic, "neutral", 600);
    let n800 = hue_entry(&semantic, "neutral", 800);
    // V8a: decrease(x, 1.9) offset = trunc(-1.9) = -1 -> 600.
    assert_eq!(semantic.decrease(&n700, 1.9), n600);
    // V8b: decrease(x, -1.9) offset = trunc(1.9) = +1 -> 800 (double negation).
    assert_eq!(semantic.decrease(&n700, -1.9), n800);
    // V8c: decrease(x, 0.9) offset = trunc(-0.9) = 0 -> unchanged.
    assert_eq!(semantic.decrease(&n700, 0.9), n700);
    // V9: decrease(x, -1) moves UP one step.
    assert_eq!(semantic.decrease(&n700, -1.0), n800);
    // V1: decrease(x, 1) -> neutral.600 #1e1e1e with {neutral,600} provenance.
    let one = semantic.decrease(&n700, 1.0);
    assert_eq!(one.rgba, hex("#1e1e1e"));
    assert_eq!(
        one.provenance,
        Some(HueSource {
            hue: "neutral".to_owned(),
            step: 600
        })
    );
}

#[test]
fn f3_shift_nonfinite_amount_is_noop() {
    let semantic = dark_semantic();
    let n700 = hue_entry(&semantic, "neutral", 700);
    // V7: NaN / +Inf / -Inf -> offset 0, value AND provenance unchanged.
    for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(semantic.decrease(&n700, amount), n700);
        assert_eq!(semantic.increase(&n700, amount), n700);
    }
}

#[test]
fn f3_shift_clamps_floor_ceiling_saturating_keeps_input_provenance() {
    let semantic = dark_semantic();
    let n100 = hue_entry(&semantic, "neutral", 100); // #ffffff
    let n700 = hue_entry(&semantic, "neutral", 700); // #141414
    let n900 = hue_entry(&semantic, "neutral", 900); // #030303
    // V5: clamp floor — result IS the input entry (provenance unchanged).
    let floor = semantic.decrease(&n100, 3.0);
    assert_eq!(floor, n100);
    assert_eq!(
        floor.provenance,
        Some(HueSource {
            hue: "neutral".to_owned(),
            step: 100
        })
    );
    // V6: clamp ceiling.
    let ceil = semantic.increase(&n900, 9.0);
    assert_eq!(ceil, n900);
    // V16: 1e30 must saturate (no wrap) to 900 / 100 respectively.
    assert_eq!(semantic.increase(&n700, 1e30).provenance.unwrap().step, 900);
    assert_eq!(semantic.decrease(&n700, 1e30).provenance.unwrap().step, 100);
}

#[test]
fn f3_shift_chains_with_provenance() {
    let semantic = dark_semantic();
    let n700 = hue_entry(&semantic, "neutral", 700);
    let n500 = hue_entry(&semantic, "neutral", 500); // #4c4c4c
    // V10/V11: decrease(decrease(x,1),1) == decrease(x,2) in value AND provenance.
    assert_eq!(semantic.decrease(&semantic.decrease(&n700, 1.0), 1.0), n500);
    assert_eq!(semantic.decrease(&n700, 2.0), n500);
}

// ── V3: default amount = 1 (types.ts amount defaults) ──────────────────────

#[test]
fn v3_default_amount_is_one() {
    let semantic = dark_semantic();
    let n700 = hue_entry(&semantic, "neutral", 700);
    assert_eq!(semantic.decrease_default(&n700), semantic.decrease(&n700, 1.0));
    assert_eq!(semantic.increase_default(&n700), semantic.increase(&n700, 1.0));
}

// ── F4: state precedence (resolve.ts statefulColor L168) ───────────────────
// Shipped dark background.action.primary: base=transparent,
// $hovered=$hue.neutral.700 (#141414), $focused=$hue.interactive.200 (#fab283),
// $selected=transparent, $disabled/$pressed absent -> fall back to base.

#[test]
fn f4_state_precedence_disabled_pressed_focused_selected_hovered() {
    let semantic = dark_semantic();
    let action = &semantic.background_action_primary;
    let transparent = [0, 0, 0, 0];
    // Base fallback when no state is active.
    assert_eq!(action.state(&state_on(&[])).rgba, transparent);
    // Single states.
    assert_eq!(action.state(&state_on(&["hovered"])).rgba, hex("#141414"));
    assert_eq!(action.state(&state_on(&["focused"])).rgba, hex("#fab283"));
    // Precedence pairs: first truthy in
    // [disabled, pressed, focused, selected, hovered] order wins.
    assert_eq!(
        action.state(&state_on(&["hovered", "focused"])).rgba,
        hex("#fab283"),
        "focused beats hovered"
    );
    assert_eq!(
        action.state(&state_on(&["hovered", "selected"])).rgba,
        transparent,
        "selected beats hovered even though selected resolves to transparent"
    );
    assert_eq!(
        action.state(&state_on(&["focused", "disabled"])).rgba,
        transparent,
        "disabled beats focused ($disabled absent -> base fallback slot)"
    );
    assert_eq!(
        action.state(&state_on(&["focused", "pressed"])).rgba,
        transparent,
        "pressed beats focused ($pressed absent -> base fallback slot)"
    );
}

// ── F5: frozen-Warning guard (ruling evidence) ─────────────────────────────
// Upstream authority: base.text.feedback.warning.base = "#f5a742" literal
// (dark); light.text.feedback.warning.base = $hue.accent.200 -> orange.200 =
// "#d68c27". Dark accent.200 (purple, #9d7cd8) feeds syntax keyword /
// markdown heading — it has NO warning authority.

#[test]
fn f5_dark_warning_matches_frozen_legacy_oracle_not_accent_purple() {
    let semantic = dark_semantic();
    assert_eq!(semantic.text_feedback_warning.rgba, hex("#f5a742"));
    assert_ne!(
        semantic.text_feedback_warning.rgba,
        hex("#9d7cd8"),
        "dark warning must NOT be modernized to dark accent.200 (purple)"
    );
}

#[test]
fn f5_light_warning_matches_frozen_legacy_oracle() {
    let asset: Value =
        serde_json::from_str(include_str!("../assets/opencode-v2.json"))
            .expect("shipped asset parses");
    let light = RuntimeTheme::from_asset("opencode-v2", &asset, ThemeMode::Light)
        .expect("light resolves");
    let semantic = light.semantic.expect("semantic present");
    assert_eq!(semantic.text_feedback_warning.rgba, hex("#d68c27"));
}

// ── F6: upstream state-key merge precedence (contextualActions) ────────────
// resolve.ts: surfaceVariant[key] ?? surfaceVariant.base ?? baseVariant[key]
// ?? baseVariant.base — a mode-level `base` beats a shared-`base` state key.
// Enabled/adapted from M02's commented f6: its synthetic minimal document
// cannot resolve (palette() requires the core roles), so the same precedence is
// pinned on the genuine shipped V2 document with a mode-level primary override.

#[test]
fn f6_state_key_merge_mode_base_beats_shared_base_state() {
    let mut doc: Value =
        serde_json::from_str(include_str!("../assets/opencode-v2.json"))
            .expect("shipped asset parses");
    let dark = doc["dark"].as_object_mut().expect("dark object");
    dark.insert(
        "background".to_owned(),
        serde_json::json!({ "action": { "primary": { "base": "#333333" } } }),
    );
    let theme = RuntimeTheme::from_asset("t", &doc, ThemeMode::Dark).expect("resolves");
    let action = &theme.semantic.expect("semantic present").background_action_primary;
    // Upstream: dark $hovered falls to dark base #333333, NOT shared
    // base $hovered ($hue.neutral.700).
    assert_eq!(action.state(&state_on(&["hovered"])).rgba, hex("#333333"));
    // Mode base also beats shared-base $focused ($hue.interactive.200).
    assert_eq!(action.state(&state_on(&["focused"])).rgba, hex("#333333"));
}
}

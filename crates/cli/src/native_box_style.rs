#![forbid(unsafe_code)]
//! Pure, serde-persistable terminal box appearance policy.
//!
//! This module owns no rendering and no IO. It resolves a global
//! [`BoxAppearance`] plus per-box overrides into [`EffectiveBox`] values and
//! maps them to the only corner geometry a terminal cell grid can display.
//!
//! Radii are logical steps in `0..=50`, not terminal pixels. Sharp and Rounded
//! have exact glyph mappings; `IosSquircle` is deliberately a rounded-cell
//! approximation and never claims continuous curvature. `Custom` is a binary
//! terminal approximation: radius `0` is sharp, `1..=50` selects rounded
//! corner cells while [`EffectiveBox::normalized_radius`] preserves the value
//! for a future pixel renderer.
//!
//! `frame` is a deterministic braille frame generator only. Callers must gate
//! animation on a true backend-busy signal; this module never fakes busy state.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use std::collections::BTreeMap;

/// Maximum logical corner radius.
pub const MAX_RADIUS: u8 = 50;
/// Maximum interior padding in terminal cells.
pub const MAX_PADDING: u8 = 4;
/// Minimum border thickness in terminal cells.
pub const MIN_THICKNESS: u8 = 1;
/// Maximum border thickness in terminal cells.
pub const MAX_THICKNESS: u8 = 2;
/// Honest description of what a terminal can and cannot draw.
pub const IOS_SQUIRCLE_APPROXIMATION: &str =
    "terminal-cell approximation; no continuous curvature; rounded corner glyphs only";

/// Global roundedness preset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RadiusPreset {
    /// Upstream single-line sharp corners: `┌ ┐ └ ┘`.
    #[default]
    Sharp,
    /// Terminal rounded corners: `╭ ╮ ╰ ╯`.
    Rounded,
    /// Rounded corner cells used as an honest iOS-squircle approximation.
    IosSquircle,
    /// Radius-driven approximation; see [`BoxAppearance::radius`].
    Custom,
}

/// User-visible box surfaces that may override the global appearance.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoxId {
    /// Chat prompt/composer.
    ChatComposer,
    /// User message block.
    User,
    /// System message block.
    System,
    /// Assistant message block.
    Assistant,
    /// Tool call/result block.
    Tool,
    /// Modal dialog frame.
    Dialog,
    /// Sidebar frame.
    Sidebar,
    /// Menu/palette frame.
    Menu,
}

impl BoxId {
    /// Every overridable box, in stable order.
    pub const ALL: [Self; 8] = [
        Self::ChatComposer,
        Self::User,
        Self::System,
        Self::Assistant,
        Self::Tool,
        Self::Dialog,
        Self::Sidebar,
        Self::Menu,
    ];
}

/// Border line weight. Rounded corners are only available for `Single`;
/// `Double` and `Heavy` keep their native sharp corners.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    /// Single line, upstream's default and `parseBorderStyle` fallback.
    #[default]
    Single,
    /// Double line.
    Double,
    /// Heavy line.
    Heavy,
}

/// The four corner glyphs in reading order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CornerGlyphs {
    /// Top-left cell.
    pub top_left: char,
    /// Top-right cell.
    pub top_right: char,
    /// Bottom-left cell.
    pub bottom_left: char,
    /// Bottom-right cell.
    pub bottom_right: char,
}

impl CornerGlyphs {
    /// Corners as `[top_left, top_right, bottom_left, bottom_right]`.
    pub const fn to_array(self) -> [char; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_left,
            self.bottom_right,
        ]
    }
}

/// Per-box fields. `None` inherits the global value.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BoxOverride {
    /// Per-box preset override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<RadiusPreset>,
    /// Per-box logical radius, `0..=50`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_optional_radius"
    )]
    pub radius: Option<u8>,
    /// Per-box border style override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_style: Option<BorderStyle>,
    /// Per-box border thickness, `1..=2`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_optional_thickness"
    )]
    pub thickness: Option<u8>,
    /// Per-box padding, `0..=4`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_optional_padding"
    )]
    pub padding: Option<u8>,
}

/// Persisted global appearance plus per-box overrides.
///
/// Defaults preserve the existing upstream single-line sharp geometry:
/// `Single`, thickness `1`, padding `0`, radius `0`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BoxAppearance {
    /// Global preset.
    #[serde(skip_serializing_if = "RadiusPreset::is_sharp")]
    pub preset: RadiusPreset,
    /// Logical custom radius in `0..=50`; ignored unless `preset == Custom`.
    #[serde(
        default,
        skip_serializing_if = "is_zero",
        deserialize_with = "de_radius"
    )]
    pub radius: u8,
    /// Global border style.
    #[serde(skip_serializing_if = "BorderStyle::is_single")]
    pub border_style: BorderStyle,
    /// Global border thickness in `1..=2`.
    #[serde(
        default = "default_thickness",
        skip_serializing_if = "is_one",
        deserialize_with = "de_thickness"
    )]
    pub thickness: u8,
    /// Global padding in `0..=4`.
    #[serde(
        default,
        skip_serializing_if = "is_zero",
        deserialize_with = "de_padding"
    )]
    pub padding: u8,
    /// Per-box overrides keyed by known [`BoxId`] only.
    #[serde(skip_serializing_if = "overrides_empty")]
    pub overrides: BTreeMap<BoxId, BoxOverride>,
}

impl Default for BoxAppearance {
    fn default() -> Self {
        Self {
            preset: RadiusPreset::Sharp,
            radius: 0,
            border_style: BorderStyle::Single,
            thickness: MIN_THICKNESS,
            padding: 0,
            overrides: BTreeMap::new(),
        }
    }
}

/// Global values with one box's overrides applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveBox {
    /// Effective preset.
    pub preset: RadiusPreset,
    /// Effective logical radius.
    pub radius: u8,
    /// Effective border style.
    pub border_style: BorderStyle,
    /// Effective border thickness.
    pub thickness: u8,
    /// Effective padding.
    pub padding: u8,
}

impl EffectiveBox {
    /// Corner cells for this effective appearance.
    ///
    /// `Sharp` always yields sharp cells. `Rounded` and `IosSquircle` yield
    /// rounded cells. `Custom` yields rounded cells only when radius is
    /// positive. Double/heavy borders have no rounded Unicode corners, so
    /// they keep their native sharp corners.
    pub fn corner_glyphs(self) -> CornerGlyphs {
        let rounded = match self.preset {
            RadiusPreset::Sharp => false,
            RadiusPreset::Rounded | RadiusPreset::IosSquircle => true,
            RadiusPreset::Custom => self.radius > 0,
        };
        let (top_left, top_right, bottom_left, bottom_right) = match self.border_style {
            BorderStyle::Single if rounded => ('╭', '╮', '╰', '╯'),
            BorderStyle::Single => ('┌', '┐', '└', '┘'),
            BorderStyle::Double => ('╔', '╗', '╚', '╝'),
            BorderStyle::Heavy => ('┏', '┓', '┗', '┛'),
        };
        CornerGlyphs {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        }
    }

    /// Radius normalized to `0.0..=1.0` for a future pixel client.
    ///
    /// Terminal glyph output does not consume this value.
    pub fn normalized_radius(self) -> f32 {
        f32::from(self.radius.min(MAX_RADIUS)) / f32::from(MAX_RADIUS)
    }
}

impl BoxAppearance {
    /// Resolve one box against the global values.
    pub fn effective(&self, id: BoxId) -> EffectiveBox {
        let over = self.overrides.get(&id);
        EffectiveBox {
            preset: over.and_then(|o| o.preset).unwrap_or(self.preset),
            radius: over.and_then(|o| o.radius).unwrap_or(self.radius),
            border_style: over
                .and_then(|o| o.border_style)
                .unwrap_or(self.border_style),
            thickness: over.and_then(|o| o.thickness).unwrap_or(self.thickness),
            padding: over.and_then(|o| o.padding).unwrap_or(self.padding),
        }
    }

    /// Convenience wrapper for [`EffectiveBox::corner_glyphs`].
    pub fn corner_glyphs(&self, id: BoxId) -> CornerGlyphs {
        self.effective(id).corner_glyphs()
    }

    /// Check programmatically constructed values. Serde already enforces the
    /// same bounds during deserialization.
    pub fn validate(&self) -> Result<(), String> {
        check_range(self.radius, 0, MAX_RADIUS, "radius")?;
        check_range(self.padding, 0, MAX_PADDING, "padding")?;
        check_range(self.thickness, MIN_THICKNESS, MAX_THICKNESS, "thickness")?;
        for (id, over) in &self.overrides {
            if let Some(radius) = over.radius {
                check_range(radius, 0, MAX_RADIUS, &format!("{id:?}.radius"))?;
            }
            if let Some(padding) = over.padding {
                check_range(padding, 0, MAX_PADDING, &format!("{id:?}.padding"))?;
            }
            if let Some(thickness) = over.thickness {
                check_range(
                    thickness,
                    MIN_THICKNESS,
                    MAX_THICKNESS,
                    &format!("{id:?}.thickness"),
                )?;
            }
        }
        Ok(())
    }
}

fn check_range(value: u8, min: u8, max: u8, field: &str) -> Result<(), String> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("{field} must be {min}..={max}, got {value}"))
    }
}

fn de_bounded<'de, D>(deserializer: D, min: u8, max: u8, field: &str) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u8::deserialize(deserializer)?;
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "{field} must be {min}..={max}, got {value}"
        )))
    }
}

fn de_optional_bounded<'de, D>(
    deserializer: D,
    min: u8,
    max: u8,
    field: &str,
) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<u8>::deserialize(deserializer)?
        .map(|value| de_bounded_value(value, min, max, field).map_err(D::Error::custom))
        .transpose()
}

fn de_bounded_value(value: u8, min: u8, max: u8, field: &str) -> Result<u8, String> {
    check_range(value, min, max, field).map(|()| value)
}

fn de_radius<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    de_bounded(deserializer, 0, MAX_RADIUS, "radius")
}

fn de_padding<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    de_bounded(deserializer, 0, MAX_PADDING, "padding")
}

fn de_thickness<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    de_bounded(deserializer, MIN_THICKNESS, MAX_THICKNESS, "thickness")
}

fn de_optional_radius<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    de_optional_bounded(deserializer, 0, MAX_RADIUS, "radius")
}

fn de_optional_padding<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    de_optional_bounded(deserializer, 0, MAX_PADDING, "padding")
}

fn de_optional_thickness<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    de_optional_bounded(deserializer, MIN_THICKNESS, MAX_THICKNESS, "thickness")
}

impl RadiusPreset {
    fn is_sharp(&self) -> bool {
        *self == Self::Sharp
    }
}

impl BorderStyle {
    fn is_single(&self) -> bool {
        *self == Self::Single
    }
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

fn is_one(value: &u8) -> bool {
    *value == MIN_THICKNESS
}
fn default_thickness() -> u8 {
    MIN_THICKNESS
}

fn overrides_empty(overrides: &BTreeMap<BoxId, BoxOverride>) -> bool {
    overrides.is_empty()
}

/// Keyboard intent for the radius control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadiusKey {
    /// One logical step down.
    Decrease,
    /// One logical step up.
    Increase,
    /// Minimum radius.
    Home,
    /// Maximum radius.
    End,
}

/// Map a pointer position to a logical radius.
///
/// `track_width` is the inclusive cell span of the drag track and `x` is
/// relative to its left edge. Values outside the track clamp to `0..=50`.
/// A track narrower than two cells has no draggable range and maps to `0`.
pub fn radius_from_pointer(track_width: u16, x: i32) -> u8 {
    let span = u32::from(track_width.saturating_sub(1));
    if span == 0 {
        return 0;
    }
    let x = x.clamp(0, span as i32) as u32;
    ((x * u32::from(MAX_RADIUS) + span / 2) / span).min(u32::from(MAX_RADIUS)) as u8
}

/// Apply one keyboard intent to a logical radius, clamping to `0..=50`.
pub fn radius_after_key(radius: u8, key: RadiusKey) -> u8 {
    let radius = radius.min(MAX_RADIUS);
    match key {
        RadiusKey::Decrease => radius.saturating_sub(1),
        RadiusKey::Increase => radius.saturating_add(1).min(MAX_RADIUS),
        RadiusKey::Home => 0,
        RadiusKey::End => MAX_RADIUS,
    }
}

/// Small optional control model for a CLI dragger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadiusControl {
    radius: u8,
}

impl RadiusControl {
    /// Create a control, clamping the initial radius.
    pub const fn new(radius: u8) -> Self {
        Self {
            radius: if radius > MAX_RADIUS {
                MAX_RADIUS
            } else {
                radius
            },
        }
    }

    /// Current logical radius.
    pub const fn radius(self) -> u8 {
        self.radius
    }

    /// Move the pointer over a track of `track_width` cells at `x`.
    pub fn pointer(&mut self, track_width: u16, x: i32) {
        self.radius = radius_from_pointer(track_width, x);
    }

    /// Apply a keyboard intent.
    pub fn key(&mut self, key: RadiusKey) {
        self.radius = radius_after_key(self.radius, key);
    }
}

/// Braille frames used by the deterministic CLI loader.
pub const BRAILLE_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// Duration of one loader frame in milliseconds.
pub const LOADER_FRAME_MS: u64 = 80;

/// Deterministic braille frame for an elapsed duration.
///
/// This is presentation only. The caller must show/animate it solely while a
/// real backend work signal is active.
pub fn frame(elapsed_ms: u64) -> char {
    BRAILLE_FRAMES[((elapsed_ms / LOADER_FRAME_MS) % BRAILLE_FRAMES.len() as u64) as usize]
}

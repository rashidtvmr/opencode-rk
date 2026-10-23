#![forbid(unsafe_code)]
//! Pure, serde-persistable terminal box appearance policy.
//!
//! This module owns no rendering and no IO. It resolves a global
//! [`BoxAppearance`] plus per-box overrides into [`EffectiveBox`] values and
//! maps them to the only corner geometry a terminal cell grid can display.
//!
//! Radius NEVER creates framing: a preset or logical radius only changes
//! the corner glyphs of surfaces that already draw corners. A full drawn
//! frame exists only through an explicit `Double`/`Heavy` line weight
//! ([`EffectiveBox::explicit_line_frame`]).
//!
//! Radii are logical steps in `0..=50`, not terminal pixels. Sharp and Rounded
//! have exact glyph mappings; `IosSquircle` is a distinct quarter-arc
//! cell-grid approximation (R2) and never claims continuous curvature. The
//! bridge renderer is a glyph grid (`bufferDrawText`/`bufferFillRect`; the
//! only pixel API is `queryPixelResolution`, which is image display, not box
//! chrome), so shapes are corner glyphs and [`BoxGeometry`] is the ONE
//! shared paint/clip contract for fill and border (R1; upstream draws a
//! dialog/toast as background + border on one box). `Custom` is a binary
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
    "cell-grid approximation: quarter-arc corner glyphs only; no continuous curvature";

/// Rounded elbow corner cells for `Single` borders, reading order
/// (top-left, top-right, bottom-left, bottom-right).
pub const ROUNDED_CORNER_GLYPHS: [char; 4] = ['╭', '╮', '╰', '╯'];
/// Quarter-arc corner cells approximating an iOS squircle, reading order.
///
/// R2: a distinct selected-shape rendering, NOT a rename of
/// [`RadiusPreset::Rounded`]; rendered only where a surface already paints
/// corners, per the radius-never-frames rule.
pub const SQUIRCLE_CORNER_GLYPHS: [char; 4] = ['◜', '◝', '◟', '◞'];

/// Global roundedness preset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RadiusPreset {
    /// Upstream single-line sharp corners: `┌ ┐ └ ┘`.
    #[default]
    Sharp,
    /// Terminal rounded corners: `╭ ╮ ╰ ╯`.
    Rounded,
    /// Quarter-arc corner cells approximating an iOS squircle.
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
    /// Returns true only for rounded single-line corner cells of this exact
    /// surface. Fill paint uses this to restore the enclosing backdrop.
    pub fn rounded_corner_cell(
        self,
        rect: (u16, u16, u16, u16),
        x: u16,
        y: u16,
        glyph: char,
    ) -> bool {
        if self.border_style != BorderStyle::Single
            || rect.2 < 2
            || rect.3 < 2
            || x < rect.0
            || y < rect.1
            || x >= rect.0.saturating_add(rect.2)
            || y >= rect.1.saturating_add(rect.3)
        {
            return false;
        }
        let corners = self.corner_glyphs();
        let expected = match (
            x == rect.0,
            x.saturating_add(1) == rect.0.saturating_add(rect.2),
            y == rect.1,
            y.saturating_add(1) == rect.1.saturating_add(rect.3),
        ) {
            (true, _, true, _) => corners.top_left,
            (_, true, true, _) => corners.top_right,
            (true, _, _, true) => corners.bottom_left,
            (_, true, _, true) => corners.bottom_right,
            _ => return false,
        };
        glyph == expected
            && (matches!(self.preset, RadiusPreset::Rounded | RadiusPreset::IosSquircle)
                || (self.preset == RadiusPreset::Custom && self.radius > 0))
    }
    /// Corner cells for this effective appearance.
    ///
    /// `Sharp` always yields sharp cells. `Rounded` yields rounded elbows,
    /// `IosSquircle` yields distinct quarter-arc cells. `Custom` yields
    /// rounded cells only when radius is positive. Double/heavy borders have
    /// no rounded Unicode corners, so they keep their native sharp corners.
    pub fn corner_glyphs(self) -> CornerGlyphs {
        let rounded = match self.preset {
            RadiusPreset::Sharp => false,
            RadiusPreset::Rounded | RadiusPreset::IosSquircle => true,
            RadiusPreset::Custom => self.radius > 0,
        };
        let (top_left, top_right, bottom_left, bottom_right) = match self.border_style {
            BorderStyle::Single if rounded => {
                let [tl, tr, bl, br] = if self.preset == RadiusPreset::IosSquircle {
                    SQUIRCLE_CORNER_GLYPHS
                } else {
                    ROUNDED_CORNER_GLYPHS
                };
                (tl, tr, bl, br)
            }
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

    /// Horizontal and vertical weight glyphs for this border style.
    #[must_use]
    pub fn border_glyphs(self) -> (char, char) {
        match self.border_style {
            BorderStyle::Single => ('─', '│'),
            BorderStyle::Double => ('═', '║'),
            BorderStyle::Heavy => ('━', '┃'),
        }
    }

    /// Radius normalized to `0.0..=1.0` for a future pixel client.
    ///
    /// Terminal glyph output does not consume this value.
    pub fn normalized_radius(self) -> f32 {
        f32::from(self.radius.min(MAX_RADIUS)) / f32::from(MAX_RADIUS)
    }

    /// True only when a full drawn frame is an EXPLICIT line-weight
    /// preference (`Double` or `Heavy`).
    ///
    /// A radius choice (preset or logical radius) must never create or
    /// widen a frame on its own: radius changes only the corner glyphs of
    /// surfaces that already paint corners. Chat message surfaces follow
    /// this gate so independent radius edits can never convert them into
    /// uniform bordered rectangles.
    pub fn explicit_line_frame(self) -> bool {
        !matches!(self.border_style, BorderStyle::Single)
    }
}

/// One-box paint geometry shared by background fill and glyph-frame drawing
/// (R1: a background rectangle and its border must come from the SAME rect).
///
/// Upstream renders a dialog/toast as one box object — background covering
/// the frame and border glyphs on its edges. This mirrors that: callers take
/// `framed`/`filled` once, then use [`BoxGeometry::fill`] for the background
/// and [`BoxGeometry::text_area`]/[`BoxGeometry::frame_rows`] for content and
/// border rows. Cell values are always the outer box rect unless a method
/// says otherwise. No field or method takes a radius: radius can never
/// activate a frame ([`EffectiveBox::explicit_line_frame`] gates whether
/// `framed` is even representable for a surface).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoxGeometry {
    /// Left edge cell (inclusive).
    pub x: u16,
    /// Top edge cell (inclusive).
    pub y: u16,
    /// Total cells, including all border columns.
    pub width: u16,
    /// Total cells, including all border rows.
    pub height: u16,
    /// Interior padding inset, `0..=[MAX_PADDING]`.
    pub pad: u8,
}

impl BoxGeometry {
    /// Minimum outer cells of any drawn frame with that padding: two border
    /// lines plus padding both sides plus one text column/row.
    #[must_use]
    pub fn min_frame(padding: u8) -> (u16, u16) {
        let cells = u16::from(padding) * 2 + 3;
        (cells, cells)
    }

    /// Geometry for a surface that ALREADY draws a frame (explicit
    /// `Double`/`Heavy` weight, or a corner-painting surface such as the
    /// composer). Fail-closed on sizes too small to hold a frame — a tiny
    /// box never shrinks into an implicit one.
    #[must_use]
    pub fn framed(x: u16, y: u16, width: u16, height: u16, pad: u8) -> Option<Self> {
        let (min_w, min_h) = Self::min_frame(pad);
        if width < min_w || height < min_h {
            return None;
        }
        Some(Self {
            x,
            y,
            width,
            height,
            pad,
        })
    }

    /// Unframed fill-only rectangle for any positive size; no border rows
    /// exist and no frame is implied.
    #[must_use]
    pub fn filled(x: u16, y: u16, width: u16, height: u16) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            x,
            y,
            width,
            height,
            pad: 0,
        })
    }

    /// Clip against the viewport (`cols` x `rows`); `None` when nothing
    /// remains visible. Partially visible boxes keep their interior
    /// alignment, which prevents a background fill from painting beyond a
    /// clipped frame edge.
    #[must_use]
    pub fn clipped(&self, cols: u16, rows: u16) -> Option<Self> {
        let right = (u32::from(self.x) + u32::from(self.width)).min(u32::from(cols));
        let bottom = (u32::from(self.y) + u32::from(self.height)).min(u32::from(rows));
        let left = u32::from(self.x.min(cols));
        let top = u32::from(self.y.min(rows));
        if right <= left || bottom <= top {
            return None;
        }
        Some(Self {
            x: left as u16,
            y: top as u16,
            width: (right - left) as u16,
            height: (bottom - top) as u16,
            pad: self.pad,
        })
    }

    /// The one-box background: the full frame rect INCLUDING the border
    /// cells (the glyph border paints over its own half of the fill).
    /// Identical to the frame rect that produced this value; explicit to
    /// document the shared source and to give fills a single call site.
    #[must_use]
    pub fn fill(self) -> Self {
        self
    }

    /// Top and bottom border rows of the frame; `None` when the rect is a
    /// fill-only/degenerate shape that cannot hold two border rows.
    #[must_use]
    pub fn frame_rows(self) -> Option<(u16, u16)> {
        if self.height < 2 {
            return None;
        }
        Some((self.y, self.y + self.height - 1))
    }

    /// Interior inside the one-cell border; `None` when nothing remains.
    #[must_use]
    pub fn interior(&self) -> Option<(u16, u16, u16, u16)> {
        if (self.width < 3) || (self.height < 3) {
            return None;
        }
        Some((self.x + 1, self.y + 1, self.width - 2, self.height - 2))
    }

    /// Text area inset by border AND padding; `None` when nothing remains.
    #[must_use]
    pub fn text_area(&self) -> Option<(u16, u16, u16, u16)> {
        let inset = u16::from(self.pad);
        if self.width < 2 * inset + 3 || self.height < 2 * inset + 3 {
            return None;
        }
        Some((
            self.x + 1 + inset,
            self.y + 1 + inset,
            self.width - 2 - 2 * inset,
            self.height - 2 - 2 * inset,
        ))
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

#[cfg(test)]
mod radius_never_frames_policy {
    use super::*;

    #[test]
    fn rounded_corner_predicate_is_style_and_cell_local() {
        let rect = (10, 4, 8, 5);
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        assert!(rounded.rounded_corner_cell(rect, 10, 4, rounded.corner_glyphs().top_left));
        assert!(!rounded.rounded_corner_cell(rect, 11, 4, rounded.corner_glyphs().top_left));
        assert!(!rounded.rounded_corner_cell(rect, 17, 8, 'x'));
        assert!(!rounded.rounded_corner_cell((10, 4, 1, 1), 10, 4, rounded.corner_glyphs().top_left));
        let sharp = EffectiveBox { preset: RadiusPreset::Sharp, ..rounded };
        assert!(!sharp.rounded_corner_cell(rect, 10, 4, sharp.corner_glyphs().top_left));
        let double = EffectiveBox { border_style: BorderStyle::Double, ..rounded };
        assert!(!double.rounded_corner_cell(rect, 10, 4, double.corner_glyphs().top_left));
        let custom_zero = EffectiveBox { preset: RadiusPreset::Custom, radius: 0, ..rounded };
        assert!(!custom_zero.rounded_corner_cell(rect, 10, 4, custom_zero.corner_glyphs().top_left));
        let custom = EffectiveBox { preset: RadiusPreset::Custom, radius: 1, ..rounded };
        assert!(custom.rounded_corner_cell(rect, 10, 4, custom.corner_glyphs().top_left));
    }

    /// A radius choice (any preset, any logical radius) can never produce
    /// a full drawn frame: only an explicit `Double`/`Heavy` line weight
    /// makes [`EffectiveBox::explicit_line_frame`] true, per box.
    #[test]
    fn radius_and_presets_never_create_a_frame() {
        for preset in [
            RadiusPreset::Sharp,
            RadiusPreset::Rounded,
            RadiusPreset::IosSquircle,
        ] {
            let mut appearance = BoxAppearance::default();
            appearance.preset = preset;
            for id in BoxId::ALL {
                assert!(
                    !appearance
                        .effective(id)
                        .explicit_line_frame(),
                    "{preset:?} must not frame {id:?}"
                );
            }
        }
        let mut custom = BoxAppearance::default();
        custom.preset = RadiusPreset::Custom;
        custom.radius = MAX_RADIUS;
        for id in BoxId::ALL {
            assert!(
                !custom.effective(id).explicit_line_frame(),
                "custom radius {MAX_RADIUS} must not frame {id:?}"
            );
        }

        // Explicit global line weight frames every box...
        let mut double = BoxAppearance::default();
        double.border_style = BorderStyle::Double;
        for id in BoxId::ALL {
            assert!(double.effective(id).explicit_line_frame(), "{id:?}");
        }
        // ...and an explicit per-box override stays scoped to that box.
        let mut heavy = BoxAppearance::default();
        heavy.overrides.insert(
            BoxId::Tool,
            BoxOverride {
                border_style: Some(BorderStyle::Heavy),
                ..BoxOverride::default()
            },
        );
        for id in BoxId::ALL {
            assert_eq!(
                heavy.effective(id).explicit_line_frame(),
                matches!(id, BoxId::Tool),
                "{id:?}"
            );
        }
    }
}

#[cfg(test)]
mod shared_box_geometry {
    use super::*;

    /// Fill and border come from ONE box rect: the fill is the full frame
    /// rect, the border rows sit ON its edge rows, text is inset by border
    /// plus padding, and radius never enters the geometry (R1).
    #[test]
    fn fill_and_border_share_one_rect_per_padding() {
        for pad in [0u8, 2, MAX_PADDING] {
            let (min_w, min_h) = BoxGeometry::min_frame(pad);
            assert_eq!(
                (min_w, min_h),
                (u16::from(pad) * 2 + 3, u16::from(pad) * 2 + 3),
                "pad {pad} needs border+borders+one text cell on each axis"
            );
            let g = BoxGeometry::framed(7, 3, 40, 12, pad)
                .expect("box at or above minimum is frameable");
            // One-box identity: fill == the same outer rect.
            let f = g.fill();
            assert_eq!(
                (f.x, f.y, f.width, f.height),
                (7, 3, 40, 12),
                "fill is the full frame rect incl. border cells, pad {pad}"
            );
            let (top, bottom) = g.frame_rows().expect("framed box has two border rows");
            assert_eq!((top, bottom), (3, 3 + g.height - 1), "border rows ON the edges");
            let (tx, ty, tw, th) = g.text_area().expect("framed guarantees a text area");
            assert_eq!(
                (tx, ty, tw, th),
                (
                    7 + 1 + u16::from(pad),
                    3 + 1 + u16::from(pad),
                    40 - 2 - u16::from(pad) * 2,
                    12 - 2 - u16::from(pad) * 2
                ),
                "text inset = border + padding, pad {pad}"
            );
            let (ix, iy, iw, ih) = g.interior().expect("framed box has an interior");
            assert!((ix <= tx) && (iy <= ty) && (ix + iw >= tx + tw) && (iy + ih >= ty + th));
        }
    }

    /// Small dimensions fail closed instead of shrinking into implicit or
    /// degenerate frames; the border style does not change the cell grid
    /// (thickness is glyph weight, not extra cells).
    #[test]
    fn small_sizes_fail_closed_and_styles_share_the_grid() {
        assert_eq!(BoxGeometry::framed(0, 0, 2, 10, 0), None, "");
        assert_eq!(BoxGeometry::framed(0, 0, 10, 2, 0), None);
        assert_eq!(BoxGeometry::framed(0, 0, 2, 2, 0), None);
        let mut appearance = BoxAppearance::default();
        for border_style in [BorderStyle::Single, BorderStyle::Double, BorderStyle::Heavy] {
            appearance.border_style = border_style;
            let g = BoxGeometry::framed(1, 1, 5, 5, 0).expect("frame at (5,5)");
            assert_eq!(g.fill(), g, "one-box regardless of {border_style:?}");
        }
        // Fill-only rects allow ANY positive size and never imply a frame.
        let tiny = BoxGeometry::filled(0, 0, 1, 1).expect("1x1 fill is legal");
        assert_eq!(tiny.pad, 0);
        assert_eq!(tiny.frame_rows(), None, "no implicit border rows");
        assert_eq!(BoxGeometry::filled(0, 0, 0, 1), None);
        assert_eq!(BoxGeometry::filled(0, 0, 1, 0), None);
    }

    /// Clipping happens in shared geometry: fills clip to the viewport
    /// instead of painting over frame edges, and fully off-viewport boxes
    /// disappear instead of leaving stray backgrounds.
    #[test]
    fn clip_matches_fills_and_frames_edges() {
        let viewport = (80u16, 24u16);
        let g = BoxGeometry::framed(0, 0, 80, 24, 0).expect("in viewport");
        assert_eq!(g.clipped(viewport.0, viewport.1), Some(g));
        let g = BoxGeometry::framed(10, 20, 80, 24, 0).expect("size-only check");
        let c = g.clipped(viewport.0, viewport.1).expect("stays visible");
        assert_eq!(
            (c.x, c.y, c.width, c.height),
            (10, 20, 70, 4),
            "bottom/right clipped to the SAME edge the frame must stop on"
        );
        assert_eq!(g.clipped(0, 0), None, "fully hidden → no paint");
        assert_eq!(BoxGeometry::framed(0, 99, 10, 3, 0).unwrap().clipped(10, 24), None);
        // A narrow viewport clips interior alignment, not the border rows.
        let t = BoxGeometry::framed(0, 0, 10, 5, 0).unwrap().clipped(4, 24).unwrap();
        assert_eq!(t.text_area().is_none() || t.width >= 3, true);
    }
}

#[cfg(test)]
mod corner_glyph_shapes {
    use super::*;

    /// R2: IosSquircle must render its OWN shape — quarter arcs, distinct
    /// from Rounded elbows — wherever a surface already paints corners, and
    /// only there. Radius never activates corners on frameless surfaces.
    #[test]
    fn squircle_is_distinct_from_rounded_and_radius_never_frames() {
        let squircle = EffectiveBox {
            preset: RadiusPreset::IosSquircle,
            radius: MAX_RADIUS,
            border_style: BorderStyle::Single,
            thickness: MIN_THICKNESS,
            padding: 0,
        };
        assert_ne!(
            squircle.corner_glyphs().to_array(),
            EffectiveBox {
                preset: RadiusPreset::Rounded,
                ..squircle
            }
            .corner_glyphs()
            .to_array(),
            "squircle may not be a silent rename of Rounded"
        );
        assert_eq!(
            squircle.corner_glyphs().to_array(),
            SQUIRCLE_CORNER_GLYPHS,
            "quarter-arc set, reading order"
        );
        assert_eq!(
            squircle.border_glyphs(),
            ('─', '│'),
            "squircle only changes corners, never weights"
        );
        for style in [BorderStyle::Double, BorderStyle::Heavy] {
            assert_ne!(
                EffectiveBox {
                    border_style: style,
                    ..squircle
                }
                .corner_glyphs()
                .to_array(),
                SQUIRCLE_CORNER_GLYPHS,
                "heavier weights keep their native sharp corners"
            );
        }
        // Custom radius stays the rounded elbows, never squircle arcs.
        let custom = EffectiveBox {
            preset: RadiusPreset::Custom,
            radius: MAX_RADIUS,
            border_style: BorderStyle::Single,
            thickness: MIN_THICKNESS,
            padding: 0,
        };
        assert_eq!(
            custom.corner_glyphs().to_array(),
            ROUNDED_CORNER_GLYPHS,
            "Custom radius maps to Rounded elbows, not squircle arcs"
        );
        // The one place rows are produced: same glyph source both ways.
        let appearance = BoxAppearance::default();
        assert_eq!(appearance.preset, RadiusPreset::Sharp);
        assert_eq!(appearance.effective(BoxId::Menu).border_glyphs(), ('─', '│'));
    }
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

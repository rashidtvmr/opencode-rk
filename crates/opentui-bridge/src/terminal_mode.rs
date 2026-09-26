#![forbid(unsafe_code)]
//! Terminal background mode and system-theme color math.
//!
//! Source: `/home/rashid/projects/opencode` at `a0d9b6c`,
//! `packages/tui/src/theme/index.ts:353-553`. The TypeScript color channels are
//! normalized to `[0, 1]`; this module uses `f32` only and converts back to
//! `u8` with the source's floor-at-output behavior.

/// Return normalized terminal luminance for an RGB byte triple.
///
/// This is the same coefficient order and threshold domain as the TypeScript
/// `terminalMode` calculation. No external color or math crate is needed.
#[must_use]
pub fn luminance(r: u8, g: u8, b: u8) -> f32 {
    0.299 * f32::from(r) / 255.0 + 0.587 * f32::from(g) / 255.0 + 0.114 * f32::from(b) / 255.0
}

/// Classify a terminal background using the upstream comparison.
///
/// `Some(true)` means the comparison `luminance > 0.5` succeeded, so the
/// terminal is light. `Some(false)` means it is dark. `None` preserves the
/// upstream `undefined` result when no default background is available.
/// Use [`is_dark`] when a semantic dark-mode flag is needed.
#[must_use]
pub fn terminal_mode(bg: Option<(u8, u8, u8)>) -> Option<bool> {
    bg.map(|(r, g, b)| luminance(r, g, b) > 0.5)
}

/// Return whether a present background selects dark mode.
///
/// A missing background is not a mode, so callers that need that distinction
/// should use [`terminal_mode`] directly.
#[must_use]
pub fn is_dark(bg: (u8, u8, u8)) -> bool {
    !terminal_mode(Some(bg)).unwrap_or(false)
}

/// Apply a precomputed channel ratio from the arbitrary-background gray path.
///
/// The source computes `newLum / luminance` after adjusting luminance, then
/// multiplies each byte channel by that ratio. Ratios above one brighten and
/// ratios below one darken. Output is clamped to the `u8` range and floored,
/// matching the TypeScript `min`/`max` plus `Math.floor` sequence.
#[must_use]
pub fn gray_scale(bg: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
    // A finite ratio is part of the caller contract. Treat invalid float
    // input as zero rather than allowing a NaN cast to vary by platform.
    let ratio = if ratio.is_finite() {
        ratio.max(0.0)
    } else {
        0.0
    };
    let scale = |channel: u8| (f32::from(channel) * ratio).clamp(0.0, 255.0).floor() as u8;
    (scale(bg.0), scale(bg.1), scale(bg.2))
}

/// Return the midpoint muted-text color for the selected terminal mode.
///
/// The TS function scales from the actual background luminance. This focused
/// helper owns the requested midpoint contract, so dark uses
/// `floor(160 + 128 * 0.3) = 198`, while light uses
/// `floor(100 - (255 - 128) * 0.2) = 74`.
#[must_use]
pub fn muted_text(is_dark: bool) -> (u8, u8, u8) {
    const MID_LUMINANCE: f32 = 128.0;
    let value = if is_dark {
        (160.0 + MID_LUMINANCE * 0.3).floor().min(200.0) as u8
    } else {
        (100.0 - (255.0 - MID_LUMINANCE) * 0.2).floor().max(60.0) as u8
    };
    (value, value, value)
}

/// Return the source's diff tint alpha for dark or light mode.
#[must_use]
pub fn diff_alpha(is_dark: bool) -> f32 {
    if is_dark {
        0.22
    } else {
        0.14
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_background_is_none() {
        assert_eq!(terminal_mode(None), None);
    }

    #[test]
    fn terminal_mode_classifies_extremes_and_mid_gray() {
        assert_eq!(terminal_mode(Some((0, 0, 0))), Some(false));
        assert_eq!(terminal_mode(Some((255, 255, 255))), Some(true));
        assert_eq!(terminal_mode(Some((127, 127, 127))), Some(false));
        assert_eq!(terminal_mode(Some((128, 128, 128))), Some(true));
        assert!(is_dark((0, 0, 0)));
        assert!(!is_dark((255, 255, 255)));
    }

    #[test]
    fn luminance_uses_normalized_coefficients() {
        assert_eq!(luminance(0, 0, 0), 0.0);
        assert_eq!(luminance(255, 255, 255), 1.0);
        assert!((luminance(255, 0, 0) - 0.299).abs() < f32::EPSILON);
    }

    #[test]
    fn gray_scale_applies_and_bounds_ratio() {
        assert_eq!(gray_scale((100, 150, 200), 0.5), (50, 75, 100));
        assert_eq!(gray_scale((200, 100, 50), 1.5), (255, 150, 75));
        assert_eq!(gray_scale((200, 100, 50), -1.0), (0, 0, 0));
    }

    #[test]
    fn muted_text_scales_at_midpoint() {
        assert_eq!(muted_text(true), (198, 198, 198));
        assert_eq!(muted_text(false), (74, 74, 74));
    }

    #[test]
    fn diff_alpha_uses_mode_constants() {
        assert_eq!(diff_alpha(true), 0.22);
        assert_eq!(diff_alpha(false), 0.14);
    }
}

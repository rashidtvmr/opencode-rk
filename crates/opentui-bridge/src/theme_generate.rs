#![forbid(unsafe_code)]
//! Generated system palette + grayscale/muted math.
//!
//! Companion to `crate::theme_registry` (luminance split at 128, extreme
//! anchors `180`/`#b4b4b4` and `75`/`#4b4b4b` mirror `muted_text_color`).
//! Integrator: add `pub mod theme_generate;` to `lib.rs` (not touched here).

/// Plain RGB triple.
pub type Rgb = (u8, u8, u8);

const fn lerp_lane(a: u8, b: u8, num: u32, den: u32) -> u8 {
    let a = a as u32;
    let b = b as u32;
    if b >= a {
        (a + (b - a) * num / den) as u8
    } else {
        (a - (a - b) * num / den) as u8
    }
}

const fn mix(a: Rgb, b: Rgb, num: u32, den: u32) -> Rgb {
    (
        lerp_lane(a.0, b.0, num, den),
        lerp_lane(a.1, b.1, num, den),
        lerp_lane(a.2, b.2, num, den),
    )
}

/// 8-step scale `black -> bg -> white`. Steps 0-3 run black to `bg`
/// (`bg` exactly at index 3); steps 4-7 run `bg` to white. Names are
/// echoed as `{name}-{i}`.
#[must_use]
pub fn generate_system(name: &str, bg: Rgb) -> [(String, Rgb); 8] {
    let black: Rgb = (0, 0, 0);
    let white: Rgb = (255, 255, 255);
    let at = |i: usize| -> Rgb {
        if i <= 3 {
            mix(black, bg, i as u32, 3)
        } else {
            mix(bg, white, (i - 3) as u32, 4)
        }
    };
    [
        (format!("{name}-0"), at(0)),
        (format!("{name}-1"), at(1)),
        (format!("{name}-2"), at(2)),
        (format!("{name}-3"), at(3)),
        (format!("{name}-4"), at(4)),
        (format!("{name}-5"), at(5)),
        (format!("{name}-6"), at(6)),
        (format!("{name}-7"), at(7)),
    ]
}

/// Linear 8-step gray ramp black to white: `step * 255 / 7`.
/// Steps past 7 clamp to white.
#[must_use]
pub const fn gray_scale(step: u8) -> Rgb {
    let s = if step > 7 { 7 } else { step };
    let v = s as u32 * 255 / 7;
    (v as u8, v as u8, v as u8)
}

/// Muted text for a background luminance: dark bg (`< 128`, mirroring
/// `theme_registry` split) gets light text `180`, else dark text `75`.
#[must_use]
pub const fn muted_text(bg_lum: u8) -> Rgb {
    if bg_lum < 128 {
        (180, 180, 180)
    } else {
        (75, 75, 75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_has_eight_steps() {
        assert_eq!(generate_system("sys", (100, 120, 140)).len(), 8);
    }

    #[test]
    fn scale_endpoints() {
        let s = generate_system("sys", (100, 120, 140));
        assert_eq!(s[0].1, (0, 0, 0));
        assert_eq!(s[3].1, (100, 120, 140));
        assert_eq!(s[7].1, (255, 255, 255));
    }

    #[test]
    fn scale_names_echoed() {
        let s = generate_system("ocean", (10, 20, 30));
        for (i, (n, _)) in s.iter().enumerate() {
            assert_eq!(*n, format!("ocean-{i}"));
        }
    }

    #[test]
    fn gray_endpoints() {
        assert_eq!(gray_scale(0), (0, 0, 0));
        assert_eq!(gray_scale(7), (255, 255, 255));
        assert_eq!(gray_scale(255), (255, 255, 255));
    }

    #[test]
    fn gray_midpoint_monotonic() {
        let a = gray_scale(2).0;
        let b = gray_scale(5).0;
        assert!(a < b && b < 255);
    }

    #[test]
    fn muted_dark_bg_light_text() {
        assert_eq!(muted_text(0), (180, 180, 180));
        assert_eq!(muted_text(127), (180, 180, 180));
    }

    #[test]
    fn muted_light_bg_dark_text() {
        assert_eq!(muted_text(128), (75, 75, 75));
        assert_eq!(muted_text(255), (75, 75, 75));
    }
}

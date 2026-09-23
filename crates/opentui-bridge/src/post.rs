//! CPU-exact post-processing filters (bridge of opentui `post/`).
//!
//! Sources: `packages/core/src/post/filters.ts` (`applyScanlines`,
//! `applyInvert`, `toU8`/`channel`/`setRgb`), `post/matrices.ts`
//! (`SEPIA_MATRIX`, `GRAYSCALE_MATRIX`, `INVERT_MATRIX`),
//! `post/effects.ts`, `buffer.ts` (`colorMatrix`/`colorMatrixUniform`).
//!
//! Ported CPU-exact: scanlines, invert, grayscale, sepia, generic 4x4
//! colorMatrix (uniform + per-cell masked). Blend semantics match native:
//! `result = orig + (xformed - orig) * t`, `t = cell * strength`.
//!
//! Shader / native-only cuts (NOT ported by design): `DistortionEffect`
//! (glitch shift/flip/color), `VignetteEffect` (zero-matrix attenuation),
//! `CloudsEffect`/`FlamesEffect` (Perlin FBM), `CRTRollingBarEffect`,
//! `applyNoise`/`applyChromaticAberration`/`BloomEffect` (need RNG/shaders).
//! Scanlines in TS target the bg plane only; here the caller selects the plane.

#![forbid(unsafe_code)]

/// Self-contained RGBA pixel, u8 channels (low byte, as in TS buffers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Row-major 4x4 RGBA matrix: row i = [R,G,B,A] coeffs for output channel i.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
    pub m: [f32; 16],
}

/// Sepia matrix, exact values from `post/matrices.ts` SEPIA_MATRIX.
pub const SEPIA: [f32; 16] = [
    0.393, 0.769, 0.189, 0.0, //
    0.349, 0.686, 0.168, 0.0, //
    0.272, 0.534, 0.131, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

impl ColorMatrix {
    pub const IDENTITY: Self = Self {
        m: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };
    pub const SEPIA: Self = Self { m: SEPIA };
    pub const GRAYSCALE: Self = Self {
        m: [
            0.299, 0.587, 0.114, 0.0, //
            0.299, 0.587, 0.114, 0.0, //
            0.299, 0.587, 0.114, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };
    pub const INVERT: Self = Self {
        m: [
            -1.0, 0.0, 0.0, 1.0, //
            0.0, -1.0, 0.0, 1.0, //
            0.0, 0.0, -1.0, 1.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };

    /// Transform one normalized pixel; returns [r,g,b,a] unclamped.
    pub fn transform(&self, r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
        let m = &self.m;
        [
            m[0] * r + m[1] * g + m[2] * b + m[3] * a,
            m[4] * r + m[5] * g + m[6] * b + m[7] * a,
            m[8] * r + m[9] * g + m[10] * b + m[11] * a,
            m[12] * r + m[13] * g + m[14] * b + m[15] * a,
        ]
    }
}

/// TS `toU8`: round(clamp01(finite ? v : 0)) * 255.
fn to_u8(v: f32) -> u8 {
    let c = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    (c * 255.0).round() as u8
}

fn chan(v: u8) -> f32 {
    v as f32 / 255.0
}

fn blend(orig: f32, xformed: f32, t: f32) -> u8 {
    to_u8(orig + (xformed - orig) * t)
}

fn apply_t(buf: &mut [Pixel], m: &ColorMatrix, t: f32) {
    for p in buf.iter_mut() {
        let (r, g, b, a) = (chan(p.r), chan(p.g), chan(p.b), chan(p.a));
        let o = m.transform(r, g, b, a);
        p.r = blend(r, o[0], t);
        p.g = blend(g, o[1], t);
        p.b = blend(b, o[2], t);
        p.a = blend(a, o[3], t);
    }
}

/// Uniform colorMatrix over the whole buffer (TS `colorMatrixUniform`).
pub fn apply_color_matrix(buf: &mut [Pixel], m: &ColorMatrix, strength: f32) {
    if strength == 0.0 {
        return;
    }
    apply_t(buf, m, strength);
}

/// Per-cell masked colorMatrix (TS `colorMatrix`); `cells` = (x, y, strength).
pub fn apply_color_matrix_masked(
    buf: &mut [Pixel],
    width: usize,
    m: &ColorMatrix,
    cells: &[(usize, usize, f32)],
    strength: f32,
) {
    if width == 0 {
        return;
    }
    let height = buf.len() / width;
    for &(x, y, s) in cells {
        if x >= width || y >= height {
            continue;
        }
        let p = &mut buf[y * width + x];
        let (r, g, b, a) = (chan(p.r), chan(p.g), chan(p.b), chan(p.a));
        let o = m.transform(r, g, b, a);
        let t = s * strength;
        p.r = blend(r, o[0], t);
        p.g = blend(g, o[1], t);
        p.b = blend(b, o[2], t);
        p.a = blend(a, o[3], t);
    }
}

/// TS `applyScanlines`: darken rows y % step == 0 by gain `strength`.
/// No-op when strength == 1.0 or step < 1. Alpha preserved (TS `setRgb`).
pub fn apply_scanlines(buf: &mut [Pixel], width: usize, strength: f32, step: usize) {
    if strength == 1.0 || step < 1 || width == 0 {
        return;
    }
    let height = buf.len() / width;
    let mut y = 0;
    while y < height {
        for x in 0..width {
            let p = &mut buf[y * width + x];
            p.r = to_u8(chan(p.r) * strength);
            p.g = to_u8(chan(p.g) * strength);
            p.b = to_u8(chan(p.b) * strength);
        }
        y += step;
    }
}

/// TS `applyInvert`: photographic negative, blended by strength.
pub fn apply_invert(buf: &mut [Pixel], strength: f32) {
    apply_color_matrix(buf, &ColorMatrix::INVERT, strength);
}

/// Grayscale via luminance weights (TS `GRAYSCALE_MATRIX`).
pub fn apply_grayscale(buf: &mut [Pixel], strength: f32) {
    apply_color_matrix(buf, &ColorMatrix::GRAYSCALE, strength);
}

/// Sepia tone (TS `SEPIA_MATRIX`).
pub fn apply_sepia(buf: &mut [Pixel], strength: f32) {
    apply_color_matrix(buf, &ColorMatrix::SEPIA, strength);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(w: usize, h: usize) -> Vec<Pixel> {
        vec![Pixel::new(255, 255, 255, 255); w * h]
    }

    #[test]
    fn scanlines_darkens_step_rows() {
        let mut buf = white(2, 4);
        apply_scanlines(&mut buf, 2, 0.5, 2);
        // 0.5 * 255 = 127.5 -> round -> 128 on rows 0 and 2 only.
        for y in 0..4 {
            for x in 0..2 {
                let p = buf[y * 2 + x];
                let want = if y % 2 == 0 { 128 } else { 255 };
                assert_eq!((p.r, p.g, p.b, p.a), (want, want, want, 255), "x={x} y={y}");
            }
        }
    }

    #[test]
    fn invert_exact() {
        let mut buf = vec![Pixel::new(255, 0, 128, 255)];
        apply_invert(&mut buf, 1.0);
        // 1 - 128/255 = 127/255 -> 127.
        assert_eq!(buf[0], Pixel::new(0, 255, 127, 255));
        // strength 0 is a no-op.
        let mut buf = vec![Pixel::new(10, 20, 30, 40)];
        apply_invert(&mut buf, 0.0);
        assert_eq!(buf[0], Pixel::new(10, 20, 30, 40));
    }

    #[test]
    fn grayscale_coefficients() {
        // Pure red: lum = 0.299 -> round(0.299*255) = round(76.245) = 76.
        let mut buf = vec![Pixel::new(255, 0, 0, 255)];
        apply_grayscale(&mut buf, 1.0);
        assert_eq!(buf[0], Pixel::new(76, 76, 76, 255));
        // Pure green: lum = 0.587 -> round(149.685) = 150.
        let mut buf = vec![Pixel::new(0, 255, 0, 200)];
        apply_grayscale(&mut buf, 1.0);
        assert_eq!(buf[0], Pixel::new(150, 150, 150, 200));
    }

    #[test]
    fn sepia_matrix_values() {
        assert_eq!(
            SEPIA,
            [
                0.393, 0.769, 0.189, 0.0, //
                0.349, 0.686, 0.168, 0.0, //
                0.272, 0.534, 0.131, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ]
        );
        // White: r,g clamp to 255; b = 0.937 -> round(238.935) = 239.
        let mut buf = white(1, 1);
        apply_sepia(&mut buf, 1.0);
        assert_eq!(buf[0], Pixel::new(255, 255, 239, 255));
    }

    #[test]
    fn color_matrix_identity_noop() {
        let mut buf = vec![Pixel::new(12, 34, 56, 78), Pixel::new(200, 100, 50, 255)];
        let orig = buf.clone();
        apply_color_matrix(&mut buf, &ColorMatrix::IDENTITY, 1.0);
        assert_eq!(buf, orig);
    }

    #[test]
    fn uniform_and_masked_agree() {
        let src = vec![Pixel::new(200, 100, 50, 255), Pixel::new(10, 220, 30, 255)];
        let mut a = src.clone();
        let mut b = src.clone();
        apply_color_matrix(&mut a, &ColorMatrix::SEPIA, 0.6);
        let cells = [(0, 0, 1.0), (1, 0, 1.0)];
        apply_color_matrix_masked(&mut b, 2, &ColorMatrix::SEPIA, &cells, 0.6);
        assert_eq!(a, b);
    }
}

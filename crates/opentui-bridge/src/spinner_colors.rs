#![forbid(unsafe_code)]
//! Scanner trail / inactive colors (mirrors `packages/tui/src/ui/spinner.ts` @ a0d9b6c).
//!
//! # SOURCE EVIDENCE (TS checkout at /home/rashid/projects/opencode, commit a0d9b6c)
//! - `deriveTrailColors` (`spinner.ts:199-231`): step 0 alpha 1.0/bf 1.0;
//!   step 1 alpha 0.9/bf 1.15 (bloom); step i>=2 alpha `0.65^(i-1)`/bf 1.0;
//!   lanes `min(1, base * bf)`, pushed via `RGBA.fromValues(r,g,b,alpha)`.
//! - `deriveInactiveColor` (`spinner.ts:239-244`): same rgb, alpha = factor (default 0.2).
//! - `createColors` (`spinner.ts:336-368`): returns a `ColorGenerator`
//!   `(frameIndex, charIndex, totalFrames, totalChars) => ColorInput` closure
//!   over `createKnightRiderTrail` (`:141-191`) - deterministic, NOT random.
//! - `RGBA.fromValues` clamps 0..1 then `toU8 = round(clamp * 255)`; `r/g/b/a`
//!   getters return byte/255 (chunk-bun-9gqvxy8c.js:1056-1058,1136-1159).
//! - Alphas reuse `crate::spinner::trail_alpha` (`spinner.rs:38-44`, same ramp).
//!
//! # DIVERGENCE: random -> deterministic LCG
//! TS has no seeded color RNG. The only `Math.random` near spinners is per-mount
//! CSS animation `delay/duration` in `packages/ui/src/components/spinner.tsx:9-10`
//! (unseeded, not colors). `ColorGen` below is a test-deterministic stand-in:
//! Knuth MMIX LCG (`a=6364136223846793005`, `c=1442695040888963407`, odd
//! increment chosen for full period); output bytes can never match a
//! `Math.random` run by construction.
//!
//! ponytail: LCG only, no `getrandom`/OS entropy. Upgrade: swap `ColorGen::next`
//! internals for an entropy-seeded RNG when nondeterminism is wanted.

use crate::color::Rgba;
use crate::spinner::trail_alpha;

/// Default trail length (`deriveTrailColors` `steps = 6`, `:199`).
pub const DEFAULT_TRAIL_STEPS: u8 = 6;
/// Bloom factor for trail step 1 (`:216`).
pub const BLOOM_FACTOR: f32 = 1.15;
/// Default inactive alpha factor (`deriveInactiveColor` `factor = 0.2`, `:239`).
pub const INACTIVE_ALPHA_FACTOR: f32 = 0.2;

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Alpha-ramp trail from a bright base (`deriveTrailColors`, `:199-231`).
/// `len` 0 yields empty; RGB lanes are `u8` so `min(1, .)` clamps naturally.
#[must_use]
pub fn derive_trail_colors(base: Rgba, len: u8) -> Vec<Rgba> {
    let (r, g, b) = (base.r as f32 / 255.0, base.g as f32 / 255.0, base.b as f32 / 255.0);
    (0..len as usize)
        .map(|i| {
            let bf = if i == 1 { BLOOM_FACTOR } else { 1.0 };
            Rgba::new(to_u8((r * bf).min(1.0)), to_u8((g * bf).min(1.0)), to_u8((b * bf).min(1.0)), to_u8(trail_alpha(i)))
        })
        .collect()
}

/// Same rgb with dimmed alpha (`deriveInactiveColor`, `:239-244`, factor 0.2).
#[must_use]
pub fn derive_inactive_color(base: Rgba) -> Rgba {
    Rgba::new(base.r, base.g, base.b, to_u8(INACTIVE_ALPHA_FACTOR))
}

/// Deterministic pseudo-random opaque color source (see DIVERGENCE note).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorGen {
    pub seed: u64,
}

impl ColorGen {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Advance the LCG and emit the top 3 state bytes as opaque RGB.
    pub fn next(&mut self) -> Rgba {
        const A: u64 = 6364136223846793005;
        const C: u64 = 1442695040888963407;
        self.seed = self.seed.wrapping_mul(A).wrapping_add(C);
        Rgba::new((self.seed >> 56) as u8, (self.seed >> 48) as u8, (self.seed >> 40) as u8, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_trail_alpha_ramp() {
        assert_eq!(
            derive_trail_colors(Rgba::new(255, 0, 0, 255), 6),
            vec![
                Rgba::new(255, 0, 0, 255), // 1.0
                Rgba::new(255, 0, 0, 230), // 0.9
                Rgba::new(255, 0, 0, 166), // 0.65
                Rgba::new(255, 0, 0, 108), // 0.65^2
                Rgba::new(255, 0, 0, 70),  // 0.65^3
                Rgba::new(255, 0, 0, 46),  // 0.65^4
            ]
        );
    }

    #[test]
    fn bloom_brightens_step_one() {
        // (128,64,32)*1.15 = (147.2,73.6,36.8) -> (147,74,37), alpha 230.
        assert_eq!(derive_trail_colors(Rgba::new(128, 64, 32, 255), 2)[1], Rgba::new(147, 74, 37, 230));
    }

    #[test]
    fn bloom_clamps_at_white() {
        // min(1, 1.0*1.15) = 1.0 per :223-225.
        assert_eq!(derive_trail_colors(Rgba::new(255, 255, 255, 255), 2)[1], Rgba::new(255, 255, 255, 230));
    }

    #[test]
    fn inactive_dims_alpha_only() {
        assert_eq!(derive_inactive_color(Rgba::new(255, 0, 0, 255)), Rgba::new(255, 0, 0, 51));
        assert_eq!(derive_inactive_color(Rgba::new(0, 128, 255, 255)), Rgba::new(0, 128, 255, 51));
    }

    #[test]
    fn empty_len_yields_empty() {
        assert!(derive_trail_colors(Rgba::new(255, 0, 0, 255), 0).is_empty());
    }

    #[test]
    fn color_gen_deterministic_literal() {
        // seed 0 -> state C=0x14057B7EF767814F -> top bytes (0x14,0x7B,0x7E).
        assert_eq!(ColorGen::new(0).next(), Rgba::new(20, 5, 123, 255));
        let (mut a, mut b) = (ColorGen::new(7), ColorGen::new(7));
        assert_eq!(a.next(), b.next());
        assert_ne!(ColorGen::new(1).next(), ColorGen::new(2).next());
    }
}

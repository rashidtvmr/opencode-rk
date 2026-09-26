#![forbid(unsafe_code)]
//! Fade timing math (`util/signal.ts:19-51` `createFadeIn`).
//! 160ms total, 16ms frames, smoothstep `t*t*(3-2t)`; pure, no timers.
//! ponytail: math only, no clock thread. Upgrade: drive from frame loop.

/// Total fade duration in ms.
pub const FADE_MS: u64 = 160;
/// Per-frame poll interval in ms.
pub const STEP_MS: u64 = 16;

/// Smoothstep easing clamped to 0..=1.
#[must_use]
pub fn smoothstep(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Frame count: `FADE_MS / STEP_MS` = 10.
#[must_use]
pub fn steps() -> u32 {
    (FADE_MS / STEP_MS) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
    }

    #[test]
    fn midpoint_is_half() {
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn clamps_outside() {
        assert_eq!(smoothstep(-2.0), 0.0);
        assert_eq!(smoothstep(2.0), 1.0);
    }

    #[test]
    fn ten_steps() {
        assert_eq!(FADE_MS, 160);
        assert_eq!(STEP_MS, 16);
        assert_eq!(steps(), 10);
    }
}

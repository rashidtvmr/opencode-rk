#![forbid(unsafe_code)]
//! Scroll acceleration selector (mirrors `packages/tui/src/util/scroll.ts:8-26`).
//!
//! TS checkout `a0d9b6c` (diverged from pinned `95daf90`; cite file:line).
//! `MacOSScrollAccel` lives in unvendored npm; mirror trait boundary + selector only.

/// TS `CustomSpeedScroll` fallback speed (`scroll.ts:26`).
pub const DEFAULT_SPEED: f64 = 3.0;

/// Finite positive speed only (rejects NaN/Inf/zero/negative).
#[must_use]
pub const fn is_valid_speed(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// TS `ScrollAcceleration` boundary (`scroll.ts:1,8-16`).
pub trait ScrollAcceleration {
    fn tick(&mut self, now_ms: Option<u64>) -> f64;
    fn reset(&mut self);
}

/// TS `CustomSpeedScroll` (`scroll.ts:8-16`): constant speed, no-op reset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomSpeedScroll {
    speed: f64,
}

impl CustomSpeedScroll {
    /// Fail-closed `None` on non-finite/non-positive speed.
    #[must_use]
    pub const fn new(speed: f64) -> Option<Self> {
        if is_valid_speed(speed) {
            Some(Self { speed })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn speed(&self) -> f64 {
        self.speed
    }
}

impl ScrollAcceleration for CustomSpeedScroll {
    fn tick(&mut self, _now_ms: Option<u64>) -> f64 {
        self.speed
    }

    fn reset(&mut self) {}
}

/// TS `ScrollConfig` (`scroll.ts:3-6`): flattened `enabled` + `speed`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollConfig {
    pub accel_enabled: bool,
    pub speed: Option<f64>,
}

/// Selectable backend: native macOS vs constant-speed fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccelKind {
    MacOsDefault,
    Custom(f64),
}

/// TS `getScrollAcceleration` (`scroll.ts:18-27`); invalid speed falls back to default.
#[must_use]
pub fn select_acceleration(cfg: &ScrollConfig) -> AccelKind {
    if cfg.accel_enabled {
        return AccelKind::MacOsDefault;
    }
    match cfg.speed {
        Some(v) if is_valid_speed(v) => AccelKind::Custom(v),
        _ => AccelKind::Custom(DEFAULT_SPEED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_returns_speed() {
        let mut s = CustomSpeedScroll::new(5.0).unwrap();
        assert_eq!(s.tick(None), 5.0);
        assert_eq!(s.tick(Some(123)), 5.0);
    }

    #[test]
    fn reset_is_green() {
        let mut s = CustomSpeedScroll::new(2.0).unwrap();
        s.reset();
        assert_eq!(s.tick(None), 2.0);
    }

    #[test]
    fn enabled_selects_macos() {
        let cfg = ScrollConfig { accel_enabled: true, speed: Some(5.0) };
        assert_eq!(select_acceleration(&cfg), AccelKind::MacOsDefault);
    }

    #[test]
    fn some_speed_selects_custom() {
        let cfg = ScrollConfig { accel_enabled: false, speed: Some(5.0) };
        assert_eq!(select_acceleration(&cfg), AccelKind::Custom(5.0));
    }

    #[test]
    fn empty_selects_default() {
        assert_eq!(select_acceleration(&ScrollConfig::default()), AccelKind::Custom(DEFAULT_SPEED));
        assert_eq!(DEFAULT_SPEED, 3.0);
    }

    #[test]
    fn invalid_speed_rejected() {
        assert!(CustomSpeedScroll::new(f64::NAN).is_none());
        assert!(CustomSpeedScroll::new(-1.0).is_none());
        assert!(CustomSpeedScroll::new(0.0).is_none());
        assert!(CustomSpeedScroll::new(f64::INFINITY).is_none());
        for bad in [f64::NAN, -2.0, 0.0] {
            let cfg = ScrollConfig { accel_enabled: false, speed: Some(bad) };
            assert_eq!(select_acceleration(&cfg), AccelKind::Custom(DEFAULT_SPEED));
        }
    }
}

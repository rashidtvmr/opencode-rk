//! Injectable deterministic frame clock (PAR-02/07/08).
//!
//! Every painted animation timestamp in the native event loop derives from a
//! single elapsed-millisecond source. Upstream keeps one `performance.now()`
//! epoch and steps it from the render loop; oc2 previously called
//! `Instant::now()` at that one epoch and derived all eight animation facts
//! from it. This wrapper preserves the exact wall-clock behaviour when
//! unfrozen while allowing a harness to freeze and step the clock so frame
//! sequences become reproducible.
//!
//! `OC2_FRAME_CLOCK_MS=<u64>` selects a fixed startup checkpoint. Invalid
//! values are ignored; the override is intended for test runs only.

/// Single source of elapsed milliseconds for the native event loop.
#[derive(Clone, Copy, Debug)]
pub struct FrameClock {
    epoch: std::time::Instant,
    base_ms: u64,
    frozen_ms: Option<u64>,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock {
    #[must_use]
    pub fn new() -> Self {
        let frozen_ms = std::env::var("OC2_FRAME_CLOCK_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok());
        Self {
            epoch: std::time::Instant::now(),
            base_ms: 0,
            frozen_ms,
        }
    }

    /// Elapsed milliseconds since the loop epoch, or the frozen checkpoint.
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.frozen_ms.unwrap_or_else(|| {
            self.base_ms.saturating_add(
                self.epoch.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            )
        })
    }

    /// Absolute deadline corresponding to an elapsed-ms wake timestamp.
    #[must_use]
    pub fn deadline(&self, at_ms: u64) -> std::time::Instant {
        self.epoch
            .checked_add(std::time::Duration::from_millis(
                at_ms.saturating_sub(self.base_ms),
            ))
            .unwrap_or_else(std::time::Instant::now)
    }

    /// Freeze at an explicit checkpoint.
    pub fn freeze(&mut self, at_ms: u64) {
        self.frozen_ms = Some(at_ms);
    }

    /// Step the frozen clock by `ms`; a no-op while wall-clock.
    pub fn advance(&mut self, ms: u64) {
        if let Some(frozen) = self.frozen_ms.as_mut() {
            *frozen = frozen.saturating_add(ms);
        }
    }

    /// Resume monotonically from the frozen checkpoint.
    pub fn release(&mut self) {
        if let Some(frozen_ms) = self.frozen_ms.take() {
            self.base_ms = frozen_ms;
            self.epoch = std::time::Instant::now();
        }
    }
}

impl crate::native_modal_layer::Clock for FrameClock {
    fn now_ms(&self) -> u64 {
        self.now_ms()
    }
}

#[cfg(test)]
mod tests {
    use super::FrameClock;

    #[test]
    fn frozen_clock_advances_and_release_remains_monotonic() {
        let mut clock = FrameClock::new();
        clock.freeze(600);
        assert_eq!(clock.now_ms(), 600);
        clock.advance(80);
        assert_eq!(clock.now_ms(), 680);
        clock.release();
        assert!(clock.now_ms() >= 680);
    }

    #[test]
    fn unfrozen_clock_tracks_the_wall_epoch() {
        let clock = FrameClock::new();
        let first = clock.now_ms();
        let epoch = clock.epoch;
        // A freshly constructed unfrozen clock sits at the origin.
        assert_eq!(first, 0);
        // Its deadline maps an elapsed timestamp back onto the epoch, exactly
        // like the previous `animation_start + Duration::from_millis(at)`.
        assert_eq!(clock.deadline(0), epoch);
        assert_eq!(clock.deadline(1_000), epoch + std::time::Duration::from_millis(1_000));
        assert!(std::time::Instant::now() >= epoch);
    }

    #[test]
    fn interrupt_flash_reaches_zero_at_the_220ms_deadline() {
        // `prompt/index.tsx:117-127`: the armed flash decays over 220 ms. With
        // a frozen clock the decay is deterministic (PAR-08).
        let mut clock = FrameClock::new();
        clock.freeze(0);
        assert_eq!(
            crate::native_status_rail::interrupt_flash_level(true, clock.now_ms()),
            0.75
        );
        clock.advance(220);
        assert_eq!(
            crate::native_status_rail::interrupt_flash_level(true, clock.now_ms()),
            0.0
        );
    }
}

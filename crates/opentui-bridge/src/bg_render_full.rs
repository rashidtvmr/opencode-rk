#![forbid(unsafe_code)]
//! Frame tick counter for bg-pulse render loop.
//! Mirrors `bg-pulse-render.ts` frame advance: elapsed loops each `PERIOD`.
//! ponytail: counter only; add when per-cell draw needed.

/// Wrapping frame tick: `0..max`, wraps to `0` at `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTick {
    pub tick: u64,
    pub max: u64,
}

impl RenderTick {
    #[must_use]
    pub const fn new(max: u64) -> Self {
        Self { tick: 0, max }
    }

    pub fn step(&mut self) {
        if self.max == 0 {
            self.tick = 0;
            return;
        }
        self.tick = (self.tick + 1) % self.max;
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        assert_eq!(RenderTick::new(3).tick(), 0);
    }

    #[test]
    fn wraps_at_max() {
        let mut r = RenderTick::new(2);
        r.step();
        assert_eq!(r.tick(), 1);
        r.step();
        assert_eq!(r.tick(), 0);
    }

    #[test]
    fn zero_max_stays_zero() {
        let mut r = RenderTick::new(0);
        r.step();
        assert_eq!(r.tick(), 0);
    }
}

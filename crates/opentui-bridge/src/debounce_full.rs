#![forbid(unsafe_code)]
//! No-timer port of `packages/tui/src/util/signal.ts`.
//! `Debounced`: last-push-wins stage + explicit `fire` (no timers).
//! `Fade`: 160ms smoothstep `p*p*(3-2p)` folded into pure `fade_step`.
//! ponytail: explicit fire/step only, no timer thread. Upgrade: drive from a real clock loop.

/// Last-push-wins staged value; `fire` commits explicitly.
#[derive(Debug, Clone)]
pub struct Debounced<T: Clone> {
    value: T,
    pending: Option<T>,
    delay_ms: u64,
}

impl<T: Clone> Debounced<T> {
    #[must_use]
    pub fn new(value: T, delay_ms: u64) -> Self {
        Self {
            value,
            pending: None,
            delay_ms,
        }
    }
    pub fn push(&mut self, next: T) {
        self.pending = Some(next);
    }
    pub fn fire(&mut self) -> bool {
        match self.pending.take() {
            Some(v) => {
                self.value = v;
                true
            }
            None => false,
        }
    }
    #[must_use]
    pub fn get(&self) -> &T {
        &self.value
    }
    #[must_use]
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }
}

/// Fade-in state; alpha kept as 0..=1000 to stay integer.
#[derive(Debug, Clone)]
pub struct Fade {
    alpha_x1000: u32,
    revealed: bool,
}

impl Fade {
    #[must_use]
    pub fn new() -> Self {
        Self {
            alpha_x1000: 0,
            revealed: false,
        }
    }
    pub fn fade_step(&mut self, show: bool, animate: bool, elapsed_ms: u64) -> u32 {
        if !show {
            self.alpha_x1000 = 0;
            return 0;
        }
        if !animate || self.revealed {
            self.revealed = true;
            self.alpha_x1000 = 1000;
            return 1000;
        }
        self.revealed = true;
        let p = (elapsed_ms as f64 / 160.0).min(1.0);
        let a = (p * p * (3.0 - 2.0 * p) * 1000.0).round() as u32;
        self.alpha_x1000 = a.min(1000);
        self.alpha_x1000
    }
    #[must_use]
    pub fn alpha_x1000(&self) -> u32 {
        self.alpha_x1000
    }
    #[must_use]
    pub fn revealed(&self) -> bool {
        self.revealed
    }
}

impl Default for Fade {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_commits_pending() {
        let mut d = Debounced::new(0, 50);
        d.push(1);
        d.push(2);
        assert!(d.fire());
        assert_eq!(*d.get(), 2);
    }

    #[test]
    fn fire_none_false() {
        let mut d = Debounced::new(7, 50);
        assert!(!d.fire());
        assert_eq!(*d.get(), 7);
        assert!(!d.fire());
    }

    #[test]
    fn fade_hidden_zero() {
        let mut f = Fade::new();
        assert_eq!(f.fade_step(false, true, 999), 0);
        assert_eq!(f.alpha_x1000(), 0);
        assert!(!f.revealed());
    }

    #[test]
    fn fade_instant_no_anim() {
        let mut f = Fade::new();
        assert_eq!(f.fade_step(true, false, 0), 1000);
        assert!(f.revealed());
    }

    #[test]
    fn fade_partial_range() {
        let mut f = Fade::new();
        let a = f.fade_step(true, true, 80);
        assert!((495..=505).contains(&a), "got {a}");
    }

    #[test]
    fn fade_full_then_sticky() {
        let mut f = Fade::new();
        assert_eq!(f.fade_step(true, true, 160), 1000);
        assert_eq!(f.fade_step(true, true, 0), 1000);
        assert_eq!(f.fade_step(false, true, 0), 0);
        assert!(f.revealed());
    }
}

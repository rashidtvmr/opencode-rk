#![forbid(unsafe_code)]
//! Manual debounce state (no upstream equivalent; `util/signal.ts:1-12`).
//!
//! Spec cited `util/signal.ts` (51 lines); actual file is 12 lines exposing
//! only `trigger`/`wait`. No debounce exists upstream, so this is new logic:
//! timer-free state machine; caller drives `flush` after `delay_ms`.
//! Fail-closed: `set` of equal value clears pending; `flush` with no change
//! yields `None`.

/// Timer-free debounced value.
#[derive(Debug, Clone)]
pub struct Debounced<T: Clone + PartialEq> {
    value: T,
    pending: Option<T>,
    delay_ms: u64,
}

impl<T: Clone + PartialEq> Debounced<T> {
    #[must_use]
    pub fn new(initial: T, delay_ms: u64) -> Self {
        Self {
            value: initial,
            pending: None,
            delay_ms,
        }
    }

    /// Stage a candidate; equal-to-current clears pending (fail-closed).
    pub fn set(&mut self, v: T) {
        if v == self.value {
            self.pending = None;
        } else {
            self.pending = Some(v);
        }
    }

    /// Commit staged value if it differs; `None` when nothing changed.
    pub fn flush(&mut self) -> Option<T> {
        match self.pending.take() {
            Some(v) if v != self.value => {
                self.value = v.clone();
                Some(v)
            }
            _ => None,
        }
    }

    /// Drop staged value.
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// Take staged value without committing.
    pub fn take(&mut self) -> Option<T> {
        self.pending.take()
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    #[must_use]
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_flush_commits() {
        let mut d = Debounced::new(0, 100);
        d.set(1);
        assert!(d.is_pending());
        assert_eq!(d.flush(), Some(1));
        assert_eq!(*d.value(), 1);
        assert!(!d.is_pending());
    }

    #[test]
    fn set_equal_clears() {
        let mut d = Debounced::new(0, 100);
        d.set(1);
        d.set(0);
        assert!(!d.is_pending());
        assert_eq!(d.flush(), None);
    }

    #[test]
    fn cancel_and_take() {
        let mut d = Debounced::new(0, 100);
        d.set(5);
        d.cancel();
        assert_eq!(d.flush(), None);
        d.set(7);
        assert_eq!(d.take(), Some(7));
        assert_eq!(*d.value(), 0);
    }
}

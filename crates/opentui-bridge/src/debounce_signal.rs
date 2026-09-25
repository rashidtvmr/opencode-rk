#![forbid(unsafe_code)]
//! Timer-free debounced signal (ports `packages/tui/src/util/signal.ts:3`
//! `createDebouncedSignal`: set-then-fire-after-`delay_ms` semantics).
//!
//! # SOURCE EVIDENCE
//! - TS `util/signal.ts` absent in this checkout (no `packages/` dir); contract
//!   per task card: `createDebouncedSignal` holds a value, `set` restages it,
//!   the staged value fires once `delay_ms` elapses after the last set.
//! - Caller-owned clock: `push` stamps `deadline_ms = now_ms + delay_ms`
//!   (saturating); `take_if_due(now_ms)` fires iff armed and `now_ms >= deadline`.
//!   No timers, no threads, no `std::time` reads. `delay_ms = 0` fires on next poll.
//!
//! # RELATION TO `debounce.rs` (NOT a port of it; do not merge)
//! - `debounce.rs:12-16` `Debounced<T: Clone + PartialEq>` is NEW logic per its
//!   own header (`:2-6`: "no upstream equivalent", "caller drives `flush`");
//!   it commits on explicit `flush` and clears pending on equal-value `set`.
//! - This module adds the missing TIME dimension the audit flagged: a deadline
//!   per push, last-push-wins re-arm, and `take_if_due(now_ms)`. Different type
//!   name would collide, so this file is intentionally standalone; the
//!   integrator picks one or renames on wiring.
//!
//! ponytail: deadline only, no callback/timer thread. Upgrade: wrap with a
//! caller-driven poll loop when a real timer backend exists.

/// Last-push-wins debounced value with a caller-clock deadline.
#[derive(Debug, Clone)]
pub struct Debounced<T: Clone> {
    value: T,
    pending: Option<T>,
    deadline_ms: u64,
    delay_ms: u64,
}

impl<T: Clone> Debounced<T> {
    /// Seed with `value`; nothing armed.
    #[must_use]
    pub fn new(value: T, delay_ms: u64) -> Self {
        Self {
            value,
            pending: None,
            deadline_ms: 0,
            delay_ms,
        }
    }

    /// Stage `value`, (re)arming the deadline to `now_ms + delay_ms`.
    pub fn push(&mut self, value: T, now_ms: u64) {
        self.pending = Some(value);
        self.deadline_ms = now_ms.saturating_add(self.delay_ms);
    }

    /// Take the staged value iff armed and `now_ms >= deadline_ms`.
    /// Commits to `value` on fire; late polls after fire yield `None`.
    pub fn take_if_due(&mut self, now_ms: u64) -> Option<T> {
        if self.pending.is_some() && now_ms >= self.deadline_ms {
            let v = self.pending.take().expect("guarded by is_some");
            self.value = v.clone();
            Some(v)
        } else {
            None
        }
    }

    /// Commit the staged value now, ignoring the deadline (`None` if idle).
    pub fn flush(&mut self) -> Option<T> {
        match self.pending.take() {
            Some(v) => {
                self.value = v.clone();
                Some(v)
            }
            None => None,
        }
    }

    /// Drop the staged value without committing.
    pub fn cancel(&mut self) {
        self.pending = None;
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
    fn no_fire_early() {
        let mut d = Debounced::new(0, 100);
        d.push(1, 1_000);
        assert_eq!(d.take_if_due(1_000), None);
        assert_eq!(d.take_if_due(1_099), None);
        assert!(d.is_pending());
        assert_eq!(*d.value(), 0);
    }

    #[test]
    fn fires_at_deadline() {
        let mut d = Debounced::new(0, 100);
        d.push(1, 1_000);
        assert_eq!(d.take_if_due(1_100), Some(1));
        assert_eq!(*d.value(), 1);
        assert!(!d.is_pending());
        assert_eq!(d.take_if_due(9_999), None);
    }

    #[test]
    fn supersede_rearms_deadline() {
        let mut d = Debounced::new(0, 100);
        d.push(1, 1_000);
        d.push(2, 1_050);
        assert_eq!(d.take_if_due(1_100), None);
        assert_eq!(d.take_if_due(1_150), Some(2));
        assert_eq!(*d.value(), 2);
    }

    #[test]
    fn flush_commits_now_and_empty_is_none() {
        let mut d = Debounced::new(0, 100);
        assert_eq!(d.flush(), None);
        d.push(7, 5_000);
        assert_eq!(d.flush(), Some(7));
        assert_eq!(*d.value(), 7);
        assert_eq!(d.take_if_due(99_999), None);
    }

    #[test]
    fn clone_only_type_no_partial_eq_bound() {
        // Compile-time proof the impl needs only Clone: OnlyClone has no PartialEq.
        #[derive(Debug, Clone)]
        struct OnlyClone(u8);
        let mut d: Debounced<OnlyClone> = Debounced::new(OnlyClone(0), 10);
        d.push(OnlyClone(3), 0);
        assert_eq!(d.take_if_due(10).unwrap().0, 3);
    }
}

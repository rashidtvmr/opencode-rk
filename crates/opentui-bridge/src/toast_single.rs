#![forbid(unsafe_code)]
//! Single-slot toast holder: controller truth (mirrors TS single `currentToast`).
//!
//! TS truth (`toast.tsx:60-67` per lane brief): one `currentToast` slot;
//! `show(opts)` replaces it; timeout clear after `duration ?? 5000`ms.
//! Reuses [`crate::toast::ToastOptions`] / `effective_duration_ms` (which
//! already defaults `None` to [`crate::toast::DEFAULT_DURATION_MS`]); defines
//! no toast data of its own.
//!
//! Divergence note: [`crate::toast::ToastQueue`] is a bounded FIFO (up to
//! `MAX_TOASTS`, drops oldest on overflow). TS has no queue: a new toast
//! discards the previous one. This holder mirrors TS (replace-on-new), not
//! the FIFO. [`crate::toast_view::ToastProvider`] mirrors the same single
//! slot for the view layer; this type is the controller truth and adds the
//! expiry check the provider leaves to the host: the caller passes its own
//! monotonic clock (`now_ms`) so expiry is deterministic and testable with
//! no timers, no `Instant`, std-only.
//!
//! `ponytail:` single struct + saturating expiry math; upgrade path is a
//! host-driven dismiss callback, add only when a real caller needs it.

use crate::toast::{ToastOptions, DEFAULT_DURATION_MS};

/// Single-slot holder; `show` replaces, expiry checked against caller clock.
#[derive(Debug, Default, Clone)]
pub struct ToastSingle {
    current: Option<ToastOptions>,
    shown_at_ms: u64,
}

impl ToastSingle {
    #[must_use]
    pub fn new() -> Self {
        Self { current: None, shown_at_ms: 0 }
    }

    /// Replace current toast, stamp caller clock (TS `show` + timeout reset).
    pub fn show(&mut self, toast: ToastOptions, now_ms: u64) {
        self.current = Some(toast);
        self.shown_at_ms = now_ms;
    }

    /// TS timeout-clear.
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// True when a toast is held and its deadline passed (`now_ms` from
    /// caller clock; deadline `shown_at + effective_duration_ms`, `None`
    /// duration defaults to 5000 via [`ToastOptions::effective_duration_ms`]).
    #[must_use]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        match &self.current {
            None => false,
            Some(t) => now_ms.saturating_sub(self.shown_at_ms) >= t.effective_duration_ms(),
        }
    }

    /// Live toast, or `None` when empty or expired (does not clear state;
    /// call [`Self::clear`] on expiry if the slot must be freed).
    #[must_use]
    pub fn current(&self, now_ms: u64) -> Option<&ToastOptions> {
        if self.is_expired(now_ms) {
            None
        } else {
            self.current.as_ref()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toast::ToastVariant;

    fn info(msg: &str, duration_ms: Option<u64>) -> ToastOptions {
        ToastOptions::new(None, msg, ToastVariant::Info, duration_ms).unwrap()
    }

    #[test]
    fn replace_on_new() {
        let mut s = ToastSingle::new();
        s.show(info("first", None), 0);
        s.show(info("second", None), 100);
        assert_eq!(s.current(100).unwrap().message, "second");
    }

    #[test]
    fn clear_empties_slot() {
        let mut s = ToastSingle::new();
        s.show(info("hi", None), 0);
        s.clear();
        assert!(s.current(0).is_none());
        assert!(!s.is_expired(0));
    }

    #[test]
    fn expiry_via_caller_clock() {
        let mut s = ToastSingle::new();
        s.show(info("brief", Some(100)), 0);
        assert!(s.current(50).is_some());
        assert!(!s.is_expired(50));
        assert!(s.current(100).is_none());
        assert!(s.is_expired(101));
    }

    #[test]
    fn default_duration_is_5000() {
        assert_eq!(DEFAULT_DURATION_MS, 5000);
        let mut s = ToastSingle::new();
        s.show(info("d", None), 0);
        assert_eq!(s.current(0).unwrap().effective_duration_ms(), 5000);
        assert!(s.current(4_999).is_some());
        assert!(s.current(5_000).is_none());
    }
}

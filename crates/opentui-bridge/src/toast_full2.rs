#![forbid(unsafe_code)]
//! Toast flow: `ToastCenter` plus shown counter.
//!
//! `ponytail:` no dismiss passthrough; add when a caller needs it.

use crate::toast_center::ToastCenter;
use crate::toast_line::{toast_line, toast_push_show};

/// Center plus total `show` count.
#[derive(Debug, Default, Clone)]
pub struct ToastFlow {
    pub center: ToastCenter,
    pub shown: u32,
}

impl ToastFlow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            center: ToastCenter::new(),
            shown: 0,
        }
    }

    /// Delegate to center, bump counter (saturating).
    pub fn show(&mut self, msg: &str) {
        toast_push_show(&mut self.center, msg);
        self.shown = self.shown.saturating_add(1);
    }

    /// Visible line clipped to `width`; `None` when empty.
    #[must_use]
    pub fn line(&self, width: usize) -> Option<String> {
        toast_line(&self.center, width)
    }

    /// Total `show` calls.
    #[must_use]
    pub fn shown(&self) -> u32 {
        self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let f = ToastFlow::new();
        assert_eq!(f.shown(), 0);
        assert_eq!(f.line(8), None);
    }

    #[test]
    fn show_bumps_counter() {
        let mut f = ToastFlow::new();
        f.show("a");
        f.show("b");
        assert_eq!(f.shown(), 2);
        assert_eq!(f.shown, 2);
    }

    #[test]
    fn line_delegates_to_center() {
        let mut f = ToastFlow::new();
        f.show("abcdef");
        assert_eq!(f.line(3), Some("abc".to_string()));
    }

    #[test]
    fn line_none_when_empty() {
        let f = ToastFlow::default();
        assert_eq!(f.line(10), None);
    }

    #[test]
    fn counts_queued_shows() {
        let mut f = ToastFlow::new();
        for i in 0..10 {
            f.show(&format!("m{i}"));
        }
        assert_eq!(f.shown(), 10);
        assert_eq!(f.line(99), Some("m0".to_string()));
    }
}

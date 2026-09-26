#![forbid(unsafe_code)]
//! One-line toast view: clip visible message to width, char-safe.
//!
//! TS truth (`toast_center.rs`): single `current` slot via
//! `ToastCenter::current`. This view never mutates the center.
//!
//! `ponytail:` char-count width, no ellipsis/padding; add when caller needs it.

use crate::toast_center::ToastCenter;

/// Visible toast clipped to `width` chars; `None` when empty.
#[must_use]
pub fn toast_line(center: &ToastCenter, width: usize) -> Option<String> {
    center
        .current()
        .map(|msg| msg.chars().take(width).collect())
}

/// Push a message (delegates to `ToastCenter::show`).
pub fn toast_push_show(center: &mut ToastCenter, msg: &str) {
    center.show(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_empty() {
        assert_eq!(toast_line(&ToastCenter::new(), 10), None);
    }

    #[test]
    fn full_when_fits() {
        let mut c = ToastCenter::new();
        c.show("hi");
        assert_eq!(toast_line(&c, 10), Some("hi".to_string()));
    }

    #[test]
    fn clips_to_width() {
        let mut c = ToastCenter::new();
        c.show("abcdef");
        assert_eq!(toast_line(&c, 3), Some("abc".to_string()));
    }

    #[test]
    fn clip_is_char_safe() {
        let mut c = ToastCenter::new();
        c.show("e\u{301}clair");
        assert_eq!(toast_line(&c, 2).unwrap().chars().count(), 2);
    }

    #[test]
    fn push_show_delegates() {
        let mut c = ToastCenter::new();
        toast_push_show(&mut c, "a");
        assert_eq!(c.current(), Some("a"));
        toast_push_show(&mut c, "b");
        assert_eq!(toast_line(&c, 8), Some("a".to_string()));
    }
}

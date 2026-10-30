#![forbid(unsafe_code)]
//! Bounded, production-independent vertical tab rail resize state.
//!
//! This mirrors OpenCode's `createPaneResize`: a left-button press owns the
//! gesture, drag clamps the preferred width, release commits only the final
//! value, and a second boundary click restores the default width.

pub const MIN_WIDTH: u16 = 5;
pub const MAX_WIDTH: u16 = 72;
pub const CONTENT_MIN_WIDTH: u16 = 44;

#[must_use]
pub fn clamp_width(width: u16, total: u16) -> u16 {
    width.max(MIN_WIDTH).min(MAX_WIDTH).min(total.saturating_sub(CONTENT_MIN_WIDTH).max(MIN_WIDTH))
}

#[must_use]
pub fn fits_vertical(total: u16, width: u16) -> bool {
    u32::from(total) >= u32::from(width) + 64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeState {
    preferred: u16,
    default: u16,
    pressed: bool,
    moved: bool,
    last_click: Option<u64>,
}

impl ResizeState {
    #[must_use]
    pub fn new(value: u16, default: u16, total: u16) -> Self {
        let _ = total;
        Self { preferred: value.clamp(MIN_WIDTH, MAX_WIDTH), default: default.clamp(MIN_WIDTH, MAX_WIDTH), pressed: false, moved: false, last_click: None }
    }

    #[must_use]
    pub const fn preferred(self) -> u16 { self.preferred }

    #[must_use]
    pub const fn is_resizing(self) -> bool { self.pressed }

    #[must_use]
    pub fn size(self, total: u16) -> u16 { clamp_width(self.preferred, total) }

    /// Boundary contains the one-cell handle at `width - 1 ..= width`.
    #[must_use]
    pub fn contains(x: u16, width: u16) -> bool { x >= width.saturating_sub(1) && x <= width }

    /// Returns true when the boundary consumes this press. A double-click
    /// commits a reset immediately and leaves `is_resizing()` false.
    pub fn press(&mut self, now_ms: u64, x: u16, total: u16) -> bool {
        let width = self.size(total);
        if !Self::contains(x, width) { return false; }
        if self.last_click.is_some_and(|last| now_ms.saturating_sub(last) < 300) {
            self.last_click = None;
            self.pressed = false;
            self.moved = false;
            self.preferred = self.default;
            return true;
        }
        self.pressed = true;
        self.moved = false;
        true
    }

    pub fn drag(&mut self, x: u16, total: u16) -> Option<u16> {
        if !self.pressed { return None; }
        self.moved = true;
        self.last_click = None;
        self.preferred = clamp_width(x.saturating_add(1), total);
        Some(self.preferred)
    }

    pub fn release(&mut self, now_ms: u64, x: u16, total: u16) -> Option<u16> {
        if !self.pressed { return None; }
        let value = if self.moved { clamp_width(x.saturating_add(1), total) } else { self.size(total) };
        self.preferred = value;
        self.pressed = false;
        self.last_click = (!self.moved).then_some(now_ms);
        self.moved = false;
        Some(value)
    }

    pub fn cancel(&mut self) { self.pressed = false; self.moved = false; self.last_click = None; }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn clamp_and_fit_match_upstream_layout() {
        assert_eq!(clamp_width(0, 120), 5);
        assert_eq!(clamp_width(100, 120), 72);
        assert_eq!(clamp_width(42, 54), 10);
        assert!(fits_vertical(69, 5));
        assert!(!fits_vertical(68, 5));
        assert!(fits_vertical(106, 42));
    }
    #[test] fn drag_release_cancel_and_double_click_are_owned() {
        let mut state = ResizeState::new(42, 42, 120);
        assert!(state.press(1000, 41, 120));
        assert_eq!(state.drag(4, 120), Some(5));
        assert_eq!(state.release(1100, 4, 120), Some(5));
        assert_eq!(state.preferred(), 5);
        assert!(state.press(2000, 4, 120));
        state.cancel();
        assert_eq!(state.release(2050, 4, 120), None);
        assert!(state.press(3000, 4, 120));
        assert_eq!(state.release(3050, 4, 120), Some(5));
        assert!(state.press(3100, 4, 120));
        assert_eq!(state.preferred(), 42);
    }
}

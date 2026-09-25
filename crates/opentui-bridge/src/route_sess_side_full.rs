#![forbid(unsafe_code)]
//! Session sidebar open/pinned state (mirrors `routes/session/sidebar.tsx`).
/// Session sidebar state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessSide {
    pub open: bool,
    pub pinned: u32,
}

impl SessSide {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open: false,
            pinned: 0,
        }
    }

    /// Flip open flag.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Bump pin count (saturating).
    pub fn pin(&mut self) {
        self.pinned = self.pinned.saturating_add(1);
    }

    /// Report open flag.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips() {
        let mut s = SessSide::new();
        assert!(!s.is_open());
        s.toggle();
        assert!(s.is_open());
        s.toggle();
        assert!(!s.is_open());
    }

    #[test]
    fn pin_bumps() {
        let mut s = SessSide::new();
        assert_eq!(s.pinned, 0);
        s.pin();
        s.pin();
        assert_eq!(s.pinned, 2);
    }

    #[test]
    fn pin_saturates() {
        let mut s = SessSide {
            open: true,
            pinned: u32::MAX,
        };
        s.pin();
        assert_eq!(s.pinned, u32::MAX);
        assert!(s.is_open());
    }
}

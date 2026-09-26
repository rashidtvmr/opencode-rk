#![forbid(unsafe_code)]
//! Theme flow pairing [`ThemePicker`] with an apply counter.
//!
//! ponytail: index-only cycling from picker; persistence later.

use crate::theme_picker::ThemePicker;

/// Picker plus count of acked applies.
pub struct ThemeFlow {
    pub picker: ThemePicker,
    pub applied: u32,
}

impl ThemeFlow {
    pub fn new() -> Self {
        Self {
            picker: ThemePicker::new(),
            applied: 0,
        }
    }
    pub fn next(&mut self) -> &str {
        self.picker.next()
    }
    pub fn apply(&mut self, name: &str) -> bool {
        let ok = self.picker.apply_known(name);
        if ok {
            self.applied = self.applied.saturating_add(1);
        }
        ok
    }
    pub fn applied(&self) -> u32 {
        self.applied
    }
}

impl Default for ThemeFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_zero() {
        let f = ThemeFlow::new();
        assert_eq!(f.applied(), 0);
        assert_eq!(f.applied, 0);
    }
    #[test]
    fn apply_known_bumps() {
        let mut f = ThemeFlow::new();
        assert!(f.apply("dracula"));
        assert_eq!(f.applied(), 1);
        assert_eq!(f.picker.current(), "dracula");
    }
    #[test]
    fn apply_unknown_no_bump() {
        let mut f = ThemeFlow::new();
        assert!(!f.apply("nope"));
        assert_eq!(f.applied(), 0);
    }
    #[test]
    fn next_delegates() {
        let mut f = ThemeFlow::new();
        let want = f.picker.current().to_string();
        assert!(!want.is_empty());
        let _ = f.next();
        assert_eq!(f.applied(), 0);
    }
    #[test]
    fn multi_apply_counts() {
        let mut f = ThemeFlow::default();
        assert!(f.apply("dracula"));
        assert!(f.apply("opencode"));
        assert_eq!(f.applied(), 2);
    }
}

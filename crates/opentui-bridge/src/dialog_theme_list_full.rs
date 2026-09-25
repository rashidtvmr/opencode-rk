#![forbid(unsafe_code)]
//! Theme list dialog over [`ThemePicker`].
//!
//! Mirrors `packages/tui/src/component/dialog-theme-list.tsx:6`
//! `DialogThemeList` (sorted options, live preview on move, confirm on
//! select, revert on cancel). `cursor` mirrors the picker index.

use crate::theme_picker::ThemePicker;

/// Cursor dialog wrapping [`ThemePicker`].
pub struct ThemeListDialog {
    pub cursor: usize,
    pub picker: ThemePicker,
}

impl ThemeListDialog {
    pub fn new() -> Self {
        let picker = ThemePicker::new();
        Self {
            cursor: picker.index,
            picker,
        }
    }

    pub fn with_picker(picker: ThemePicker) -> Self {
        Self {
            cursor: picker.index,
            picker,
        }
    }

    /// Step cursor; positive calls `next`, negative calls `prev`.
    pub fn move_cursor(&mut self, delta: isize) {
        if delta > 0 {
            for _ in 0..delta {
                self.picker.next();
            }
        } else {
            for _ in 0..-delta {
                self.picker.prev();
            }
        }
        self.cursor = self.picker.index;
    }

    pub fn current(&self) -> &str {
        self.picker.current()
    }

    /// Re-apply previewed theme (confirm). False when locked.
    pub fn apply_current(&mut self) -> bool {
        let name = self.picker.current().to_string();
        let ok = self.picker.apply_known(&name);
        self.cursor = self.picker.index;
        ok
    }
}

impl Default for ThemeListDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cursor_matches_picker() {
        let d = ThemeListDialog::new();
        assert_eq!(d.cursor, d.picker.index);
        assert_eq!(d.current(), d.picker.current());
    }

    #[test]
    fn move_forward_steps_next() {
        let mut d = ThemeListDialog::new();
        let first = d.current().to_string();
        d.move_cursor(1);
        assert_eq!(d.cursor, d.picker.index);
        assert_ne!(d.current(), first);
    }

    #[test]
    fn move_backward_wraps() {
        let mut d = ThemeListDialog::new();
        d.move_cursor(-1);
        assert_eq!(d.cursor, d.picker.index);
        d.move_cursor(1);
        assert_eq!(d.current(), ThemePicker::new().current());
    }

    #[test]
    fn apply_current_confirms() {
        let mut d = ThemeListDialog::new();
        d.move_cursor(2);
        assert!(d.apply_current());
        assert_eq!(d.current(), d.picker.engine.name());
    }

    #[test]
    fn locked_apply_fails() {
        let mut d = ThemeListDialog::new();
        d.picker.engine.toggle_lock();
        assert!(!d.apply_current());
    }
}

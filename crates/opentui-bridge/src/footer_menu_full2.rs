#![forbid(unsafe_code)]
//! Open-counting wrapper over `FooterMenuFull`.

use crate::footer_menu_full::FooterMenuFull;

/// Menu plus successful-open counter.
#[derive(Debug, Default)]
pub struct MenuFlow {
    pub menu: FooterMenuFull,
    opened: u32,
}

impl MenuFlow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) -> bool {
        if self.menu.open_menu() {
            self.opened += 1;
            true
        } else {
            false
        }
    }

    pub fn close(&mut self) {
        self.menu.close_menu();
    }

    pub fn select_next(&mut self) -> bool {
        if !self.menu.is_open() {
            return false;
        }
        self.menu.move_cursor(1);
        true
    }

    pub fn opened_count(&self) -> u32 {
        self.opened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow2() -> MenuFlow {
        let mut f = MenuFlow::new();
        f.menu.add_item("a");
        f.menu.add_item("b");
        f
    }

    #[test]
    fn open_empty_no_count() {
        let mut f = MenuFlow::new();
        assert!(!f.open());
        assert_eq!(f.opened_count(), 0);
    }

    #[test]
    fn open_counts_success() {
        let mut f = flow2();
        assert!(f.open());
        assert!(f.open());
        assert_eq!(f.opened_count(), 2);
    }

    #[test]
    fn close_keeps_count() {
        let mut f = flow2();
        assert!(f.open());
        f.close();
        assert!(!f.menu.is_open());
        assert_eq!(f.opened_count(), 1);
    }

    #[test]
    fn select_next_gates_and_wraps() {
        let mut f = flow2();
        assert!(!f.select_next());
        assert!(f.open());
        assert!(f.select_next());
        assert_eq!(f.menu.cursor(), 1);
        assert!(f.select_next());
        assert_eq!(f.menu.cursor(), 0);
    }
}

#![forbid(unsafe_code)]
//! Bounded collapsible item list: collapsed shows first + count, open shows all.

/// Collapsible list, max 64 items of 512 chars each (char-safe truncation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollapseFull {
    pub collapsed: bool,
    pub items: Vec<String>,
}

impl CollapseFull {
    const MAX_ITEMS: usize = 64;
    const MAX_CHARS: usize = 512;

    #[must_use]
    pub fn new(collapsed: bool) -> Self {
        Self {
            collapsed,
            items: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }

    pub fn push(&mut self, item: &str) -> bool {
        if self.items.len() >= Self::MAX_ITEMS {
            return false;
        }
        self.items
            .push(item.chars().take(Self::MAX_CHARS).collect());
        true
    }

    #[must_use]
    pub fn visible(&self) -> Vec<String> {
        if !self.collapsed {
            return self.items.clone();
        }
        match self.items.as_slice() {
            [] => Vec::new(),
            [only] => vec![only.clone()],
            [first, rest @ ..] => vec![first.clone(), format!("... {} more ...", rest.len())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_shows_all() {
        let mut c = CollapseFull::new(false);
        c.push("a");
        c.push("b");
        assert_eq!(c.visible(), vec!["a", "b"]);
    }

    #[test]
    fn collapsed_shows_first_plus_count() {
        let mut c = CollapseFull::new(true);
        for s in ["a", "b", "c"] {
            c.push(s);
        }
        assert_eq!(c.visible(), vec!["a", "... 2 more ..."]);
    }

    #[test]
    fn collapsed_single_no_marker() {
        let mut c = CollapseFull::new(true);
        c.push("only");
        assert_eq!(c.visible(), vec!["only"]);
    }

    #[test]
    fn collapsed_empty() {
        assert!(CollapseFull::new(true).visible().is_empty());
    }

    #[test]
    fn push_caps_at_64() {
        let mut c = CollapseFull::new(false);
        for i in 0..64 {
            assert!(c.push(&i.to_string()));
        }
        assert!(!c.push("overflow"));
        assert_eq!(c.items.len(), 64);
    }

    #[test]
    fn push_truncates_to_512_chars() {
        let mut c = CollapseFull::new(false);
        assert!(c.push(&"😀".repeat(600)));
        assert_eq!(c.items[0].chars().count(), 512);
    }

    #[test]
    fn toggle_flips() {
        let mut c = CollapseFull::new(true);
        c.push("a");
        c.push("b");
        c.toggle();
        assert_eq!(c.visible(), vec!["a", "b"]);
        c.toggle();
        assert_eq!(c.visible(), vec!["a", "... 1 more ..."]);
    }
}

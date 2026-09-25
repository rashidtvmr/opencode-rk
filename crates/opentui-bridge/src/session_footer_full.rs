#![forbid(unsafe_code)]
//! Full session footer line (extends `session_footer::SessionFooter`).
//!
//! TS truth `packages/tui/src/routes/session/footer.tsx` renders a
//! directory slot plus status items; this type owns the joined line.

/// Full footer line: named slot plus bounded status items.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionFooterFull {
    /// Active slot name, capped at 32 chars.
    pub slot: String,
    /// Status items, capped at 16 entries of 128 chars each.
    pub items: Vec<String>,
    /// Busy indicator appended on render.
    pub busy: bool,
}

impl SessionFooterFull {
    #[must_use]
    pub fn new(slot: &str, busy: bool) -> Self {
        let mut f = Self {
            slot: String::new(),
            items: Vec::new(),
            busy,
        };
        f.set_slot(slot);
        f
    }

    /// Set slot name, truncated to 32 chars.
    pub fn set_slot(&mut self, slot: &str) {
        self.slot = slot.chars().take(32).collect();
    }

    /// Push item truncated to 128 chars; false when full (16).
    pub fn add_item(&mut self, item: &str) -> bool {
        if self.items.len() >= 16 {
            return false;
        }
        self.items.push(item.chars().take(128).collect());
        true
    }

    /// Render `"slot: item1 | item2"` plus `" (busy)"`, capped at 512 chars.
    #[must_use]
    pub fn render(&self) -> String {
        let mut s = if self.items.is_empty() {
            self.slot.clone()
        } else {
            format!("{}: {}", self.slot, self.items.join(" | "))
        };
        if self.busy {
            s.push_str(" (busy)");
        }
        s.chars().take(512).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_truncates_to_32() {
        let mut f = SessionFooterFull::default();
        f.set_slot(&"s".repeat(40));
        assert_eq!(f.slot.chars().count(), 32);
    }

    #[test]
    fn add_item_caps_at_16() {
        let mut f = SessionFooterFull::default();
        for i in 0..16 {
            assert!(f.add_item(&format!("i{i}")));
        }
        assert!(!f.add_item("overflow"));
        assert_eq!(f.items.len(), 16);
    }

    #[test]
    fn item_truncates_to_128() {
        let mut f = SessionFooterFull::default();
        assert!(f.add_item(&"x".repeat(200)));
        assert_eq!(f.items[0].chars().count(), 128);
    }

    #[test]
    fn render_joins_parts() {
        let mut f = SessionFooterFull::new("dir", false);
        f.add_item("1 LSP");
        f.add_item("2 MCP");
        assert_eq!(f.render(), "dir: 1 LSP | 2 MCP");
    }

    #[test]
    fn render_marks_busy() {
        let busy = SessionFooterFull::new("dir", true).render();
        assert!(busy.contains("(busy)"));
        let idle = SessionFooterFull::new("dir", false).render();
        assert!(!idle.contains("(busy)"));
    }

    #[test]
    fn render_empty_items_is_slot() {
        assert_eq!(SessionFooterFull::new("dir", false).render(), "dir");
    }
}

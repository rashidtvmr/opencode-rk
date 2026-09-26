#![forbid(unsafe_code)]
//! Run footer full state (direct interactive mode).
//!
//! TS truth `packages/opencode/src/cli/cmd/run/footer.ts` (`RunFooter`)
//! owns the mutable footer control surface; sibling
//! `crate::session_footer_full::SessionFooterFull` owns the joined
//! slot line. This type owns the run-mode line: mode slot plus
//! bounded status items plus busy flag.

/// Full run footer: mode slot plus bounded items plus busy flag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FooterFull {
    /// Active mode name, capped at 32 chars.
    pub mode: String,
    /// Busy indicator.
    pub busy: bool,
    /// Status items, capped at 16 entries of 128 chars each.
    pub items: Vec<String>,
}

impl FooterFull {
    #[must_use]
    pub fn new(mode: &str, busy: bool) -> Self {
        let mut f = Self {
            mode: String::new(),
            items: Vec::new(),
            busy,
        };
        f.set_mode(mode);
        f
    }

    /// Set mode name, truncated to 32 chars. Always true.
    pub fn set_mode(&mut self, mode: &str) -> bool {
        self.mode = mode.chars().take(32).collect();
        true
    }

    /// Set busy flag.
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// Push item truncated to 128 chars; false when full (16).
    pub fn push_item(&mut self, item: &str) -> bool {
        if self.items.len() >= 16 {
            return false;
        }
        self.items.push(item.chars().take(128).collect());
        true
    }

    /// Render `"<mode> (busy)? <n> items"`, capped at 256 chars.
    #[must_use]
    pub fn summary(&self) -> String {
        let s = if self.busy {
            format!("{} (busy) {} items", self.mode, self.items.len())
        } else {
            format!("{} {} items", self.mode, self.items.len())
        };
        s.chars().take(256).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_truncates_to_32() {
        let mut f = FooterFull::default();
        assert!(f.set_mode(&"m".repeat(40)));
        assert_eq!(f.mode.chars().count(), 32);
    }

    #[test]
    fn push_item_caps_at_16() {
        let mut f = FooterFull::default();
        for i in 0..16 {
            assert!(f.push_item(&format!("i{i}")));
        }
        assert!(!f.push_item("overflow"));
        assert_eq!(f.items.len(), 16);
    }

    #[test]
    fn push_item_truncates_to_128() {
        let mut f = FooterFull::default();
        assert!(f.push_item(&"x".repeat(200)));
        assert_eq!(f.items[0].chars().count(), 128);
    }

    #[test]
    fn set_busy_toggles_marker() {
        let mut f = FooterFull::new("run", false);
        assert!(!f.summary().contains("(busy)"));
        f.set_busy(true);
        assert!(f.summary().contains("(busy)"));
    }

    #[test]
    fn summary_has_mode_and_count() {
        let mut f = FooterFull::new("run", false);
        f.push_item("a");
        f.push_item("b");
        let s = f.summary();
        assert!(s.contains("run"));
        assert!(s.contains("2 items"));
    }

    #[test]
    fn summary_caps_at_256() {
        let f = FooterFull::new(&"m".repeat(300), true);
        assert!(f.summary().chars().count() <= 256);
    }
}

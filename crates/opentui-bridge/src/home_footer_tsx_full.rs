#![forbid(unsafe_code)]
//! `HomeFoot`: tsx-level footer text wrapper over `crate::home_footer`.
//! Source: `feature-plugins/home/footer.tsx:1-40` Directory/Mcp/version line.
//! ponytail: single capped string only; upgrade: structured segments when needed.

/// Max chars retained.
pub const MAX_FOOT_LEN: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomeFoot {
    text: String,
}

impl HomeFoot {
    /// Set text, truncating to [`MAX_FOOT_LEN`] chars.
    pub fn set(&mut self, s: &str) {
        self.text = s.chars().take(MAX_FOOT_LEN).collect();
    }

    /// Current text.
    #[must_use]
    pub fn text_of(&self) -> &str {
        &self.text
    }

    /// Clear text.
    pub fn clear(&mut self) {
        self.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut f = HomeFoot::default();
        f.set("hi");
        assert_eq!(f.text_of(), "hi");
    }

    #[test]
    fn caps_256() {
        let mut f = HomeFoot::default();
        f.set(&"x".repeat(300));
        assert_eq!(f.text_of().chars().count(), MAX_FOOT_LEN);
    }

    #[test]
    fn clear_empties() {
        let mut f = HomeFoot::default();
        f.set("hi");
        f.clear();
        assert_eq!(f.text_of(), "");
    }
}

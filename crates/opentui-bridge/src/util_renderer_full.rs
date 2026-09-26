#![forbid(unsafe_code)]
//! Renderer tag: bounded title mirror (`util/renderer.ts` `destroyRenderer`).

/// Max tag chars.
pub const MAX_TAG_LEN: usize = 64;

/// Bounded renderer title tag; empty = unset/destroyed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RendererTag {
    name: String,
}

impl RendererTag {
    /// Empty tag.
    pub fn new() -> Self {
        Self::default()
    }
    /// Set tag, truncating to [`MAX_TAG_LEN`] chars.
    pub fn set(&mut self, name: &str) {
        self.name = name.chars().take(MAX_TAG_LEN).collect();
    }
    /// Current tag (`""` when unset).
    pub fn name_of(&self) -> &str {
        &self.name
    }
    /// False when empty (mirrors cleared title on destroy).
    pub fn is_set(&self) -> bool {
        !self.name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_unset() {
        let t = RendererTag::new();
        assert!(!t.is_set());
        assert_eq!(t.name_of(), "");
    }

    #[test]
    fn set_roundtrip() {
        let mut t = RendererTag::new();
        t.set("term");
        assert!(t.is_set());
        assert_eq!(t.name_of(), "term");
    }

    #[test]
    fn caps_at_64_chars() {
        let mut t = RendererTag::new();
        t.set(&"x".repeat(100));
        assert_eq!(t.name_of().chars().count(), MAX_TAG_LEN);
    }
}

#![forbid(unsafe_code)]
//! Plugin system list (full).
//!
//! TS truth: `packages/tui/src/feature-plugins/system/plugins.tsx`
//! (plugin manager dialog lists status/meta per plugin, clipped to width).
//! ponytail: no enable/disable state; add `Vec<bool>` when needed.

/// Bounded plugin name list: 16 items, each 128 chars.
pub struct PluginSystem {
    items: Vec<String>,
}

impl PluginSystem {
    /// Empty list.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Push name; false when full (16).
    pub fn push(&mut self, name: &str) -> bool {
        if self.items.len() >= 16 {
            return false;
        }
        let clipped: String = name.chars().take(128).collect();
        self.items.push(clipped);
        true
    }

    /// Item count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Names clipped to `width` chars, at most 18 lines.
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        self.items
            .iter()
            .take(18)
            .map(|s| s.chars().take(width).collect())
            .collect()
    }
}

impl Default for PluginSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_len() {
        let mut p = PluginSystem::new();
        assert!(p.is_empty());
        assert!(p.push("a"));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn rejects_past_16() {
        let mut p = PluginSystem::new();
        for i in 0..16 {
            assert!(p.push(&format!("p{i}")));
        }
        assert!(!p.push("extra"));
        assert_eq!(p.len(), 16);
    }

    #[test]
    fn clips_item_to_128() {
        let mut p = PluginSystem::new();
        let long = "x".repeat(200);
        assert!(p.push(&long));
        assert_eq!(p.lines(500)[0].chars().count(), 128);
    }

    #[test]
    fn lines_clip_width_and_cap() {
        let mut p = PluginSystem::new();
        p.push("abcdef");
        let out = p.lines(3);
        assert_eq!(out, vec!["abc".to_string()]);
    }

    #[test]
    fn default_empty() {
        let p = PluginSystem::default();
        assert!(p.is_empty());
        assert!(p.lines(10).is_empty());
    }
}

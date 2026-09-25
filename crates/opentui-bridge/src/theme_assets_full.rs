#![forbid(unsafe_code)]
//! Custom theme catalog over bundled assets.
//! ponytail: Vec only, no persistence. Upgrade when registration persists.

use crate::theme_assets::{fallback_chain, is_known, ASSET_NAMES};

/// Known assets plus up to 16 custom names (each max 64 bytes).
pub struct ThemeCatalog {
    pub custom: Vec<String>,
}

impl ThemeCatalog {
    pub fn new() -> Self {
        Self { custom: Vec::new() }
    }
    pub fn register(&mut self, name: &str) -> bool {
        if name.is_empty() || name.len() > 64 {
            return false;
        }
        if is_known(name) {
            return false;
        }
        if self.custom.iter().any(|c| c == name) || self.custom.len() >= 16 {
            return false;
        }
        self.custom.push(name.to_string());
        true
    }
    pub fn resolve(&self, name: &str) -> String {
        if self.custom.iter().any(|c| c == name) {
            return name.to_string();
        }
        fallback_chain(name)[0].to_string()
    }
    pub fn list(&self) -> Vec<String> {
        let mut out: Vec<String> = ASSET_NAMES.iter().take(8).map(|s| s.to_string()).collect();
        out.extend(self.custom.iter().cloned());
        out
    }
}

impl Default for ThemeCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_ok_and_resolve() {
        let mut c = ThemeCatalog::new();
        assert!(c.register("my-theme"));
        assert_eq!(c.resolve("my-theme"), "my-theme");
    }
    #[test]
    fn register_rejects_known() {
        let mut c = ThemeCatalog::new();
        assert!(!c.register("dracula"));
        assert!(!c.register("opencode"));
    }
    #[test]
    fn register_rejects_empty_long() {
        let mut c = ThemeCatalog::new();
        assert!(!c.register(""));
        assert!(!c.register(&"x".repeat(65)));
        assert!(c.register(&"x".repeat(64)));
    }
    #[test]
    fn register_rejects_dup_and_caps() {
        let mut c = ThemeCatalog::new();
        assert!(c.register("a"));
        assert!(!c.register("a"));
        for i in 0..15 {
            assert!(c.register(&format!("t{i}")));
        }
        assert_eq!(c.custom.len(), 16);
        assert!(!c.register("overflow"));
    }
    #[test]
    fn resolve_known_and_fallback() {
        let c = ThemeCatalog::new();
        assert_eq!(c.resolve("dracula"), "dracula");
        assert_eq!(c.resolve("nope"), "opencode");
        assert_eq!(c.resolve(""), "opencode");
    }
    #[test]
    fn list_known8_plus_custom() {
        let mut c = ThemeCatalog::new();
        assert!(c.register("mine"));
        let l = c.list();
        assert_eq!(l.len(), 9);
        assert_eq!(l[8], "mine");
        let want = [
            "aura",
            "ayu",
            "carbonfox",
            "catppuccin",
            "catppuccin-frappe",
            "catppuccin-macchiato",
            "cobalt2",
            "cursor",
        ];
        assert_eq!(&l[..8], &want);
    }
}

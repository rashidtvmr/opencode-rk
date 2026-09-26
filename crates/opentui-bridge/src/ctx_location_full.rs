#![forbid(unsafe_code)]
//! Location context (mirrors `packages/tui/src/context/location.tsx`).
//!
//! Evidence: `location.tsx:1-14` `LocationRef` accessor + provider.
/// Max path chars (fail-closed).
pub const MAX_PATH: usize = 512;
/// Max file base chars.
pub const MAX_FILE: usize = 128;

/// Bounded location context: current path only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CtxLoc {
    path: String,
}

impl CtxLoc {
    /// Set path; false on empty/over-cap (keeps old).
    pub fn set(&mut self, path: &str) -> bool {
        if path.is_empty() || path.chars().count() > MAX_PATH {
            return false;
        }
        self.path.clear();
        self.path.push_str(path);
        true
    }

    #[must_use]
    pub fn path_of(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn file_of(&self) -> String {
        self.path
            .split(['/', '\\'])
            .next_back()
            .unwrap_or("")
            .chars()
            .take(MAX_FILE)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_ok() {
        let mut c = CtxLoc::default();
        assert!(c.set("/tmp/a.txt"));
        assert_eq!(c.path_of(), "/tmp/a.txt");
        assert_eq!(c.file_of(), "a.txt");
    }

    #[test]
    fn set_rejects() {
        let mut c = CtxLoc::default();
        assert!(!c.set(""));
        assert!(!c.set(&"x".repeat(MAX_PATH + 1)));
        assert_eq!(c.path_of(), "");
    }

    #[test]
    fn file_caps() {
        let mut c = CtxLoc::default();
        let base = "y".repeat(MAX_FILE + 10);
        assert!(c.set(&format!("/a/{base}")));
        assert_eq!(c.file_of().chars().count(), MAX_FILE);
        let mut w = CtxLoc::default();
        assert!(w.set("C:\\a\\b.rs"));
        assert_eq!(w.file_of(), "b.rs");
    }
}

#![forbid(unsafe_code)]
//! Location context (std-only).
//!
//! Evidence (TS `packages/tui/src/context/location.tsx:1-14`):
//! - `LocationRef` from `@opencode-ai/sdk/v2` via Solid `createContext`
//! - `LocationProvider` wraps `props.location`; `useLocation` throws outside
//!   provider (fail-closed, no default).
//! - Rust mirror stores resolved `path` + `exists` probe; empty path rejected.

/// Max path chars.
pub const MAX_PATH: usize = 1024;
/// Max label chars (`"loc " + path + suffix`).
pub const MAX_LABEL: usize = 1100;

/// Location context snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationCtx {
    pub path: String,
    pub exists: bool,
}

impl LocationCtx {
    #[must_use]
    pub fn new() -> Self {
        Self {
            path: String::new(),
            exists: false,
        }
    }

    /// Set path + probe flag. Empty rejects (`false`); longer truncates.
    pub fn set(&mut self, path: &str, exists: bool) -> bool {
        if path.is_empty() {
            return false;
        }
        self.path = path.chars().take(MAX_PATH).collect();
        self.exists = exists;
        true
    }

    /// Reset to empty/missing.
    pub fn clear(&mut self) {
        self.path.clear();
        self.exists = false;
    }

    /// `"loc <path> ok|missing"`, truncated to [`MAX_LABEL`] chars.
    #[must_use]
    pub fn label(&self) -> String {
        let s = format!(
            "loc {} {}",
            self.path,
            if self.exists { "ok" } else { "missing" }
        );
        s.chars().take(MAX_LABEL).collect()
    }
}

impl Default for LocationCtx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_rejected() {
        let mut c = LocationCtx::new();
        assert!(!c.set("", true));
        assert_eq!(c.path, "");
        assert!(!c.exists);
    }

    #[test]
    fn set_ok_stores_both() {
        let mut c = LocationCtx::new();
        assert!(c.set("/tmp/w", true));
        assert_eq!(c.path, "/tmp/w");
        assert!(c.exists);
    }

    #[test]
    fn clear_resets() {
        let mut c = LocationCtx::new();
        assert!(c.set("/tmp/w", true));
        c.clear();
        assert_eq!(c.path, "");
        assert!(!c.exists);
        assert_eq!(c.label(), "loc  missing");
    }

    #[test]
    fn label_ok_parts() {
        let mut c = LocationCtx::new();
        assert!(c.set("/tmp/w", true));
        let l = c.label();
        assert!(l.starts_with("loc /tmp/w "));
        assert!(l.ends_with("ok"));
    }

    #[test]
    fn label_missing_parts() {
        let mut c = LocationCtx::new();
        assert!(c.set("/tmp/w", false));
        assert_eq!(c.label(), "loc /tmp/w missing");
    }

    #[test]
    fn trunc_path_and_label_caps() {
        let mut c = LocationCtx::new();
        let long = "p".repeat(MAX_PATH + 100);
        assert!(c.set(&long, true));
        assert_eq!(c.path.chars().count(), MAX_PATH);
        assert!(c.label().chars().count() <= MAX_LABEL);
    }
}

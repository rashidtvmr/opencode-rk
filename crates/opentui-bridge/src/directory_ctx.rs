#![forbid(unsafe_code)]
//! Directory display context (pure, std only, no IO/FFI).
//!
//! Minimal port of `useDirectory` memo
//! (`packages/tui/src/context/directory.ts:7-17`): project directory
//! with home abbreviation and vcs branch suffix resolved at the SDK
//! layer, not here. Bounded fail-closed store: 1024 bytes max cwd,
//! 512 entries max, 256 bytes max per entry.

/// Max cwd bytes (fail-closed, truncate).
pub const MAX_CWD: usize = 1024;
/// Max entries (fail-closed).
pub const MAX_ENTRIES: usize = 512;
/// Max entry bytes (fail-closed, truncate).
pub const MAX_ENTRY: usize = 256;

fn trunc(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Bounded cwd + entry list.
#[derive(Debug, Default, Clone)]
pub struct DirectoryCtx {
    cwd: String,
    entries: Vec<String>,
}

impl DirectoryCtx {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cwd: String::new(),
            entries: Vec::new(),
        }
    }

    /// Set cwd. False when empty; truncates to [`MAX_CWD`] bytes.
    pub fn set_cwd(&mut self, cwd: &str) -> bool {
        if cwd.is_empty() {
            return false;
        }
        self.cwd = trunc(cwd, MAX_CWD).to_string();
        true
    }

    /// Push entry. False when empty or full; truncates to [`MAX_ENTRY`].
    pub fn add_entry(&mut self, entry: &str) -> bool {
        if entry.is_empty() || self.entries.len() >= MAX_ENTRIES {
            return false;
        }
        self.entries.push(trunc(entry, MAX_ENTRY).to_string());
        true
    }

    #[must_use]
    pub fn list(&self) -> &[String] {
        &self.entries
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cwd_false() {
        let mut c = DirectoryCtx::new();
        assert!(!c.set_cwd(""));
    }

    #[test]
    fn set_cwd_ok() {
        let mut c = DirectoryCtx::new();
        assert!(c.set_cwd("/home/u/proj"));
        assert!(!c.set_cwd(""));
    }

    #[test]
    fn add_list_count() {
        let mut c = DirectoryCtx::new();
        assert!(c.add_entry("a.ts"));
        assert!(c.add_entry("b.ts"));
        assert_eq!(c.list(), &["a.ts".to_string(), "b.ts".to_string()]);
        assert_eq!(c.count(), 2);
        assert!(!c.add_entry(""));
    }

    #[test]
    fn cap_false() {
        let mut c = DirectoryCtx::new();
        for i in 0..MAX_ENTRIES {
            assert!(c.add_entry(&format!("f-{i}")));
        }
        assert_eq!(c.count(), MAX_ENTRIES);
        assert!(!c.add_entry("extra"));
    }

    #[test]
    fn trunc_cwd() {
        let mut c = DirectoryCtx::new();
        let long = "x".repeat(MAX_CWD + 10);
        assert!(c.set_cwd(&long));
        assert_eq!(c.cwd.len(), MAX_CWD);
    }

    #[test]
    fn trunc_entry() {
        let mut c = DirectoryCtx::new();
        let long = "y".repeat(MAX_ENTRY + 10);
        assert!(c.add_entry(&long));
        assert_eq!(c.list()[0].len(), MAX_ENTRY);
    }
}

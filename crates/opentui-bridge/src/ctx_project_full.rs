#![forbid(unsafe_code)]
//! Project root + display name (std-only).
//! Evidence: `packages/tui/src/context/project.tsx:1-50`.
pub const MAX_ROOT: usize = 512;
pub const MAX_NAME: usize = 128;
pub const MAX_LABEL: usize = 256;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxProject {
    pub root: String,
    pub name: String,
}
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}
impl CtxProject {
    #[must_use]
    pub fn new(root: &str, name: &str) -> Self {
        Self {
            root: truncate(root, MAX_ROOT),
            name: truncate(name, MAX_NAME),
        }
    }
    pub fn set_root(&mut self, root: &str) -> bool {
        if root.is_empty() {
            return false;
        }
        self.root = truncate(root, MAX_ROOT);
        true
    }
    pub fn set_name(&mut self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        self.name = truncate(name, MAX_NAME);
        true
    }
    #[must_use]
    pub fn label(&self) -> String {
        let s = if self.name.is_empty() {
            self.root.clone()
        } else if self.root.is_empty() {
            self.name.clone()
        } else {
            format!("{} @ {}", self.name, self.root)
        };
        truncate(&s, MAX_LABEL)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_truncates_caps() {
        let c = CtxProject::new(&"r".repeat(600), &"n".repeat(200));
        assert_eq!(c.root.chars().count(), MAX_ROOT);
        assert_eq!(c.name.chars().count(), MAX_NAME);
    }
    #[test]
    fn set_root_empty_false() {
        let mut c = CtxProject::new("/a", "x");
        assert!(!c.set_root(""));
        assert_eq!(c.root, "/a");
    }
    #[test]
    fn set_name_empty_false() {
        let mut c = CtxProject::new("/a", "x");
        assert!(!c.set_name(""));
        assert_eq!(c.name, "x");
    }
    #[test]
    fn label_format_and_cap() {
        let c = CtxProject::new("/repo", "opencode");
        assert_eq!(c.label(), "opencode @ /repo");
        let big = CtxProject::new(&"r".repeat(300), &"n".repeat(100));
        assert!(big.label().chars().count() <= MAX_LABEL);
    }
}

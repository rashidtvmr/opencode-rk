#![forbid(unsafe_code)]
//! Side files list (mirrors `files.tsx:14-52` modified files).
pub const MAX_FILES: usize = 32;
pub const MAX_PATH: usize = 512;

/// Bounded modified-file paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SideFiles {
    pub files: Vec<String>,
}

impl SideFiles {
    #[must_use]
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }
    /// Push path; false when full or over [`MAX_PATH`].
    pub fn add(&mut self, path: &str) -> bool {
        if path.len() > MAX_PATH || self.files.len() >= MAX_FILES {
            return false;
        }
        self.files.push(path.to_string());
        true
    }
    /// Remove by index; false when out of bounds.
    pub fn remove(&mut self, idx: usize) -> bool {
        if idx >= self.files.len() {
            return false;
        }
        self.files.remove(idx);
        true
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_grows_len() {
        let mut s = SideFiles::new();
        assert!(s.add("a.ts"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn bounds_reject() {
        let mut s = SideFiles::new();
        assert!(!s.add(&"x".repeat(MAX_PATH + 1)));
        for i in 0..MAX_FILES {
            assert!(s.add(&i.to_string()));
        }
        assert!(!s.add("over"));
    }

    #[test]
    fn remove_bounds() {
        let mut s = SideFiles::new();
        assert!(!s.remove(0));
        s.add("a");
        assert!(s.remove(0));
        assert_eq!(s.len(), 0);
    }
}

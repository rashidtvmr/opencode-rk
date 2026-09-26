#![forbid(unsafe_code)]

//! Prompt attachment list.
//! TS truth: packages/tui/src/component/prompt/local-attachment.ts (accept rule only; list is new).

/// Attached file paths, capped.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AttachList {
    files: Vec<String>,
}

/// Max attachments held.
pub const MAX_ATTACH: usize = 16;
/// Max path length in bytes.
pub const MAX_PATH_LEN: usize = 512;

impl AttachList {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn files(&self) -> &[String] {
        &self.files
    }
    pub fn len(&self) -> usize {
        self.files.len()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
    pub fn add(&mut self, path: &str) -> bool {
        if path.is_empty() || path.len() > MAX_PATH_LEN {
            return false;
        }
        if self.files.len() >= MAX_ATTACH {
            return false;
        }
        if self.files.iter().any(|f| f == path) {
            return false;
        }
        self.files.push(path.to_string());
        true
    }
    pub fn remove(&mut self, idx: usize) -> bool {
        if idx >= self.files.len() {
            return false;
        }
        self.files.remove(idx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_and_len() {
        let mut a = AttachList::new();
        assert!(a.add("a.png"));
        assert!(a.add("b.pdf"));
        assert_eq!(a.len(), 2);
    }
    #[test]
    fn add_rejects_empty_and_long() {
        let mut a = AttachList::new();
        assert!(!a.add(""));
        assert!(!a.add(&"x".repeat(513)));
        assert!(a.is_empty());
    }
    #[test]
    fn add_rejects_dup_and_full() {
        let mut a = AttachList::new();
        assert!(a.add("a.png"));
        assert!(!a.add("a.png"));
        for i in 0..15 {
            assert!(a.add(&format!("f{i}.png")));
        }
        assert_eq!(a.len(), 16);
        assert!(!a.add("extra.png"));
    }
    #[test]
    fn remove_ok_and_oob() {
        let mut a = AttachList::new();
        a.add("a.png");
        a.add("b.png");
        assert!(a.remove(0));
        assert_eq!(a.files(), ["b.png"]);
        assert!(!a.remove(5));
    }
}

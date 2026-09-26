#![forbid(unsafe_code)]

//! Home session destination: current directory plus recent directories.
//!
//! TS truth: `packages/tui/src/routes/home/session-destination.tsx` -
//! `HomeSessionDestination` defaults to `sync.path.directory || paths.cwd`.

/// Max chars per directory.
pub const MAX_DIR_LEN: usize = 512;
/// Max recent directories retained.
pub const MAX_RECENT: usize = 16;

/// Destination state for the home session route.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionDestination {
    pub dir: String,
    pub recent: Vec<String>,
}

impl SessionDestination {
    /// New destination with `dir` clipped to 512 chars.
    #[must_use]
    pub fn new(dir: &str) -> Self {
        let mut s = Self::default();
        s.set_dir(dir);
        s
    }

    /// Replace current directory; clipped to 512 chars.
    pub fn set_dir(&mut self, dir: &str) {
        self.dir = clip(dir);
    }

    /// Push a recent directory; false on empty/duplicate. Evicts oldest at 16.
    pub fn push_recent(&mut self, dir: &str) -> bool {
        let dir = clip(dir);
        if dir.is_empty() || self.recent.iter().any(|r| *r == dir) {
            return false;
        }
        if self.recent.len() >= MAX_RECENT {
            self.recent.remove(0);
        }
        self.recent.push(dir);
        true
    }

    /// Current directory.
    #[must_use]
    pub fn current(&self) -> &str {
        &self.dir
    }
}

fn clip(s: &str) -> String {
    s.chars().take(MAX_DIR_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clips_dir_to_512() {
        let s = SessionDestination::new(&"a".repeat(600));
        assert_eq!(s.current().chars().count(), MAX_DIR_LEN);
    }

    #[test]
    fn set_dir_replaces() {
        let mut s = SessionDestination::new("/a");
        s.set_dir("/b");
        assert_eq!(s.current(), "/b");
    }

    #[test]
    fn push_recent_adds() {
        let mut s = SessionDestination::new("/a");
        assert!(s.push_recent("/b"));
        assert_eq!(s.recent, vec!["/b".to_string()]);
    }

    #[test]
    fn push_recent_dup_false() {
        let mut s = SessionDestination::new("/a");
        assert!(s.push_recent("/b"));
        assert!(!s.push_recent("/b"));
        assert_eq!(s.recent.len(), 1);
    }

    #[test]
    fn push_recent_empty_false() {
        let mut s = SessionDestination::new("/a");
        assert!(!s.push_recent(""));
        assert!(s.recent.is_empty());
    }

    #[test]
    fn push_recent_evicts_oldest_at_16() {
        let mut s = SessionDestination::new("/cwd");
        for i in 0..MAX_RECENT {
            assert!(s.push_recent(&format!("/d{i}")));
        }
        assert!(s.push_recent("/new"));
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert!(!s.recent.contains(&"/d0".to_string()));
        assert!(s.recent.contains(&"/new".to_string()));
    }
}

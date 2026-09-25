#![forbid(unsafe_code)]

//! Diff viewer state backing `diff-viewer.tsx` (ROUTE="diff").
pub struct DiffView {
    path: String,
    lines: u32,
}

impl DiffView {
    pub fn new() -> Self {
        Self {
            path: String::new(),
            lines: 0,
        }
    }

    pub fn set_path(&mut self, p: &str) -> bool {
        if p.is_empty() || p.len() > 512 {
            return false;
        }
        self.path = p.to_string();
        true
    }

    pub fn bump_lines(&mut self, delta: u32) {
        self.lines = self.lines.saturating_add(delta);
    }

    pub fn summary(&self) -> String {
        let base = if self.path.is_empty() {
            "diff".to_string()
        } else {
            self.path.clone()
        };
        let mut s = format!("{base}: {} lines", self.lines);
        if s.len() > 512 {
            let mut end = 512;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s.truncate(end);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_summary() {
        let mut v = DiffView::new();
        assert!(v.set_path("src/main.rs"));
        assert_eq!(v.summary(), "src/main.rs: 0 lines");
        assert!(!v.set_path(""));
        assert!(!v.set_path(&"x".repeat(513)));
    }

    #[test]
    fn bump_saturates() {
        let mut v = DiffView::new();
        v.bump_lines(5);
        v.bump_lines(3);
        assert_eq!(v.summary(), "diff: 8 lines");
        v.bump_lines(u32::MAX);
        assert_eq!(v.lines, u32::MAX);
    }

    #[test]
    fn summary_caps_512() {
        let mut v = DiffView::new();
        v.set_path(&"a".repeat(512));
        v.bump_lines(1);
        assert!(v.summary().len() <= 512);
    }
}

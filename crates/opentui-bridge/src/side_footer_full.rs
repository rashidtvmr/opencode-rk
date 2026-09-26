#![forbid(unsafe_code)]
//! Sidebar footer path split (mirrors `footer.tsx` parent/name).

fn cap(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Sidebar footer path tail.
#[derive(Debug, Clone, Default)]
pub struct SideFoot {
    parent: String,
    name: String,
}

impl SideFoot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, parent: &str, name: &str) {
        self.parent = cap(parent, 128);
        self.name = cap(name, 128);
    }

    #[must_use]
    pub fn line(&self) -> String {
        let out = if self.parent.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.parent, self.name)
        };
        cap(&out, 256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_parent_name() {
        let mut f = SideFoot::new();
        f.set("/home/u/proj", "proj");
        assert_eq!(f.line(), "/home/u/proj/proj");
    }

    #[test]
    fn empty_parent_returns_name() {
        let mut f = SideFoot::new();
        f.set("", "proj");
        assert_eq!(f.line(), "proj");
    }

    #[test]
    fn caps_fields_and_line() {
        let mut f = SideFoot::new();
        f.set(&"p".repeat(200), &"n".repeat(200));
        assert_eq!(f.line().chars().count(), 256);
    }
}

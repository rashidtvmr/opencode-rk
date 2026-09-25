#![forbid(unsafe_code)]
//! Named reusable workflow = named skill (V2: workflows live under skills).

pub const MAX_NAME: usize = 64;
pub const MAX_KIND: usize = 32;
pub const MAX_STEPS: usize = 32;

/// One named step inside a workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WfStep {
    pub name: String,
    pub kind: String,
}

/// Named reusable workflow (named skill).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workflow {
    pub name: String,
    pub steps: Vec<WfStep>,
}

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

impl Workflow {
    pub fn new(name: &str) -> Self {
        Self {
            name: trunc(name.trim(), MAX_NAME),
            steps: Vec::new(),
        }
    }
    /// Append step; false on blank/dup/over-cap.
    pub fn add_step(&mut self, name: &str, kind: &str) -> bool {
        if name.trim().is_empty() || kind.trim().is_empty() {
            return false;
        }
        if self.steps.len() >= MAX_STEPS {
            return false;
        }
        let n = trunc(name.trim(), MAX_NAME);
        if self.steps.iter().any(|s| s.name == n) {
            return false;
        }
        self.steps.push(WfStep {
            name: n,
            kind: trunc(kind.trim(), MAX_KIND),
        });
        true
    }
    /// True when named, >=1 step, no duplicate step names.
    pub fn validate(&self) -> bool {
        if self.name.trim().is_empty() || self.steps.is_empty() {
            return false;
        }
        let mut seen = std::collections::HashSet::new();
        self.steps.iter().all(|s| seen.insert(s.name.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ok_path() {
        let mut w = Workflow::new("deploy");
        assert!(w.add_step("build", "shell"));
        assert!(w.validate());
    }
    #[test]
    fn empty_invalid() {
        assert!(!Workflow::new("").validate());
        assert!(!Workflow::new("w").validate());
    }
    #[test]
    fn dup_rejected() {
        let mut w = Workflow::new("w");
        assert!(w.add_step("a", "k"));
        assert!(!w.add_step("a", "k2"));
    }
    #[test]
    fn blank_and_cap_rejected() {
        let mut w = Workflow::new("w");
        assert!(!w.add_step("", "k"));
        assert!(!w.add_step("a", ""));
        for i in 0..MAX_STEPS {
            assert!(w.add_step(&format!("s{i}"), "k"));
        }
        assert!(!w.add_step("extra", "k"));
    }
    #[test]
    fn truncates_char_safe() {
        let mut w = Workflow::new(&"e\u{301}".repeat(100));
        assert!(w.add_step(&"n\u{301}".repeat(100), &"k\u{301}".repeat(100)));
        assert_eq!(w.name.chars().count(), MAX_NAME);
        assert_eq!(w.steps[0].kind.chars().count(), MAX_KIND);
        assert!(w.validate());
    }
}

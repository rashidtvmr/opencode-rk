#![forbid(unsafe_code)]
//! Session subagent footer label (BRIDGE-PAR-363).
//! Port of label logic in `subagent-footer.tsx`: title match else "Subagent".

/// Max label chars.
pub const MAX_LABEL: usize = 128;
/// Max [`SessSubfoot::line`] chars.
pub const MAX_LINE: usize = 256;

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Footer label + active subagent count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessSubfoot {
    pub label: String,
    pub active: u32,
}

impl SessSubfoot {
    /// New footer, label truncated to [`MAX_LABEL`].
    #[must_use]
    pub fn new(label: &str) -> Self {
        Self {
            label: trunc(label, MAX_LABEL),
            active: 0,
        }
    }
    /// Replace label, truncated to [`MAX_LABEL`].
    pub fn set_label(&mut self, label: &str) {
        self.label = trunc(label, MAX_LABEL);
    }
    /// Increment active count (saturating).
    pub fn bump(&mut self) {
        self.active = self.active.saturating_add(1);
    }
    /// `"label (N)"`, capped at [`MAX_LINE`] chars.
    #[must_use]
    pub fn line(&self) -> String {
        trunc(&format!("{} ({})", self.label, self.active), MAX_LINE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_truncated_to_cap() {
        let f = SessSubfoot::new(&"x".repeat(MAX_LABEL + 9));
        assert_eq!(f.label.chars().count(), MAX_LABEL);
    }

    #[test]
    fn set_label_and_bump() {
        let mut f = SessSubfoot::new("Subagent");
        f.set_label("Explore");
        f.bump();
        f.bump();
        assert_eq!(f.active, 2);
        assert_eq!(f.line(), "Explore (2)");
    }

    #[test]
    fn line_capped_at_256() {
        let f = SessSubfoot::new(&"y".repeat(MAX_LABEL + 40));
        assert!(f.line().chars().count() <= MAX_LINE);
    }
}

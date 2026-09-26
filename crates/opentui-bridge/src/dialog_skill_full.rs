#![forbid(unsafe_code)]
//! Skill-name pick list (mirrors `packages/tui/src/component/dialog-skill.tsx:13`
//! `DialogSkill` skill `onSelect(skill.name)` + `dialog.clear()` picks).
//! Async load/error states stay with the caller; this is the owned cursor list.

/// Max chars per skill name.
pub const MAX_NAME: usize = 128;
/// Max skills retained.
pub const MAX_SKILLS: usize = 32;

/// Cursor list of skill names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillDialog {
    pub skills: Vec<String>,
    pub cursor: usize,
}

impl SkillDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append skill; false on blank name or when at cap.
    pub fn push(&mut self, name: &str) -> bool {
        if name.trim().is_empty() || self.skills.len() >= MAX_SKILLS {
            return false;
        }
        self.skills.push(name.chars().take(MAX_NAME).collect());
        true
    }

    /// Move cursor by signed delta, clamped; noop when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.skills.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.skills.len() as isize - 1) as usize;
    }

    /// Selected skill name, if any.
    pub fn selected(&self) -> Option<&str> {
        self.skills.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut d = SkillDialog::new();
        assert!(d.push("review"));
        assert_eq!(d.selected(), Some("review"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut d = SkillDialog::new();
        assert!(!d.push("   "));
        assert!(d.skills.is_empty());
    }

    #[test]
    fn push_truncates_name() {
        let mut d = SkillDialog::new();
        assert!(d.push(&"s".repeat(200)));
        assert_eq!(d.skills[0].chars().count(), MAX_NAME);
    }

    #[test]
    fn push_at_cap_rejected() {
        let mut d = SkillDialog::new();
        for i in 0..MAX_SKILLS {
            assert!(d.push(&format!("s{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.skills.len(), MAX_SKILLS);
    }

    #[test]
    fn cursor_clamps() {
        let mut d = SkillDialog::new();
        d.push("a");
        d.push("b");
        d.move_cursor(9);
        assert_eq!(d.cursor, 1);
        d.move_cursor(-9);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), Some("a"));
    }

    #[test]
    fn cursor_empty_noop() {
        let mut d = SkillDialog::new();
        d.move_cursor(1);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), None);
    }
}

//! Permission prompt view state (TS truth: permission.tsx PermissionStage).

#![forbid(unsafe_code)]

/// Cyclical permission stage: ask once, always allow, reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermStage {
    #[default]
    Ask,
    Always,
    Reject,
}

impl PermStage {
    pub fn label(&self) -> &'static str {
        match self {
            PermStage::Ask => "ask",
            PermStage::Always => "always",
            PermStage::Reject => "reject",
        }
    }

    fn next(&self) -> PermStage {
        match self {
            PermStage::Ask => PermStage::Always,
            PermStage::Always => PermStage::Reject,
            PermStage::Reject => PermStage::Ask,
        }
    }
}

fn trunc(s: &str, cap: usize) -> String {
    if s.chars().count() > cap {
        s.chars().take(cap).collect()
    } else {
        s.to_string()
    }
}

/// View model for a tool permission prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionView {
    pub tool: String,
    pub stage: PermStage,
    pub note: String,
}

impl PermissionView {
    pub fn new(tool: &str) -> Self {
        Self {
            tool: trunc(tool, 64),
            stage: PermStage::Ask,
            note: String::new(),
        }
    }

    pub fn advance(&mut self) -> bool {
        self.stage = self.stage.next();
        true
    }

    pub fn set_note(&mut self, note: &str) {
        self.note = trunc(note, 512);
    }

    pub fn stage_label(&self) -> &'static str {
        self.stage.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stage_is_ask() {
        assert_eq!(PermissionView::new("bash").stage, PermStage::Ask);
    }

    #[test]
    fn advance_cycles_in_order() {
        let mut v = PermissionView::new("bash");
        assert!(v.advance());
        assert_eq!(v.stage, PermStage::Always);
        assert!(v.advance());
        assert_eq!(v.stage, PermStage::Reject);
        assert!(v.advance());
        assert_eq!(v.stage, PermStage::Ask);
    }

    #[test]
    fn advance_wraps_from_reject() {
        let mut v = PermissionView::new("bash");
        v.stage = PermStage::Reject;
        assert!(v.advance());
        assert_eq!(v.stage, PermStage::Ask);
    }

    #[test]
    fn tool_truncated_to_64() {
        let v = PermissionView::new(&"x".repeat(100));
        assert_eq!(v.tool.chars().count(), 64);
    }

    #[test]
    fn note_truncated_to_512() {
        let mut v = PermissionView::new("bash");
        v.set_note(&"y".repeat(600));
        assert_eq!(v.note.chars().count(), 512);
    }

    #[test]
    fn stage_labels() {
        let mut v = PermissionView::new("bash");
        assert_eq!(v.stage_label(), "ask");
        v.advance();
        assert_eq!(v.stage_label(), "always");
        v.advance();
        assert_eq!(v.stage_label(), "reject");
    }
}

//! Native permission prompt state and bounded request data.

pub const MAX_REJECT_NOTE_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Ask,
    Always,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionReq {
    pub tool: String,
    pub always: bool,
    pub reject_note: String,
}

impl PermissionReq {
    pub fn new(
        tool: impl Into<String>,
        always: bool,
        reject_note: impl Into<String>,
    ) -> Result<Self, &'static str> {
        let tool = tool.into();
        if tool.trim().is_empty() {
            return Err("tool must not be empty");
        }

        let reject_note: String = reject_note
            .into()
            .chars()
            .take(MAX_REJECT_NOTE_CHARS)
            .collect();

        Ok(Self {
            tool,
            always,
            reject_note,
        })
    }
}

pub fn advance(stage: Stage, approve: bool) -> (Stage, bool) {
    match stage {
        Stage::Ask if approve => (Stage::Ask, true),
        Stage::Ask => (Stage::Reject, false),
        Stage::Always => (Stage::Always, true),
        Stage::Reject => (Stage::Reject, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_approve_allows_without_persisting_always() {
        assert_eq!(advance(Stage::Ask, true), (Stage::Ask, true));
    }

    #[test]
    fn ask_deny_advances_to_reject() {
        assert_eq!(advance(Stage::Ask, false), (Stage::Reject, false));
    }

    #[test]
    fn always_bypasses_prompt_and_denial_flag() {
        assert_eq!(advance(Stage::Always, false), (Stage::Always, true));
    }

    #[test]
    fn reject_is_terminal_and_denies() {
        assert_eq!(advance(Stage::Reject, true), (Stage::Reject, false));
    }

    #[test]
    fn reject_note_truncates_to_512_unicode_characters() {
        let req = PermissionReq::new("bash", false, "界".repeat(600)).unwrap();
        assert_eq!(req.reject_note.chars().count(), MAX_REJECT_NOTE_CHARS);
    }

    #[test]
    fn empty_tool_is_rejected() {
        assert!(PermissionReq::new(" \t", false, "").is_err());
    }
}

pub const MAX_PROMPT_TOOL_CHARS: usize = 64;
pub const MAX_PROMPT_DETAIL_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermDecision {
    Allow,
    AllowAlways,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermPrompt {
    pub tool: String,
    pub detail: String,
    pub decision: Option<PermDecision>,
}

impl PermPrompt {
    pub fn new(tool: impl Into<String>, detail: impl Into<String>) -> Result<Self, &'static str> {
        let tool: String = tool.into().chars().take(MAX_PROMPT_TOOL_CHARS).collect();
        if tool.trim().is_empty() {
            return Err("tool must not be empty");
        }
        let detail: String = detail
            .into()
            .chars()
            .take(MAX_PROMPT_DETAIL_CHARS)
            .collect();
        Ok(Self {
            tool,
            detail,
            decision: None,
        })
    }

    pub fn decide(&mut self, decision: PermDecision) {
        self.decision = Some(decision);
    }

    pub fn is_decided(&self) -> bool {
        self.decision.is_some()
    }

    pub fn decision_label(&self) -> &'static str {
        match self.decision {
            None => "pending",
            Some(PermDecision::Allow) => "allow",
            Some(PermDecision::AllowAlways) => "always",
            Some(PermDecision::Reject) => "reject",
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn prompt_starts_undecided() {
        let p = PermPrompt::new("bash", "run ls").unwrap();
        assert!(!p.is_decided());
        assert_eq!(p.decision, None);
        assert_eq!(p.decision_label(), "pending");
    }

    #[test]
    fn decide_allow_sets_and_labels() {
        let mut p = PermPrompt::new("bash", "run ls").unwrap();
        p.decide(PermDecision::Allow);
        assert!(p.is_decided());
        assert_eq!(p.decision, Some(PermDecision::Allow));
        assert_eq!(p.decision_label(), "allow");
    }

    #[test]
    fn decide_always_labels_always() {
        let mut p = PermPrompt::new("bash", "run ls").unwrap();
        p.decide(PermDecision::AllowAlways);
        assert!(p.is_decided());
        assert_eq!(p.decision_label(), "always");
    }

    #[test]
    fn decide_reject_labels_reject() {
        let mut p = PermPrompt::new("bash", "run ls").unwrap();
        p.decide(PermDecision::Reject);
        assert!(p.is_decided());
        assert_eq!(p.decision_label(), "reject");
    }

    #[test]
    fn decide_overwrite_ok() {
        let mut p = PermPrompt::new("bash", "run ls").unwrap();
        p.decide(PermDecision::Allow);
        p.decide(PermDecision::Reject);
        assert_eq!(p.decision, Some(PermDecision::Reject));
        assert_eq!(p.decision_label(), "reject");
    }

    #[test]
    fn prompt_truncates_tool_and_detail() {
        let p = PermPrompt::new("x".repeat(100), "界".repeat(600)).unwrap();
        assert_eq!(p.tool.chars().count(), MAX_PROMPT_TOOL_CHARS);
        assert_eq!(p.detail.chars().count(), MAX_PROMPT_DETAIL_CHARS);
    }

    #[test]
    fn empty_tool_is_rejected_in_prompt() {
        assert!(PermPrompt::new("  ", "d").is_err());
    }
}

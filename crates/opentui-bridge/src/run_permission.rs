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

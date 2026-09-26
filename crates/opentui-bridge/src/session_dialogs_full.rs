#![forbid(unsafe_code)]
//! Session dialog tags (mirrors `packages/tui/src/routes/session/` 10 files:
//! `permission.tsx`, `question.tsx`, `sidebar.tsx:12`, `footer.tsx:9`,
//! `subagent-footer.tsx:11`, `dialog-message.tsx`, `dialog-timeline.tsx`,
//! `dialog-fork-from-timeline.tsx`, `dialog-subagent.tsx`, `index.tsx`).

/// Session dialog tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessDialog {
    #[default]
    Permission,
    Question,
    Sidebar,
    Footer,
    SubagentFooter,
    Msg,
    Timeline,
    Fork,
    Subagent,
}

/// Variant count.
pub const DLG_COUNT: usize = 9;

impl SessDialog {
    /// Stable slug per variant.
    #[must_use]
    pub const fn dlg_name(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Sidebar => "sidebar",
            Self::Footer => "footer",
            Self::SubagentFooter => "subagent-footer",
            Self::Msg => "message",
            Self::Timeline => "timeline",
            Self::Fork => "fork",
            Self::Subagent => "subagent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_matches_variants() {
        let all = [
            SessDialog::Permission,
            SessDialog::Question,
            SessDialog::Sidebar,
            SessDialog::Footer,
            SessDialog::SubagentFooter,
            SessDialog::Msg,
            SessDialog::Timeline,
            SessDialog::Fork,
            SessDialog::Subagent,
        ];
        assert_eq!(all.len(), DLG_COUNT);
    }
    #[test]
    fn names_unique() {
        let names = [
            SessDialog::Permission.dlg_name(),
            SessDialog::Question.dlg_name(),
            SessDialog::Sidebar.dlg_name(),
            SessDialog::Footer.dlg_name(),
            SessDialog::SubagentFooter.dlg_name(),
            SessDialog::Msg.dlg_name(),
            SessDialog::Timeline.dlg_name(),
            SessDialog::Fork.dlg_name(),
            SessDialog::Subagent.dlg_name(),
        ];
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(names[i], names[j]);
            }
        }
    }
    #[test]
    fn default_is_permission() {
        assert_eq!(SessDialog::default().dlg_name(), "permission");
    }
}

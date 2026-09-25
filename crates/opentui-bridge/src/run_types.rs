#![forbid(unsafe_code)]
//! Shared, bounded run/footer value types.
//!
//! Evidence: `/home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/types.ts`
//! at a0d9b6c: footer surfaces are `FooterView` (lines 169-176), and
//! `StreamCommit` is an append-only scrollback value (lines 299-320).

/// Maximum UTF-8 bytes in a stream commit identifier.
pub const MAX_STREAM_COMMIT_ID: usize = 1024;
/// Maximum UTF-8 bytes in stream commit text.
pub const MAX_STREAM_COMMIT_TEXT: usize = 1024;

/// Semantic phase displayed by the run footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FooterPhase {
    #[default]
    Prompt,
    Permission,
    Question,
    Subagent,
    Done,
}

/// Atomic footer state: the active surface and whether work is in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FooterView {
    pub phase: FooterPhase,
    pub busy: bool,
}

impl FooterView {
    #[must_use]
    pub const fn new(phase: FooterPhase, busy: bool) -> Self {
        Self { phase, busy }
    }
}

/// One append-only text commit emitted by the run stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCommit {
    pub id: String,
    pub text: String,
}

impl StreamCommit {
    #[must_use]
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }

    /// Validate UTF-8 byte bounds without changing the commit.
    pub fn validate(&self) -> Result<(), StreamCommitError> {
        let id_len = self.id.len();
        if id_len > MAX_STREAM_COMMIT_ID {
            return Err(StreamCommitError::IdTooLong { len: id_len });
        }
        let text_len = self.text.len();
        if text_len > MAX_STREAM_COMMIT_TEXT {
            return Err(StreamCommitError::TextTooLong { len: text_len });
        }
        Ok(())
    }
}

/// Validation failure for an over-sized stream commit field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamCommitError {
    IdTooLong { len: usize },
    TextTooLong { len: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_prompt() {
        assert_eq!(FooterPhase::Prompt, FooterPhase::default());
    }

    #[test]
    fn phase_permission() {
        assert_ne!(FooterPhase::Permission, FooterPhase::Prompt);
    }

    #[test]
    fn phase_question() {
        assert_ne!(FooterPhase::Question, FooterPhase::Permission);
    }

    #[test]
    fn phase_subagent() {
        assert_ne!(FooterPhase::Subagent, FooterPhase::Question);
    }

    #[test]
    fn phase_done() {
        assert_ne!(FooterPhase::Done, FooterPhase::Subagent);
    }

    #[test]
    fn footer_view_keeps_phase_and_busy() {
        let view = FooterView::new(FooterPhase::Subagent, true);
        assert_eq!(view.phase, FooterPhase::Subagent);
        assert!(view.busy);
    }

    #[test]
    fn empty_commit_is_valid() {
        assert_eq!(StreamCommit::new("", "").validate(), Ok(()));
    }

    #[test]
    fn exact_byte_boundary_is_valid() {
        let commit = StreamCommit::new(
            "i".repeat(MAX_STREAM_COMMIT_ID),
            "t".repeat(MAX_STREAM_COMMIT_TEXT),
        );
        assert_eq!(commit.validate(), Ok(()));
    }

    #[test]
    fn oversized_id_is_rejected() {
        let commit = StreamCommit::new("i".repeat(MAX_STREAM_COMMIT_ID + 1), "");
        assert_eq!(
            commit.validate(),
            Err(StreamCommitError::IdTooLong {
                len: MAX_STREAM_COMMIT_ID + 1
            })
        );
    }

    #[test]
    fn oversized_text_is_rejected() {
        let commit = StreamCommit::new("", "t".repeat(MAX_STREAM_COMMIT_TEXT + 1));
        assert_eq!(
            commit.validate(),
            Err(StreamCommitError::TextTooLong {
                len: MAX_STREAM_COMMIT_TEXT + 1
            })
        );
    }
}

#![forbid(unsafe_code)]
//! Fork-from-timeline confirm dialog (mirrors
//! `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12` `DialogForkFromTimeline`
//! message pick + `sdk.client.session.fork` confirm step).
//!
//! Divergences:
//! - TS forks immediately on `onSelect`; Rust models pick-then-confirm gate.
//! - TS `undefined` full-session fork not modeled; `open` requires message id.
//! - Fork RPC + navigation are host concerns, not modeled.

/// Max chars for `ForkDialog.message_id` (mirrors `session_timeline::MAX_ID`).
pub const MAX_MESSAGE_ID: usize = 64;

/// Pending fork confirmation for one timeline message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkDialog {
    pub message_id: String,
    pub confirmed: bool,
}

impl ForkDialog {
    /// Open dialog for `message_id`; errs on empty/blank id.
    pub fn open(message_id: &str) -> Result<Self, String> {
        if message_id.trim().is_empty() {
            return Err("message_id must not be empty".to_string());
        }
        Ok(Self {
            message_id: message_id.chars().take(MAX_MESSAGE_ID).collect(),
            confirmed: false,
        })
    }

    /// Confirm the fork.
    pub fn confirm(&mut self) {
        self.confirmed = true;
    }

    /// Dismiss without forking.
    pub fn cancel(&mut self) {
        self.confirmed = false;
    }

    /// Whether fork was confirmed.
    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_errs() {
        assert!(ForkDialog::open("").is_err());
    }

    #[test]
    fn open_blank_errs() {
        assert!(ForkDialog::open("   ").is_err());
    }

    #[test]
    fn open_keeps_id_unconfirmed() {
        let d = ForkDialog::open("msg1").unwrap();
        assert_eq!(d.message_id, "msg1");
        assert!(!d.is_confirmed());
    }

    #[test]
    fn confirm_sets_confirmed() {
        let mut d = ForkDialog::open("msg1").unwrap();
        d.confirm();
        assert!(d.is_confirmed());
    }

    #[test]
    fn cancel_clears_confirmed() {
        let mut d = ForkDialog::open("msg1").unwrap();
        d.confirm();
        d.cancel();
        assert!(!d.is_confirmed());
    }

    #[test]
    fn double_confirm_idempotent() {
        let mut d = ForkDialog::open("msg1").unwrap();
        d.confirm();
        d.confirm();
        assert!(d.is_confirmed());
        assert_eq!(d.message_id, "msg1");
    }

    #[test]
    fn id_truncates_to_cap() {
        let d = ForkDialog::open(&"m".repeat(100)).unwrap();
        assert_eq!(d.message_id.chars().count(), MAX_MESSAGE_ID);
    }
}

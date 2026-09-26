#![forbid(unsafe_code)]
//! Full move-session dialog: capped destination plus confirm/cancel.
//!
//! Mirrors `packages/tui/src/component/dialog-move-session.tsx:34`
//! `DialogMoveSession` (directory selection, confirm on non-empty dest).

/// Max chars kept in destination.
pub const MAX_DEST_CHARS: usize = 512;

/// Move dialog with pending destination and confirmed flag.
#[derive(Debug, Clone, Default)]
pub struct MoveDialog {
    pub dest: String,
    pub confirmed: bool,
}

impl MoveDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_dest(&mut self, dest: &str) {
        self.dest = dest.chars().take(MAX_DEST_CHARS).collect();
        self.confirmed = false;
    }

    pub fn confirm(&mut self) -> bool {
        self.confirmed = !self.dest.is_empty();
        self.confirmed
    }

    pub fn cancel(&mut self) {
        self.dest.clear();
        self.confirmed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_dest_stores_and_resets_confirmed() {
        let mut d = MoveDialog::new();
        d.confirm();
        d.set_dest("/tmp/x");
        assert_eq!(d.dest, "/tmp/x");
        assert!(!d.confirmed);
    }

    #[test]
    fn set_dest_caps_at_512_chars() {
        let mut d = MoveDialog::new();
        d.set_dest(&"a".repeat(600));
        assert_eq!(d.dest.chars().count(), MAX_DEST_CHARS);
    }

    #[test]
    fn confirm_true_when_dest_non_empty() {
        let mut d = MoveDialog::new();
        d.set_dest("/tmp/x");
        assert!(d.confirm());
        assert!(d.confirmed);
    }

    #[test]
    fn confirm_false_when_empty() {
        let mut d = MoveDialog::new();
        assert!(!d.confirm());
        assert!(!d.confirmed);
    }

    #[test]
    fn cancel_clears() {
        let mut d = MoveDialog::new();
        d.set_dest("/tmp/x");
        d.confirm();
        d.cancel();
        assert!(d.dest.is_empty());
        assert!(!d.confirmed);
    }
}

#![forbid(unsafe_code)]
//! Full confirm dialog: capped title plus boolean answer.
//!
//! Mirrors `packages/tui/src/ui/dialog-confirm.tsx:19`
//! `DialogConfirm` (confirm/cancel selection, `DialogConfirmResult`).

/// Max chars kept in title.
pub const MAX_TITLE_CHARS: usize = 128;

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Title prompt with tri-state answer (none = pending).
#[derive(Debug, Clone, Default)]
pub struct ConfirmDialog {
    pub title: String,
    pub confirmed: Option<bool>,
}

impl ConfirmDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ask(&mut self, title: &str) {
        self.title = trunc(title, MAX_TITLE_CHARS);
        self.confirmed = None;
    }

    pub fn answer(&mut self, yes: bool) {
        self.confirmed = Some(yes);
    }

    pub fn result(&self) -> Option<bool> {
        self.confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_caps_title_and_resets() {
        let mut d = ConfirmDialog::new();
        d.answer(true);
        d.ask(&"t".repeat(200));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(d.result(), None);
    }

    #[test]
    fn answer_true() {
        let mut d = ConfirmDialog::new();
        d.ask("sure?");
        d.answer(true);
        assert_eq!(d.result(), Some(true));
    }

    #[test]
    fn answer_false() {
        let mut d = ConfirmDialog::new();
        d.ask("sure?");
        d.answer(false);
        assert_eq!(d.result(), Some(false));
    }

    #[test]
    fn ask_unicode_safe_cap() {
        let mut d = ConfirmDialog::new();
        d.ask(&"é".repeat(200));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn default_pending() {
        assert_eq!(ConfirmDialog::new().result(), None);
    }
}

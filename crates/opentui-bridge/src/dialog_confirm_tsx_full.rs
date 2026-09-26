#![forbid(unsafe_code)]
//! TSX confirm dialog: `dialog-confirm.tsx:9` title + confirm/cancel.
/// Max chars kept in title.
pub const MAX_TITLE_CHARS: usize = 128;

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Confirm dialog with pending tri-state.
#[derive(Debug, Clone, Default)]
pub struct ConfirmTsx {
    pub title: String,
    pub ok: bool,
    done: bool,
}

impl ConfirmTsx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = trunc(title, MAX_TITLE_CHARS);
        self.done = false;
    }

    pub fn confirm(&mut self) {
        self.ok = true;
        self.done = true;
    }

    pub fn cancel(&mut self) {
        self.ok = false;
        self.done = true;
    }

    pub fn decided(&self) -> Option<bool> {
        self.done.then_some(self.ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_caps_and_pending() {
        let mut d = ConfirmTsx::new();
        d.set_title(&"t".repeat(200));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(d.decided(), None);
    }

    #[test]
    fn confirm_decides_true() {
        let mut d = ConfirmTsx::new();
        d.set_title("sure?");
        d.confirm();
        assert_eq!(d.decided(), Some(true));
    }

    #[test]
    fn cancel_decides_false() {
        let mut d = ConfirmTsx::new();
        d.set_title("sure?");
        d.cancel();
        assert_eq!(d.decided(), Some(false));
    }

    #[test]
    fn retitle_resets_decision() {
        let mut d = ConfirmTsx::new();
        d.set_title("a");
        d.confirm();
        d.set_title("b");
        assert_eq!(d.decided(), None);
    }
}

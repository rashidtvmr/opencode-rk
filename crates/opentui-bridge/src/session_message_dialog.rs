#![forbid(unsafe_code)]
//! Message dialog (mirrors `dialog-message.tsx:22-23` title + message lookup).
//!
//! TS: `DialogMessage { messageID }` looks up `sync.data.message[sessionID]`
//! and opens `DialogSelect` with Revert/Copy/Fork actions. This type covers
//! the open/body/preview state; actions live in `session_msg_dialogs.rs`.

/// Max message id chars.
pub const MAX_MESSAGE_ID: usize = 64;
/// Max body chars (8KiB).
pub const MAX_BODY: usize = 8192;
/// Render preview chars.
pub const PREVIEW_LEN: usize = 200;

/// Open message dialog state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDialog {
    pub message_id: String,
    pub body: String,
    pub open: bool,
}

impl MessageDialog {
    /// Open with id/body; empty id errs, overlong fields truncate.
    pub fn open(id: &str, body: &str) -> Result<Self, String> {
        if id.is_empty() {
            return Err("message id must not be empty".to_string());
        }
        Ok(Self {
            message_id: id.chars().take(MAX_MESSAGE_ID).collect(),
            body: body.chars().take(MAX_BODY).collect(),
            open: true,
        })
    }

    /// Close the dialog (keeps id/body for preview reuse).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Render `"<id>: <preview(200)>"`, or `None` when closed.
    pub fn render(&self) -> Option<String> {
        if !self.open {
            return None;
        }
        let preview: String = self.body.chars().take(PREVIEW_LEN).collect();
        Some(format!("{}: {}", self.message_id, preview))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_id_errs() {
        assert!(MessageDialog::open("", "hi").is_err());
    }

    #[test]
    fn open_body_truncates() {
        let long = "x".repeat(MAX_BODY + 10);
        let d = MessageDialog::open("m1", &long).unwrap();
        assert_eq!(d.body.chars().count(), MAX_BODY);
        assert!(d.open);
    }

    #[test]
    fn open_id_truncates() {
        let long = "y".repeat(MAX_MESSAGE_ID + 5);
        let d = MessageDialog::open(&long, "b").unwrap();
        assert_eq!(d.message_id.chars().count(), MAX_MESSAGE_ID);
    }

    #[test]
    fn render_none_when_closed() {
        let mut d = MessageDialog::open("m1", "body").unwrap();
        d.close();
        assert_eq!(d.render(), None);
    }

    #[test]
    fn render_preview_format() {
        let d = MessageDialog::open("m1", "hello world").unwrap();
        assert_eq!(d.render(), Some("m1: hello world".to_string()));
    }

    #[test]
    fn render_preview_truncates_200() {
        let long = "z".repeat(PREVIEW_LEN + 50);
        let d = MessageDialog::open("m1", &long).unwrap();
        let out = d.render().unwrap();
        assert_eq!(out.len(), "m1: ".len() + PREVIEW_LEN);
    }

    #[test]
    fn close_clears_open() {
        let mut d = MessageDialog::open("m1", "b").unwrap();
        assert!(d.open);
        d.close();
        assert!(!d.open);
    }
}

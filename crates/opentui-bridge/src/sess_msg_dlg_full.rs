#![forbid(unsafe_code)]
//! Send-once message dialog (mirrors `dialog-message.tsx:10` `DialogMessage`).
/// Max text chars (4KiB).
pub const MAX_TEXT: usize = 4096;
/// Send-once dialog: staged text plus sent flag.
#[derive(Debug, Clone, Default)]
pub struct MsgDlg {
    pub text: String,
    pub sent: bool,
}
impl MsgDlg {
    pub fn new() -> Self {
        Self::default()
    }
    /// Stage text (char-capped); re-arms `sent`.
    pub fn set(&mut self, text: &str) {
        self.text = text.chars().take(MAX_TEXT).collect();
        self.sent = false;
    }
    /// Send once: true on first non-empty send, false after.
    pub fn send(&mut self) -> bool {
        if self.sent || self.text.is_empty() {
            return false;
        }
        self.sent = true;
        true
    }
    /// Whether the staged text was sent.
    pub fn is_sent(&self) -> bool {
        self.sent
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_caps_4kib() {
        let mut d = MsgDlg::new();
        d.set(&"x".repeat(MAX_TEXT + 10));
        assert_eq!(d.text.chars().count(), MAX_TEXT);
        assert!(!d.is_sent());
    }
    #[test]
    fn send_true_once() {
        let mut d = MsgDlg::new();
        d.set("hi");
        assert!(d.send());
        assert!(d.is_sent());
        assert!(!d.send());
    }
    #[test]
    fn empty_never_sends() {
        let mut d = MsgDlg::new();
        assert!(!d.send());
        d.set("");
        assert!(!d.send());
    }
    #[test]
    fn set_rearms_after_send() {
        let mut d = MsgDlg::new();
        d.set("a");
        assert!(d.send());
        d.set("b");
        assert!(!d.is_sent());
        assert!(d.send());
    }
}

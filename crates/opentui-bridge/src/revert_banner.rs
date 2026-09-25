#![forbid(unsafe_code)]
//! Revert banner state (std-only).
//!
//! Mirrors `crate::revert_diff` naming (`FileDiff`, `MAX_FILES` style caps).
//! Fail-closed: empty `show` errs, never shows an empty banner.

/// Cap on files retained per banner.
pub const MAX_FILES: usize = 32;
/// Cap on rendered message chars.
pub const MAX_MESSAGE_LEN: usize = 512;

/// One-line revert notice; hidden until [`RevertBanner::show`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RevertBanner {
    pub visible: bool,
    pub files: Vec<String>,
    pub message: String,
}

impl RevertBanner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Show banner for `files`. Empty errs; over-cap keeps first 32.
    pub fn show(&mut self, files: Vec<String>) -> Result<(), String> {
        if files.is_empty() {
            return Err("no files to revert".to_string());
        }
        let kept: Vec<String> = files.into_iter().take(MAX_FILES).collect();
        let n = kept.len();
        let body: String = kept.join(", ").chars().take(MAX_MESSAGE_LEN).collect();
        let mut msg = format!("Revert {n} file(s): {body}");
        if msg.chars().count() > MAX_MESSAGE_LEN {
            msg = msg.chars().take(MAX_MESSAGE_LEN).collect();
        }
        self.files = kept;
        self.message = msg;
        self.visible = true;
        Ok(())
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.files.clear();
        self.message.clear();
    }

    #[must_use]
    pub fn render(&self) -> Option<String> {
        if self.visible {
            Some(self.message.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_by_default() {
        let b = RevertBanner::new();
        assert!(!b.visible);
        assert_eq!(b.render(), None);
    }

    #[test]
    fn show_renders_one_line() {
        let mut b = RevertBanner::new();
        assert!(b.show(vec!["a.ts".into(), "b.ts".into()]).is_ok());
        assert_eq!(b.render(), Some("Revert 2 file(s): a.ts, b.ts".into()));
    }

    #[test]
    fn empty_show_errs_fail_closed() {
        let mut b = RevertBanner::new();
        assert!(b.show(vec![]).is_err());
        assert!(!b.visible);
        assert_eq!(b.render(), None);
    }

    #[test]
    fn over_cap_truncates_to_32() {
        let mut b = RevertBanner::new();
        let files: Vec<String> = (0..40).map(|i| format!("f{i}")).collect();
        assert!(b.show(files).is_ok());
        assert_eq!(b.files.len(), MAX_FILES);
        assert!(b.render().unwrap().starts_with("Revert 32 file(s): "));
    }

    #[test]
    fn hide_clears_state() {
        let mut b = RevertBanner::new();
        b.show(vec!["a.ts".into()]).unwrap();
        b.hide();
        assert!(!b.visible);
        assert!(b.files.is_empty());
        assert_eq!(b.render(), None);
    }

    #[test]
    fn message_capped_at_512_chars() {
        let mut b = RevertBanner::new();
        b.show(vec!["x".repeat(600)]).unwrap();
        assert!(b.message.chars().count() <= MAX_MESSAGE_LEN);
    }
}

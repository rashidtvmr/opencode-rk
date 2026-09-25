#![forbid(unsafe_code)]
//! Full-session fork dialog (mirrors
//! `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12`
//! `DialogForkFromTimeline` full-session pick + confirm gate).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkDlg {
    pub src: String,
    pub dst: Option<String>,
}
/// Max chars for `src`/`dst` ids.
pub const MAX_ID: usize = 128;
impl ForkDlg {
    pub fn new() -> Self {
        Self::default()
    }
    /// Open dialog for `src`; truncates to cap, clears pending `dst`.
    pub fn open(&mut self, src: &str) {
        self.src = src.chars().take(MAX_ID).collect();
        self.dst = None;
    }
    /// Confirm fork into `dst`; false on blank `src`/`dst`.
    pub fn confirm(&mut self, dst: &str) -> bool {
        if self.src.trim().is_empty() || dst.trim().is_empty() {
            return false;
        }
        self.dst = Some(dst.chars().take(MAX_ID).collect());
        true
    }
    /// Confirmed fork target, if any.
    pub fn dst_of(&self) -> Option<&str> {
        self.dst.as_deref()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_sets_src_clears_dst() {
        let mut d = ForkDlg::new();
        d.open("s1");
        assert!(d.confirm("d1"));
        d.open("s2");
        assert_eq!(d.src, "s2");
        assert_eq!(d.dst_of(), None);
    }
    #[test]
    fn confirm_blank_fails() {
        let mut d = ForkDlg::new();
        d.open("s1");
        assert!(!d.confirm("  "));
        assert_eq!(d.dst_of(), None);
    }
    #[test]
    fn confirm_ok_exposes_dst() {
        let mut d = ForkDlg::new();
        d.open("s1");
        assert!(d.confirm("d1"));
        assert_eq!(d.dst_of(), Some("d1"));
    }
    #[test]
    fn src_truncates_to_cap() {
        let mut d = ForkDlg::new();
        d.open(&"x".repeat(200));
        assert_eq!(d.src.chars().count(), MAX_ID);
    }
}

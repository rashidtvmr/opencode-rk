#![forbid(unsafe_code)]
//! Delete-failed dialog: capped session/reason + width-clipped rows.
//!
//! Mirrors `packages/tui/src/component/dialog-session-delete-failed.tsx:8`
//! `DialogSessionDeleteFailed` (failed-session notice with recovery options).

/// Max chars kept in session.
pub const MAX_SESSION_CHARS: usize = 128;
/// Max chars kept in reason.
pub const MAX_REASON_CHARS: usize = 512;
/// Max rows returned by [`DeleteFailed::lines`].
pub const MAX_LINES: usize = 8;

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn push_wrapped(out: &mut Vec<String>, text: &str, w: usize) {
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if cur.chars().count() == w {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() || text.is_empty() {
        out.push(cur);
    }
}

/// Failed session-delete notice.
#[derive(Debug, Clone, Default)]
pub struct DeleteFailed {
    session: String,
    reason: String,
}

impl DeleteFailed {
    pub fn new(session: &str, reason: &str) -> Self {
        Self {
            session: trunc(session, MAX_SESSION_CHARS),
            reason: trunc(reason, MAX_REASON_CHARS),
        }
    }
    pub fn session_of(&self) -> &str {
        &self.session
    }
    pub fn lines(&self, width: usize) -> Vec<String> {
        let w = width.max(1);
        let mut out = Vec::new();
        for raw in ["Delete failed", &self.session, &self.reason] {
            push_wrapped(&mut out, raw, w);
            if out.len() >= MAX_LINES {
                break;
            }
        }
        out.truncate(MAX_LINES);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_caps_fields() {
        let d = DeleteFailed::new(&"s".repeat(200), &"r".repeat(600));
        assert_eq!(d.session.chars().count(), MAX_SESSION_CHARS);
        assert_eq!(d.reason.chars().count(), MAX_REASON_CHARS);
    }
    #[test]
    fn session_of_returns_session() {
        assert_eq!(DeleteFailed::new("abc", "why").session_of(), "abc");
    }
    #[test]
    fn lines_clip_width() {
        let d = DeleteFailed::new("abcdef", "ghijkl");
        let rows = d.lines(3);
        assert!(!rows.is_empty() && rows.iter().all(|r| r.chars().count() <= 3));
        assert_eq!(rows[0], "Del");
    }
    #[test]
    fn lines_cap_eight_rows() {
        let d = DeleteFailed::new(&"s".repeat(100), &"r".repeat(500));
        assert!(d.lines(4).len() <= MAX_LINES);
    }
    #[test]
    fn lines_zero_width_no_panic_unicode_safe() {
        let d = DeleteFailed::new("éééé", "üüüü");
        assert!(d.lines(0).iter().all(|r| r.chars().count() <= 1));
    }
}

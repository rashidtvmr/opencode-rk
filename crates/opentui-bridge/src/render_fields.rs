#![forbid(unsafe_code)]
//! Homeless render fields: focused colors + single-toast replace (BRIDGE-GAP-59).
//!
//! SOURCE EVIDENCE (TS checkout /home/rashid/projects/opencode):
//! - `focusedTextColor`/`textColor` + `focusedBackgroundColor`
//!   (`ui/dialog-select.tsx:580,582`, `component/prompt/index.tsx:1370-1371`).
//!   Exact TS blue hex unevidenced; default blue (0,0,255) documented here.
//! - Single `currentToast` replace semantics (`ui/toast.tsx`); FIFO cap 8
//!   mirrors `crate::toast::MAX_TOASTS` (queue drops oldest on overflow).
//!   Companion [`toast_queue_push`] documents the difference: replace keeps
//!   one slot, queue keeps up to 8.

/// Focused-state colors (`focused_text`, `focused_bg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusedColors {
    pub focused_text: (u8, u8, u8),
    pub focused_bg: (u8, u8, u8),
}

/// Defaults white-on-blue.
#[must_use]
pub const fn focused_style() -> FocusedColors {
    FocusedColors {
        focused_text: (255, 255, 255),
        focused_bg: (0, 0, 255),
    }
}

/// Max queued toasts (mirrors `crate::toast::MAX_TOASTS`).
pub const TOAST_QUEUE_CAP: usize = 8;

/// TS single-`currentToast` semantics: always replaces, returns true.
pub fn toast_single_replace(current: &mut Option<String>, next: String) -> bool {
    *current = Some(next);
    true
}

/// Companion FIFO: push, drop oldest past cap 8. Differs from
/// [`toast_single_replace`] which keeps one slot.
pub fn toast_queue_push(queue: &mut Vec<String>, next: String) {
    if queue.len() >= TOAST_QUEUE_CAP {
        queue.remove(0);
    }
    queue.push(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_defaults() {
        assert_eq!(focused_style().focused_text, (255, 255, 255));
        assert_eq!(focused_style().focused_bg, (0, 0, 255));
    }

    #[test]
    fn replace_always_true() {
        let mut cur = None;
        assert!(toast_single_replace(&mut cur, "a".into()));
        assert_eq!(cur.as_deref(), Some("a"));
    }

    #[test]
    fn replace_overwrites_existing() {
        let mut cur = Some("old".into());
        assert!(toast_single_replace(&mut cur, "new".into()));
        assert_eq!(cur.as_deref(), Some("new"));
    }

    #[test]
    fn queue_fifo() {
        let mut q = vec!["a".to_string()];
        toast_queue_push(&mut q, "b".into());
        assert_eq!(q, ["a", "b"]);
    }

    #[test]
    fn queue_cap_8() {
        let mut q: Vec<String> = (0..TOAST_QUEUE_CAP).map(|i| i.to_string()).collect();
        toast_queue_push(&mut q, "new".into());
        assert_eq!(q.len(), TOAST_QUEUE_CAP);
        assert_eq!(q[0], "1");
        assert_eq!(q[TOAST_QUEUE_CAP - 1], "new");
    }

    #[test]
    fn replace_vs_queue_differ() {
        let mut cur = Some("old".into());
        toast_single_replace(&mut cur, "x".into());
        let mut q = vec!["old".to_string()];
        toast_queue_push(&mut q, "x".into());
        assert_eq!(cur.as_deref(), Some("x"));
        assert_eq!(q, ["old", "x"]);
    }
}

#![forbid(unsafe_code)]

//! Two-line session header for run views (std-only).
//!
//! TS truth: `session_shared_full::SessionSharedFull` header half plus
//! `session_index::SessionIndex` task/action counts.

use crate::session_index::SessionIndex;
use crate::session_shared_full::SessionSharedFull;

/// Title when set, else id.
#[must_use]
pub fn title_or_id(sess: &SessionSharedFull) -> String {
    if sess.title.is_empty() {
        sess.id.clone()
    } else {
        sess.title.clone()
    }
}

/// Two width-clipped, char-safe lines: header + `tasks N actions M`.
#[must_use]
pub fn header_lines(sess: &SessionSharedFull, idx: &SessionIndex, width: usize) -> Vec<String> {
    vec![
        clip(&sess.header(), width),
        clip(
            &format!(
                "tasks {} actions {}",
                idx.foreground_tasks,
                idx.actions.len()
            ),
            width,
        ),
    ]
}

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess() -> SessionSharedFull {
        let mut s = SessionSharedFull::new("123456789abcdef", "Hi");
        s.bump();
        s
    }

    #[test]
    fn two_lines() {
        let lines = header_lines(&sess(), &SessionIndex::new(), 80);
        assert_eq!(
            lines,
            vec![
                "12345678 Hi (1)".to_string(),
                "tasks 0 actions 0".to_string()
            ]
        );
    }

    #[test]
    fn counts_shown() {
        let mut idx = SessionIndex::new();
        idx.foreground_tasks = 2;
        idx.register_action("a.b".to_string());
        let lines = header_lines(&sess(), &idx, 80);
        assert_eq!(lines[1], "tasks 2 actions 1");
    }

    #[test]
    fn width_clip_char_safe() {
        let s = SessionSharedFull::new("id", "h\u{e9}llo w\u{f6}rld");
        let lines = header_lines(&s, &SessionIndex::new(), 4);
        assert!(lines.iter().all(|l| l.chars().count() <= 4));
        assert_eq!(lines[0].chars().count(), 4);
    }

    #[test]
    fn title_or_id_prefers_title() {
        assert_eq!(title_or_id(&sess()), "Hi");
    }

    #[test]
    fn title_or_id_falls_back() {
        let s = SessionSharedFull::new("abc", "");
        assert_eq!(title_or_id(&s), "abc");
    }

    #[test]
    fn zero_width_empty() {
        let lines = header_lines(&sess(), &SessionIndex::new(), 0);
        assert_eq!(lines, vec![String::new(), String::new()]);
    }
}

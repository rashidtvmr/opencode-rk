//! Render-counting session header flow (std-only).
#![forbid(unsafe_code)]

use crate::session_header::header_lines;
use crate::session_index::SessionIndex;
use crate::session_shared_full::SessionSharedFull;

/// Counts header renders; delegates lines to TS-truth `header_lines`.
pub struct HeaderFlow {
    pub renders: u64,
}

impl HeaderFlow {
    #[must_use]
    pub fn new() -> Self {
        Self { renders: 0 }
    }

    #[must_use]
    pub fn render(
        &mut self,
        sess: &SessionSharedFull,
        idx: &SessionIndex,
        width: usize,
    ) -> Vec<String> {
        self.renders = self.renders.saturating_add(1);
        header_lines(sess, idx, width)
    }
}

impl Default for HeaderFlow {
    fn default() -> Self {
        Self::new()
    }
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
    fn starts_zero() {
        assert_eq!(HeaderFlow::new().renders, 0);
    }

    #[test]
    fn render_bumps_once() {
        let mut f = HeaderFlow::new();
        let lines = f.render(&sess(), &SessionIndex::new(), 80);
        assert_eq!(f.renders, 1);
        assert_eq!(lines, header_lines(&sess(), &SessionIndex::new(), 80));
    }

    #[test]
    fn render_bumps_each_call() {
        let mut f = HeaderFlow::new();
        f.render(&sess(), &SessionIndex::new(), 80);
        f.render(&sess(), &SessionIndex::new(), 80);
        assert_eq!(f.renders, 2);
    }

    #[test]
    fn render_matches_truth_counts() {
        let mut idx = SessionIndex::new();
        idx.foreground_tasks = 2;
        idx.register_action("a.b".to_string());
        let mut f = HeaderFlow::default();
        let lines = f.render(&sess(), &idx, 80);
        assert_eq!(lines[1], "tasks 2 actions 1");
    }

    #[test]
    fn render_respects_width() {
        let mut f = HeaderFlow::new();
        let lines = f.render(&sess(), &SessionIndex::new(), 4);
        assert!(lines.iter().all(|l| l.chars().count() <= 4));
    }
}

#![forbid(unsafe_code)]
//! Full permission view: gate summary plus hint row.

use crate::perm_gate::PermGate;

/// Full view over [`PermGate`]: summary row plus hint row.
#[derive(Debug, Default)]
pub struct PermView {
    pub gate: PermGate,
}

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

impl PermView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Summary row plus hint row, each width-clipped char-safe, capped at 4 rows.
    pub fn lines(&self, id: &str, width: usize) -> Vec<String> {
        [self.gate.gate_summary(id), String::from("[a]llow [d]eny")]
            .into_iter()
            .map(|s| clip(&s, width))
            .take(4)
            .collect()
    }

    /// Number of views (always one).
    pub fn count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_then_hint() {
        let v = PermView::new();
        let rows = v.lines("bash", 64);
        assert_eq!(
            rows,
            vec!["bash pending".to_string(), "[a]llow [d]eny".to_string()]
        );
    }

    #[test]
    fn width_clips_char_safe() {
        let v = PermView::new();
        let rows = v.lines("héllo⚒tool", 5);
        assert!(rows.iter().all(|r| r.chars().count() <= 5));
        assert_eq!(rows[0], "héllo");
    }

    #[test]
    fn width_zero_empty() {
        let v = PermView::new();
        assert_eq!(v.lines("bash", 0), vec!["".to_string(), "".to_string()]);
    }

    #[test]
    fn caps_four_rows() {
        let v = PermView::new();
        assert!(v.lines(&"x".repeat(200), 128).len() <= 4);
    }

    #[test]
    fn count_is_one() {
        assert_eq!(PermView::new().count(), 1);
    }
}

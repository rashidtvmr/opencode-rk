#![forbid(unsafe_code)]
//! Turn summary rows (TS `run/turn-summary.ts` read-only ref).
//!
//! [`TurnSummary`] collects per-turn rows capped at [`MAX_ROWS`];
//! [`TurnSummary::render`] joins `"[x]"/"[ ]"` lines capped at [`RENDER_CAP`].

/// Max chars per row label.
pub const LABEL_CAP: usize = 128;
/// Max rows per summary.
pub const MAX_ROWS: usize = 32;
/// Byte cap for [`TurnSummary::render`] output.
pub const RENDER_CAP: usize = 2048;

/// One turn row: label (capped) + done flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRow {
    pub label: String,
    pub done: bool,
}

/// Turn summary: ordered rows, capped at [`MAX_ROWS`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnSummary {
    pub rows: Vec<TurnRow>,
}

fn clip_label(label: &str) -> String {
    label.chars().take(LABEL_CAP).collect()
}

fn cap_bytes(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_string();
    }
    let mut end = cap;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl TurnSummary {
    /// Empty summary.
    #[must_use]
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Push row; false when full ([`MAX_ROWS`]).
    pub fn add(&mut self, label: &str) -> bool {
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(TurnRow {
            label: clip_label(label),
            done: false,
        });
        true
    }

    /// Mark row done; false on out-of-bounds index.
    pub fn mark_done(&mut self, idx: usize) -> bool {
        match self.rows.get_mut(idx) {
            Some(row) => {
                row.done = true;
                true
            }
            None => false,
        }
    }

    /// Render `"[x] label"` / `"[ ] label"` lines, capped at [`RENDER_CAP`] bytes.
    #[must_use]
    pub fn render(&self) -> String {
        let joined = self
            .rows
            .iter()
            .map(|r| format!("{} {}", if r.done { "[x]" } else { "[ ]" }, r.label))
            .collect::<Vec<_>>()
            .join("\n");
        cap_bytes(&joined, RENDER_CAP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_caps_rows_at_32() {
        let mut s = TurnSummary::new();
        for i in 0..MAX_ROWS {
            assert!(s.add(&format!("row{i}")), "row {i} must fit");
        }
        assert_eq!(s.rows.len(), MAX_ROWS);
        assert!(!s.add("overflow"));
        assert_eq!(s.rows.len(), MAX_ROWS);
    }

    #[test]
    fn add_clips_label_to_128_chars() {
        let mut s = TurnSummary::new();
        let long = "a".repeat(LABEL_CAP + 50);
        assert!(s.add(&long));
        assert_eq!(s.rows[0].label.chars().count(), LABEL_CAP);
        assert!(!s.rows[0].done);
    }

    #[test]
    fn mark_done_oob_returns_false() {
        let mut s = TurnSummary::new();
        assert!(!s.mark_done(0));
        s.add("a");
        assert!(!s.mark_done(1));
        assert!(!s.mark_done(99));
        assert!(!s.rows[0].done);
    }

    #[test]
    fn render_checks_prefixes() {
        let mut s = TurnSummary::new();
        s.add("todo");
        s.add("done");
        assert!(s.mark_done(1));
        assert_eq!(s.render(), "[ ] todo\n[x] done");
    }

    #[test]
    fn render_empty_is_empty() {
        assert_eq!(TurnSummary::new().render(), String::new());
    }

    #[test]
    fn done_flag_set_by_mark() {
        let mut s = TurnSummary::new();
        s.add("task");
        assert!(!s.rows[0].done);
        assert!(s.mark_done(0));
        assert!(s.rows[0].done);
        assert!(s.render().starts_with("[x] "));
    }

    #[test]
    fn render_caps_at_2kib() {
        let mut s = TurnSummary::new();
        for _ in 0..MAX_ROWS {
            s.add(&"b".repeat(LABEL_CAP));
        }
        let out = s.render();
        assert!(out.len() <= RENDER_CAP, "len {}", out.len());
    }
}

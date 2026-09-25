#![forbid(unsafe_code)]
//! Help dialog key/desc rows (mirrors
//! `packages/tui/src/ui/dialog-help.tsx:6` `DialogHelp` esc/enter close + ok
//! dismiss; rows generalize its bindings list).

/// Max chars per key/desc cell.
pub const MAX_CELL: usize = 64;
/// Max line chars per rendered row.
pub const MAX_LINE: usize = 34;
/// Max rows retained.
pub const MAX_ROWS: usize = 32;

/// Key/desc help rows for the help dialog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelpDialog {
    pub rows: Vec<(String, String)>,
}

impl HelpDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append row; false on blank key or when at cap.
    pub fn add(&mut self, key: &str, desc: &str) -> bool {
        if key.trim().is_empty() || self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push((
            key.chars().take(MAX_CELL).collect(),
            desc.chars().take(MAX_CELL).collect(),
        ));
        true
    }

    /// One line per row (`key  desc`), clipped to `min(width, MAX_LINE)` chars.
    pub fn lines(&self, width: usize) -> Vec<String> {
        let cap = width.min(MAX_LINE);
        self.rows
            .iter()
            .map(|(k, d)| format!("{k}  {d}").chars().take(cap).collect())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_ok() {
        let mut d = HelpDialog::new();
        assert!(d.add("esc", "Close help"));
        assert_eq!(d.rows.len(), 1);
        assert_eq!(d.rows[0], ("esc".to_string(), "Close help".to_string()));
    }

    #[test]
    fn add_blank_key_rejected() {
        let mut d = HelpDialog::new();
        assert!(!d.add("   ", "Close help"));
        assert!(d.rows.is_empty());
    }

    #[test]
    fn add_truncates_cells() {
        let mut d = HelpDialog::new();
        assert!(d.add(&"k".repeat(100), &"d".repeat(100)));
        assert_eq!(d.rows[0].0.chars().count(), MAX_CELL);
        assert_eq!(d.rows[0].1.chars().count(), MAX_CELL);
    }

    #[test]
    fn add_at_cap_rejected() {
        let mut d = HelpDialog::new();
        for i in 0..MAX_ROWS {
            assert!(d.add(&format!("k{i}"), "desc"));
        }
        assert!(!d.add("extra", "desc"));
        assert_eq!(d.rows.len(), MAX_ROWS);
    }

    #[test]
    fn lines_clip_width_and_max() {
        let mut d = HelpDialog::new();
        d.add("return", "Close help dialog now please ok");
        assert_eq!(d.lines(10)[0].chars().count(), 10);
        assert!(d.lines(999)[0].chars().count() <= MAX_LINE);
        assert!(d.lines(10)[0].starts_with("return"));
        assert_eq!(d.lines(0)[0], "");
    }
}

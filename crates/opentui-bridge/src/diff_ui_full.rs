#![forbid(unsafe_code)]
//! Diff header line: current file + hunk cursor.
//!
//! TS truth (`diff-viewer-ui.tsx:1-40`): panel chrome around a
//! focused file/hunk; this ports only the header text.
//!
//! `ponytail:` no hunk-count clamp; add when caller tracks totals.

/// Max chars kept in `file` and in [`DiffUi::line`].
pub const MAX_DIFF_LINE: usize = 512;

/// Focused diff position (file path + hunk index).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffUi {
    pub file: String,
    pub hunk: u32,
}

impl DiffUi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Set file (capped at 512 chars); resets hunk cursor.
    pub fn set_file(&mut self, path: &str) {
        self.file = path.chars().take(MAX_DIFF_LINE).collect();
        self.hunk = 0;
    }
    /// Advance hunk cursor (saturating).
    pub fn next_hunk(&mut self) {
        self.hunk = self.hunk.saturating_add(1);
    }
    /// Header line `"file @@ hunk"`, capped at 512 chars.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} @@ {}", self.file, self.hunk)
            .chars()
            .take(MAX_DIFF_LINE)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_file_caps_and_resets() {
        let mut d = DiffUi {
            file: "a".into(),
            hunk: 3,
        };
        d.set_file(&"x".repeat(600));
        assert_eq!(d.file.len(), 512);
        assert_eq!(d.hunk, 0);
    }
    #[test]
    fn next_hunk_saturates() {
        let mut d = DiffUi {
            file: "a".into(),
            hunk: u32::MAX,
        };
        d.next_hunk();
        assert_eq!(d.hunk, u32::MAX);
    }
    #[test]
    fn line_format_and_cap() {
        let d = DiffUi {
            file: "b.ts".into(),
            hunk: 2,
        };
        assert_eq!(d.line(), "b.ts @@ 2");
        let big = DiffUi {
            file: "y".repeat(600),
            hunk: 1,
        };
        assert!(big.line().chars().count() <= 512);
    }
}

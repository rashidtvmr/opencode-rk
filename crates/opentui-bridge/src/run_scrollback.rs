#![forbid(unsafe_code)]
//! Bounded retained run scrollback.
//!
//! The migration inventory names `RunScrollbackStream` and `entryLook` as
//! the TS compatibility boundary (`BRIDGE_MIGRATION_DETAIL.md:270-274`).
//! This module keeps the retained row contract explicit and dependency-free.

/// Maximum retained rows. Older rows are evicted first.
pub const CAP: usize = 2_000;

/// Alias useful to callers that prefer an explicit bound name.
pub const MAX_ROWS: usize = CAP;

/// A bounded, optionally frozen run transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scrollback {
    pub rows: Vec<String>,
    pub frozen: bool,
}

impl Scrollback {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            frozen: false,
        }
    }

    #[must_use]
    pub const fn cap() -> usize {
        CAP
    }

    /// Append one row, evicting the oldest rows beyond [`CAP`].
    /// Frozen scrollback is immutable.
    pub fn push(&mut self, row: impl AsRef<str>) {
        if self.frozen {
            return;
        }
        self.rows.push(row.as_ref().to_owned());
        if self.rows.len() > Self::cap() {
            let excess = self.rows.len() - Self::cap();
            self.rows.drain(..excess);
        }
    }

    /// Commit a completed row into retained scrollback.
    pub fn commit(&mut self, row: impl AsRef<str>) {
        self.push(row);
    }

    /// Append the standard turn separator.
    pub fn separator(&mut self) {
        self.push("---");
    }

    /// Freeze future writes while preserving the current snapshot.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Return an owned copy of the retained rows.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.rows.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_starts_empty_and_mutable() {
        let s = Scrollback::new();
        assert!(s.rows.is_empty());
        assert!(!s.frozen);
        assert!(s.snapshot().is_empty());
    }

    #[test]
    fn cap_evicts_oldest_rows() {
        let mut s = Scrollback::new();
        for i in 0..=CAP {
            s.push(format!("row-{i}"));
        }
        assert_eq!(s.rows.len(), CAP);
        assert_eq!(s.rows.first().map(String::as_str), Some("row-1"));
        assert_eq!(s.rows.last().map(String::as_str), Some("row-2000"));
    }

    #[test]
    fn separator_is_three_hyphens() {
        let mut s = Scrollback::new();
        s.separator();
        assert_eq!(s.rows, vec!["---"]);
    }

    #[test]
    fn freeze_blocks_all_writes() {
        let mut s = Scrollback::new();
        s.commit("before");
        s.freeze();
        s.push("after");
        s.commit("also-after");
        s.separator();
        assert_eq!(s.rows, vec!["before"]);
        assert!(s.frozen);
    }

    #[test]
    fn snapshot_is_an_independent_clone() {
        let mut s = Scrollback::new();
        s.push("original");
        let mut snapshot = s.snapshot();
        snapshot[0] = "changed".to_owned();
        s.push("newer");
        assert_eq!(s.snapshot(), vec!["original", "newer"]);
        assert_eq!(snapshot, vec!["changed"]);
    }
}

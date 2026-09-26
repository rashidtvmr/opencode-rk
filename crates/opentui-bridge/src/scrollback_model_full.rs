#![forbid(unsafe_code)]
//! Bounded scrollback model: cap 500 rows, 4KiB per row.
use std::collections::VecDeque;
const CAP: usize = 500;
const ROW_CAP: usize = 4 * 1024;
#[derive(Debug, Clone, Default)]
pub struct ScrollModel {
    rows: VecDeque<String>,
}
impl ScrollModel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: VecDeque::with_capacity(CAP),
        }
    }
    pub fn push(&mut self, row: impl AsRef<str>) {
        let truncated = if row.as_ref().len() > ROW_CAP {
            let mut end = ROW_CAP;
            while !row.as_ref().is_char_boundary(end) {
                end -= 1;
            }
            &row.as_ref()[..end]
        } else {
            row.as_ref()
        };
        self.rows.push_back(truncated.to_owned());
        if self.rows.len() > CAP {
            self.rows.pop_front();
        }
    }
    #[must_use]
    pub fn window(&self, n: usize) -> Vec<String> {
        let start = self.rows.len().saturating_sub(n);
        self.rows.iter().skip(start).cloned().collect()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_is_empty() {
        let m = ScrollModel::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }
    #[test]
    fn push_adds_row() {
        let mut m = ScrollModel::new();
        m.push("hello");
        assert_eq!(m.len(), 1);
        assert_eq!(m.window(1), vec!["hello".to_owned()]);
    }
    #[test]
    fn window_returns_last_n() {
        let mut m = ScrollModel::new();
        for i in 0..5 {
            m.push(format!("line{}", i));
        }
        assert_eq!(m.window(2), vec!["line3".to_owned(), "line4".to_owned()]);
    }
    #[test]
    fn push_evicts_oldest_when_full() {
        let mut m = ScrollModel::new();
        for i in 0..CAP + 10 {
            m.push(format!("row{}", i));
        }
        assert_eq!(m.len(), CAP);
        assert!(!m.window(CAP).contains(&"row0".to_owned()));
        assert!(m.window(CAP).contains(&"row10".to_owned()));
    }
}

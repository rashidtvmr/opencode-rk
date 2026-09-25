#![forbid(unsafe_code)]

//! Full session index route state: id + title with char-safe caps.
//!
//! TS truth: `packages/tui/src/routes/session/index.tsx` (session route,
//! id/title header inputs).

/// Max chars for id and title each.
pub const MAX_FIELD: usize = 128;
/// Max chars for rendered header.
pub const MAX_HEADER: usize = 256;

/// Session index route state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessIndex {
    pub id: String,
    pub title: String,
}

impl SessIndex {
    /// Empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set id, truncated to 128 chars.
    pub fn set_id(&mut self, id: &str) {
        self.id = id.chars().take(MAX_FIELD).collect();
    }

    /// Set title, truncated to 128 chars.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.chars().take(MAX_FIELD).collect();
    }

    /// Title when set else id; `title (id)` when both; capped at 256 chars.
    #[must_use]
    pub fn header(&self) -> String {
        let raw = if self.title.is_empty() {
            self.id.clone()
        } else if self.id.is_empty() {
            self.title.clone()
        } else {
            format!("{} ({})", self.title, self.id)
        };
        raw.chars().take(MAX_HEADER).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_header_empty() {
        assert_eq!(SessIndex::new().header(), "");
    }

    #[test]
    fn id_only_header_is_id() {
        let mut s = SessIndex::new();
        s.set_id("abc");
        assert_eq!(s.header(), "abc");
    }

    #[test]
    fn title_preferred_with_id() {
        let mut s = SessIndex::new();
        s.set_id("abc");
        s.set_title("Hi");
        assert_eq!(s.header(), "Hi (abc)");
    }

    #[test]
    fn title_only_header_is_title() {
        let mut s = SessIndex::new();
        s.set_title("Hi");
        assert_eq!(s.header(), "Hi");
    }

    #[test]
    fn fields_cap_128() {
        let mut s = SessIndex::new();
        s.set_id(&"x".repeat(200));
        s.set_title(&"y".repeat(200));
        assert_eq!(s.id.chars().count(), MAX_FIELD);
        assert_eq!(s.title.chars().count(), MAX_FIELD);
        assert!(s.header().chars().count() <= MAX_HEADER);
    }
}

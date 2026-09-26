#![forbid(unsafe_code)]
//! Full TUI state bundle (BRIDGE-PAR-232).
//! Composes router + transcript + draft + tick counter.

use crate::draft_store::DraftStore;
use crate::page_router::PageRouter;
use crate::transcript_store::TranscriptStore;

/// Owned TUI state: page, transcript tail, draft, tick count.
#[derive(Debug, Clone, Default)]
pub struct TuiState {
    pub router: PageRouter,
    pub transcript: TranscriptStore,
    pub draft: DraftStore,
    pub ticks: u32,
}

impl TuiState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance one tick (wrapping).
    pub fn tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
    }

    /// `"page label + N msgs"`, char-capped at 256.
    #[must_use]
    pub fn status(&self) -> String {
        let s = format!("{} ({} msgs)", self.router.label(), self.transcript.len());
        if s.chars().count() <= 256 {
            return s;
        }
        s.chars().take(256).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_adapter::Page;

    #[test]
    fn new_defaults() {
        let s = TuiState::new();
        assert_eq!(s.ticks, 0);
        assert!(s.transcript.is_empty());
        assert_eq!(s.draft.text(), "");
        assert!(s.router.is_chat());
    }

    #[test]
    fn tick_increments() {
        let mut s = TuiState::new();
        s.tick();
        s.tick();
        assert_eq!(s.ticks, 2);
    }

    #[test]
    fn tick_wraps() {
        let mut s = TuiState::new();
        s.ticks = u32::MAX;
        s.tick();
        assert_eq!(s.ticks, 0);
    }

    #[test]
    fn status_reports_page_and_count() {
        let mut s = TuiState::new();
        s.router.show(Page::Help);
        s.transcript.push("a");
        s.transcript.push("b");
        assert_eq!(s.status(), format!("{} (2 msgs)", s.router.label()));
    }

    #[test]
    fn status_empty_zero() {
        assert_eq!(TuiState::new().status(), "chat (0 msgs)");
    }

    #[test]
    fn status_caps_256() {
        let s = TuiState::new();
        assert!(s.status().chars().count() <= 256);
    }
}

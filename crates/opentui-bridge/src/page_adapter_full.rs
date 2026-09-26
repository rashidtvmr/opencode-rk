#![forbid(unsafe_code)]
//! Page visit counter (BRIDGE-PAR-258).
//! Thin wrapper over `page_router::PageRouter`; TS truth is `Page` itself.
//! ponytail: no history vec; upgrade: keep ring of last N pages.

use crate::page_adapter::Page;
use crate::page_router::PageRouter;

/// Router plus saturating visit count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlow {
    pub router: PageRouter,
    pub visits: u64,
}

impl PageFlow {
    #[must_use]
    pub fn new(page: Page) -> Self {
        Self {
            router: PageRouter::new(page),
            visits: 0,
        }
    }

    pub fn show(&mut self, page: Page) {
        self.router.show(page);
        self.visits = self.visits.saturating_add(1);
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        self.router.label()
    }

    #[must_use]
    pub fn visits(&self) -> u64 {
        self.visits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let f = PageFlow::new(Page::Chat);
        assert_eq!(f.visits(), 0);
        assert_eq!(f.label(), "chat");
    }

    #[test]
    fn show_bumps_visits_and_switches() {
        let mut f = PageFlow::new(Page::Chat);
        f.show(Page::Palette);
        assert_eq!(f.visits(), 1);
        assert_eq!(f.router.current(), Page::Palette);
        assert_eq!(f.label(), "palette");
    }

    #[test]
    fn visits_accumulate() {
        let mut f = PageFlow::new(Page::Chat);
        f.show(Page::Help);
        f.show(Page::Context);
        assert_eq!(f.visits(), 2);
        assert_eq!(f.label(), "context");
    }

    #[test]
    fn saturates_at_max() {
        let mut f = PageFlow {
            router: PageRouter::new(Page::Chat),
            visits: u64::MAX,
        };
        f.show(Page::Help);
        assert_eq!(f.visits(), u64::MAX);
        assert_eq!(f.router.current(), Page::Help);
    }
}

#![forbid(unsafe_code)]
//! Page router state (BRIDGE-PAR-196).
//! Thin holder over `page_adapter::Page`; TS truth is `Page` itself.

use crate::page_adapter::{page_label, Page};

/// Current page holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRouter {
    pub page: Page,
}

impl Default for PageRouter {
    fn default() -> Self {
        Self { page: Page::Chat }
    }
}

impl PageRouter {
    /// New router on `page`.
    pub fn new(page: Page) -> Self {
        Self { page }
    }

    /// Switch to `page`.
    pub fn show(&mut self, page: Page) {
        self.page = page;
    }

    /// Current page.
    pub fn current(&self) -> Page {
        self.page
    }

    /// Stable label via `page_label`.
    pub fn label(&self) -> &'static str {
        page_label(self.page)
    }

    /// True only on chat page.
    pub fn is_chat(&self) -> bool {
        self.page == Page::Chat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_holds_page() {
        assert_eq!(PageRouter::new(Page::Help).current(), Page::Help);
    }

    #[test]
    fn show_switches_page() {
        let mut r = PageRouter::new(Page::Chat);
        r.show(Page::Palette);
        assert_eq!(r.current(), Page::Palette);
    }

    #[test]
    fn label_delegates() {
        let r = PageRouter::new(Page::Context);
        assert_eq!(r.label(), page_label(Page::Context));
        assert_eq!(PageRouter::new(Page::Chat).label(), "chat");
    }

    #[test]
    fn is_chat_only_on_chat() {
        assert!(PageRouter::new(Page::Chat).is_chat());
        assert!(!PageRouter::new(Page::Palette).is_chat());
        assert!(!PageRouter::new(Page::Help).is_chat());
    }

    #[test]
    fn default_is_chat() {
        assert_eq!(PageRouter::default().current(), Page::Chat);
    }
}

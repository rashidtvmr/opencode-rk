#![forbid(unsafe_code)]
//! View-counted home footer flow (wraps `home_footer::HomeFooter`).

use crate::home_footer::HomeFooter;

/// Footer plus read counter.
#[derive(Debug, Clone, Default)]
pub struct HomeFootFlow {
    footer: HomeFooter,
    views: u64,
}

impl HomeFootFlow {
    #[must_use]
    pub fn new(footer: HomeFooter) -> Self {
        Self { footer, views: 0 }
    }

    /// Current tip (empty when none); bumps view count.
    pub fn current(&mut self) -> String {
        self.views = self.views.saturating_add(1);
        self.footer.current().unwrap_or_default().to_string()
    }

    #[must_use]
    pub fn views(&self) -> u64 {
        self.views
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> HomeFootFlow {
        HomeFootFlow::new(HomeFooter::new(vec!["a".into(), "b".into()]))
    }

    #[test]
    fn starts_zero() {
        assert_eq!(flow().views(), 0);
    }

    #[test]
    fn current_returns_tip() {
        assert_eq!(flow().current(), "a");
    }

    #[test]
    fn current_bumps_views() {
        let mut f = flow();
        f.current();
        f.current();
        assert_eq!(f.views(), 2);
    }

    #[test]
    fn empty_footer_empty_string() {
        let mut f = HomeFootFlow::new(HomeFooter::new(vec![]));
        assert_eq!(f.current(), "");
        assert_eq!(f.views(), 1);
    }

    #[test]
    fn follows_rotation() {
        let mut f = flow();
        assert_eq!(f.current(), "a");
        f.footer.next();
        assert_eq!(f.current(), "b");
    }
}

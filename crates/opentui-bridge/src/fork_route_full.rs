#![forbid(unsafe_code)]
//! Full fork route list (extends `crate::fork_route::ForkRoute` plan gate
//! with a bounded multi-route picker; host still drives fork RPC).

/// Max number of stored routes.
pub const MAX_ROUTES: usize = 16;
/// Max chars per route id.
pub const MAX_ROUTE_LEN: usize = 128;

/// Bounded route list with an active index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkRouteFull {
    pub routes: Vec<String>,
    pub active: usize,
}

impl ForkRouteFull {
    /// Push `route`; false on blank or when full.
    pub fn add(&mut self, route: &str) -> bool {
        if route.trim().is_empty() || self.routes.len() >= MAX_ROUTES {
            return false;
        }
        self.routes
            .push(route.chars().take(MAX_ROUTE_LEN).collect());
        true
    }

    /// Set active index; false when out of bounds.
    pub fn select(&mut self, idx: usize) -> bool {
        if idx < self.routes.len() {
            self.active = idx;
            true
        } else {
            false
        }
    }

    /// Active route, if any.
    pub fn active_of(&self) -> Option<&str> {
        self.routes.get(self.active).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_ok_and_active() {
        let mut f = ForkRouteFull::default();
        assert!(f.add("ses1"));
        assert_eq!(f.active_of(), Some("ses1"));
    }

    #[test]
    fn add_blank_rejected() {
        let mut f = ForkRouteFull::default();
        assert!(!f.add("   "));
        assert_eq!(f.active_of(), None);
    }

    #[test]
    fn add_caps_at_16_and_truncates() {
        let mut f = ForkRouteFull::default();
        for i in 0..MAX_ROUTES {
            assert!(f.add(&format!("r{i}")));
        }
        assert!(!f.add("overflow"));
        assert_eq!(f.routes.len(), MAX_ROUTES);
        let mut g = ForkRouteFull::default();
        assert!(g.add(&"x".repeat(200)));
        assert_eq!(g.routes[0].chars().count(), MAX_ROUTE_LEN);
    }

    #[test]
    fn select_moves_active() {
        let mut f = ForkRouteFull::default();
        f.add("a");
        f.add("b");
        assert!(f.select(1));
        assert_eq!(f.active_of(), Some("b"));
        assert!(!f.select(9));
        assert_eq!(f.active_of(), Some("b"));
    }
}

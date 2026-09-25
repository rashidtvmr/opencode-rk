#![forbid(unsafe_code)]
//! App root: current route plus readiness gate.
//!
//! Evidence: `packages/tui/src/app.tsx:1-60` composes RouteProvider and
//! gates first draw on startup; `context/route.tsx:37-39` `navigate(route)`.

/// Max route name chars (fail-closed).
pub const MAX_ROUTE: usize = 64;

/// App root state: bounded route name plus ready flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRoot {
    route: String,
    ready: bool,
}

impl AppRoot {
    #[must_use]
    pub fn new() -> Self {
        Self {
            route: String::from("home"),
            ready: false,
        }
    }

    pub fn navigate(&mut self, route: &str) -> bool {
        if route.is_empty() || route.chars().count() > MAX_ROUTE {
            return false;
        }
        self.route = route.to_string();
        true
    }

    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
    }

    #[must_use]
    pub fn route_of(&self) -> &str {
        &self.route
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
    }
}

impl Default for AppRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_home_not_ready() {
        let a = AppRoot::new();
        assert_eq!(a.route_of(), "home");
        assert!(!a.is_ready());
    }

    #[test]
    fn navigate_sets_route() {
        let mut a = AppRoot::new();
        assert!(a.navigate("session"));
        assert_eq!(a.route_of(), "session");
    }

    #[test]
    fn navigate_rejects_empty_and_long() {
        let mut a = AppRoot::new();
        assert!(!a.navigate(""));
        assert!(!a.navigate(&"x".repeat(MAX_ROUTE + 1)));
        assert_eq!(a.route_of(), "home");
    }

    #[test]
    fn set_ready_toggles() {
        let mut a = AppRoot::default();
        a.set_ready(true);
        assert!(a.is_ready());
        a.set_ready(false);
        assert!(!a.is_ready());
    }
}

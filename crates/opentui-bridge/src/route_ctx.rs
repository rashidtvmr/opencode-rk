#![forbid(unsafe_code)]
//! Route nav context (mirrors `packages/tui/src/context/route.tsx`).
//!
//! Evidence: `route.tsx:25-42` single `Route` store + `navigate(route)`;
//! `route.tsx:44-53` fail-closed `initialRoute`; `route.tsx:11-23` variants.
//! This port adds the bounded stack companion (`current` + history).

/// Max route name chars (fail-closed).
pub const MAX_ROUTE: usize = 128;
/// Max history depth.
pub const MAX_DEPTH: usize = 16;

/// Bounded route context: current name plus back-stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCtx {
    current: String,
    stack: Vec<String>,
}

impl RouteCtx {
    fn name_ok(name: &str) -> bool {
        !name.is_empty() && name.chars().count() <= MAX_ROUTE
    }

    #[must_use]
    pub fn new(initial: &str) -> Option<Self> {
        Self::name_ok(initial).then(|| Self {
            current: initial.to_string(),
            stack: Vec::new(),
        })
    }

    #[must_use]
    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn go(&mut self, name: &str) -> bool {
        if !Self::name_ok(name) || self.stack.len() >= MAX_DEPTH {
            return false;
        }
        let prev = std::mem::replace(&mut self.current, name.to_string());
        self.stack.push(prev);
        true
    }

    pub fn back(&mut self) -> bool {
        match self.stack.pop() {
            Some(prev) => {
                self.current = prev;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_pushes_current_and_back_restores() {
        let mut r = RouteCtx::new("home").unwrap();
        assert!(r.go("session"));
        assert_eq!(r.current(), "session");
        assert!(r.back());
        assert_eq!(r.current(), "home");
    }

    #[test]
    fn go_empty_is_false() {
        let mut r = RouteCtx::new("home").unwrap();
        assert!(!r.go(""));
        assert_eq!(r.current(), "home");
    }

    #[test]
    fn back_empty_is_false() {
        let mut r = RouteCtx::new("home").unwrap();
        assert!(!r.back());
        assert_eq!(r.current(), "home");
    }

    #[test]
    fn name_cap_rejected() {
        assert!(RouteCtx::new("").is_none());
        assert!(RouteCtx::new(&"x".repeat(MAX_ROUTE + 1)).is_none());
        let mut r = RouteCtx::new("home").unwrap();
        assert!(!r.go(&"x".repeat(MAX_ROUTE + 1)));
        assert_eq!(r.current(), "home");
    }

    #[test]
    fn depth_cap_rejected() {
        let mut r = RouteCtx::new("home").unwrap();
        for i in 0..MAX_DEPTH {
            assert!(r.go(&format!("p{i}")));
        }
        assert!(!r.go("overflow"));
        assert_eq!(r.stack.len(), MAX_DEPTH);
    }

    #[test]
    fn back_chain_empties_stack() {
        let mut r = RouteCtx::new("home").unwrap();
        assert!(r.go("a"));
        assert!(r.go("b"));
        assert!(r.back());
        assert!(r.back());
        assert!(!r.back());
        assert_eq!(r.current(), "home");
    }
}

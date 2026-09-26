#![forbid(unsafe_code)]
//! Full topic router: known-route set with delivery counter.

/// Max routes / topic chars.
pub const MAX_ROUTES: usize = 32;
/// Max topic chars (mirrors `message_router::MAX_MESSAGE_ID`).
pub const MAX_TOPIC: usize = 64;

/// Known-topic router with wrapping delivery count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterFull {
    routes: Vec<String>,
    delivered: u64,
}

impl RouterFull {
    /// Empty router.
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            delivered: 0,
        }
    }

    fn norm(topic: &str) -> String {
        topic.chars().take(MAX_TOPIC).collect()
    }

    /// Add topic; false on duplicate or when full.
    pub fn add(&mut self, topic: &str) -> bool {
        let t = Self::norm(topic);
        if self.routes.iter().any(|r| r == &t) || self.routes.len() >= MAX_ROUTES {
            return false;
        }
        self.routes.push(t);
        true
    }

    /// Deliver to known topic; counts with wrapping add.
    pub fn route(&mut self, topic: &str) -> bool {
        let t = Self::norm(topic);
        if self.routes.iter().any(|r| r == &t) {
            self.delivered = self.delivered.wrapping_add(1);
            return true;
        }
        false
    }

    /// Total successful deliveries.
    pub fn delivered(&self) -> u64 {
        self.delivered
    }
}

impl Default for RouterFull {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_ok() {
        let mut r = RouterFull::new();
        assert!(r.add("news"));
        assert!(r.route("news"));
        assert_eq!(r.delivered(), 1);
    }

    #[test]
    fn add_dup_false() {
        let mut r = RouterFull::new();
        assert!(r.add("news"));
        assert!(!r.add("news"));
        assert_eq!(r.delivered(), 0);
    }

    #[test]
    fn add_cap_32() {
        let mut r = RouterFull::new();
        for i in 0..MAX_ROUTES {
            assert!(r.add(&format!("t{i}")));
        }
        assert!(!r.add("overflow"));
    }

    #[test]
    fn route_known_counts() {
        let mut r = RouterFull::new();
        r.add("a");
        r.add("b");
        assert!(r.route("a"));
        assert!(r.route("b"));
        assert!(r.route("a"));
        assert_eq!(r.delivered(), 3);
    }

    #[test]
    fn route_unknown_false() {
        let mut r = RouterFull::new();
        r.add("a");
        assert!(!r.route("missing"));
        assert_eq!(r.delivered(), 0);
    }
}

#![forbid(unsafe_code)]
//! Bounded system notification feed (full mirror of TS `notify` fan-out).
//! TS truth `notifications.ts:9-18`: notify() per question/permission/error.
//! Rust keeps last 32 messages, each truncated to 256 chars.
//! `ponytail:` Vec + truncate; add kind/sound struct when caller needs it.

/// Notification list; `push` drops oldest past 32, truncates to 256 chars.
#[derive(Debug, Default, Clone)]
pub struct NotifSys {
    pub items: Vec<String>,
}

impl NotifSys {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    /// Append message; drop oldest when full, truncate to 256 chars.
    pub fn push(&mut self, msg: &str) {
        let t: String = msg.chars().take(256).collect();
        if self.items.len() >= 32 {
            self.items.remove(0);
        }
        self.items.push(t);
    }
    /// Clear all items (TS session/error set reset).
    pub fn clear(&mut self) {
        self.items.clear();
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_and_len() {
        let mut n = NotifSys::new();
        assert!(n.is_empty());
        n.push("a");
        n.push("b");
        assert_eq!(n.len(), 2);
        assert_eq!(n.items, ["a", "b"]);
    }
    #[test]
    fn truncates_to_256_chars() {
        let mut n = NotifSys::new();
        n.push(&"x".repeat(300));
        assert_eq!(n.items[0].chars().count(), 256);
    }
    #[test]
    fn caps_at_32_and_clears() {
        let mut n = NotifSys::new();
        for i in 0..33 {
            n.push(&i.to_string());
        }
        assert_eq!(n.len(), 32);
        assert_eq!(n.items[0], "1");
        n.clear();
        assert_eq!(n.len(), 0);
        assert!(n.is_empty());
    }
}

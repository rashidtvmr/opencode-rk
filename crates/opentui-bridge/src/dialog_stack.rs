#![forbid(unsafe_code)]
//! Dialog id stack (mirrors `packages/tui/src/ui/dialog.tsx`).
//!
//! Source: dialog.tsx:70-76 (`stack` store), :116-118 (`at(-1)` + pop on
//! close), :130-132 (esc close pops top), :141-146 (close-all drains stack),
//! :150-151 (`replace` when stack empty).

/// Max stacked dialogs.
pub const MAX_STACK: usize = 8;
/// Max dialog id chars.
pub const MAX_ID: usize = 64;

/// LIFO stack of open dialog ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogStack {
    stack: Vec<String>,
}

impl DialogStack {
    /// Empty stack.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push id; dup moves to top. False when full or id too long.
    pub fn open(&mut self, id: &str) -> bool {
        if id.len() > MAX_ID {
            return false;
        }
        if let Some(pos) = self.stack.iter().position(|s| s == id) {
            self.stack.remove(pos);
            self.stack.push(id.to_string());
            return true;
        }
        if self.stack.len() >= MAX_STACK {
            return false;
        }
        self.stack.push(id.to_string());
        true
    }

    /// Pop top id.
    pub fn close(&mut self) -> Option<String> {
        self.stack.pop()
    }

    /// Remove id wherever it sits.
    pub fn close_id(&mut self, id: &str) -> bool {
        if let Some(pos) = self.stack.iter().position(|s| s == id) {
            self.stack.remove(pos);
            return true;
        }
        false
    }

    /// Peek top id.
    pub fn top(&self) -> Option<&str> {
        self.stack.last().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_roundtrip() {
        let mut s = DialogStack::new();
        assert!(s.open("a"));
        assert_eq!(s.top(), Some("a"));
        assert_eq!(s.close(), Some("a".to_string()));
        assert_eq!(s.top(), None);
    }

    #[test]
    fn dup_moves_to_top() {
        let mut s = DialogStack::new();
        s.open("a");
        s.open("b");
        assert!(s.open("a"));
        assert_eq!(s.top(), Some("a"));
        assert_eq!(s.close(), Some("a".to_string()));
        assert_eq!(s.top(), Some("b"));
    }

    #[test]
    fn full_returns_false() {
        let mut s = DialogStack::new();
        for i in 0..MAX_STACK {
            assert!(s.open(&format!("d{i}")));
        }
        assert!(!s.open("overflow"));
        assert_eq!(s.top(), Some("d7"));
    }

    #[test]
    fn close_id_missing_false() {
        let mut s = DialogStack::new();
        s.open("a");
        assert!(!s.close_id("zzz"));
        assert!(s.close_id("a"));
        assert_eq!(s.top(), None);
    }

    #[test]
    fn top_none_when_empty() {
        let s = DialogStack::new();
        assert_eq!(s.top(), None);
        let mut s = s;
        assert_eq!(s.close(), None);
    }

    #[test]
    fn id_too_long_rejected() {
        let mut s = DialogStack::new();
        assert!(!s.open(&"x".repeat(MAX_ID + 1)));
        assert_eq!(s.top(), None);
    }
}

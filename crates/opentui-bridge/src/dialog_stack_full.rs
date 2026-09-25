#![forbid(unsafe_code)]
//! Dialog flow pairing [`DialogHost`] with an open-depth counter.
//!
//! Mirrors `packages/tui/src/ui/dialog.tsx` stack depth: `open` bumps on
//! push-ack, `close` drops on pop-ack (saturating).

use crate::dialog_host::DialogHost;

/// Host plus count of acked opens minus acked closes.
#[derive(Debug, Clone, Default)]
pub struct DialogFlow {
    pub host: DialogHost,
    pub depth: u32,
}

impl DialogFlow {
    /// Empty flow.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push id; bumps depth only when host acks.
    pub fn open(&mut self, id: &str) -> bool {
        let ok = self.host.open(id);
        if ok {
            self.depth = self.depth.saturating_add(1);
        }
        ok
    }

    /// Pop top id; drops depth only when something popped.
    pub fn close(&mut self) -> Option<String> {
        let popped = self.host.close_top();
        if popped.is_some() {
            self.depth = self.depth.saturating_sub(1);
        }
        popped
    }

    /// Acked depth.
    pub fn depth(&self) -> u32 {
        self.depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_bumps_depth() {
        let mut f = DialogFlow::new();
        assert!(f.open("a"));
        assert!(f.open("b"));
        assert_eq!(f.depth(), 2);
        assert_eq!(f.host.top(), Some("b"));
    }

    #[test]
    fn close_drops_depth() {
        let mut f = DialogFlow::new();
        f.open("a");
        f.open("b");
        assert_eq!(f.close(), Some("b".to_string()));
        assert_eq!(f.depth(), 1);
        assert_eq!(f.close(), Some("a".to_string()));
        assert_eq!(f.depth(), 0);
    }

    #[test]
    fn failed_open_no_bump() {
        let mut f = DialogFlow::new();
        let long = "x".repeat(65);
        assert!(!f.open(&long));
        assert_eq!(f.depth(), 0);
    }

    #[test]
    fn close_empty_no_underflow() {
        let mut f = DialogFlow::new();
        assert_eq!(f.close(), None);
        assert_eq!(f.depth(), 0);
        f.depth = 0;
        f.close();
        assert_eq!(f.depth(), 0);
    }

    #[test]
    fn dup_open_still_bumps_once() {
        let mut f = DialogFlow::new();
        assert!(f.open("a"));
        assert!(f.open("a"));
        assert_eq!(f.depth(), 2);
        assert_eq!(f.host.top(), Some("a"));
    }
}

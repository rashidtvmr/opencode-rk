#![forbid(unsafe_code)]
//! Queued toast center: single visible slot plus bounded FIFO.
//!
//! TS truth (`toast.tsx:54-56`): one `currentToast` slot; `show` replaces it.
//! This center keeps that single-slot display (`current`) and parks extra
//! `show` calls in a bounded queue (cap 8, oldest evicted) so bursts keep
//! the visible toast. Plain `String` messages, char-boundary-safe 512 cap.
//! std-only.
//!
//! `ponytail:` no timers/duration; caller drives `dismiss`. Add expiry only
//! when a real caller needs it.

/// Visible-slot plus bounded-queue toast center.
#[derive(Debug, Default, Clone)]
pub struct ToastCenter {
    current: Option<String>,
    queue: Vec<String>,
}

impl ToastCenter {
    /// Max chars per message.
    pub const MAX_MSG: usize = 512;
    /// Max waiting (excluding visible) messages.
    pub const MAX_QUEUE: usize = 8;

    #[must_use]
    pub fn new() -> Self {
        Self {
            current: None,
            queue: Vec::new(),
        }
    }

    fn truncate(msg: &str) -> String {
        if msg.chars().count() <= Self::MAX_MSG {
            msg.to_string()
        } else {
            msg.chars().take(Self::MAX_MSG).collect()
        }
    }

    /// Fill empty slot, else queue (evict oldest at cap).
    pub fn show(&mut self, msg: &str) {
        let msg = Self::truncate(msg);
        if self.current.is_none() {
            self.current = Some(msg);
        } else {
            if self.queue.len() >= Self::MAX_QUEUE {
                self.queue.remove(0);
            }
            self.queue.push(msg);
        }
    }

    /// Drop visible toast; promote queue front when present.
    pub fn dismiss(&mut self) {
        if self.queue.is_empty() {
            self.current = None;
        } else {
            self.current = Some(self.queue.remove(0));
        }
    }

    /// Visible message, if any.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_sets_current_when_empty() {
        let mut c = ToastCenter::new();
        c.show("hi");
        assert_eq!(c.current(), Some("hi"));
    }

    #[test]
    fn show_queues_when_busy() {
        let mut c = ToastCenter::new();
        c.show("first");
        c.show("second");
        assert_eq!(c.current(), Some("first"));
        assert_eq!(c.queue, vec!["second".to_string()]);
    }

    #[test]
    fn overflow_evicts_oldest() {
        let mut c = ToastCenter::new();
        c.show("visible");
        for i in 0..9 {
            c.show(&format!("q{i}"));
        }
        assert_eq!(c.queue.len(), ToastCenter::MAX_QUEUE);
        assert_eq!(c.queue[0], "q1");
        assert_eq!(c.queue[7], "q8");
    }

    #[test]
    fn dismiss_promotes_front() {
        let mut c = ToastCenter::new();
        c.show("a");
        c.show("b");
        c.show("c");
        c.dismiss();
        assert_eq!(c.current(), Some("b"));
        c.dismiss();
        assert_eq!(c.current(), Some("c"));
    }

    #[test]
    fn dismiss_empty_yields_none() {
        let mut c = ToastCenter::new();
        c.dismiss();
        assert!(c.current().is_none());
        c.show("x");
        c.dismiss();
        assert!(c.current().is_none());
    }

    #[test]
    fn truncates_to_512_chars() {
        let mut c = ToastCenter::new();
        let long = "y".repeat(600);
        c.show(&long);
        assert_eq!(c.current().unwrap().chars().count(), 512);
        let uni = "e".repeat(600);
        c.show(&uni);
        c.dismiss();
        assert_eq!(c.current().unwrap().chars().count(), 512);
    }
}

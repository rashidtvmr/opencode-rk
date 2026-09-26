//! Serial prompt queue with reuse-pending slot.
//!
//! Mirrors `runtime.queue.ts` drain order: FIFO via [`RunQueue::take`],
//! interrupted prompt stashed in `pending` requeued to front via
//! [`RunQueue::reuse_pending`].
// ponytail: Vec front-remove O(n), fine at cap 64; switch to VecDeque when cap grows.

/// Bounded FIFO prompt queue (cap 64) plus one reuse-pending slot.
#[forbid(unsafe_code)]
pub struct RunQueue {
    pub items: Vec<String>,
    pub pending: Option<String>,
}

impl RunQueue {
    pub const CAPACITY: usize = 64;
    pub const MAX_CHARS: usize = 4096;

    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            pending: None,
        }
    }

    /// Push back. False when text exceeds 4KiB chars or queue full.
    pub fn push(&mut self, text: String) -> bool {
        if text.chars().count() > Self::MAX_CHARS {
            return false;
        }
        if self.items.len() >= Self::CAPACITY {
            return false;
        }
        self.items.push(text);
        true
    }

    /// Move pending to front. False when none or queue full (pending kept).
    pub fn reuse_pending(&mut self) -> bool {
        let Some(text) = self.pending.take() else {
            return false;
        };
        if self.items.len() >= Self::CAPACITY {
            self.pending = Some(text);
            return false;
        }
        self.items.insert(0, text);
        true
    }

    /// Pop front FIFO.
    pub fn take(&mut self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut q = RunQueue::new();
        assert!(q.push("hello".to_string()));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn push_overlong_false() {
        let mut q = RunQueue::new();
        let big = "a".repeat(RunQueue::MAX_CHARS + 1);
        assert!(!q.push(big));
        assert!(q.is_empty());
    }

    #[test]
    fn take_fifo() {
        let mut q = RunQueue::new();
        q.push("a".to_string());
        q.push("b".to_string());
        assert_eq!(q.take().as_deref(), Some("a"));
        assert_eq!(q.take().as_deref(), Some("b"));
        assert_eq!(q.take(), None);
    }

    #[test]
    fn reuse_moves_front() {
        let mut q = RunQueue::new();
        q.push("b".to_string());
        q.pending = Some("a".to_string());
        assert!(q.reuse_pending());
        assert_eq!(q.pending, None);
        assert_eq!(q.take().as_deref(), Some("a"));
        assert_eq!(q.take().as_deref(), Some("b"));
    }

    #[test]
    fn reuse_none_false() {
        let mut q = RunQueue::new();
        assert!(!q.reuse_pending());
    }

    #[test]
    fn reuse_full_false_keeps_pending() {
        let mut q = RunQueue::new();
        for i in 0..RunQueue::CAPACITY {
            assert!(q.push(i.to_string()));
        }
        q.pending = Some("p".to_string());
        assert!(!q.reuse_pending());
        assert_eq!(q.pending.as_deref(), Some("p"));
        assert_eq!(q.len(), RunQueue::CAPACITY);
    }

    #[test]
    fn push_full_false() {
        let mut q = RunQueue::new();
        for i in 0..RunQueue::CAPACITY {
            assert!(q.push(i.to_string()));
        }
        assert!(!q.push("overflow".to_string()));
    }
}

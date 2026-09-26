#![forbid(unsafe_code)]

//! `createEffect` queue: schedule named effects, drain one batch per flush.
//!
//! Models the SolidJS `createEffect` idiom: effects scheduled during a
//! reactive pass run once at flush; cleanup is the next flush dropping
//! the prior output. Tag bookkeeping only; std-only, no JS runtime.

/// Max queued effects per flush batch.
pub const MAX_EFFECTS: usize = 16;
/// Max chars kept per effect tag.
pub const MAX_EFFECT_LEN: usize = 256;

/// Queued effect tags plus total run count.
#[derive(Debug, Clone, Default)]
pub struct EffectQueue {
    /// Pending effect tags (capped at [`MAX_EFFECTS`]).
    pub pending: Vec<String>,
    /// Total effects run (wrapping).
    pub ran: u32,
}

fn cap_tag(tag: &str) -> String {
    tag.chars().take(MAX_EFFECT_LEN).collect()
}

impl EffectQueue {
    /// Empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule one effect tag. False when full.
    pub fn schedule(&mut self, tag: &str) -> bool {
        if self.pending.len() >= MAX_EFFECTS {
            return false;
        }
        self.pending.push(cap_tag(tag));
        true
    }

    /// Drain pending tags; adds drained count via wrapping add.
    pub fn run_all(&mut self) -> Vec<String> {
        let out: Vec<String> = self.pending.drain(..).collect();
        self.ran = self.ran.wrapping_add(out.len() as u32);
        out
    }

    /// Total effects run so far.
    #[must_use]
    pub fn ran(&self) -> u32 {
        self.ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_then_run_drains() {
        let mut q = EffectQueue::new();
        assert!(q.schedule("a"));
        assert!(q.schedule("b"));
        assert_eq!(q.run_all(), vec!["a".to_string(), "b".to_string()]);
        assert!(q.pending.is_empty());
        assert_eq!(q.ran(), 2);
    }

    #[test]
    fn full_queue_rejects() {
        let mut q = EffectQueue::new();
        for i in 0..MAX_EFFECTS {
            assert!(q.schedule(&format!("e{i}")));
        }
        assert!(!q.schedule("overflow"));
        assert_eq!(q.pending.len(), MAX_EFFECTS);
    }

    #[test]
    fn tag_truncated_to_cap() {
        let mut q = EffectQueue::new();
        let long = "x".repeat(MAX_EFFECT_LEN + 10);
        assert!(q.schedule(&long));
        assert_eq!(q.pending[0].chars().count(), MAX_EFFECT_LEN);
    }

    #[test]
    fn ran_counts_across_flushes() {
        let mut q = EffectQueue::new();
        q.schedule("a");
        q.run_all();
        q.schedule("b");
        q.schedule("c");
        q.run_all();
        assert_eq!(q.ran(), 3);
    }

    #[test]
    fn ran_wraps_on_overflow() {
        let mut q = EffectQueue::new();
        q.ran = u32::MAX;
        q.schedule("a");
        q.run_all();
        assert_eq!(q.ran(), 0);
    }
}

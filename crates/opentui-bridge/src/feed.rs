#![forbid(unsafe_code)]
//! Bounded span-feed queue discipline.
//!
//! Mirrors `packages/core/src/NativeSpanFeed.ts` queue semantics without FFI:
//! `attach` = register+attach handshake (fails closed natively; here always
//! live), `drain_once` = one FIFO pop with copy-before-ack, `detach` clears
//! all state like `finalizeDestroy`. Async pinning (`pending_async`/`pinned`)
//! models TS `pendingAsyncHandlers`/`hasPinnedChunks` for `is_backpressured`
//! and `is_idle`. Slow consumers evict oldest; bounds `MAX_SPANS` (256-slot
//! drain buffer mirror) and `MAX_BYTES` (256 KiB) always hold.

use std::collections::VecDeque;

/// Max queued spans (mirrors TS 256-slot drain buffer).
pub const MAX_SPANS: usize = 256;
/// Max queued bytes (256 KiB cap).
pub const MAX_BYTES: usize = 256 * 1024;

/// Push/drain failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    /// Feed detached; push/drain refused.
    Detached,
    /// Single span exceeds [`MAX_BYTES`].
    TooLarge,
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detached => write!(f, "span feed detached"),
            Self::TooLarge => write!(f, "span exceeds byte cap"),
        }
    }
}

impl std::error::Error for FeedError {}

/// Bounded FIFO of owned frame-byte spans with slow-consumer eviction.
#[derive(Debug, Default)]
pub struct SpanFeed {
    queue: VecDeque<Vec<u8>>,
    bytes: usize,
    attached: bool,
    pending_async: usize,
    pinned: usize,
    evicted: u64,
}

impl SpanFeed {
    /// Attach a live feed (register+attach handshake; fails closed natively).
    pub fn attach() -> Self {
        Self {
            attached: true,
            ..Self::default()
        }
    }

    /// Detach and drop all queued/pinned state (mirrors `finalizeDestroy`).
    pub fn detach(&mut self) {
        self.queue.clear();
        self.bytes = 0;
        self.attached = false;
        self.pending_async = 0;
        self.pinned = 0;
    }

    /// True while attached.
    pub fn is_attached(&self) -> bool {
        self.attached
    }

    /// Queue one span; empty spans are no-ops (mirrors `len == 0` skip).
    /// Slow-consumer eviction drops oldest while at cap; a single span over
    /// [`MAX_BYTES`] is rejected [`FeedError::TooLarge`].
    pub fn push(&mut self, span: &[u8]) -> Result<(), FeedError> {
        if !self.attached {
            return Err(FeedError::Detached);
        }
        if span.is_empty() {
            return Ok(());
        }
        if span.len() > MAX_BYTES {
            return Err(FeedError::TooLarge);
        }
        while self.queue.len() >= MAX_SPANS || self.bytes + span.len() > MAX_BYTES {
            if self.queue.pop_front().is_none() {
                break;
            }
            self.evicted += 1;
            self.bytes = self.queue.iter().map(Vec::len).sum();
        }
        self.bytes += span.len();
        self.queue.push_back(span.to_vec());
        Ok(())
    }

    /// Pop the oldest span; the returned bytes are copied before the queue
    /// entry is released (copy-before-ack), so the caller owns its copy.
    pub fn drain_once(&mut self) -> Option<Vec<u8>> {
        if !self.attached || self.queue.is_empty() {
            return None;
        }
        let owned = self.queue.front()?.clone();
        self.queue.pop_front();
        self.bytes -= owned.len();
        Some(owned)
    }

    /// Backpressured while chunks are pinned, async handlers are pending,
    /// or either bound is reached (mirrors TS `isBackpressured`).
    pub fn is_backpressured(&self) -> bool {
        self.pending_async > 0
            || self.pinned > 0
            || self.queue.len() >= MAX_SPANS
            || self.bytes >= MAX_BYTES
    }

    /// Idle when attached, drained, and nothing is pinned or pending
    /// (mirrors TS `idle()` predicate).
    pub fn is_idle(&self) -> bool {
        self.attached && self.queue.is_empty() && self.pending_async == 0 && self.pinned == 0
    }

    /// Mark one async handler outstanding (pins its chunk).
    pub fn pin(&mut self) {
        self.pending_async += 1;
        self.pinned += 1;
    }

    /// Settle one async handler (releases its chunk refcount).
    pub fn unpin(&mut self) {
        self.pending_async = self.pending_async.saturating_sub(1);
        self.pinned = self.pinned.saturating_sub(1);
    }

    /// Queued span count.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True when no spans are queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Queued byte total (always `<= MAX_BYTES`).
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Total spans evicted by slow-consumer eviction.
    pub fn evicted(&self) -> u64 {
        self.evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_live_then_detach_clears() {
        let mut feed = SpanFeed::attach();
        assert!(feed.is_attached());
        assert!(feed.is_idle());
        feed.push(b"frame").unwrap();
        assert_eq!(feed.len(), 1);
        feed.detach();
        assert!(!feed.is_attached());
        assert!(feed.is_empty());
        assert_eq!(feed.bytes(), 0);
        assert_eq!(feed.push(b"x"), Err(FeedError::Detached));
        assert_eq!(feed.drain_once(), None);
    }

    #[test]
    fn drain_once_fifo_order() {
        let mut feed = SpanFeed::attach();
        feed.push(b"a").unwrap();
        feed.push(b"b").unwrap();
        feed.push(b"c").unwrap();
        assert_eq!(feed.drain_once(), Some(b"a".to_vec()));
        assert_eq!(feed.drain_once(), Some(b"b".to_vec()));
        assert_eq!(feed.drain_once(), Some(b"c".to_vec()));
        assert_eq!(feed.drain_once(), None);
        assert!(feed.is_idle());
    }

    #[test]
    fn backpressure_at_span_cap() {
        let mut feed = SpanFeed::attach();
        assert!(!feed.is_backpressured());
        for i in 0..MAX_SPANS {
            feed.push(&[i as u8]).unwrap();
        }
        assert!(feed.is_backpressured());
        assert_eq!(feed.len(), MAX_SPANS);
    }

    #[test]
    fn slow_consumer_eviction_bounded() {
        let mut feed = SpanFeed::attach();
        for i in 0..(MAX_SPANS + 5) {
            feed.push(&[(i % 251) as u8]).unwrap();
        }
        assert_eq!(feed.len(), MAX_SPANS);
        assert!(feed.bytes() <= MAX_BYTES);
        assert_eq!(feed.evicted(), 5);
        // Oldest evicted: front holds the 6th span pushed.
        assert_eq!(feed.drain_once(), Some(vec![5u8]));
    }

    #[test]
    fn idle_signal_transitions() {
        let mut feed = SpanFeed::attach();
        assert!(feed.is_idle());
        feed.push(b"x").unwrap();
        assert!(!feed.is_idle());
        feed.pin();
        feed.drain_once();
        assert!(!feed.is_idle());
        feed.unpin();
        assert!(feed.is_idle());
    }

    #[test]
    fn copy_before_ack_returns_owned_bytes() {
        let mut feed = SpanFeed::attach();
        feed.push(b"abc").unwrap();
        let out = feed.drain_once().expect("span");
        assert_eq!(out, b"abc");
        out.iter().for_each(|_| {});
        assert!(feed.is_empty());
        assert_eq!(feed.bytes(), 0);
        // Mutating the returned copy cannot affect later drains.
        let mut feed2 = SpanFeed::attach();
        feed2.push(b"zz").unwrap();
        let mut first = feed2.drain_once().expect("span");
        first[0] = b'q';
        feed2.push(b"zz").unwrap();
        assert_eq!(feed2.drain_once(), Some(b"zz".to_vec()));
    }

    #[test]
    fn oversize_span_rejected() {
        let mut feed = SpanFeed::attach();
        let big = vec![0u8; MAX_BYTES + 1];
        assert_eq!(feed.push(&big), Err(FeedError::TooLarge));
        assert!(feed.is_empty());
        assert!(feed.is_idle());
    }
}

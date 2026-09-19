#![forbid(unsafe_code)]
//! Bounded coalesced event bus (pure, no FFI, no threads).
//!
//! `push` drops oldest on overflow. Consecutive `Tick`s merge into one.

use std::collections::VecDeque;

/// Max queued events. Oldest dropped past this.
pub const MAX_EVENTS: usize = 256;

/// Coalescable event kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Paint,
    Input,
    Resize,
    Tick,
}

/// Queued event with monotonic sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusEvent {
    pub kind: EventKind,
    pub seq: u64,
}

/// Bounded FIFO with tick coalescing.
#[derive(Debug, Clone)]
pub struct EventBus {
    queue: VecDeque<BusEvent>,
    cap: usize,
    next_seq: u64,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            cap: MAX_EVENTS,
            next_seq: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Push one event. Merges if `kind` is `Tick` and back is `Tick`
    /// (keeps original `seq`). Drops oldest when at `cap`.
    pub fn push(&mut self, kind: EventKind) {
        if kind == EventKind::Tick && matches!(self.queue.back(), Some(e) if e.kind == EventKind::Tick) {
            return;
        }
        let event = BusEvent {
            kind,
            seq: self.next_seq,
        };
        self.next_seq += 1;
        if self.queue.len() >= self.cap {
            self.queue.pop_front();
        }
        self.queue.push_back(event);
    }

    /// Drain all queued events in FIFO order.
    pub fn drain(&mut self) -> Vec<BusEvent> {
        self.queue.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_drops_oldest() {
        let mut bus = EventBus::new();
        for _ in 0..MAX_EVENTS {
            bus.push(EventKind::Input);
        }
        assert_eq!(bus.len(), MAX_EVENTS);
        let first_seq = bus.drain().into_iter().next().unwrap().seq;

        let mut bus = EventBus::new();
        for _ in 0..MAX_EVENTS {
            bus.push(EventKind::Input);
        }
        bus.push(EventKind::Paint);
        assert_eq!(bus.len(), MAX_EVENTS);
        let events = bus.drain();
        assert_eq!(events.len(), MAX_EVENTS);
        // Oldest (first_seq) evicted, newest Paint kept at back.
        assert_eq!(events[0].seq, first_seq + 1);
        assert_eq!(events.last().unwrap().kind, EventKind::Paint);
        assert!(events.is_empty() || bus.is_empty());
    }

    #[test]
    fn tick_coalesce() {
        let mut bus = EventBus::new();
        bus.push(EventKind::Tick);
        bus.push(EventKind::Tick);
        bus.push(EventKind::Tick);
        assert_eq!(bus.len(), 1);
        bus.push(EventKind::Input);
        bus.push(EventKind::Tick);
        bus.push(EventKind::Tick);
        assert_eq!(bus.len(), 3);
        let events = bus.drain();
        assert_eq!(
            events.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [EventKind::Tick, EventKind::Input, EventKind::Tick]
        );
    }

    #[test]
    fn seq_monotonic() {
        let mut bus = EventBus::new();
        bus.push(EventKind::Paint);
        bus.push(EventKind::Tick);
        bus.push(EventKind::Tick); // coalesced, no seq consumed
        bus.push(EventKind::Resize);
        let events = bus.drain();
        assert!(!events.is_empty());
        for w in events.windows(2) {
            assert!(w[0].seq < w[1].seq);
        }
        // Next push continues monotonically after drain.
        bus.push(EventKind::Input);
        let next = bus.drain().pop().unwrap();
        assert!(next.seq > events.last().unwrap().seq);
    }
}

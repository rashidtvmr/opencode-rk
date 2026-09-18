//! APP-007: event replay cursor types for shared daemon state recovery.
//!
//! Caller-owned replay window: subscribers reconnect with a [`Cursor`]
//! (`seq` of the last delivered event plus its chain [`digest_of`]) and
//! receive each durable event exactly once. A cursor behind retention, ahead
//! of the head, or with a digest mismatch fails closed with
//! [`CursorError::ResyncRequired`] so the caller takes a bounded snapshot
//! instead of replaying a gapped prefix. Zero `seq` with nonzero digest is
//! [`CursorError::BadCursor`].
//!
//! Replay durability follows `app_protocols::classify` at HEAD 5af7884: the
//! eight lifecycle/history types replay, everything else — including unknown
//! future types — is [`Durability::Ephemeral`] and never replays as durable
//! state, so a projector rebuild can never invent messages from deltas.
//!
//! Bounds: at most [`REPLAY_BUFFER_CAP`] (512) stored events; overflow evicts
//! the oldest (no global queue, no cross-subscriber blocking). A slow client
//! whose pending count reaches the cap is disconnected via
//! [`slow_client_should_disconnect`]. No I/O, no clock, no threads, no
//! globals, no logging. The caller owns sockets and snapshots.
//!
//! Digest note (`ponytail:`): [`digest_of`] is FNV-1a 64, a non-cryptographic
//! chain-integrity check only. Ceiling: same-process/adjacent-task replay.
//! Upgrade path: swap in a cryptographic hash when cursors cross trust
//! boundaries.

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;

/// Maximum retained events in one replay window.
pub const REPLAY_BUFFER_CAP: usize = 512;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x1000_0000_01b3;

/// Deterministic chain digest over `seq` plus the event-type bytes.
#[must_use]
pub fn digest_of(seq: u64, event_type: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in seq.to_le_bytes().iter().chain(event_type.as_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Replay position: `seq` is the last delivered sequence (0 = genesis,
/// nothing delivered yet), `digest` is [`digest_of`] that event (0 at genesis).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cursor {
    seq: u64,
    digest: u64,
}

impl Cursor {
    /// Genesis cursor: nothing delivered yet.
    #[must_use]
    pub const fn genesis() -> Self {
        Self { seq: 0, digest: 0 }
    }

    /// Wrap a raw position. Validity is checked by [`ReplayBuffer::replay`].
    #[must_use]
    pub const fn new(seq: u64, digest: u64) -> Self {
        Self { seq, digest }
    }

    /// Position after delivering `event`.
    #[must_use]
    pub fn after(event: &StoredEvent) -> Self {
        Self {
            seq: event.seq,
            digest: event.digest,
        }
    }

    /// Last delivered sequence (0 at genesis).
    #[must_use]
    pub const fn seq(self) -> u64 {
        self.seq
    }

    /// Chain digest of the last delivered event (0 at genesis).
    #[must_use]
    pub const fn digest(self) -> u64 {
        self.digest
    }
}

/// Replay failures. No payload carried, so output is secret-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorError {
    /// Malformed cursor (zero `seq` with nonzero digest).
    BadCursor,
    /// Cursor behind retention, ahead of the head, or digest-mismatched;
    /// the subscriber must take a fresh snapshot instead of replaying.
    ResyncRequired,
}

impl fmt::Display for CursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadCursor => write!(f, "bad cursor"),
            Self::ResyncRequired => write!(f, "resync required"),
        }
    }
}

impl std::error::Error for CursorError {}

/// Replay durability of one event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Durability {
    /// Rebuilds durable state on replay (lifecycle, history, completed
    /// tool outcomes, permission resolutions).
    Durable,
    /// Never replays as durable state (streaming deltas, progress,
    /// presence, transient prompts, keepalives, anything unknown).
    Ephemeral,
}

impl Durability {
    /// True for [`Durability::Durable`].
    #[must_use]
    pub const fn is_durable(self) -> bool {
        matches!(self, Self::Durable)
    }
}

/// Classify an event type for replay. Total function: the eight durable
/// lifecycle/history types replay, everything else is ephemeral.
#[must_use]
pub fn classify(event_type: &str) -> Durability {
    match event_type {
        "session.created" | "session.renamed" | "session.archived" | "message.appended"
        | "message.compacted" | "tool.completed" | "permission.granted"
        | "permission.denied" => Durability::Durable,
        _ => Durability::Ephemeral,
    }
}

/// One retained event. `seq` starts at 1, never zero, never reused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEvent {
    /// 1-based sequence.
    pub seq: u64,
    /// Event-type token.
    pub event_type: String,
    /// [`digest_of`] (`seq`, `event_type`).
    pub digest: u64,
}

impl StoredEvent {
    /// Replay durability of this event.
    #[must_use]
    pub fn durability(&self) -> Durability {
        classify(&self.event_type)
    }
}

/// Bounded caller-owned replay window (FIFO, cap [`REPLAY_BUFFER_CAP`]).
#[derive(Clone, Debug, Default)]
pub struct ReplayBuffer {
    events: VecDeque<StoredEvent>,
    next_seq: u64,
}

impl ReplayBuffer {
    /// Empty window; the first pushed event gets `seq` 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            next_seq: 1,
        }
    }

    /// Append one event of `event_type`, evicting the oldest while at the
    /// cap. Returns the stored event (caller derives its cursor from it).
    pub fn push(&mut self, event_type: &str) -> StoredEvent {
        while self.events.len() >= REPLAY_BUFFER_CAP.max(1) {
            self.events.pop_front();
        }
        let stored = StoredEvent {
            seq: self.next_seq,
            event_type: event_type.to_string(),
            digest: digest_of(self.next_seq, event_type),
        };
        self.next_seq = self.next_seq.saturating_add(1).max(1);
        self.events.push_back(stored.clone());
        stored
    }

    /// Stored event count (<= [`REPLAY_BUFFER_CAP`]).
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True when nothing is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Last stored sequence (0 when empty).
    #[must_use]
    pub fn head_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }

    /// Oldest retained sequence (`next_seq` when empty).
    #[must_use]
    pub fn retained_from(&self) -> u64 {
        self.events.front().map_or(self.next_seq, |e| e.seq)
    }

    /// Replay durable events after `cursor`, each exactly once, in `seq`
    /// order. Ephemeral events are skipped, never promoted to durable.
    pub fn replay(&self, cursor: &Cursor) -> Result<Vec<StoredEvent>, CursorError> {
        if cursor.seq == 0 {
            if cursor.digest != 0 {
                return Err(CursorError::BadCursor);
            }
        } else {
            let expected = self.events.iter().find(|e| e.seq == cursor.seq);
            match expected {
                Some(event) if event.digest == cursor.digest => {}
                _ => return Err(CursorError::ResyncRequired),
            }
        }
        let mut out = Vec::new();
        for event in &self.events {
            if event.seq > cursor.seq && event.durability().is_durable() {
                out.push(event.clone());
            }
        }
        Ok(out)
    }
}

/// Slow-client disconnect check: a client with `pending` unflushed events at
/// the replay cap must be disconnected (resync on reconnect) rather than
/// blocking other subscribers or growing an unbounded queue.
#[must_use]
pub const fn slow_client_should_disconnect(pending: usize) -> bool {
    pending >= REPLAY_BUFFER_CAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_replay_delivers_each_durable_event_once() {
        let mut log = ReplayBuffer::new();
        let mut cursor = Cursor::genesis();
        for _ in 0..3 {
            log.push("message.appended");
        }
        let replayed = log.replay(&cursor).unwrap();
        assert_eq!(replayed.len(), 3);
        assert_eq!(replayed[0].seq, 1);
        assert_eq!(replayed[2].seq, 3);
        cursor = Cursor::after(replayed.last().unwrap());
        let again = log.replay(&cursor).unwrap();
        assert!(again.is_empty(), "replay after catch-up must not duplicate");
    }

    #[test]
    fn expired_cursor_triggers_resync() {
        let mut log = ReplayBuffer::new();
        let first = log.push("message.appended");
        for _ in 1..=REPLAY_BUFFER_CAP {
            log.push("message.appended");
        }
        assert_eq!(log.len(), REPLAY_BUFFER_CAP);
        let stale = Cursor::after(&first);
        assert_eq!(log.replay(&stale), Err(CursorError::ResyncRequired));
    }

    #[test]
    fn ephemeral_events_never_replay_as_durable() {
        assert_eq!(classify("token.delta"), Durability::Ephemeral);
        assert_eq!(classify("presence.update"), Durability::Ephemeral);
        assert_eq!(classify("future.unknown"), Durability::Ephemeral);
        assert_eq!(classify("message.appended"), Durability::Durable);
        let mut log = ReplayBuffer::new();
        log.push("token.delta");
        let durable = log.push("message.appended");
        log.push("assistant.delta");
        let replayed = log.replay(&Cursor::genesis()).unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0], durable);
        assert!(replayed.iter().all(|e| e.durability().is_durable()));
    }

    #[test]
    fn slow_client_disconnect_at_bound() {
        assert!(!slow_client_should_disconnect(REPLAY_BUFFER_CAP - 1));
        assert!(slow_client_should_disconnect(REPLAY_BUFFER_CAP));
        assert!(slow_client_should_disconnect(REPLAY_BUFFER_CAP + 1));
    }

    #[test]
    fn malformed_cursor_is_bad_cursor() {
        let log = ReplayBuffer::new();
        assert_eq!(
            log.replay(&Cursor::new(0, 7)),
            Err(CursorError::BadCursor)
        );
    }
}

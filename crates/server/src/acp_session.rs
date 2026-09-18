//! DISC-112: live ACP session wiring against the bridge with daemon events.
//!
//! The [`acp_bridge`](crate::acp_bridge) codec is pure: it decodes frames and
//! mutates a caller-owned map but emits no events, owns no transport, and
//! knows nothing of cancellation or restart. This module is the live seam:
//! [`LiveBridge`] drives start/message/cancel/status round-trips, publishes
//! one [`DaemonEvent`] per state transition into a bounded in-file event bus
//! ([`LiveBus`], mirroring `event_bus` slow-consumer semantics), and
//! recovers truthfully across restarts from a caller-provided snapshot.
//!
//! Authority boundary: this file is std-only and self-contained (no
//! `use crate::`, no I/O inside the bridge itself; the caller moves snapshot
//! bytes). It compiles both standalone (`rustc --test`) and as a future
//! `pub mod acp_session` of the server crate.
//!
//! Bounds: [`MAX_LIVE_SESSIONS`] live sessions, [`MAX_QUEUE_PER_SESSION`]
//! queued inbound messages each, [`MAX_EVENTS_PER_SUBSCRIBER`] buffered
//! events per bus subscriber. A slow subscriber is disconnected on overflow
//! (resync via [`LiveBridge::resync`]); other subscribers are unaffected.
//! Errors carry codes only, never session ids, tokens, or message text.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};

/// Largest live session count; further starts fail with [`LiveError::TooManySessions`].
pub const MAX_LIVE_SESSIONS: usize = 64;
/// Largest queued inbound message count per session.
pub const MAX_QUEUE_PER_SESSION: usize = 32;
/// Largest buffered event count per bus subscriber before slow-disconnect.
pub const MAX_EVENTS_PER_SUBSCRIBER: usize = 64;
/// Largest accepted message text in chars.
pub const MAX_MESSAGE_CHARS: usize = 65_536;
/// Largest accepted session id in chars (mirrors the codec bound).
pub const MAX_SESSION_ID_CHARS: usize = 128;

/// Typed live-session failure. Display carries the code only, never ids,
/// tokens, or content, so unauthorized probes learn nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveErrorCode {
    TooManySessions,
    UnknownSession,
    Unauthorized,
    InvalidInput,
    QueueFull,
    AlreadyCancelled,
    SlowConsumer,
    Closed,
}

impl LiveErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::TooManySessions => "too_many_sessions",
            Self::UnknownSession => "unknown_session",
            Self::Unauthorized => "unauthorized",
            Self::InvalidInput => "invalid_input",
            Self::QueueFull => "queue_full",
            Self::AlreadyCancelled => "already_cancelled",
            Self::SlowConsumer => "slow_consumer",
            Self::Closed => "closed",
        }
    }
}

/// Live failure; carries a code only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveError {
    pub code: LiveErrorCode,
}

impl LiveError {
    fn new(code: LiveErrorCode) -> Self {
        Self { code }
    }
}

impl std::fmt::Display for LiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "acp session error: {}", self.code.as_str())
    }
}

impl std::error::Error for LiveError {}

/// Lifecycle of one live session. There is deliberately no `Completed`
/// variant: a restart may only recover [`Status::Recovered`], never fabricate
/// a finished response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Cancelled,
    Recovered,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Cancelled => "cancelled",
            Self::Recovered => "recovered",
        }
    }
}

/// One daemon-visible event per state transition. `session` echoes the live
/// id so consumers can match stream entries to sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    SessionStarted { session: String },
    MessageQueued { session: String, seq: u64 },
    MessageDelivered { session: String, seq: u64 },
    SessionCancelled { session: String },
    SessionReclaimed { session: String },
    SessionRecovered { session: String },
}

impl DaemonEvent {
    /// Session id this event belongs to.
    #[must_use]
    pub fn session(&self) -> &str {
        match self {
            Self::SessionStarted { session }
            | Self::MessageQueued { session, .. }
            | Self::MessageDelivered { session, .. }
            | Self::SessionCancelled { session }
            | Self::SessionReclaimed { session }
            | Self::SessionRecovered { session } => session,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::SessionStarted { .. } => "started",
            Self::MessageQueued { .. } => "queued",
            Self::MessageDelivered { session: _, seq: _ } => "delivered",
            Self::SessionCancelled { .. } => "cancelled",
            Self::SessionReclaimed { .. } => "reclaimed",
            Self::SessionRecovered { .. } => "recovered",
        }
    }
}

/// Bounded per-subscriber fan-out. Mirrors `event_bus` semantics: each
/// subscriber owns a bounded FIFO; a full buffer disconnects that subscriber
/// as slow ([`LiveErrorCode::SlowConsumer`]) without blocking the others.
#[derive(Debug, Default)]
pub struct LiveBus {
    next_id: u64,
    subs: BTreeMap<u64, VecDeque<DaemonEvent>>,
    slow: BTreeMap<u64, bool>,
}

impl LiveBus {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe one consumer; returns its subscriber id.
    pub fn subscribe(&mut self) -> u64 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.subs.insert(id, VecDeque::new());
        self.slow.insert(id, false);
        id
    }

    /// Publish one event to every live subscriber. Slow subscribers are
    /// dropped from this fan-out and reported; the rest receive the event.
    pub fn publish(&mut self, event: DaemonEvent) -> PublishSummary {
        let mut summary = PublishSummary::default();
        for (id, buffer) in self.subs.iter_mut() {
            if *self.slow.get(id).unwrap_or(&true) {
                continue;
            }
            if buffer.len() >= MAX_EVENTS_PER_SUBSCRIBER.max(1) {
                self.slow.insert(*id, true);
                summary.slow_disconnected.push(*id);
                continue;
            }
            buffer.push_back(event.clone());
            summary.delivered += 1;
        }
        summary
    }

    /// Drain buffered events for one subscriber.
    pub fn drain(&mut self, id: u64) -> Result<Vec<DaemonEvent>, LiveError> {
        let Some(buffer) = self.subs.get_mut(&id) else {
            return Err(LiveError::new(LiveErrorCode::Closed));
        };
        if *self.slow.get(&id).unwrap_or(&true) {
            return Err(LiveError::new(LiveErrorCode::SlowConsumer));
        }
        Ok(buffer.drain(..).collect())
    }

    /// Whether `id` is still connected (subscribed and not slow).
    #[must_use]
    pub fn is_connected(&self, id: u64) -> bool {
        self.subs.contains_key(&id) && !self.slow.get(&id).copied().unwrap_or(true)
    }

    /// Live (connected) subscriber count.
    #[must_use]
    pub fn live_subscribers(&self) -> usize {
        self.subs
            .keys()
            .filter(|id| self.is_connected(**id))
            .count()
    }

    /// Disconnect `id`, reclaiming its buffer. Returns false if unknown.
    pub fn unsubscribe(&mut self, id: u64) -> bool {
        self.slow.remove(&id);
        self.subs.remove(&id).is_some()
    }
}

/// Outcome of one [`LiveBus::publish`] fan-out.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PublishSummary {
    /// Connected subscribers that buffered the event.
    pub delivered: usize,
    /// Subscribers disconnected in this call as slow consumers.
    pub slow_disconnected: Vec<u64>,
}

#[derive(Debug)]
struct LiveSession {
    owner: String,
    status: Status,
    queue: VecDeque<(u64, String)>,
    next_seq: u64,
    delivered: u64,
}

/// Live ACP session bridge: owns sessions plus the daemon event bus.
///
/// Caller-owned and synchronous: the caller performs snapshot-byte movement
/// (see [`LiveBridge::snapshot`] / [`LiveBridge::restore`]); this bridge
/// never touches the filesystem, network, or threads.
#[derive(Debug, Default)]
pub struct LiveBridge {
    sessions: BTreeMap<String, LiveSession>,
    next: u64,
    bus: LiveBus,
}

impl LiveBridge {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Live session count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether no live session exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Access the daemon event bus (subscribe/drain).
    pub fn bus_mut(&mut self) -> &mut LiveBus {
        &mut self.bus
    }

    /// Access the daemon event bus (read-only).
    #[must_use]
    pub fn bus(&self) -> &LiveBus {
        &self.bus
    }

    /// Start one live session owned by `owner_token`. Publishes
    /// [`DaemonEvent::SessionStarted`].
    pub fn start(&mut self, owner_token: &str) -> Result<(String, Status), LiveError> {
        check_token(owner_token)?;
        if self.sessions.len() >= MAX_LIVE_SESSIONS {
            return Err(LiveError::new(LiveErrorCode::TooManySessions));
        }
        self.next = self.next.saturating_add(1).max(1);
        let id = format!("ses_{:06}", self.next);
        self.sessions.insert(
            id.clone(),
            LiveSession {
                owner: owner_token.to_owned(),
                status: Status::Running,
                queue: VecDeque::new(),
                next_seq: 0,
                delivered: 0,
            },
        );
        self.bus.publish(DaemonEvent::SessionStarted {
            session: id.clone(),
        });
        Ok((id, Status::Running))
    }

    /// Queue one inbound message and immediately record its delivery against
    /// the live bridge. Publishes `MessageQueued` then `MessageDelivered`
    /// with matching session id and sequence number.
    pub fn send(&mut self, id: &str, owner_token: &str, text: &str) -> Result<u64, LiveError> {
        check_text(text)?;
        let session = self.authed(id, owner_token)?;
        if session.status != Status::Running && session.status != Status::Recovered {
            return Err(LiveError::new(LiveErrorCode::AlreadyCancelled));
        }
        if session.queue.len() >= MAX_QUEUE_PER_SESSION {
            return Err(LiveError::new(LiveErrorCode::QueueFull));
        }
        session.next_seq = session.next_seq.saturating_add(1);
        let seq = session.next_seq;
        session.queue.push_back((seq, text.to_owned()));
        session.delivered = session.delivered.saturating_add(1);
        self.bus.publish(DaemonEvent::MessageQueued {
            session: id.to_owned(),
            seq,
        });
        self.bus.publish(DaemonEvent::MessageDelivered {
            session: id.to_owned(),
            seq,
        });
        Ok(seq)
    }

    /// Cancel one live session. The session row stays until
    /// [`LiveBridge::reclaim`] so restart recovery observes the truthful
    /// cancelled state. Publishes [`DaemonEvent::SessionCancelled`].
    pub fn cancel(&mut self, id: &str, owner_token: &str) -> Result<Status, LiveError> {
        let session = self.authed(id, owner_token)?;
        if session.status == Status::Cancelled {
            return Err(LiveError::new(LiveErrorCode::AlreadyCancelled));
        }
        session.status = Status::Cancelled;
        self.bus.publish(DaemonEvent::SessionCancelled {
            session: id.to_owned(),
        });
        Ok(Status::Cancelled)
    }

    /// Status of one live session (owner-checked; see [`LiveBridge::cancel`]
    /// for the identical-error rule).
    pub fn status(&self, id: &str, owner_token: &str) -> Result<Status, LiveError> {
        let Some(session) = self.sessions.get(id) else {
            return Err(LiveError::new(LiveErrorCode::Unauthorized));
        };
        if session.owner != owner_token {
            return Err(LiveError::new(LiveErrorCode::Unauthorized));
        }
        Ok(session.status)
    }

    /// Reclaim a cancelled session's slot, freeing one of the
    /// [`MAX_LIVE_SESSIONS`] entries. Only cancelled sessions are
    /// reclaimable; running sessions are never dropped silently. Publishes
    /// [`DaemonEvent::SessionReclaimed`].
    pub fn reclaim(&mut self, id: &str, owner_token: &str) -> Result<(), LiveError> {
        let session = self.authed(id, owner_token)?;
        if session.status != Status::Cancelled {
            return Err(LiveError::new(LiveErrorCode::InvalidInput));
        }
        self.sessions.remove(id);
        self.bus.publish(DaemonEvent::SessionReclaimed {
            session: id.to_owned(),
        });
        Ok(())
    }

    /// Drain buffered daemon events for one bus subscriber.
    pub fn drain(&mut self, subscriber: u64) -> Result<Vec<DaemonEvent>, LiveError> {
        self.bus.drain(subscriber)
    }

    /// Re-deliver the buffered daemon events for `subscriber` without
    /// consuming them (slow-peer resync probe). Slow or unknown subscribers
    /// get the matching typed error instead of a fabricated backlog.
    pub fn resync(&self, subscriber: u64) -> Result<Vec<DaemonEvent>, LiveError> {
        let Some(buffer) = self.bus.subs.get(&subscriber) else {
            return Err(LiveError::new(LiveErrorCode::Closed));
        };
        if self.bus.slow.get(&subscriber).copied().unwrap_or(true) {
            return Err(LiveError::new(LiveErrorCode::SlowConsumer));
        }
        Ok(buffer.iter().cloned().collect())
    }

    /// Encode durable state for the caller to persist. Carries ids, owners,
    /// statuses, and queue depths only (never message text: content stays
    /// out of the snapshot so restores cannot fabricate replies).
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        let mut entries = Vec::new();
        for (id, session) in &self.sessions {
            entries.push(format!(
                "{}|{}|{}|{}",
                id,
                session.owner,
                session.status.as_str(),
                session.queue.len()
            ));
        }
        entries.sort();
        format!("acp-live-v1\n{}", entries.join("\n")).into_bytes()
    }

    /// Restore from caller-persisted snapshot bytes. Running sessions come
    /// back as [`Status::Recovered`] (truthful: work may have been lost, so
    /// the bridge never claims `Running`-with-replies nor any completed
    /// response); cancelled sessions stay cancelled. Each recovered session
    /// publishes [`DaemonEvent::SessionRecovered`]. Malformed bytes fail
    /// with `InvalidInput` and change nothing.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<usize, LiveError> {
        let text =
            std::str::from_utf8(bytes).map_err(|_| LiveError::new(LiveErrorCode::InvalidInput))?;
        let mut lines = text.lines();
        if lines.next() != Some("acp-live-v1") {
            return Err(LiveError::new(LiveErrorCode::InvalidInput));
        }
        let mut parsed: Vec<(String, String, Status)> = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(4, '|');
            let (Some(id), Some(owner), Some(status), Some(_depth)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(LiveError::new(LiveErrorCode::InvalidInput));
            };
            check_session_id(id)?;
            check_token(owner)?;
            let status = match status {
                "running" | "recovered" => Status::Recovered,
                "cancelled" => Status::Cancelled,
                _ => return Err(LiveError::new(LiveErrorCode::InvalidInput)),
            };
            if parsed.len() >= MAX_LIVE_SESSIONS {
                return Err(LiveError::new(LiveErrorCode::TooManySessions));
            }
            if parsed.iter().any(|(known, _, _)| known == id) {
                return Err(LiveError::new(LiveErrorCode::InvalidInput));
            }
            parsed.push((id.to_owned(), owner.to_owned(), status));
        }
        for (id, owner, status) in parsed {
            let counter = id
                .strip_prefix("ses_")
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0);
            self.next = self.next.max(counter);
            self.sessions.insert(
                id.clone(),
                LiveSession {
                    owner,
                    status,
                    queue: VecDeque::new(),
                    next_seq: 0,
                    delivered: 0,
                },
            );
            if status == Status::Recovered {
                self.bus.publish(DaemonEvent::SessionRecovered { session: id });
            }
        }
        Ok(self.sessions.len())
    }

    /// Resolve an authed mutable session. Unknown ids and wrong tokens map
    /// to the SAME [`LiveErrorCode::Unauthorized`] so probes cannot tell
    /// whether a session exists.
    fn authed(&mut self, id: &str, owner_token: &str) -> Result<&mut LiveSession, LiveError> {
        let Some(session) = self.sessions.get_mut(id) else {
            return Err(LiveError::new(LiveErrorCode::Unauthorized));
        };
        if session.owner != owner_token {
            return Err(LiveError::new(LiveErrorCode::Unauthorized));
        }
        Ok(session)
    }
}

fn check_session_id(id: &str) -> Result<(), LiveError> {
    let len = id.chars().count();
    if len == 0 || len > MAX_SESSION_ID_CHARS {
        return Err(LiveError::new(LiveErrorCode::InvalidInput));
    }
    Ok(())
}

fn check_token(token: &str) -> Result<(), LiveError> {
    if token.is_empty() || token.chars().count() > MAX_SESSION_ID_CHARS {
        return Err(LiveError::new(LiveErrorCode::InvalidInput));
    }
    Ok(())
}

fn check_text(text: &str) -> Result<(), LiveError> {
    let len = text.chars().count();
    if len == 0 || len > MAX_MESSAGE_CHARS {
        return Err(LiveError::new(LiveErrorCode::InvalidInput));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "owner-token-a";
    const OTHER: &str = "owner-token-b";

    fn bridge_with_sub() -> (LiveBridge, u64) {
        let mut bridge = LiveBridge::new();
        let sub = bridge.bus_mut().subscribe();
        (bridge, sub)
    }

    // DISC-112-T01: start, message, cancel and status round-trip + reclaim.
    #[test]
    fn disc112_t01_start_message_cancel_status_round_trip() {
        let (mut bridge, sub) = bridge_with_sub();
        let (id, status) = bridge.start(OWNER).expect("start");
        assert_eq!(status, Status::Running);
        assert_eq!(bridge.status(&id, OWNER), Ok(Status::Running));

        let seq = bridge.send(&id, OWNER, "hello bridge").expect("send");
        assert_eq!(seq, 1);
        let seq2 = bridge.send(&id, OWNER, "second").expect("send again");
        assert_eq!(seq2, 2);

        assert_eq!(bridge.cancel(&id, OWNER), Ok(Status::Cancelled));
        assert_eq!(bridge.status(&id, OWNER), Ok(Status::Cancelled));
        assert_eq!(
            bridge.send(&id, OWNER, "after cancel"),
            Err(LiveError::new(LiveErrorCode::AlreadyCancelled))
        );
        assert_eq!(
            bridge.cancel(&id, OWNER),
            Err(LiveError::new(LiveErrorCode::AlreadyCancelled))
        );

        // Cancel reclaims nothing by itself; reclaim frees the slot.
        assert_eq!(bridge.len(), 1);
        bridge.reclaim(&id, OWNER).expect("reclaim");
        assert!(bridge.is_empty());
        assert_eq!(
            bridge.status(&id, OWNER),
            Err(LiveError::new(LiveErrorCode::Unauthorized))
        );

        // Event stream observed the full lifecycle with matching ids.
        let events = bridge.drain(sub).expect("drain");
        let kinds: Vec<&str> = events.iter().map(DaemonEvent::kind).collect();
        assert_eq!(
            kinds,
            vec![
                "started",
                "queued",
                "delivered",
                "queued",
                "delivered",
                "cancelled",
                "reclaimed"
            ]
        );
        assert!(events.iter().all(|e| e.session() == id));
    }

    // DISC-112-T02: ACP messages appear in the daemon event stream, matched.
    #[test]
    fn disc112_t02_messages_appear_in_daemon_stream_with_matching_ids() {
        let (mut bridge, sub) = bridge_with_sub();
        let other_sub = bridge.bus_mut().subscribe();
        let (first, _) = bridge.start(OWNER).expect("start first");
        let (second, _) = bridge.start(OWNER).expect("start second");
        let _ = second;

        let seq = bridge.send(&first, OWNER, "ping-first").expect("send");
        let events = bridge.drain(sub).expect("drain");
        let queued: Vec<&DaemonEvent> = events
            .iter()
            .filter(|e| matches!(e, DaemonEvent::MessageQueued { .. }))
            .collect();
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0],
            &DaemonEvent::MessageQueued {
                session: first.clone(),
                seq
            }
        );
        let delivered: Vec<&DaemonEvent> = events
            .iter()
            .filter(|e| matches!(e, DaemonEvent::MessageDelivered { .. }))
            .collect();
        assert_eq!(
            delivered,
            &[&DaemonEvent::MessageDelivered {
                session: first.clone(),
                seq
            }]
        );
        // No event leaks across sessions: no queued/delivered event names `second`.
        assert!(events
            .iter()
            .filter(|e| matches!(
                e,
                DaemonEvent::MessageQueued { .. } | DaemonEvent::MessageDelivered { .. }
            ))
            .all(|e| e.session() == first));

        // Second subscriber sees the same backlog independently (resync probe).
        let backlog = bridge.resync(other_sub).expect("resync");
        assert_eq!(backlog, events);
    }

    // DISC-112-T03: unauthorized rejected, identical error, nothing leaked.
    #[test]
    fn disc112_t03_unauthorized_rejected_without_leak() {
        let (mut bridge, sub) = bridge_with_sub();
        let (id, _) = bridge.start(OWNER).expect("start");

        let unknown = bridge.send("ses_999999", OTHER, "probe");
        let wrong_owner = bridge.send(&id, OTHER, "probe");
        assert_eq!(unknown, wrong_owner);
        assert_eq!(unknown, Err(LiveError::new(LiveErrorCode::Unauthorized)));
        assert_eq!(
            bridge.status("ses_999999", OTHER),
            bridge.status(&id, OTHER)
        );
        assert_eq!(
            bridge.cancel(&id, OTHER),
            Err(LiveError::new(LiveErrorCode::Unauthorized))
        );
        assert_eq!(
            bridge.reclaim(&id, OTHER),
            Err(LiveError::new(LiveErrorCode::Unauthorized))
        );

        // Error text carries the code only: no id, token, or content.
        let rendered = format!("{}", LiveError::new(LiveErrorCode::Unauthorized));
        assert!(!rendered.contains(&id));
        assert!(!rendered.contains(OWNER));
        assert!(!rendered.contains("probe"));
        // Failed probes changed no state and emitted no events naming them.
        assert_eq!(bridge.status(&id, OWNER), Ok(Status::Running));
        let events = bridge.drain(sub).expect("drain");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            DaemonEvent::SessionStarted { session: id }
        );
    }

    // DISC-112-T04: restart recovers truthful state via disposable snapshot.
    #[test]
    fn disc112_t04_restart_recovers_truthful_state_from_snapshot() {
        let dir = std::env::temp_dir().join(format!(
            "rk-disc112-restart-{}-{}",
            std::process::id(),
            "t04"
        ));
        std::fs::create_dir_all(&dir).expect("disposable fixture");
        let snap_path = dir.join("live.snap");

        let mut bridge = LiveBridge::new();
        let (live, _) = bridge.start(OWNER).expect("start live");
        bridge.send(&live, OWNER, "work in flight").expect("send");
        let (done, _) = bridge.start(OWNER).expect("start done");
        bridge.cancel(&done, OWNER).expect("cancel");
        std::fs::write(snap_path.clone(), bridge.snapshot()).expect("persist snapshot");

        // Restart: fresh bridge, restore from caller-moved bytes.
        let bytes = std::fs::read(&snap_path).expect("reload snapshot");
        let mut fresh = LiveBridge::new();
        let sub = fresh.bus_mut().subscribe();
        fresh.restore(&bytes).expect("restore");
        assert_eq!(fresh.status(&live, OWNER), Ok(Status::Recovered));
        assert_eq!(fresh.status(&done, OWNER), Ok(Status::Cancelled));
        // Truthful: no fabricated completed response exists anywhere.
        let events = fresh.drain(sub).expect("drain");
        assert_eq!(
            events,
            vec![DaemonEvent::SessionRecovered { session: live.clone() }]
        );
        assert!(
            !events.iter().any(|e| e.session() == done),
            "cancelled session must not emit recovery chatter"
        );
        // Recovered sessions accept new work; cancelled ones stay terminal.
        fresh.send(&live, OWNER, "after restart").expect("send");
        assert_eq!(
            fresh.send(&done, OWNER, "after cancel"),
            Err(LiveError::new(LiveErrorCode::AlreadyCancelled))
        );

        // Corrupt bytes fail closed and change nothing.
        let before = fresh.snapshot();
        assert_eq!(
            fresh.restore(b"garbage-without-header"),
            Err(LiveError::new(LiveErrorCode::InvalidInput))
        );
        assert_eq!(fresh.snapshot(), before);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // DISC-112-T05: bounded queues; slow peer disconnects, others unblocked.
    #[test]
    fn disc112_t05_bounded_queues_and_slow_peer_disconnect() {
        let (mut bridge, fast) = bridge_with_sub();
        let slow = bridge.bus_mut().subscribe();
        let (id, _) = bridge.start(OWNER).expect("start");
        // Start events already buffered (2 subscribers x 1 event each).
        let _ = bridge.drain(fast).expect("drain start");

        // Per-session queue bound holds.
        for n in 0..MAX_QUEUE_PER_SESSION {
            bridge
                .send(&id, OWNER, &format!("m{n}"))
                .expect("within bound");
        }
        assert_eq!(
            bridge.send(&id, OWNER, "overflow"),
            Err(LiveError::new(LiveErrorCode::QueueFull))
        );

        // Fresh slow subscriber, then fill exactly to cap (fast drains along).
        assert!(bridge.bus_mut().unsubscribe(slow));
        let slow = bridge.bus_mut().subscribe();
        let _ = bridge.drain(fast).expect("drain sends");
        for _ in 0..MAX_EVENTS_PER_SUBSCRIBER {
            bridge.bus_mut().publish(DaemonEvent::SessionStarted {
                session: "ses_padding".to_owned(),
            });
            let _ = bridge.drain(fast).expect("keep fast below cap");
        }
        // Slow now holds MAX events; fast holds none. One more tips slow over.
        assert!(bridge.bus().is_connected(slow));
        let summary = bridge.bus_mut().publish(DaemonEvent::SessionStarted {
            session: "ses_tip".to_owned(),
        });
        assert_eq!(summary.slow_disconnected, vec![slow]);
        assert!(!bridge.bus().is_connected(slow));
        assert_eq!(
            bridge.resync(slow),
            Err(LiveError::new(LiveErrorCode::SlowConsumer))
        );
        // Fast peer unaffected: still connected, still receives.
        assert!(bridge.bus().is_connected(fast));
        let summary = bridge.bus_mut().publish(DaemonEvent::MessageQueued {
            session: id.clone(),
            seq: 999,
        });
        assert_eq!(summary.delivered, 1);
        let events = bridge.drain(fast).expect("fast drains");
        assert!(events
            .iter()
            .any(|e| e == &DaemonEvent::MessageQueued { session: id.clone(), seq: 999 }));
        // Disconnect reclaims the slow slot.
        assert!(bridge.bus_mut().unsubscribe(slow));
        assert_eq!(bridge.bus().live_subscribers(), 1);
    }

    // Extra: reclaim frees a session slot at the cap.
    #[test]
    fn disc112_t06_reclaim_frees_slot_at_cap() {
        let mut bridge = LiveBridge::new();
        let mut ids = Vec::new();
        for _ in 0..MAX_LIVE_SESSIONS {
            let (id, _) = bridge.start(OWNER).expect("start");
            ids.push(id);
        }
        assert_eq!(
            bridge.start(OWNER),
            Err(LiveError::new(LiveErrorCode::TooManySessions))
        );
        // Running sessions are never silently dropped: reclaim refuses.
        assert_eq!(
            bridge.reclaim(&ids[0], OWNER),
            Err(LiveError::new(LiveErrorCode::InvalidInput))
        );
        bridge.cancel(&ids[0], OWNER).expect("cancel");
        bridge.reclaim(&ids[0], OWNER).expect("reclaim");
        let (fresh, _) = bridge.start(OWNER).expect("slot freed");
        assert_eq!(bridge.len(), MAX_LIVE_SESSIONS);
        assert_ne!(fresh, ids[0]);
    }
}

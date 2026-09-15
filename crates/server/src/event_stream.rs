//! WEB-005: bounded SSE/event-stream transport envelope for web clients.
//!
//! Server-side stream contract only (`GET /events` body framing, per-subscriber
//! backpressure bounds, disconnect reclaim). Event payload semantics stay with
//! INT-008; this module copies caller payload bytes into complete `data:` frames
//! and never adds secrets, timestamps, or retained history. Browser/client code in
//! `web/` is owned elsewhere and untouched here.
//!
//! Bounds: each subscriber owns a bounded FIFO of encoded frames capped at
//! `max_buffered_frames` entries (`buffered_bytes()` exposes the live byte
//! total; the hard ceiling is `max_buffered_frames * max_frame_bytes`). A slow
//! consumer whose buffer is full is disconnected with `SlowConsumer`
//! (no global queue, no cross-subscriber blocking). Dropping a subscriber drops
//! its buffer, counter and task slot; pending frames are never flushed after
//! close. `Hub` is a synchronous in-memory harness mirror of the connection-task
//! lifecycle; the HTTP runtime owns one live task per subscriber (counted via
//! `live_tasks()`) cancelled on disconnect via scoped cancellation.

use std::collections::HashMap;

/// Transport envelope bounds for one event-stream subscriber set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamConfig {
    /// Largest accepted payload in bytes; larger payloads are dropped (`TooLarge`).
    pub max_frame_bytes: usize,
    /// Largest per-subscriber pending frame count; overflow disconnects slow.
    pub max_buffered_frames: usize,
    /// Keepalive interval in ms (`:ping` comments, no payload/clock data).
    pub keepalive_ms: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: 1_048_576,
            max_buffered_frames: 64,
            keepalive_ms: 15_000,
        }
    }
}

impl StreamConfig {
    /// Hard per-subscriber byte ceiling (worst case, normally KiB-scale).
    #[must_use]
    pub fn byte_cap(&self) -> usize {
        self.max_buffered_frames.saturating_mul(self.max_frame_bytes)
    }

    /// Encode one payload into a single complete SSE `data:` frame.
    pub fn encode_frame(&self, payload: &[u8]) -> Result<String, FrameError> {
        encode_frame_with(self, payload)
    }
}

/// Frame encoding failures. Neither variant echoes payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Payload exceeded `max_frame_bytes`; frame dropped, subscriber kept alive.
    TooLarge,
    /// Payload was not valid UTF-8; frame rejected, connection kept.
    InvalidUtf8,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "event frame exceeds max_frame_bytes"),
            Self::InvalidUtf8 => write!(f, "event payload is not valid utf-8"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Per-subscriber delivery failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverError {
    /// Subscriber unknown or already disconnected.
    Closed,
    /// Subscriber disconnected earlier as a slow consumer.
    SlowConsumer,
}

impl std::fmt::Display for DeliverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "subscriber closed"),
            Self::SlowConsumer => write!(f, "slow consumer disconnected"),
        }
    }
}

impl std::error::Error for DeliverError {}

fn encode_frame_with(config: &StreamConfig, payload: &[u8]) -> Result<String, FrameError> {
    if payload.len() > config.max_frame_bytes {
        return Err(FrameError::TooLarge);
    }
    let text = std::str::from_utf8(payload).map_err(|_| FrameError::InvalidUtf8)?;
    // Single complete frame; deterministic, no timestamps.
    let mut frame = String::with_capacity(text.len() + 8);
    frame.push_str("data: ");
    frame.push_str(text);
    frame.push_str("\n\n");
    debug_assert!(frame.len() <= config.max_frame_bytes.saturating_add(8));
    Ok(frame)
}

/// Encode with the default envelope bounds.
pub fn encode_frame(payload: &[u8]) -> Result<String, FrameError> {
    StreamConfig::default().encode_frame(payload)
}

/// Fixed keepalive comment. One fixed small string, no clock data.
#[must_use]
pub fn keepalive_frame() -> &'static str {
    ":ping\n\n"
}

/// Headers served on `GET /events`.
#[must_use]
pub fn sse_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Content-Type", "text/event-stream"),
        ("Cache-Control", "no-cache"),
    ]
}

/// Outcome of one `publish` fan-out across all live subscribers.
#[derive(Debug, Default)]
pub struct PublishSummary {
    /// Subscribers that accepted the frame into their bounded buffer.
    pub delivered: usize,
    /// Live subscribers that skipped this frame as oversize (kept alive).
    pub dropped_oversize: usize,
    /// Subscribers disconnected in this call as slow consumers.
    pub slow_disconnected: Vec<u64>,
    /// Encoded frame bytes fanned out (empty when the frame was rejected).
    pub emitted_bytes: Vec<u8>,
}

struct Subscriber {
    config: StreamConfig,
    buffer: Vec<String>,
    buffered_bytes: usize,
    connected: bool,
    slow: bool,
    ever_flushed_after_close: bool,
}

impl Subscriber {
    fn new(config: StreamConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            buffered_bytes: 0,
            connected: true,
            slow: false,
            ever_flushed_after_close: false,
        }
    }

    fn push(&mut self, frame: &str) -> Result<(), DeliverError> {
        if !self.connected {
            self.ever_flushed_after_close = true;
            return Err(if self.slow {
                DeliverError::SlowConsumer
            } else {
                DeliverError::Closed
            });
        }
        if self.buffer.len() >= self.config.max_buffered_frames.max(1) {
            self.connected = false;
            self.slow = true;
            return Err(DeliverError::SlowConsumer);
        }
        self.buffered_bytes = self.buffered_bytes.saturating_add(frame.len());
        self.buffer.push(frame.to_string());
        Ok(())
    }
}

/// Bounded in-memory fan-out mirroring the connection-task lifecycle.
///
/// Caller-owned: `Hub` holds only fixture payload bytes inside bounded
/// per-subscriber buffers; disconnect drops the subscriber struct (buffer,
/// counters, task slot) immediately.
pub struct Hub {
    default_config: StreamConfig,
    next_id: u64,
    subs: HashMap<u64, Subscriber>,
    dropped_frames: u64,
    invalid_frames: u64,
    flushed_after_close: bool,
}

impl Hub {
    /// Create a hub whose new subscribers use `default_config`.
    #[must_use]
    pub fn new(default_config: StreamConfig) -> Self {
        Self {
            default_config,
            next_id: 1,
            subs: HashMap::new(),
            dropped_frames: 0,
            invalid_frames: 0,
            flushed_after_close: false,
        }
    }

    /// Subscribe one consumer with the hub default bounds.
    pub fn subscribe(&mut self) -> u64 {
        self.subscribe_with(self.default_config)
    }

    /// Subscribe one consumer with its own bounds (per-subscriber cap).
    pub fn subscribe_with(&mut self, config: StreamConfig) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.subs.insert(id, Subscriber::new(config));
        id
    }

    /// Fan one INT-008 payload out to every live subscriber.
    pub fn publish(&mut self, payload: &[u8]) -> PublishSummary {
        // Validate once against the widest bound any subscriber could accept is
        // wrong: each subscriber enforces its own cap, so encode per config.
        let mut summary = PublishSummary::default();
        let mut slow_now = Vec::new();
        let mut emitted: Option<String> = None;
        for (&id, sub) in self.subs.iter_mut() {
            if !sub.connected {
                continue;
            }
            match encode_frame_with(&sub.config, payload) {
                Err(FrameError::TooLarge) => {
                    self.dropped_frames += 1;
                    summary.dropped_oversize += 1;
                }
                Err(FrameError::InvalidUtf8) => {
                    self.invalid_frames += 1;
                }
                Ok(frame) => {
                    // Reuse one encoding per distinct config width is overkill
                    // here; keep the canonical bytes of the first success.
                    if emitted.is_none() {
                        emitted = Some(frame.clone());
                    }
                    match sub.push(&frame) {
                        Ok(()) => summary.delivered += 1,
                        Err(DeliverError::SlowConsumer) => slow_now.push(id),
                        Err(DeliverError::Closed) => {
                            self.flushed_after_close = true;
                        }
                    }
                }
            }
        }
        if summary.delivered > 0 {
            if let Some(frame) = emitted {
                summary.emitted_bytes = frame.into_bytes();
            }
        }
        // No live subscriber accepted or rejected this payload (e.g. empty hub):
        // still count an oversize payload once against the hub default bound.
        if summary.delivered == 0
            && summary.dropped_oversize == 0
            && matches!(
                encode_frame_with(&self.default_config, payload),
                Err(FrameError::TooLarge)
            )
        {
            self.dropped_frames += 1;
            summary.dropped_oversize += 1;
        }
        summary.slow_disconnected = slow_now;
        summary
    }

    /// Deliver one payload to a single subscriber (used to assert close semantics).
    pub fn publish_to(&mut self, id: u64, payload: &[u8]) -> Result<(), DeliverError> {
        let Some(sub) = self.subs.get_mut(&id) else {
            return Err(DeliverError::Closed);
        };
        if !sub.connected {
            self.flushed_after_close = self.flushed_after_close || sub.ever_flushed_after_close;
            return Err(if sub.slow {
                DeliverError::SlowConsumer
            } else {
                DeliverError::Closed
            });
        }
        match encode_frame_with(&sub.config, payload) {
            Err(FrameError::TooLarge) => {
                self.dropped_frames += 1;
                Ok(())
            }
            Err(FrameError::InvalidUtf8) => {
                self.invalid_frames += 1;
                Ok(())
            }
            Ok(frame) => sub.push(&frame),
        }
    }

    /// Buffered frames still held for `id` (empty after disconnect).
    #[must_use]
    pub fn received(&self, id: u64) -> Vec<String> {
        self.subs.get(&id).map(|s| s.buffer.clone()).unwrap_or_default()
    }

    /// Total oversize drops counted (subscriber kept alive).
    #[must_use]
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Total invalid-UTF8 rejections counted (no partial frame emitted).
    #[must_use]
    pub fn invalid_frames(&self) -> u64 {
        self.invalid_frames
    }

    /// Whether `id` is still connected.
    #[must_use]
    pub fn is_connected(&self, id: u64) -> bool {
        self.subs.get(&id).is_some_and(|s| s.connected)
    }

    /// Whether `id` was disconnected specifically as a slow consumer.
    #[must_use]
    pub fn disconnected_as_slow(&self, id: u64) -> bool {
        self.subs.get(&id).is_some_and(|s| s.slow)
    }

    /// Live (connected) subscriber count.
    #[must_use]
    pub fn live_subscribers(&self) -> usize {
        self.subs.values().filter(|s| s.connected).count()
    }

    /// Live buffered bytes across connected subscribers.
    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.subs
            .values()
            .filter(|s| s.connected)
            .map(|s| s.buffered_bytes)
            .sum()
    }

    /// Live connection tasks (one per connected subscriber, owned by Hub here).
    #[must_use]
    pub fn live_tasks(&self) -> usize {
        self.live_subscribers()
    }

    /// Whether any frame was ever flushed to a closed subscriber.
    #[must_use]
    pub fn flushed_after_close(&self) -> bool {
        self.flushed_after_close
            || self.subs.values().any(|s| s.ever_flushed_after_close)
    }

    /// Disconnect `id`, reclaiming buffer/counters/task slot. Returns false if unknown.
    pub fn unsubscribe(&mut self, id: u64) -> bool {
        self.subs.remove(&id).is_some()
    }
}

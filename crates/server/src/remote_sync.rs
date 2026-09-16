//! Reconnecting remote sync loop over a caller-owned transport.
//!
//! Caller owns the loop, transport, and socket handle. All methods are
//! synchronous, spawn no thread, perform no I/O beyond the injected
//! [`SyncTransport`] fixture, and retain no state past `dispose`/drop.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;

/// Base reconnect delay in seconds: `backoff_secs(n)` starts here.
pub const REMOTE_RECONNECT_BASE_SECS: u64 = 2;
/// Hard cap for reconnect delays in seconds.
pub const REMOTE_RECONNECT_CAP_SECS: u64 = 30;
/// Max queued events.
pub const MAX_SYNC_QUEUE_ITEMS: usize = 256;
/// Max queued event bytes total.
pub const MAX_SYNC_QUEUE_BYTES: usize = 4_194_304;
/// Max bytes in a single event.
pub const MAX_SYNC_EVENT_BYTES: usize = 262_144;

/// One queued sync event. `seq` is assigned by [`RemoteSync::push`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncEvent {
    pub seq: u64,
    pub bytes: Vec<u8>,
}

/// Connection lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    Disconnected,
    Connecting,
    Connected,
    Error { retry_in_secs: u64 },
    Disposed,
}

/// Typed failures. Variant names only; never carry event bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Transport,
    QueueFull,
    TooLarge,
    Disposed,
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport => write!(f, "transport"),
            Self::QueueFull => write!(f, "queue full"),
            Self::TooLarge => write!(f, "too large"),
            Self::Disposed => write!(f, "disposed"),
        }
    }
}

impl std::error::Error for SyncError {}

/// Caller-supplied transport. Test fixture only; no direct socket I/O here.
pub trait SyncTransport {
    fn open(&self) -> Result<(), SyncError>;
    fn send(&self, event: &SyncEvent) -> Result<(), SyncError>;
    fn close(&self);
    fn is_closed(&self) -> bool;
}

/// Overflow-safe reconnect delay: `min(BASE << min(failures, 31), CAP)`.
pub fn backoff_secs(failures: u32) -> u64 {
    let shift = failures.min(31);
    let delay = REMOTE_RECONNECT_BASE_SECS
        .checked_shl(shift)
        .unwrap_or(u64::MAX);
    delay.min(REMOTE_RECONNECT_CAP_SECS)
}

/// Caller-owned reconnect loop with bounded FIFO queue.
pub struct RemoteSync<'a> {
    transport: &'a dyn SyncTransport,
    state: SyncState,
    queue: VecDeque<SyncEvent>,
    queued_bytes: usize,
    failures: u32,
    next_seq: u64,
    closed: bool,
}

impl<'a> RemoteSync<'a> {
    /// Empty queue; no transport call, no allocation beyond the queue.
    pub fn new(transport: &'a dyn SyncTransport) -> Self {
        Self {
            transport,
            state: SyncState::Disconnected,
            queue: VecDeque::new(),
            queued_bytes: 0,
            failures: 0,
            next_seq: 1,
            closed: false,
        }
    }

    /// Open the transport and resync queued events in FIFO order.
    ///
    /// Sends do not dequeue: removal happens only via [`Self::ack`].
    /// Any transport failure leaves the queue byte-identical.
    pub fn connect(&mut self) -> Result<(), SyncError> {
        if self.state == SyncState::Disposed {
            return Err(SyncError::Disposed);
        }
        self.state = SyncState::Connecting;
        if self.transport.open().is_err() {
            return self.fail();
        }
        for event in &self.queue {
            if self.transport.send(event).is_err() {
                return self.fail();
            }
        }
        self.failures = 0;
        self.state = SyncState::Connected;
        Ok(())
    }

    fn fail(&mut self) -> Result<(), SyncError> {
        let retry_in_secs = backoff_secs(self.failures);
        self.failures = self.failures.saturating_add(1);
        self.state = SyncState::Error { retry_in_secs };
        Err(SyncError::Transport)
    }

    /// Buffer one event. Assigns the next sequence number.
    pub fn push(&mut self, mut event: SyncEvent) -> Result<(), SyncError> {
        if self.state == SyncState::Disposed {
            return Err(SyncError::Disposed);
        }
        if event.bytes.len() > MAX_SYNC_EVENT_BYTES {
            return Err(SyncError::TooLarge);
        }
        if self.queue.len() >= MAX_SYNC_QUEUE_ITEMS
            || self.queued_bytes.saturating_add(event.bytes.len()) > MAX_SYNC_QUEUE_BYTES
        {
            return Err(SyncError::QueueFull);
        }
        event.seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1).max(1);
        self.queued_bytes = self.queued_bytes.saturating_add(event.bytes.len());
        self.queue.push_back(event);
        Ok(())
    }

    /// Remove exactly the acked sequence. Unacked events keep their order.
    pub fn ack(&mut self, seq: u64) -> bool {
        if self.state == SyncState::Disposed {
            return false;
        }
        if let Some(pos) = self.queue.iter().position(|e| e.seq == seq) {
            if let Some(removed) = self.queue.remove(pos) {
                self.queued_bytes = self.queued_bytes.saturating_sub(removed.bytes.len());
            }
            return true;
        }
        false
    }

    /// Terminal: clear buffered bytes, close the handle. Idempotent.
    pub fn dispose(&mut self) {
        self.queue.clear();
        self.queued_bytes = 0;
        self.state = SyncState::Disposed;
        self.close_handle();
    }

    fn close_handle(&mut self) {
        if !self.closed {
            self.closed = true;
            self.transport.close();
        }
    }

    pub fn state(&self) -> SyncState {
        self.state.clone()
    }

    pub fn queue(&self) -> &VecDeque<SyncEvent> {
        &self.queue
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }
}

impl Drop for RemoteSync<'_> {
    fn drop(&mut self) {
        self.queue.clear();
        self.queued_bytes = 0;
        self.close_handle();
    }
}

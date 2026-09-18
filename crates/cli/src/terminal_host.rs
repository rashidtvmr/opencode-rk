#![forbid(unsafe_code)]
//! Terminal lifecycle guard types (TUI-002 slice).
//!
//! Pure state only: no raw-mode syscalls, no FFI, no threads, no clock, no IO.
//! The native host supplies the actual terminal enter/exit calls; these types
//! record the obligations so misuse fails loudly and queues stay bounded.
//!
//! Invariants:
//! - [`RawModeGuard`] is RAII: dropping it marks its [`RestoreFlag`] restored.
//!   `Drop` does not run under `panic=abort`, so the host's real exit path must
//!   still restore through its supported mechanism and mark the flag; the flag
//!   only records that the obligation was discharged, it is not the mechanism.
//! - [`EventCoalescer`] never holds more than [`MAX_QUEUED_EVENTS`] events and
//!   collapses consecutive resizes to the latest size, so resize bursts cannot
//!   starve the async executor. Admission is explicit: overflow and oversize
//!   pastes return [`HostError`], never silently drop.
//! - [`RendererDomain`]/[`RendererOwner`] enforce one live renderer owner.
//!   The domain is `!Sync` (`Cell`) so it cannot be shared across threads, and
//!   the owner is `!Send` so it cannot be moved across threads. A second
//!   acquire while one is live returns [`HostError::RendererBusy`].
//! - Paste admission is byte-bounded by [`MAX_PASTE_BYTES`] (mirrors
//!   `MAX_DRAFT_BYTES` in `opencode-rk-sessions` `tui_state`, so an admitted
//!   paste can still fit the composer draft budget).

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Max events retained by [`EventCoalescer`] (TUI-002 bounded input).
pub const MAX_QUEUED_EVENTS: usize = 512;
/// Max pasted bytes admitted into the queue (TUI-004 explicit paste policy).
/// Larger pastes are rejected with [`HostError::PasteTooLarge`]; the caller
/// surfaces the rejection instead of truncating silently or allocating
/// without bound.
pub const MAX_PASTE_BYTES: usize = 32_768;

/// Terminal lifecycle failures. Denials carry the bound so callers can report
/// it; a denied push leaves the queue unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostError {
    /// Event queue is at [`MAX_QUEUED_EVENTS`].
    QueueFull { limit: usize },
    /// Paste exceeds [`MAX_PASTE_BYTES`].
    PasteTooLarge { bytes: usize, limit: usize },
    /// A renderer owner is already live on this domain.
    RendererBusy,
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull { limit } => write!(f, "terminal event queue full ({limit})"),
            Self::PasteTooLarge { bytes, limit } => {
                write!(f, "paste of {bytes} bytes exceeds {limit} byte policy")
            }
            Self::RendererBusy => write!(f, "renderer already has a live owner"),
        }
    }
}

impl std::error::Error for HostError {}

/// Minimal host event set for the TUI-002 lifecycle slice. Richer input
/// (graphemes, mouse, bracketed paste framing) arrives with TUI-003/004; this
/// enum only needs resize coalescing and byte-bounded paste admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEvent {
    /// Terminal resize to `cols` x `rows`. Consecutive resizes coalesce.
    Resize { cols: u16, rows: u16 },
    /// A single key press (decoded by the host).
    Key(char),
    /// Bracketed-paste body, admitted only within [`MAX_PASTE_BYTES`].
    Paste(String),
    /// Timer/wake marker; never coalesced.
    Tick,
}

/// Admission result for [`EventCoalescer::push`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushOutcome {
    /// Event appended; queue grew by one.
    Admitted,
    /// Resize merged into the pending trailing resize; length unchanged.
    Coalesced,
}

/// Bounded event queue with resize coalescing. Never exceeds
/// [`MAX_QUEUED_EVENTS`]; never holds a paste over [`MAX_PASTE_BYTES`].
#[derive(Clone, Debug, Default)]
pub struct EventCoalescer {
    queue: VecDeque<TerminalEvent>,
}

impl EventCoalescer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
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

    /// Push an event. A resize following a pending resize replaces it
    /// (returns [`PushOutcome::Coalesced`]); anything else appends until the
    /// cap, then returns [`HostError::QueueFull`]. An oversize paste returns
    /// [`HostError::PasteTooLarge`] without touching the queue.
    pub fn push(&mut self, event: TerminalEvent) -> Result<PushOutcome, HostError> {
        if let TerminalEvent::Paste(ref text) = event {
            if text.len() > MAX_PASTE_BYTES {
                return Err(HostError::PasteTooLarge {
                    bytes: text.len(),
                    limit: MAX_PASTE_BYTES,
                });
            }
        }
        if let TerminalEvent::Resize { cols, rows } = event {
            if matches!(self.queue.back(), Some(TerminalEvent::Resize { .. })) {
                *self.queue.back_mut().expect("back checked above") =
                    TerminalEvent::Resize { cols, rows };
                return Ok(PushOutcome::Coalesced);
            }
        }
        if self.queue.len() >= MAX_QUEUED_EVENTS {
            return Err(HostError::QueueFull {
                limit: MAX_QUEUED_EVENTS,
            });
        }
        self.queue.push_back(event);
        Ok(PushOutcome::Admitted)
    }

    /// Take all pending events in order; the queue is empty afterwards.
    /// The returned vector holds at most [`MAX_QUEUED_EVENTS`] events.
    #[must_use]
    pub fn drain(&mut self) -> Vec<TerminalEvent> {
        self.queue.drain(..).collect()
    }
}

/// Shared restore record between [`RawModeGuard`] and the host/test observer.
/// Cloning shares one flag; any clone observes the restore.
#[derive(Clone, Debug, Default)]
pub struct RestoreFlag {
    restored: Arc<AtomicBool>,
}

impl RestoreFlag {
    #[must_use]
    pub fn new() -> Self {
        Self {
            restored: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub fn was_restored(&self) -> bool {
        self.restored.load(Ordering::SeqCst)
    }

    /// Record that the terminal was restored. Called by [`RawModeGuard`]'s
    /// `Drop`; the host's real exit path calls it too when it restores
    /// through its supported mechanism (needed under `panic=abort`, where
    /// `Drop` never runs).
    pub fn mark_restored(&self) {
        self.restored.store(true, Ordering::SeqCst);
    }
}

/// RAII restore obligation for raw mode. The host enters raw mode, then holds
/// this guard; dropping it marks the [`RestoreFlag`]. This type performs no
/// terminal calls itself.
#[derive(Debug)]
pub struct RawModeGuard {
    restore: RestoreFlag,
    armed: bool,
}

impl RawModeGuard {
    /// Take ownership of the restore obligation for an already-entered raw
    /// terminal. `restore` is marked on drop.
    #[must_use]
    pub fn armed(restore: RestoreFlag) -> Self {
        Self {
            restore,
            armed: true,
        }
    }

    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.armed && !self.restore.was_restored()
    }

    #[must_use]
    pub fn restore_flag(&self) -> &RestoreFlag {
        &self.restore
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.armed {
            self.restore.mark_restored();
        }
    }
}

/// Single-owner domain for the renderer (TUI-002 one-owner rule). Holds the
/// `held` bit; `Cell` makes the domain `!Sync`, so it cannot be shared
/// across threads. Owners borrow the domain, so the borrow plus the runtime
/// bit both enforce single ownership.
#[derive(Debug, Default)]
pub struct RendererDomain {
    held: Cell<bool>,
}

impl RendererDomain {
    #[must_use]
    pub fn new() -> Self {
        Self {
            held: Cell::new(false),
        }
    }

    #[must_use]
    pub fn is_held(&self) -> bool {
        self.held.get()
    }

    /// Acquire the single owner. Fails with [`HostError::RendererBusy`] while
    /// a previous owner is live; succeeds again after it is dropped.
    pub fn try_acquire(&self) -> Result<RendererOwner<'_>, HostError> {
        if self.held.get() {
            return Err(HostError::RendererBusy);
        }
        self.held.set(true);
        Ok(RendererOwner {
            domain: self,
            _no_send: PhantomData,
        })
    }

    fn release(&self) {
        self.held.set(false);
    }
}

/// The single live renderer owner. `!Send` (`PhantomData<*const ()>`), so it
/// cannot be moved to another thread; dropping releases the domain.
#[derive(Debug)]
pub struct RendererOwner<'a> {
    domain: &'a RendererDomain,
    _no_send: PhantomData<*const ()>,
}

impl Drop for RendererOwner<'_> {
    fn drop(&mut self) {
        self.domain.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_bursts_coalesce_to_latest() {
        let mut q = EventCoalescer::new();
        assert_eq!(
            q.push(TerminalEvent::Resize { cols: 80, rows: 24 }),
            Ok(PushOutcome::Admitted)
        );
        for (cols, rows) in [(81, 24), (100, 30), (120, 40)] {
            assert_eq!(
                q.push(TerminalEvent::Resize { cols, rows }),
                Ok(PushOutcome::Coalesced)
            );
        }
        assert_eq!(q.len(), 1);
        assert_eq!(
            q.drain(),
            vec![TerminalEvent::Resize {
                cols: 120,
                rows: 40
            }]
        );
        assert!(q.is_empty());
    }

    #[test]
    fn non_resize_events_break_coalescing() {
        let mut q = EventCoalescer::new();
        q.push(TerminalEvent::Resize { cols: 80, rows: 24 }).unwrap();
        q.push(TerminalEvent::Tick).unwrap();
        assert_eq!(
            q.push(TerminalEvent::Resize { cols: 90, rows: 30 }),
            Ok(PushOutcome::Admitted)
        );
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn queue_rejects_beyond_cap_without_growth() {
        let mut q = EventCoalescer::new();
        for _ in 0..MAX_QUEUED_EVENTS {
            assert_eq!(q.push(TerminalEvent::Tick), Ok(PushOutcome::Admitted));
        }
        assert_eq!(
            q.push(TerminalEvent::Tick),
            Err(HostError::QueueFull {
                limit: MAX_QUEUED_EVENTS
            })
        );
        assert_eq!(q.len(), MAX_QUEUED_EVENTS);
    }

    #[test]
    fn oversize_paste_rejected_without_enqueue() {
        let mut q = EventCoalescer::new();
        q.push(TerminalEvent::Tick).unwrap();
        let big = "x".repeat(MAX_PASTE_BYTES + 1);
        let bytes = big.len();
        assert_eq!(
            q.push(TerminalEvent::Paste(big)),
            Err(HostError::PasteTooLarge {
                bytes,
                limit: MAX_PASTE_BYTES
            })
        );
        assert_eq!(q.len(), 1);
        let ok = "y".repeat(MAX_PASTE_BYTES);
        assert_eq!(
            q.push(TerminalEvent::Paste(ok)),
            Ok(PushOutcome::Admitted)
        );
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn renderer_double_acquire_rejected_until_released() {
        let domain = RendererDomain::new();
        let first = domain.try_acquire();
        assert!(first.is_ok());
        assert!(domain.is_held());
        assert_eq!(domain.try_acquire().unwrap_err(), HostError::RendererBusy);
        drop(first);
        assert!(!domain.is_held());
        assert!(domain.try_acquire().is_ok());
    }

    #[test]
    fn raw_guard_marks_restore_on_drop() {
        let flag = RestoreFlag::new();
        assert!(!flag.was_restored());
        {
            let guard = RawModeGuard::armed(flag.clone());
            assert!(guard.is_armed());
            assert!(!flag.was_restored());
        }
        assert!(flag.was_restored());
    }
}

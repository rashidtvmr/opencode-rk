#![forbid(unsafe_code)]
//! Terminal lifecycle guard types (TUI-002 slice).
//!
//! Pure state + Tokio sync channel only: no raw-mode syscalls, no FFI, no
//! threads spawned, no clock, no IO. The native host (`native_host.rs:89-113`
//! owns its own `stty` guard today and blocks on `stdin.read`) supplies the
//! actual terminal enter/exit calls; these types record the obligations so
//! misuse fails loudly and queues stay bounded. The [`event_channel`] bridge
//! is the sanctioned wiring: a stdin-reader task feeds via
//! [`BridgeSender::try_push`] (never blocks the executor), the render loop
//! drains via [`BridgeReceiver::try_drain`]/`recv`, pastes enter only via
//! [`admit_paste`] so [`MAX_PASTE_BYTES`] cannot be bypassed, and decoders
//! must not expand input into an unbounded `Vec<char>`.
//!
//! Invariants:
//! - [`RawModeGuard`] is RAII: dropping it marks its [`RestoreFlag`] restored.
//!   `Drop` does not run under `panic=abort` (release profile), so the host's
//!   real exit path must still restore through its supported mechanism and
//!   mark the flag; the flag only records that the obligation was discharged,
//!   it is not the mechanism.
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
//!   paste can still fit the composer draft budget). Use [`admit_paste`];
//!   never push an unbounded decode buffer directly.
//! - [`event_channel`] is bounded (`1..=MAX_QUEUED_EVENTS`): the sender's
//!   [`BridgeSender::try_push`] returns [`PushOutcome::Admitted`] or
//!   [`HostError`], never blocks; the receiver's [`BridgeReceiver::try_drain`]
//!   collapses consecutive resizes like [`EventCoalescer`], so bursts stay
//!   bounded end to end.
//!
//! Guard unification (doc only, no code moves):
//! - `terminal_host::RawModeGuard` only marks a [`RestoreFlag`]; no syscalls.
//! - `shutdown::RestoreGuard` (`shutdown.rs`) runs a caller-supplied closure
//!   on `Drop` (unwind-safe; abort/`SIGKILL`/power-loss ceiling applies).
//! - `app_start::TerminalRestoreGuard` (`app_start.rs`) runs a boxed
//!   `FnOnce() + Send` on drop unless disarmed via `disarm()`.
//! All three share one ceiling: `Drop` never runs under `panic=abort`,
//! `SIGKILL`, or power loss, so the host's real exit path must restore via
//! its supported mechanism and mark/record the flag/receipt there too. The
//! restore closure itself must not panic.

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::mpsc;

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
                if let Some(back) = self.queue.back_mut() {
                    *back = TerminalEvent::Resize { cols, rows };
                    return Ok(PushOutcome::Coalesced);
                }
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

/// Admit a bracketed-paste body. Returns [`TerminalEvent::Paste`] only when
/// `text` fits [`MAX_PASTE_BYTES`]; larger bodies return
/// [`HostError::PasteTooLarge`]. Push the returned event (via
/// [`EventCoalescer::push`] or [`BridgeSender::try_push`); never bypass this
/// check with a raw decode buffer.
pub fn admit_paste(text: String) -> Result<TerminalEvent, HostError> {
    if text.len() > MAX_PASTE_BYTES {
        return Err(HostError::PasteTooLarge {
            bytes: text.len(),
            limit: MAX_PASTE_BYTES,
        });
    }
    Ok(TerminalEvent::Paste(text))
}

/// Tokio bridge between the stdin-reader task and the render loop.
/// `bound` is clamped to `1..=MAX_QUEUED_EVENTS`; the sender's
/// [`BridgeSender::try_push`] never blocks (returns [`HostError::QueueFull`]
/// when full) and the receiver drains with resize coalescing, so bursts stay
/// bounded end to end.
#[must_use]
pub fn event_channel(bound: usize) -> (BridgeSender, BridgeReceiver) {
    let bound = bound.clamp(1, MAX_QUEUED_EVENTS);
    let (tx, rx) = mpsc::channel::<TerminalEvent>(bound);
    (BridgeSender { tx, limit: bound }, BridgeReceiver { rx })
}

/// Non-blocking feeder end of [`event_channel`]. `Clone + Send`: the
/// stdin-reader task owns a clone; [`try_push`](Self::try_push) is the only
/// admission path.
#[derive(Clone, Debug)]
pub struct BridgeSender {
    tx: mpsc::Sender<TerminalEvent>,
    limit: usize,
}

impl BridgeSender {
    /// Feed one event without blocking. Oversize pastes return
    /// [`HostError::PasteTooLarge`] without touching the channel; a full (or
    /// closed) channel returns [`HostError::QueueFull`] without blocking.
    /// Resizes are admitted here and collapsed on the receive side
    /// ([`BridgeReceiver::try_drain`]), so this returns
    /// [`PushOutcome::Admitted`] on success.
    pub fn try_push(&self, event: TerminalEvent) -> Result<PushOutcome, HostError> {
        if let TerminalEvent::Paste(ref text) = event {
            if text.len() > MAX_PASTE_BYTES {
                return Err(HostError::PasteTooLarge {
                    bytes: text.len(),
                    limit: MAX_PASTE_BYTES,
                });
            }
        }
        match self.tx.try_send(event) {
            Ok(()) => Ok(PushOutcome::Admitted),
            Err(mpsc::error::TrySendError::Full(_)) => {
                Err(HostError::QueueFull { limit: self.limit })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(HostError::QueueFull { limit: self.limit })
            }
        }
    }

    /// Channel capacity chosen at [`event_channel`] time (post-clamp).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.limit
    }
}

/// Drain end of [`event_channel`]. `!Sync` is not required; the render loop
/// owns it and calls [`try_drain`](Self::try_drain) or
/// [`recv`](Self::recv).
#[derive(Debug)]
pub struct BridgeReceiver {
    rx: mpsc::Receiver<TerminalEvent>,
}

impl BridgeReceiver {
    /// Take all currently buffered events without blocking, collapsing
    /// consecutive resizes to the latest size (same rule as
    /// [`EventCoalescer::push`]). Returned vec holds at most the channel
    /// bound (<= [`MAX_QUEUED_EVENTS`]).
    pub fn try_drain(&mut self) -> Vec<TerminalEvent> {
        let mut out: Vec<TerminalEvent> = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            if let TerminalEvent::Resize { cols, rows } = event {
                if matches!(out.last(), Some(TerminalEvent::Resize { .. })) {
                    if let Some(last) = out.last_mut() {
                        *last = TerminalEvent::Resize { cols, rows };
                        continue;
                    }
                }
            }
            out.push(event);
        }
        out
    }

    /// Async receive of the next event (`None` when all senders dropped).
    pub async fn recv(&mut self) -> Option<TerminalEvent> {
        self.rx.recv().await
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
        q.push(TerminalEvent::Resize { cols: 80, rows: 24 })
            .unwrap();
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
        assert_eq!(q.push(TerminalEvent::Paste(ok)), Ok(PushOutcome::Admitted));
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

    #[test]
    fn try_acquire_release_reacquire() {
        let domain = RendererDomain::new();
        assert!(!domain.is_held());
        {
            let _owner = domain.try_acquire().expect("first acquire");
            assert!(domain.is_held());
        }
        assert!(!domain.is_held(), "drop must release");
        assert!(domain.try_acquire().is_ok(), "release allows reacquire");
    }

    #[test]
    fn admit_paste_enforces_byte_bound() {
        let ok = "a".repeat(MAX_PASTE_BYTES);
        assert_eq!(
            admit_paste(ok.clone()),
            Ok(TerminalEvent::Paste(ok)),
            "exact-bound paste admits"
        );
        let big = "b".repeat(MAX_PASTE_BYTES + 1);
        let bytes = big.len();
        assert_eq!(
            admit_paste(big),
            Err(HostError::PasteTooLarge {
                bytes,
                limit: MAX_PASTE_BYTES
            }),
            "oversize paste rejects without enqueue"
        );
    }

    #[test]
    fn bridge_constructor_clamps_bound() {
        let (tx, _rx) = event_channel(0);
        assert_eq!(tx.capacity(), 1, "zero bound clamps to 1");
        let (tx, _rx) = event_channel(MAX_QUEUED_EVENTS + 99);
        assert_eq!(tx.capacity(), MAX_QUEUED_EVENTS, "huge bound clamps");
    }

    #[test]
    fn bridge_feeder_returns_push_outcome() {
        let (tx, mut rx) = event_channel(4);
        assert_eq!(tx.try_push(TerminalEvent::Tick), Ok(PushOutcome::Admitted));
        assert_eq!(
            tx.try_push(TerminalEvent::Resize { cols: 80, rows: 24 }),
            Ok(PushOutcome::Admitted),
            "sender admits; drain side coalesces"
        );
        assert_eq!(
            tx.try_push(TerminalEvent::Resize { cols: 90, rows: 30 }),
            Ok(PushOutcome::Admitted)
        );
        assert_eq!(
            rx.try_drain(),
            vec![
                TerminalEvent::Tick,
                TerminalEvent::Resize { cols: 90, rows: 30 }
            ],
            "consecutive resizes coalesce on drain"
        );
    }

    #[test]
    fn bridge_queue_full_errors_without_blocking() {
        let (tx, _rx) = event_channel(2);
        tx.try_push(TerminalEvent::Tick).unwrap();
        tx.try_push(TerminalEvent::Tick).unwrap();
        assert_eq!(
            tx.try_push(TerminalEvent::Tick),
            Err(HostError::QueueFull { limit: 2 })
        );
    }

    #[test]
    fn bridge_rejects_oversize_paste_without_enqueue() {
        let (tx, mut rx) = event_channel(4);
        tx.try_push(TerminalEvent::Tick).unwrap();
        let big = "x".repeat(MAX_PASTE_BYTES + 1);
        let bytes = big.len();
        assert_eq!(
            tx.try_push(TerminalEvent::Paste(big)),
            Err(HostError::PasteTooLarge {
                bytes,
                limit: MAX_PASTE_BYTES
            })
        );
        assert_eq!(rx.try_drain(), vec![TerminalEvent::Tick]);
    }

    #[test]
    fn renderer_create_drop_cycles_exactly_once() {
        let domain = RendererDomain::new();
        for _ in 0..8 {
            assert!(!domain.is_held(), "no live owner between cycles");
            {
                let owner = domain.try_acquire().expect("acquire each cycle");
                assert!(domain.is_held());
                assert_eq!(
                    domain.try_acquire().unwrap_err(),
                    HostError::RendererBusy,
                    "double acquire mid-cycle stays busy"
                );
                drop(owner);
            }
            assert!(!domain.is_held(), "drop releases exactly once per cycle");
        }
        assert!(domain.try_acquire().is_ok(), "cycles leave domain reusable");
    }

    #[test]
    fn resize_burst_bridge_stays_bounded_no_starvation() {
        let (tx, mut rx) = event_channel(8);
        for i in 0..8u16 {
            tx.try_push(TerminalEvent::Resize {
                cols: 80 + i,
                rows: 24,
            })
            .unwrap();
        }
        assert_eq!(
            tx.try_push(TerminalEvent::Tick),
            Err(HostError::QueueFull { limit: 8 }),
            "full feeder errors without blocking the executor"
        );
        assert_eq!(
            rx.try_drain(),
            vec![TerminalEvent::Resize { cols: 87, rows: 24 }],
            "resize storm coalesces to the latest size on drain"
        );
        let mut q = EventCoalescer::new();
        for i in 0..600u16 {
            q.push(TerminalEvent::Resize {
                cols: 80 + (i % 50),
                rows: 24,
            })
            .unwrap();
        }
        assert_eq!(q.len(), 1, "600-resize storm collapses to one event");
        assert_eq!(
            q.drain(),
            vec![TerminalEvent::Resize { cols: 129, rows: 24 }]
        );
    }

    #[test]
    fn bridge_sender_cross_thread_send_path() {
        fn assert_send<T: Send>() {}
        assert_send::<TerminalEvent>();
        assert_send::<PushOutcome>();
        assert_send::<HostError>();
        assert_send::<BridgeSender>();
        let (tx, mut rx) = event_channel(4);
        let handle = std::thread::spawn(move || tx.try_push(TerminalEvent::Tick));
        assert_eq!(
            handle.join().expect("feeder thread joins"),
            Ok(PushOutcome::Admitted),
            "sanctioned cross-thread feed path works and joins"
        );
        assert_eq!(rx.try_drain(), vec![TerminalEvent::Tick]);
    }

    #[test]
    fn cancel_drop_senders_recv_none_joins() {
        let (tx, mut rx) = event_channel(4);
        assert!(rx.try_drain().is_empty(), "empty drain yields no events");
        drop(tx);
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime builds without IO/time drivers");
        assert_eq!(
            rt.block_on(rx.recv()),
            None,
            "all senders dropped is the render loop join signal"
        );
    }

    #[test]
    fn restore_explicit_mark_idempotent_across_drop() {
        let flag = RestoreFlag::new();
        {
            let guard = RawModeGuard::armed(flag.clone());
            flag.mark_restored();
            assert!(
                !guard.is_armed(),
                "explicit host restore disarms the pending obligation"
            );
        }
        assert!(flag.was_restored(), "drop after explicit restore stays restored");
        flag.mark_restored();
        assert!(flag.was_restored(), "restore marking is idempotent");
    }
}

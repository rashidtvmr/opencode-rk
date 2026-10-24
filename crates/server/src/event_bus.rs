//! Bounded fan-out event bus for the server daemon.
use opencode_rk_contracts::{MessageId, SessionId};
use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::mpsc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerEvent {
    SessionCreated(SessionId),
    MessageAppended {
        session: SessionId,
        seq: u64,
    },
    /// Durable message publication: emitted **after** a successful persisted
    /// append, carrying the actual `MessageRecord.id` (UUID) plus the
    /// session. Explicit local extension of the upstream-derived
    /// `message.appended` wire type (properties gain `message_id`), not a
    /// claim of full upstream parity. The legacy [`Self::MessageAppended`]
    /// helper carries only `{session, seq}` and must never be described as a
    /// durable message event.
    MessageStored {
        session: SessionId,
        message_id: MessageId,
    },
    ToolExecuted {
        name: String,
        duration_ms: u64,
    },
    PermissionRequested(String),
    TodoUpdated { session: SessionId, todos: Vec<opencode_rk_sessions::Todo> },
    Shutdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    SessionCreated,
    MessageAppended,
    ToolExecuted,
    PermissionRequested,
    TodoUpdated,
    Shutdown,
}
impl ServerEvent {
    #[must_use]
    pub fn kind(&self) -> EventKind {
        match self {
            Self::SessionCreated(_) => EventKind::SessionCreated,
            Self::MessageAppended { .. } | Self::MessageStored { .. } => EventKind::MessageAppended,
            Self::ToolExecuted { .. } => EventKind::ToolExecuted,
            Self::PermissionRequested(_) => EventKind::PermissionRequested,
            Self::TodoUpdated { .. } => EventKind::TodoUpdated,
            Self::Shutdown => EventKind::Shutdown,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    Full,
    Closed,
    InvalidPayload,
}
impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "event bus full"),
            Self::Closed => write!(f, "subscriber closed"),
            Self::InvalidPayload => write!(f, "event payload exceeds its supported bounds"),
        }
    }
}
impl std::error::Error for BusError {}
/// One bus publication: the stable identity is allocated **once by the bus,
/// before fanout**, so every subscriber of this publication observes the
/// same `id` and an equal payload (never a per-client wire-layer counter).
/// `id` comes from the per-bus monotonic publication counter and is a fresh
/// publication identity only — it is **not** a durable replay cursor; a
/// reconnecting client must resnapshot durable history (no seq-based
/// side-effect replay is ever performed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    /// Stable per-publication identity, identical for every subscriber.
    pub id: u64,
    /// The domain event carried by this publication.
    pub event: ServerEvent,
}
struct SubEntry {
    id: u64,
    filter: Option<EventKind>,
    tx: mpsc::Sender<Published>,
    /// Overflow counter for one subscriber: `publish` increments it when the
    /// per-subscriber bounded channel is full (the event is dropped for that
    /// subscriber, the publish still reports `BusError::Full`, and the entry
    /// is otherwise untouched — preserving pre-existing semantics for every
    /// existing caller/test). A consumer detects its own stall via
    /// [`Subscription::overflow_count`] and disconnects (EOF), so an
    /// unreadable subscriber never receives silently dropped frames forever.
    overflow: std::sync::atomic::AtomicU64,
}
struct BusInner {
    capacity: usize,
    next: AtomicU64,
    /// Per-bus monotonic event-ordering / publication-identity counter.
    /// Explicitly *not* a durable per-session replay cursor: reconnect
    /// requires a snapshot, never a seq-based replay of side effects.
    seq: AtomicU64,
    subs: Mutex<Vec<SubEntry>>,
    /// Finite subscriber admission ceiling for this bus instance (per router
    /// bus, so routers are independent). `usize::MAX` (the default) is
    /// unlimited; the production router lowers it at construction.
    max_subscribers: AtomicUsize,
}
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<BusInner>,
}
impl EventBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(BusInner {
                capacity: capacity.max(1),
                next: AtomicU64::new(1),
                seq: AtomicU64::new(0),
                subs: Mutex::new(Vec::new()),
                max_subscribers: AtomicUsize::new(usize::MAX),
            }),
        }
    }
    /// Caps concurrent subscribers on this bus instance. Call once at router
    /// construction, before serving. Applies to [`Self::try_subscribe`];
    /// plain [`Self::subscribe`] (SDK/internal callers) is unchanged.
    pub fn set_max_subscribers(&self, max: usize) {
        self.inner.max_subscribers.store(max, Ordering::Release);
    }
    /// Current subscriber admission ceiling of this bus instance.
    #[must_use]
    pub fn max_subscribers(&self) -> usize {
        self.inner.max_subscribers.load(Ordering::Acquire)
    }
    /// Next per-bus ordering counter (starts at 1; `seq0` never issued).
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.inner.seq.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub fn publish(&self, event: ServerEvent) -> Result<(), BusError> {
        // Internal publishers must obey the same collection bound as HTTP
        // callers, before cloning a potentially large payload for subscribers.
        if let ServerEvent::TodoUpdated { todos, .. } = &event {
            opencode_rk_sessions::validate_todos(todos).map_err(|_| BusError::InvalidPayload)?;
        }
        let mut guard = self.inner.subs.lock().unwrap();
        // Allocate the stable publication identity ONCE, before fanout: every
        // subscriber receives this same `id`/payload (the HTTP wire layer
        // must never allocate per-client ids).
        let published = Published {
            id: self.inner.seq.fetch_add(1, Ordering::Relaxed) + 1,
            event,
        };
        let mut full = false;
        let mut closed_ids = Vec::new();
        for entry in guard.iter() {
            if entry.filter.is_some_and(|f| f != published.event.kind()) {
                continue;
            }
            match entry.tx.try_send(published.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    full = true;
                    entry.overflow.fetch_add(1, Ordering::Relaxed);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => closed_ids.push(entry.id),
            }
        }
        if !closed_ids.is_empty() {
            guard.retain(|e| !closed_ids.contains(&e.id))
        }
        if full {
            Err(BusError::Full)
        } else {
            Ok(())
        }
    }
    /// Unbounded subscribe: unchanged signature/semantics for SDK and
    /// internal callers. HTTP admission uses [`Self::try_subscribe`].
    #[must_use]
    pub fn subscribe(&self, filter: Option<EventKind>) -> Subscription {
        let mut guard = self.inner.subs.lock().unwrap();
        self.push_subscription(&mut guard, filter)
    }
    /// Admission-checked subscribe. Returns `None` (attaching nothing) when
    /// this bus instance already holds [`Self::max_subscribers`] entries;
    /// the length check and the push happen under the same `subs` mutex, so
    /// concurrent admissions can never exceed the ceiling.
    pub fn try_subscribe(&self, filter: Option<EventKind>) -> Option<Subscription> {
        let mut guard = self.inner.subs.lock().unwrap();
        if guard.len() >= self.inner.max_subscribers.load(Ordering::Acquire) {
            return None;
        }
        Some(self.push_subscription(&mut guard, filter))
    }
    fn push_subscription(
        &self,
        guard: &mut Vec<SubEntry>,
        filter: Option<EventKind>,
    ) -> Subscription {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(self.inner.capacity);
        guard.push(SubEntry {
            id,
            filter,
            tx,
            overflow: std::sync::atomic::AtomicU64::new(0),
        });
        Subscription {
            id,
            filter,
            rx,
            bus: Arc::clone(&self.inner),
        }
    }
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.inner.subs.lock().map(|g| g.len()).unwrap_or(0)
    }
}
pub struct Subscription {
    id: u64,
    filter: Option<EventKind>,
    rx: mpsc::Receiver<Published>,
    bus: Arc<BusInner>,
}
impl Subscription {
    /// Legacy receive: returns only the event, discarding the shared
    /// publication envelope (kept for existing callers/tests).
    pub async fn recv(&mut self) -> Option<ServerEvent> {
        self.rx.recv().await.map(|published| published.event)
    }
    pub fn try_recv(&mut self) -> Result<ServerEvent, mpsc::error::TryRecvError> {
        self.rx.try_recv().map(|published| published.event)
    }
    /// Additive envelope receive: the shared publication identity plus its
    /// event. Every subscriber observes the same `id` for one publication.
    pub async fn recv_published(&mut self) -> Option<Published> {
        self.rx.recv().await
    }
    pub fn try_recv_published(&mut self) -> Result<Published, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
    #[must_use]
    pub fn filter(&self) -> Option<EventKind> {
        self.filter
    }
    /// Times this subscriber's bounded channel was full (dropped frames).
    #[must_use]
    pub fn overflow_count(&self) -> u64 {
        self.bus
            .subs
            .lock()
            .map(|guard| {
                guard
                    .iter()
                    .find(|e| e.id == self.id)
                    .map(|e| e.overflow.load(Ordering::Relaxed))
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
    /// True once any frame was dropped for this subscriber (slow consumer).
    #[must_use]
    pub fn has_overflowed(&self) -> bool {
        self.overflow_count() > 0
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.bus.subs.lock() {
            guard.retain(|e| e.id != self.id)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn publish_and_receive() {
        let bus = EventBus::new(16);
        let mut sub = bus.subscribe(None);
        bus.publish(ServerEvent::SessionCreated(SessionId::new()))
            .unwrap();
        let got = sub.recv().await.unwrap();
        assert!(matches!(got, ServerEvent::SessionCreated(_)));
    }
    #[tokio::test]
    async fn bounded_rejects_overflow() {
        let bus = EventBus::new(2);
        let _sub = bus.subscribe(None);
        bus.publish(ServerEvent::Shutdown).unwrap();
        bus.publish(ServerEvent::Shutdown).unwrap();
        assert_eq!(bus.publish(ServerEvent::Shutdown), Err(BusError::Full));
    }
    #[tokio::test]
    async fn multiple_subscribers() {
        let bus = EventBus::new(16);
        let mut a = bus.subscribe(None);
        let mut b = bus.subscribe(None);
        let mut c = bus.subscribe(None);
        bus.publish(ServerEvent::Shutdown).unwrap();
        assert_eq!(a.recv().await.unwrap(), ServerEvent::Shutdown);
        assert_eq!(b.recv().await.unwrap(), ServerEvent::Shutdown);
        assert_eq!(c.recv().await.unwrap(), ServerEvent::Shutdown);
    }
    #[tokio::test]
    async fn filtered_subscription() {
        let bus = EventBus::new(16);
        let mut filtered = bus.subscribe(Some(EventKind::SessionCreated));
        let sid = SessionId::new();
        bus.publish(ServerEvent::MessageAppended {
            session: sid,
            seq: 1,
        })
        .unwrap();
        bus.publish(ServerEvent::SessionCreated(sid)).unwrap();
        assert_eq!(
            filtered.recv().await.unwrap(),
            ServerEvent::SessionCreated(sid)
        );
        assert!(filtered.try_recv().is_err());
    }
    #[tokio::test]
    async fn drop_subscription_cleanup() {
        let bus = EventBus::new(16);
        {
            let _sub = bus.subscribe(None);
            assert_eq!(bus.subscriber_count(), 1);
        }
        assert_eq!(bus.subscriber_count(), 0);
    }
}

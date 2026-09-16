//! Bounded fan-out event bus for the server daemon.
use opencode_rk_contracts::SessionId;
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::mpsc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerEvent {
    SessionCreated(SessionId),
    MessageAppended { session: SessionId, seq: u64 },
    ToolExecuted { name: String, duration_ms: u64 },
    PermissionRequested(String),
    Shutdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    SessionCreated,
    MessageAppended,
    ToolExecuted,
    PermissionRequested,
    Shutdown,
}
impl ServerEvent {
    #[must_use]
    pub fn kind(&self) -> EventKind {
        match self {
            Self::SessionCreated(_) => EventKind::SessionCreated,
            Self::MessageAppended { .. } => EventKind::MessageAppended,
            Self::ToolExecuted { .. } => EventKind::ToolExecuted,
            Self::PermissionRequested(_) => EventKind::PermissionRequested,
            Self::Shutdown => EventKind::Shutdown,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusError {
    Full,
    Closed,
}
impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "event bus full"),
            Self::Closed => write!(f, "subscriber closed"),
        }
    }
}
impl std::error::Error for BusError {}
struct SubEntry {
    id: u64,
    filter: Option<EventKind>,
    tx: mpsc::Sender<ServerEvent>,
}
struct BusInner {
    capacity: usize,
    next: AtomicU64,
    subs: Mutex<Vec<SubEntry>>,
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
                subs: Mutex::new(Vec::new()),
            }),
        }
    }
    pub fn publish(&self, event: ServerEvent) -> Result<(), BusError> {
        let mut guard = self.inner.subs.lock().unwrap();
        let mut full = false;
        let mut closed_ids = Vec::new();
        for entry in guard.iter() {
            if entry.filter.is_some_and(|f| f != event.kind()) {
                continue;
            }
            match entry.tx.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => full = true,
                Err(mpsc::error::TrySendError::Closed(_)) => closed_ids.push(entry.id),
            }
        }
        if !closed_ids.is_empty() {
            guard.retain(|e| !closed_ids.contains(&e.id))
        }
        if full { Err(BusError::Full) } else { Ok(()) }
    }
    #[must_use]
    pub fn subscribe(&self, filter: Option<EventKind>) -> Subscription {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(self.inner.capacity);
        self.inner
            .subs
            .lock()
            .unwrap()
            .push(SubEntry { id, filter, tx });
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
    rx: mpsc::Receiver<ServerEvent>,
    bus: Arc<BusInner>,
}
impl Subscription {
    pub async fn recv(&mut self) -> Option<ServerEvent> {
        self.rx.recv().await
    }
    pub fn try_recv(&mut self) -> Result<ServerEvent, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
    #[must_use]
    pub fn filter(&self) -> Option<EventKind> {
        self.filter
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

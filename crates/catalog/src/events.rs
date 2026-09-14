//! Catalog events module for event-driven notifications.
//! Provides an event bus for catalog-related events such as plugin lifecycle
//! and index updates.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

/// Catalog events representing significant state changes.
#[derive(Debug, Clone)]
pub enum CatalogEvent {
    /// A plugin was loaded successfully.
    PluginLoaded { plugin_id: String },
    /// A plugin was unloaded successfully.
    PluginUnloaded { plugin_id: String },
    /// An error occurred with a plugin.
    PluginError { plugin_id: String, error: String },
    /// The catalog index was updated.
    IndexUpdated {
        provider_count: usize,
        model_count: usize,
    },
}

impl CatalogEvent {
    /// Returns a short string identifying the event variant.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::PluginLoaded { .. } => "PluginLoaded",
            Self::PluginUnloaded { .. } => "PluginUnloaded",
            Self::PluginError { .. } => "PluginError",
            Self::IndexUpdated { .. } => "IndexUpdated",
        }
    }
}

/// A handle returned by [`CatalogEventBus::subscribe`] allowing later removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// Maximum number of subscribers allowed on a single bus.
pub const MAX_SUBSCRIBERS: usize = 100;

type EventHandler = Arc<dyn Fn(&CatalogEvent) + Send + Sync>;

/// A subscriber entry pairing an id with its callback.
struct Subscriber {
    id: SubscriptionId,
    callback: EventHandler,
}

/// Error type for event bus operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventBusError {
    #[error("maximum number of subscribers ({MAX_SUBSCRIBERS}) reached")]
    MaxSubscribersReached,
}

/// An in-memory event bus for [`CatalogEvent`]s.
///
/// Subscribers register callbacks and are notified in registration order when
/// events are emitted. The bus enforces a hard cap of [`MAX_SUBSCRIBERS`]
/// subscribers.
pub struct CatalogEventBus {
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
    next_id: Arc<AtomicU64>,
    last_event: Arc<Mutex<Option<String>>>,
}

impl Default for CatalogEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogEventBus {
    /// Creates a new empty event bus.
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            last_event: Arc::new(Mutex::new(None)),
        }
    }

    /// Registers a handler for catalog events.
    ///
    /// Returns a [`SubscriptionId`] that can be used with [`unsubscribe`].
    /// Returns [`EventBusError::MaxSubscribersReached`] if the bus is full.
    ///
    /// [`unsubscribe`]: Self::unsubscribe
    pub fn subscribe<F>(&self, handler: F) -> Result<SubscriptionId, EventBusError>
    where
        F: Fn(&CatalogEvent) + Send + Sync + 'static,
    {
        let callback: EventHandler = Arc::new(handler);
        let mut subs = self.subscribers.lock().unwrap();
        if subs.len() >= MAX_SUBSCRIBERS {
            return Err(EventBusError::MaxSubscribersReached);
        }
        let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        subs.push(Subscriber { id, callback });
        Ok(id)
    }

    /// Removes a previously registered subscription.
    ///
    /// Returns `true` if a subscription was removed, `false` otherwise.
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut subs = self.subscribers.lock().unwrap();
        if let Some(pos) = subs.iter().position(|s| s.id == id) {
            subs.remove(pos);
            true
        } else {
            false
        }
    }

    /// Emits an event to all current subscribers.
    ///
    /// Subscribers are invoked in registration order. The callback receives a
    /// shared reference to the event.
    pub fn emit(&self, event: &CatalogEvent) {
        let event_name = event.variant_name().to_string();
        let callbacks: Vec<EventHandler> = {
            let subs = self.subscribers.lock().unwrap();
            subs.iter().map(|s| s.callback.clone()).collect()
        };

        for cb in callbacks {
            cb(event);
        }

        *self.last_event.lock().unwrap() = Some(event_name);
    }

    /// Returns the current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }

    /// Returns stats about the event bus.
    ///
    /// Returns a tuple of `(active_subscribers, last_event_name)`.
    pub fn stats(&self) -> (usize, Option<String>) {
        let count = self.subscribers.lock().unwrap().len();
        let last = self.last_event.lock().unwrap().clone();
        (count, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[test]
    fn emit_delivers() {
        let bus = CatalogEventBus::new();
        let received = Arc::new(AtomicUsize::new(0));
        let received_ref = received.clone();

        bus.subscribe(move |event| {
            if matches!(event, CatalogEvent::PluginLoaded { .. }) {
                received_ref.fetch_add(1, AtomicOrdering::SeqCst);
            }
        })
        .unwrap();

        bus.emit(&CatalogEvent::PluginLoaded {
            plugin_id: "test-plugin".to_string(),
        });

        assert_eq!(received.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn subscribe_unsubscribe() {
        let bus = CatalogEventBus::new();
        let received = Arc::new(AtomicUsize::new(0));
        let received_clone = received.clone();

        let id = bus
            .subscribe(move |_event| {
                received_clone.fetch_add(1, AtomicOrdering::SeqCst);
            })
            .unwrap();

        bus.emit(&CatalogEvent::IndexUpdated {
            provider_count: 1,
            model_count: 5,
        });
        assert_eq!(received.load(AtomicOrdering::SeqCst), 1);

        assert!(bus.unsubscribe(id));
        assert!(!bus.unsubscribe(id));

        bus.emit(&CatalogEvent::IndexUpdated {
            provider_count: 2,
            model_count: 10,
        });
        assert_eq!(received.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn max_subscribers() {
        let bus = CatalogEventBus::new();

        for _ in 0..MAX_SUBSCRIBERS {
            bus.subscribe(|_| {}).unwrap();
        }

        let result = bus.subscribe(|_| {});
        assert_eq!(result, Err(EventBusError::MaxSubscribersReached));
    }

    #[test]
    fn stats_track() {
        let bus = CatalogEventBus::new();

        // Initially zero subscribers and no last event.
        let (count, last) = bus.stats();
        assert_eq!(count, 0);
        assert!(last.is_none());

        let id = bus
            .subscribe(|event| {
                let _ = event;
            })
            .unwrap();

        let (count, _) = bus.stats();
        assert_eq!(count, 1);

        bus.emit(&CatalogEvent::PluginLoaded {
            plugin_id: "my-plugin".to_string(),
        });

        let (count, last) = bus.stats();
        assert_eq!(count, 1);
        assert_eq!(last, Some("PluginLoaded".to_string()));

        assert!(bus.unsubscribe(id));

        let (count, _) = bus.stats();
        assert_eq!(count, 0);
    }

    #[test]
    fn unload_event_works() {
        let bus = CatalogEventBus::new();
        let received = Arc::new(AtomicUsize::new(0));
        let received_ref = received.clone();

        bus.subscribe(move |event| {
            if let CatalogEvent::PluginUnloaded { plugin_id } = event {
                assert_eq!(plugin_id, "unloading-plugin");
                received_ref.fetch_add(1, AtomicOrdering::SeqCst);
            }
        })
        .unwrap();

        bus.emit(&CatalogEvent::PluginUnloaded {
            plugin_id: "unloading-plugin".to_string(),
        });

        assert_eq!(received.load(AtomicOrdering::SeqCst), 1);
    }
}

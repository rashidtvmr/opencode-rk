//! EXT-004 deferred reload/watch boundary: bounded, auditable record of
//! reload requests and watcher interest with zero filesystem watching,
//! zero host launch, zero external activation.
//!
//! Pure in-memory, synchronous, non-blocking; no I/O, no threads, no clock.
#![forbid(unsafe_code)]

use thiserror::Error;

/// Maximum deferred events retained before [`DeferredLog`] overflows.
pub const EXT4_MAX_DEFERRED: usize = 128;

/// Maximum watcher-interest records retained before overflow.
pub const EXT4_MAX_WATCHERS: usize = 32;

/// Maximum watch-label length in bytes (labels are ASCII-only, so bytes == chars).
const MAX_LABEL_LEN: usize = 64;

/// Opaque plugin handle. The boundary records intent only and never
/// validates ids against any registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(pub u64);

/// Why an event is deferred instead of executed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredReason {
    /// An external JS/TS activation attempt, refused-by-design here.
    ExternalActivationDeferred,
    /// A watcher registration: interest recorded, no OS watcher started.
    WatchDeferred,
    /// A reload request: recorded, never re-executes anything.
    ReloadDeferred,
}

/// One bounded deferred-activation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredEvent {
    /// Monotonic sequence, starts at 1, never reused after [`DeferredLog::drain`].
    pub seq: u64,
    /// `None` = registry-wide request or watcher-interest record.
    pub plugin: Option<PluginId>,
    pub reason: DeferredReason,
}

/// Deferred-boundary failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BoundaryError {
    /// Event vec (`len == EXT4_MAX_DEFERRED`) or watcher table
    /// (`len == EXT4_MAX_WATCHERS`) full; state unchanged.
    #[error("deferred log or watcher table full")]
    Overflow,
    /// Watch label already registered; state unchanged, no id consumed.
    #[error("duplicate watcher label")]
    Duplicate,
    /// Watch label empty, too long, or outside
    /// `[A-Za-z0-9][A-Za-z0-9._-]*`; state unchanged.
    #[error("invalid watch label")]
    InvalidLabel,
}

/// One watcher-interest record: label only, no OS handle, no thread.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Watcher {
    id: u32,
    label: String,
}

fn label_ok(label: &str) -> bool {
    if label.is_empty() || label.len() > MAX_LABEL_LEN {
        return false;
    }
    let mut bytes = label.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// Bounded record of deferred reload/external/watch intent.
///
/// Caller-owned lifetime; all methods synchronous and constant-time.
/// Starts zero OS watchers, zero threads, zero file handles.
#[derive(Debug, Clone, Default)]
pub struct DeferredLog {
    events: Vec<DeferredEvent>,
    watchers: Vec<Watcher>,
    next_seq: u64,
    next_watcher: u32,
}

impl DeferredLog {
    /// Empty log: no allocation beyond empty vecs, no I/O, no threads.
    pub fn new() -> Self {
        DeferredLog {
            events: Vec::new(),
            watchers: Vec::new(),
            next_seq: 1,
            next_watcher: 1,
        }
    }

    fn push(&mut self, plugin: Option<PluginId>, reason: DeferredReason) -> Result<u64, BoundaryError> {
        if self.events.len() >= EXT4_MAX_DEFERRED {
            return Err(BoundaryError::Overflow);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(DeferredEvent {
            seq,
            plugin,
            reason,
        });
        Ok(seq)
    }

    /// Record a reload request as `ReloadDeferred`, return its `seq`.
    /// Never touches any registry: no add, remove, or readiness change.
    /// Unknown plugin ids still record intent.
    pub fn request_reload(&mut self, plugin: Option<PluginId>) -> Result<u64, BoundaryError> {
        self.push(plugin, DeferredReason::ReloadDeferred)
    }

    /// Record a refused-by-design external-activation attempt as
    /// `ExternalActivationDeferred`, return its `seq`.
    /// Zero host launch, zero `dlopen`, zero subprocess.
    pub fn note_external(&mut self, plugin: Option<PluginId>) -> Result<u64, BoundaryError> {
        self.push(plugin, DeferredReason::ExternalActivationDeferred)
    }

    /// Record watcher interest only, return the watcher id.
    /// Starts zero OS watchers, zero threads, zero file handles.
    /// Also appends a `WatchDeferred` event so the log stays auditable.
    pub fn add_watcher(&mut self, label: &str) -> Result<u32, BoundaryError> {
        if !label_ok(label) {
            return Err(BoundaryError::InvalidLabel);
        }
        if self.watchers.iter().any(|w| w.label == label) {
            return Err(BoundaryError::Duplicate);
        }
        if self.watchers.len() >= EXT4_MAX_WATCHERS {
            return Err(BoundaryError::Overflow);
        }
        if self.events.len() >= EXT4_MAX_DEFERRED {
            return Err(BoundaryError::Overflow);
        }
        let id = self.next_watcher;
        self.next_watcher += 1;
        self.watchers.push(Watcher {
            id,
            label: label.to_string(),
        });
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(DeferredEvent {
            seq,
            plugin: None,
            reason: DeferredReason::WatchDeferred,
        });
        Ok(id)
    }

    /// Drop one watcher-interest record. Unknown id => `false`, log
    /// otherwise unchanged. The event history is append-only and untouched.
    pub fn remove_watcher(&mut self, id: u32) -> bool {
        match self.watchers.iter().position(|w| w.id == id) {
            Some(i) => {
                self.watchers.remove(i);
                true
            }
            None => false,
        }
    }

    /// Pending events sorted by `seq` ascending.
    pub fn list_deferred(&self) -> Vec<DeferredEvent> {
        let mut out = self.events.clone();
        out.sort_by_key(|e| e.seq);
        out
    }

    /// Remove and return the oldest `n` events (or fewer).
    /// `drain(0)` is a no-op; `drain(n > len)` empties; never panics.
    /// Sequence numbers are never reused.
    pub fn drain(&mut self, n: usize) -> Vec<DeferredEvent> {
        let k = n.min(self.events.len());
        self.events.drain(..k).collect()
    }
}

//! OPS-003: local-only repository cache-store lifecycle.
//! Bounded, offline, no network. Transport is an explicit boundary this
//! slice never calls; [`NoNetworkTransport`] refuses with a typed error.
#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};

/// Marker files are tiny tick records; anything larger is corrupt.
pub const MAX_MARKER_BYTES: usize = 4096;
/// Slot-id charset/length bound (caller-visible).
pub const MAX_SLOT_ID_LEN: usize = 64;
/// Cap on ids processed in one sweep listing pass.
pub const MAX_SWEEP_IDS: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelPath(String);

impl RelPath {
    pub fn new(s: &str) -> Result<Self, StoreError> {
        if s.is_empty() || s.len() > 2048 || s.contains('\0') {
            return Err(StoreError::InvalidSlot(s.to_string()));
        }
        let p = Path::new(s);
        if p.is_absolute() {
            return Err(StoreError::InvalidSlot(s.to_string()));
        }
        for comp in p.components() {
            use std::path::Component::*;
            match comp {
                CurDir | ParentDir | Prefix(_) | RootDir => {
                    return Err(StoreError::InvalidSlot(s.to_string()))
                }
                Normal(_) => {}
            }
        }
        if s.split('/').any(|seg| seg.is_empty()) {
            return Err(StoreError::InvalidSlot(s.to_string()));
        }
        Ok(RelPath(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotState {
    Missing,
    Fresh,
    Stale { reason: String },
    Corrupt { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheSlot {
    pub cache_id: String,
    pub path: RelPath,
    pub branch: String,
    pub state: SlotState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheCfg {
    pub max_slots: usize,
    pub max_bytes: u64,
    pub stale_after_idle: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPolicy(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepReport {
    pub removed: Vec<CacheSlot>,
    pub retained: Vec<CacheSlot>,
    pub freed_bytes: u64,
    pub policy: RetentionPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    InvalidCap(String),
    RootUnreadable,
    InvalidSlot(String),
    SlotLocked,
    Cancelled,
    TransportRefused,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::InvalidCap(n) => write!(f, "invalid cap: {n}"),
            StoreError::RootUnreadable => write!(f, "root unreadable"),
            StoreError::InvalidSlot(_) => write!(f, "invalid slot"),
            StoreError::SlotLocked => write!(f, "slot locked"),
            StoreError::Cancelled => write!(f, "cancelled"),
            StoreError::TransportRefused => write!(f, "transport refused"),
        }
    }
}

impl std::error::Error for StoreError {}

fn valid_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_SLOT_ID_LEN {
        return false;
    }
    let mut ch = id.bytes();
    match ch.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    id.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

fn marker_bytes(tick: u64) -> Vec<u8> {
    format!("v1 tick={tick}\n").into_bytes()
}

fn parse_marker(bytes: &[u8]) -> Option<u64> {
    if bytes.len() > MAX_MARKER_BYTES {
        return None;
    }
    let s = std::str::from_utf8(bytes).ok()?;
    let rest = s.strip_prefix("v1 tick=")?;
    rest.strip_suffix('\n').unwrap_or(rest).parse::<u64>().ok()
}

pub struct CacheStore {
    root: PathBuf,
    slots_dir: PathBuf,
    cfg: CacheCfg,
    registry: Mutex<BTreeSet<String>>,
    locked: Mutex<HashSet<String>>,
    max_tick: AtomicU64,
}

impl CacheStore {
    pub fn open(root: &Path, cfg: CacheCfg) -> Result<Self, StoreError> {
        if cfg.max_slots == 0 {
            return Err(StoreError::InvalidCap("max_slots".to_string()));
        }
        if cfg.max_bytes == 0 {
            return Err(StoreError::InvalidCap("max_bytes".to_string()));
        }
        if !root.is_dir() {
            return Err(StoreError::RootUnreadable);
        }
        let slots_dir = root.join("slots");
        if let Err(_) = std::fs::create_dir_all(&slots_dir) {
            return Err(StoreError::RootUnreadable);
        }
        Ok(CacheStore {
            root: root.to_path_buf(),
            slots_dir,
            cfg,
            registry: Mutex::new(BTreeSet::new()),
            locked: Mutex::new(HashSet::new()),
            max_tick: AtomicU64::new(0),
        })
    }

    fn register(&self, id: &str) {
        if let Ok(mut r) = self.registry.lock() {
            r.insert(id.to_string());
        }
    }

    fn is_locked(&self, id: &str) -> bool {
        self.locked.lock().map(|l| l.contains(id)).unwrap_or(false)
    }

    fn slot_dir(&self, id: &str) -> PathBuf {
        self.slots_dir.join(id)
    }

    fn marker_path(&self, id: &str) -> PathBuf {
        self.slot_dir(id).join("fresh.marker")
    }

    fn read_state(&self, id: &str, now: Option<u64>) -> SlotState {
        let bytes = match std::fs::read(self.marker_path(id)) {
            Ok(b) => b,
            Err(_) => return SlotState::Missing,
        };
        let tick = match parse_marker(&bytes) {
            Some(t) => t,
            None => {
                return SlotState::Corrupt {
                    reason: "unparseable".to_string(),
                }
            }
        };
        let now = now.unwrap_or_else(|| self.max_tick.load(Ordering::SeqCst));
        if now.saturating_sub(tick) >= self.cfg.stale_after_idle.max(1) {
            SlotState::Stale {
                reason: "idle".to_string(),
            }
        } else {
            SlotState::Fresh
        }
    }

    pub fn mark_fresh(&self, slot: &CacheSlot, tick: u64) -> Result<(), StoreError> {
        if !valid_id(&slot.cache_id) {
            return Err(StoreError::InvalidSlot(slot.cache_id.clone()));
        }
        if self.is_locked(&slot.cache_id) {
            return Err(StoreError::SlotLocked);
        }
        let dir = self.slot_dir(&slot.cache_id);
        std::fs::create_dir_all(&dir).map_err(|_| StoreError::RootUnreadable)?;
        // crash-safe write-temp-then-rename of one bounded marker
        let tmp = dir.join("fresh.marker.tmp");
        std::fs::write(&tmp, marker_bytes(tick)).map_err(|_| StoreError::RootUnreadable)?;
        std::fs::rename(&tmp, self.marker_path(&slot.cache_id))
            .map_err(|_| StoreError::RootUnreadable)?;
        self.register(&slot.cache_id);
        self.max_tick.fetch_max(tick, Ordering::SeqCst);
        Ok(())
    }

    pub fn inspect(&self, slot: &CacheSlot) -> SlotState {
        self.register(&slot.cache_id);
        self.read_state(&slot.cache_id, None)
    }

    pub fn hold_lock(&self, slot: &CacheSlot) -> Result<SlotGuard<'_>, StoreError> {
        let mut l = self.locked.lock().map_err(|_| StoreError::SlotLocked)?;
        if !l.insert(slot.cache_id.clone()) {
            return Err(StoreError::SlotLocked);
        }
        Ok(SlotGuard {
            store: self,
            id: slot.cache_id.clone(),
        })
    }

    fn release(&self, id: &str) {
        if let Ok(mut l) = self.locked.lock() {
            l.remove(id);
        }
    }

    fn snapshot(&self, slot: &CacheSlot, state: SlotState) -> CacheSlot {
        CacheSlot {
            cache_id: slot.cache_id.clone(),
            path: slot.path.clone(),
            branch: slot.branch.clone(),
            state,
        }
    }

    fn lookup_slot(&self, id: &str) -> CacheSlot {
        // registry stores ids only; rebuild a canonical slot view
        CacheSlot {
            cache_id: id.to_string(),
            path: RelPath::new(&format!("slots/{id}"))
                .unwrap_or(RelPath::new("slots/x").unwrap()),
            branch: "main".to_string(),
            state: SlotState::Missing,
        }
    }

    fn collect_ids(&self) -> Vec<String> {
        let mut ids = BTreeSet::new();
        if let Ok(r) = self.registry.lock() {
            for id in r.iter().take(MAX_SWEEP_IDS) {
                ids.insert(id.clone());
            }
        }
        // streamed bounded directory listing
        if let Ok(rd) = std::fs::read_dir(&self.slots_dir) {
            for ent in rd.flatten().take(MAX_SWEEP_IDS) {
                if let Some(n) = ent.file_name().to_str() {
                    if valid_id(n) {
                        ids.insert(n.to_string());
                    }
                }
                if ids.len() >= MAX_SWEEP_IDS {
                    break;
                }
            }
        }
        ids.into_iter().collect()
    }

    fn blob_bytes(&self, id: &str) -> u64 {
        std::fs::metadata(self.slot_dir(id).join("blob.bin"))
            .map(|m| m.len())
            .unwrap_or(0)
    }

    pub fn sweep_slot(&self, slot: &CacheSlot, tick: u64) -> Result<bool, StoreError> {
        if self.is_locked(&slot.cache_id) {
            return Err(StoreError::SlotLocked);
        }
        match self.read_state(&slot.cache_id, Some(tick)) {
            SlotState::Stale { .. } => {
                let freed = self.blob_bytes(&slot.cache_id);
                let _ = freed;
                let _ = std::fs::remove_dir_all(self.slot_dir(&slot.cache_id));
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn sweep(&self, tick: u64) -> Result<SweepReport, StoreError> {
        self.sweep_inner(tick, None)
    }

    fn sweep_inner(
        &self,
        tick: u64,
        cancel: Option<&AtomicBool>,
    ) -> Result<SweepReport, StoreError> {
        let mut removed = Vec::new();
        let mut retained = Vec::new();
        let mut freed_bytes = 0u64;
        for id in self.collect_ids() {
            if let Some(c) = cancel {
                if c.load(Ordering::SeqCst) {
                    return Err(StoreError::Cancelled);
                }
            }
            if self.is_locked(&id) {
                retained.push(self.snapshot(&self.lookup_slot(&id), SlotState::Fresh));
                continue;
            }
            match self.read_state(&id, Some(tick)) {
                SlotState::Stale { .. } => {
                    if removed.len() >= self.cfg.max_slots {
                        let cur = self.read_state(&id, Some(tick));
                        retained.push(self.snapshot(&self.lookup_slot(&id), cur));
                        continue;
                    }
                    freed_bytes = freed_bytes.saturating_add(self.blob_bytes(&id));
                    let _ = std::fs::remove_dir_all(self.slot_dir(&id));
                    let mut s = self.lookup_slot(&id);
                    s.state = SlotState::Stale {
                        reason: "idle".to_string(),
                    };
                    removed.push(s);
                }
                st => {
                    retained.push(self.snapshot(&self.lookup_slot(&id), st));
                }
            }
        }
        removed.sort_by(|a, b| a.cache_id.cmp(&b.cache_id));
        retained.sort_by(|a, b| a.cache_id.cmp(&b.cache_id));
        Ok(SweepReport {
            removed,
            retained,
            freed_bytes,
            policy: RetentionPolicy(format!(
                "stale_after_idle={} max_slots={}",
                self.cfg.stale_after_idle, self.cfg.max_slots
            )),
        })
    }

    /// Basename-only one-line log; never includes blob bytes or root paths.
    pub fn log_line(&self, slot: &CacheSlot) -> String {
        format!(
            "cache_id={} marker=fresh.marker state={:?}",
            slot.cache_id,
            self.read_state(&slot.cache_id, None)
        )
    }
}

pub struct SlotGuard<'a> {
    store: &'a CacheStore,
    id: String,
}

impl<'a> Drop for SlotGuard<'a> {
    fn drop(&mut self) {
        self.store.release(&self.id);
    }
}

pub fn sweep_cancel(
    store: &CacheStore,
    tick: u64,
    cancel: &AtomicBool,
) -> Result<SweepReport, StoreError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(StoreError::Cancelled);
    }
    store.sweep_inner(tick, Some(cancel))
}

pub trait Transport {
    fn fetch(&self, id: &str) -> Result<(), StoreError>;
    fn checkout(&self, id: &str, branch: &str) -> Result<(), StoreError>;
    fn reset(&self, id: &str) -> Result<(), StoreError>;
}

/// Test default: counts invocations, always refuses with a typed error.
#[derive(Default)]
pub struct NoNetworkTransport {
    calls: AtomicU64,
}

impl NoNetworkTransport {
    pub fn invocations(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Transport for NoNetworkTransport {
    fn fetch(&self, _id: &str) -> Result<(), StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::TransportRefused)
    }
    fn checkout(&self, _id: &str, _branch: &str) -> Result<(), StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::TransportRefused)
    }
    fn reset(&self, _id: &str) -> Result<(), StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::TransportRefused)
    }
}

// Keep BTreeMap referenced for future per-slot metadata without dead-dep churn.
#[allow(dead_code)]
fn _shape_note(_m: &BTreeMap<String, String>) -> usize {
    _m.len()
}

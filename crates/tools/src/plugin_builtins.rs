//! Built-in native plugin wiring (EXT-002) over the EXT-001 registry contract.
//!
//! Fixed auditable table of built-in plugin declarations registered into a
//! caller-provided [`PluginRegistry`] at startup, marked ready without any
//! external host, removed cleanly on shutdown. No JS host, no filesystem
//! scan, no network, no threads, no persistence, no environment lookup.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod plugin_builtins;` into `lib.rs` later. No dependency on
//! other crate modules.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;

/// Contract version this wiring speaks. Must equal the EXT-001 value.
pub const SUPPORTED_CONTRACT_VERSION: u32 = 1;
/// Maximum plugins one registry may hold (inherits EXT-001 bound).
pub const MAX_PLUGINS: usize = 64;
/// Maximum capabilities per declaration (inherits EXT-001 bound).
pub const MAX_CAPABILITIES: usize = 16;
/// Maximum name bytes (inherits EXT-001 bound).
pub const MAX_NAME_LEN: usize = 128;
/// Maximum single capability bytes (inherits EXT-001 bound).
pub const MAX_CAP_LEN: usize = 64;

/// Fixed auditable table of built-in plugin name => capability labels.
/// Exactly 2 entries, each with 0..=4 capabilities.
pub const BUILTINS: &[(&str, &[&str])] =
    &[("core.commands", &[]), ("core.skills", &["skill.list"])];

/// Monotonically increasing plugin id, per registry, starting at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(pub u64);

/// Native plugin declaration: name + contract version + capability labels.
/// Holds labels only; no file bodies, no credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDecl {
    pub name: String,
    pub contract_version: u32,
    pub capabilities: Vec<String>,
}

/// Lifecycle state of one registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Registered,
    Ready,
}

/// One registry entry: id + declaration + state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    pub id: PluginId,
    pub decl: PluginDecl,
    pub state: PluginState,
}

/// Typed lifecycle failures (EXT-001 contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PluginError {
    #[error("invalid plugin name")]
    InvalidName,
    #[error("duplicate plugin name")]
    Duplicate,
    #[error("invalid capabilities")]
    InvalidCapabilities,
    #[error("unsupported contract version")]
    UnsupportedContract,
    #[error("plugin registry full")]
    Overflow,
    #[error("unknown plugin id")]
    Unknown,
    #[error("wait cancelled")]
    Cancelled,
}

fn name_ok(n: &str) -> bool {
    if n.is_empty() || n.len() > MAX_NAME_LEN {
        return false;
    }
    let mut chars = n.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn caps_ok(caps: &[String]) -> bool {
    if caps.len() > MAX_CAPABILITIES {
        return false;
    }
    for (i, c) in caps.iter().enumerate() {
        if c.is_empty() || c.len() > MAX_CAP_LEN || !name_ok(c) {
            return false;
        }
        if caps[..i].contains(c) {
            return false;
        }
    }
    true
}

/// Scoped native plugin registry: one caller-owned in-memory store.
/// No I/O, no threads, no host process. Deterministic: same
/// add/mark_ready/remove sequence yields identical `list()` output.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    entries: Vec<PluginInfo>,
    next: u64,
}

impl PluginRegistry {
    /// Empty registry. Allocates only the empty vec; no threads, no watchers.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next: 1,
        }
    }

    /// Validate `decl`, reject duplicates and overflow, insert as
    /// `Registered`, return the fresh id. Any error leaves the registry
    /// unchanged and consumes no id.
    pub fn add(&mut self, decl: PluginDecl) -> Result<PluginId, PluginError> {
        if !name_ok(&decl.name) {
            return Err(PluginError::InvalidName);
        }
        if !caps_ok(&decl.capabilities) {
            return Err(PluginError::InvalidCapabilities);
        }
        if decl.contract_version != SUPPORTED_CONTRACT_VERSION {
            return Err(PluginError::UnsupportedContract);
        }
        if self.entries.iter().any(|e| e.decl.name == decl.name) {
            return Err(PluginError::Duplicate);
        }
        if self.entries.len() >= MAX_PLUGINS {
            return Err(PluginError::Overflow);
        }
        let id = PluginId(self.next);
        self.next += 1;
        self.entries.push(PluginInfo {
            id,
            decl,
            state: PluginState::Registered,
        });
        Ok(id)
    }

    /// Owner hook modelling scoped activation completing: flip one entry to
    /// `Ready`. Unknown id => `Err(Unknown)`; no state change.
    pub fn mark_ready(&mut self, id: PluginId) -> Result<(), PluginError> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                e.state = PluginState::Ready;
                Ok(())
            }
            None => Err(PluginError::Unknown),
        }
    }

    /// Remove exactly `id`. Unknown id => `false`, registry unchanged.
    pub fn remove(&mut self, id: PluginId) -> bool {
        match self.entries.iter().position(|e| e.id == id) {
            Some(i) => {
                self.entries.remove(i);
                true
            }
            None => false,
        }
    }

    /// `Ok(())` iff the entry is `Ready`. A `Registered` entry parks in
    /// bounded 1 ms slices and returns `Cancelled` promptly once `cancel`
    /// is set (checks at least every 1 ms, returns within 50 ms). Unknown
    /// or removed id => `Unknown`. Spawns no thread, executes no callback.
    pub fn wait_ready(&self, id: PluginId, cancel: &AtomicBool) -> Result<(), PluginError> {
        loop {
            match self.entries.iter().find(|e| e.id == id) {
                None => return Err(PluginError::Unknown),
                Some(e) if e.state == PluginState::Ready => return Ok(()),
                Some(_) => {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(PluginError::Cancelled);
                    }
                    std::thread::park_timeout(Duration::from_millis(1));
                    if cancel.load(Ordering::SeqCst) {
                        return Err(PluginError::Cancelled);
                    }
                }
            }
        }
    }

    /// All entries sorted by `PluginId` ascending (insertion order).
    pub fn list(&self) -> Vec<PluginInfo> {
        let mut out = self.entries.clone();
        out.sort_by_key(|e| e.id.0);
        out
    }
}

/// Register every [`BUILTINS`] entry in table order with
/// `contract_version`, then `mark_ready` each. Returns ids in table order.
///
/// Atomic per call: on the first validation failure returns the EXT-001
/// typed error and rolls back any ids already added in this call, leaving
/// the registry byte-identical to its entry state.
pub fn register_builtins(
    reg: &mut PluginRegistry,
    contract_version: u32,
) -> Result<Vec<PluginId>, PluginError> {
    if contract_version != SUPPORTED_CONTRACT_VERSION {
        return Err(PluginError::UnsupportedContract);
    }
    if reg.entries.len() + BUILTINS.len() > MAX_PLUGINS {
        return Err(PluginError::Overflow);
    }
    let mut added: Vec<PluginId> = Vec::with_capacity(BUILTINS.len());
    for (name, caps) in BUILTINS {
        let decl = PluginDecl {
            name: (*name).to_string(),
            contract_version,
            capabilities: caps.iter().map(|c| (*c).to_string()).collect(),
        };
        match reg.add(decl) {
            Ok(id) => added.push(id),
            Err(e) => {
                for id in added {
                    reg.remove(id);
                }
                return Err(e);
            }
        }
    }
    for id in &added {
        if let Err(e) = reg.mark_ready(*id) {
            for id in &added {
                reg.remove(*id);
            }
            return Err(e);
        }
    }
    Ok(added)
}

/// Remove each id in order; returns the count actually removed. Unknown
/// ids are skipped harmlessly; never errors on unknown.
pub fn unregister_builtins(reg: &mut PluginRegistry, ids: &[PluginId]) -> usize {
    let mut removed = 0;
    for id in ids {
        if reg.remove(*id) {
            removed += 1;
        }
    }
    removed
}

/// `true` iff `name` is in [`BUILTINS`]. Exact match; no trimming, no
/// case folding.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|(n, _)| *n == name)
}

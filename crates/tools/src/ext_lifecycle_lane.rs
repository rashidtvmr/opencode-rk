//! Scoped native plugin add/remove/wait lifecycle (EXT-001).
//!
//! Pure in-memory registry: no I/O, no threads, no clock in ordering or ids.
//! The caller owns the [`PluginRegistry`] lifetime and the [`AtomicBool`]
//! cancel flag; [`PluginRegistry::wait_ready`] parks in bounded 1 ms slices
//! and never spawns a thread.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod ext_lifecycle_lane;` into `lib.rs` later. No dependency on
//! other crate modules.
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;

/// Maximum plugins retained by one registry.
pub const MAX_PLUGINS: usize = 64;
/// Maximum capabilities per declaration.
pub const MAX_CAPABILITIES: usize = 16;
/// Maximum plugin-name length in bytes.
pub const MAX_NAME_LEN: usize = 128;
/// Maximum single-capability length in bytes.
pub const MAX_CAP_LEN: usize = 64;
/// Only contract version accepted by [`PluginRegistry::add`].
pub const SUPPORTED_CONTRACT_VERSION: u32 = 1;

/// Contract version of a plugin declaration.
pub type ContractVersion = u32;

/// Monotonically increasing plugin id, starts at 1 per registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginId(pub u64);

/// Native plugin declaration: name/labels only, never file bodies or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDecl {
    pub name: String,
    pub contract_version: ContractVersion,
    pub capabilities: Vec<String>,
}

/// Lifecycle state of one registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Registered,
    Ready,
}

/// Stored registration: id + declaration + state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    pub id: PluginId,
    pub decl: PluginDecl,
    pub state: PluginState,
}

/// Lifecycle failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
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

fn valid_label(s: &str, max_len: usize) -> bool {
    if s.is_empty() || s.len() > max_len {
        return false;
    }
    let mut chars = s.bytes();
    match chars.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// In-memory scoped plugin registry. Single owner, no shared global state.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    entries: Vec<PluginInfo>,
    next: u64,
}

impl PluginRegistry {
    /// Empty registry: allocates only the empty vec, no threads or watchers.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next: 1,
        }
    }

    /// Validate and register `decl` as `Registered`, returning its id.
    /// Any rejection leaves the registry unchanged and consumes no id.
    pub fn add(&mut self, decl: PluginDecl) -> Result<PluginId, PluginError> {
        if !valid_label(&decl.name, MAX_NAME_LEN) {
            return Err(PluginError::InvalidName);
        }
        if decl.contract_version != SUPPORTED_CONTRACT_VERSION {
            return Err(PluginError::UnsupportedContract);
        }
        if decl.capabilities.len() > MAX_CAPABILITIES {
            return Err(PluginError::InvalidCapabilities);
        }
        let mut seen = Vec::with_capacity(decl.capabilities.len());
        for cap in &decl.capabilities {
            if !valid_label(cap, MAX_CAP_LEN) || seen.contains(cap) {
                return Err(PluginError::InvalidCapabilities);
            }
            seen.push(cap.clone());
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

    /// Flip `id` to `Ready`. In-memory state flip modelling scoped
    /// activation completing; launches no host. Unknown id => `Unknown`.
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

    /// Block until `id` is `Ready`. Returns `Ok(())` iff the state is
    /// `Ready`; a `Registered` plugin parks in bounded 1 ms slices and
    /// returns `Cancelled` promptly once `cancel` is set. Unknown or
    /// removed id => `Unknown`. Spawns no thread, runs no callback.
    pub fn wait_ready(&self, id: PluginId, cancel: &AtomicBool) -> Result<(), PluginError> {
        loop {
            match self.entries.iter().find(|e| e.id == id) {
                None => return Err(PluginError::Unknown),
                Some(e) if e.state == PluginState::Ready => return Ok(()),
                Some(_) => {}
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(PluginError::Cancelled);
            }
            std::thread::park_timeout(Duration::from_millis(1));
        }
    }

    /// Current registrations sorted by `PluginId` ascending.
    pub fn list(&self) -> Vec<PluginInfo> {
        let mut out = self.entries.clone();
        out.sort_by_key(|e| e.id);
        out
    }
}

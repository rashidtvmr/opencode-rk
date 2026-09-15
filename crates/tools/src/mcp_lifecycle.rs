//! Bounded, caller-owned MCP lifecycle state.
//!
//! This module models server power and readiness only. It never starts a
//! process, registers a tool, reads persisted data, or performs network I/O.
//! A caller must explicitly drive every transition and owns the registry
//! lifetime.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum MCP servers retained by one registry.
pub const MAX_SERVERS: usize = 64;
/// Maximum MCP identifier length in bytes.
pub const MAX_ID_LEN: usize = 128;
/// Maximum safe error-code length in bytes.
pub const MAX_ERROR_CODE_LEN: usize = 64;
/// Maximum serialized persistence shape size in bytes.
pub const MAX_PERSISTED_BYTES: usize = 16 * 1024;

/// Lifecycle of one configured MCP server.
///
/// `Enabled` means that the user has enabled the server, not that a live
/// connection exists. Only `Ready` is eligible for tool registration or
/// invocation by a caller using [`is_runnable`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "error")]
    Error { code: String },
}

/// One configured MCP server and its lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEntry {
    pub id: String,
    pub state: LifecycleState,
}

/// A serializable safe persistence entry.
pub type PersistedEntry = LifecycleEntry;

/// Safe persisted lifecycle projection.
///
/// The shape contains identifiers and lifecycle state codes only. Runtime
/// handles, tools, commands, environments, credentials, and process output
/// have no representation here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedShape {
    pub entries: Vec<PersistedEntry>,
}

impl PersistedShape {
    /// Number of persisted entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the shape has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Persisted entries in deterministic identifier order.
    #[must_use]
    pub fn entries(&self) -> &[PersistedEntry] {
        &self.entries
    }

    /// Encode this shape after enforcing the persistence byte bound.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LifecycleError> {
        let bytes = serde_json::to_vec(self).map_err(|_| LifecycleError::InvalidPersisted)?;
        if bytes.len() > MAX_PERSISTED_BYTES {
            return Err(LifecycleError::PersistedTooLarge);
        }
        Ok(bytes)
    }
}

/// Lifecycle operation failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LifecycleError {
    #[error("MCP id is empty")]
    EmptyId,
    #[error("MCP id exceeds the maximum length")]
    IdTooLong,
    #[error("MCP id is already registered")]
    Duplicate,
    #[error("MCP registry is full")]
    Overflow,
    #[error("unknown MCP id")]
    Unknown,
    #[error("MCP server is not enabled")]
    NotEnabled,
    #[error("MCP error code exceeds the maximum length")]
    CodeTooLong,
    #[error("persisted MCP state exceeds the byte bound")]
    PersistedTooLarge,
    #[error("invalid persisted MCP state")]
    InvalidPersisted,
}

fn validate_id(id: &str) -> Result<(), LifecycleError> {
    if id.is_empty() {
        return Err(LifecycleError::EmptyId);
    }
    if id.len() > MAX_ID_LEN {
        return Err(LifecycleError::IdTooLong);
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), LifecycleError> {
    if code.len() > MAX_ERROR_CODE_LEN {
        return Err(LifecycleError::CodeTooLong);
    }
    Ok(())
}

fn persisted_state(state: &LifecycleState) -> LifecycleState {
    match state {
        LifecycleState::Disabled => LifecycleState::Disabled,
        LifecycleState::Enabled => LifecycleState::Enabled,
        LifecycleState::Starting | LifecycleState::Ready => LifecycleState::Enabled,
        LifecycleState::Error { code } => LifecycleState::Error { code: code.clone() },
    }
}

/// Caller-owned bounded MCP lifecycle registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleRegistry {
    entries: Vec<LifecycleEntry>,
}

impl LifecycleRegistry {
    /// Create an empty registry. No service is started.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a configured MCP server in the disabled state.
    ///
    /// Rejections leave the registry unchanged.
    pub fn add(&mut self, id: impl Into<String>) -> Result<(), LifecycleError> {
        let id = id.into();
        validate_id(&id)?;
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(LifecycleError::Duplicate);
        }
        if self.entries.len() >= MAX_SERVERS {
            return Err(LifecycleError::Overflow);
        }
        self.entries.push(LifecycleEntry {
            id,
            state: LifecycleState::Disabled,
        });
        self.sort_entries();
        Ok(())
    }

    /// Alias for [`LifecycleRegistry::add`].
    pub fn register(&mut self, id: impl Into<String>) -> Result<(), LifecycleError> {
        self.add(id)
    }

    /// Alias for [`LifecycleRegistry::add`].
    pub fn add_server(&mut self, id: impl Into<String>) -> Result<(), LifecycleError> {
        self.add(id)
    }

    /// Number of configured servers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no servers are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return entries sorted by identifier.
    #[must_use]
    pub fn list(&self) -> Vec<LifecycleEntry> {
        self.entries.clone()
    }

    /// Look up an entry by identifier.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&LifecycleEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Return the current state, or [`LifecycleError::Unknown`].
    pub fn state(&self, id: &str) -> Result<LifecycleState, LifecycleError> {
        self.lookup(id).map(|entry| entry.state.clone())
    }

    /// Toggle the user power preference.
    ///
    /// Enabling is idempotent for enabled or live states. An error state must
    /// use [`LifecycleRegistry::reconnect`] explicitly. Disabling always
    /// removes runnability immediately by moving the entry to `Disabled`.
    pub fn set_enabled(&mut self, id: &str, on: bool) -> Result<LifecycleState, LifecycleError> {
        let entry = self.lookup_mut(id)?;
        if !on {
            entry.state = LifecycleState::Disabled;
            return Ok(entry.state.clone());
        }

        match entry.state {
            LifecycleState::Disabled => entry.state = LifecycleState::Enabled,
            LifecycleState::Enabled | LifecycleState::Starting | LifecycleState::Ready => {}
            LifecycleState::Error { .. } => return Err(LifecycleError::NotEnabled),
        }
        Ok(entry.state.clone())
    }

    /// Move an enabled server to `Starting`.
    pub fn mark_starting(&mut self, id: &str) -> Result<LifecycleState, LifecycleError> {
        let entry = self.lookup_mut(id)?;
        match entry.state {
            LifecycleState::Enabled | LifecycleState::Starting => {
                entry.state = LifecycleState::Starting;
                Ok(entry.state.clone())
            }
            LifecycleState::Disabled | LifecycleState::Ready | LifecycleState::Error { .. } => {
                Err(LifecycleError::NotEnabled)
            }
        }
    }

    /// Mark an enabled or starting server ready.
    pub fn mark_ready(&mut self, id: &str) -> Result<LifecycleState, LifecycleError> {
        let entry = self.lookup_mut(id)?;
        match entry.state {
            LifecycleState::Enabled | LifecycleState::Starting | LifecycleState::Ready => {
                entry.state = LifecycleState::Ready;
                Ok(entry.state.clone())
            }
            LifecycleState::Disabled | LifecycleState::Error { .. } => {
                Err(LifecycleError::NotEnabled)
            }
        }
    }

    /// Move an enabled or starting server to a safe error code.
    pub fn mark_error(
        &mut self,
        id: &str,
        code: impl Into<String>,
    ) -> Result<LifecycleState, LifecycleError> {
        let entry = self.lookup_mut(id)?;
        if !matches!(
            entry.state,
            LifecycleState::Enabled | LifecycleState::Starting
        ) {
            return Err(LifecycleError::NotEnabled);
        }
        let code = code.into();
        validate_code(&code)?;
        entry.state = LifecycleState::Error { code };
        Ok(entry.state.clone())
    }

    /// Explicitly recover an enabled preference from an error state.
    ///
    /// This only changes state to `Enabled`; it never starts or reconnects a
    /// server. A disabled server cannot be revived by reconnect.
    pub fn reconnect(&mut self, id: &str) -> Result<LifecycleState, LifecycleError> {
        let entry = self.lookup_mut(id)?;
        if matches!(entry.state, LifecycleState::Disabled) {
            return Err(LifecycleError::NotEnabled);
        }
        entry.state = LifecycleState::Enabled;
        Ok(entry.state.clone())
    }

    /// Explicit refresh operation. Refresh has no implicit background work.
    pub fn refresh(&mut self, id: &str) -> Result<LifecycleState, LifecycleError> {
        self.reconnect(id)
    }

    /// True only when the server is ready for a tool snapshot or invocation.
    #[must_use]
    pub fn is_runnable(&self, id: &str) -> bool {
        matches!(
            self.get(id).map(|entry| &entry.state),
            Some(LifecycleState::Ready)
        )
    }

    /// Return only servers eligible to contribute tools, sorted by id.
    #[must_use]
    pub fn runnable_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.state, LifecycleState::Ready))
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Alias for the caller's tool-registration projection.
    #[must_use]
    pub fn registered_server_ids(&self) -> Vec<String> {
        self.runnable_ids()
    }

    /// Build the bounded declarative persistence projection.
    #[must_use]
    pub fn persist(&self) -> PersistedShape {
        PersistedShape {
            entries: self
                .entries
                .iter()
                .map(|entry| PersistedEntry {
                    id: entry.id.clone(),
                    state: persisted_state(&entry.state),
                })
                .collect(),
        }
    }

    /// Restore a registry from a safe persistence projection.
    ///
    /// `Starting` and `Ready` are deliberately restored as `Enabled`: the
    /// persisted state records preference, never a false live connection.
    pub fn restore(shape: &PersistedShape) -> Result<Self, LifecycleError> {
        if shape.entries.len() > MAX_SERVERS {
            return Err(LifecycleError::Overflow);
        }

        for entry in &shape.entries {
            validate_id(&entry.id)?;
            if let LifecycleState::Error { code } = &entry.state {
                validate_code(code)?;
            }
        }
        if shape.to_bytes()?.len() > MAX_PERSISTED_BYTES {
            return Err(LifecycleError::PersistedTooLarge);
        }

        let mut restored = Self::new();
        for entry in &shape.entries {
            if restored
                .entries
                .iter()
                .any(|existing| existing.id == entry.id)
            {
                return Err(LifecycleError::Duplicate);
            }
            restored.entries.push(LifecycleEntry {
                id: entry.id.clone(),
                state: persisted_state(&entry.state),
            });
        }
        restored.sort_entries();
        Ok(restored)
    }

    /// Decode and restore a bounded JSON persistence projection.
    pub fn restore_bytes(bytes: &[u8]) -> Result<Self, LifecycleError> {
        if bytes.len() > MAX_PERSISTED_BYTES {
            return Err(LifecycleError::PersistedTooLarge);
        }
        let shape: PersistedShape =
            serde_json::from_slice(bytes).map_err(|_| LifecycleError::InvalidPersisted)?;
        Self::restore(&shape)
    }

    fn lookup(&self, id: &str) -> Result<&LifecycleEntry, LifecycleError> {
        validate_id(id)?;
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(LifecycleError::Unknown)
    }

    fn lookup_mut(&mut self, id: &str) -> Result<&mut LifecycleEntry, LifecycleError> {
        validate_id(id)?;
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(LifecycleError::Unknown)
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|left, right| left.id.cmp(&right.id));
    }
}

/// Toggle one server's power preference.
pub fn set_enabled(
    registry: &mut LifecycleRegistry,
    id: &str,
    on: bool,
) -> Result<LifecycleState, LifecycleError> {
    registry.set_enabled(id, on)
}

/// Mark one server as starting.
pub fn mark_starting(
    registry: &mut LifecycleRegistry,
    id: &str,
) -> Result<LifecycleState, LifecycleError> {
    registry.mark_starting(id)
}

/// Mark one server ready.
pub fn mark_ready(
    registry: &mut LifecycleRegistry,
    id: &str,
) -> Result<LifecycleState, LifecycleError> {
    registry.mark_ready(id)
}

/// Mark one server failed with a safe code.
pub fn mark_error(
    registry: &mut LifecycleRegistry,
    id: &str,
    code: impl Into<String>,
) -> Result<LifecycleState, LifecycleError> {
    registry.mark_error(id, code)
}

/// Explicitly reconnect one non-disabled server preference.
pub fn reconnect(
    registry: &mut LifecycleRegistry,
    id: &str,
) -> Result<LifecycleState, LifecycleError> {
    registry.reconnect(id)
}

/// Explicitly refresh one non-disabled server preference.
pub fn refresh(
    registry: &mut LifecycleRegistry,
    id: &str,
) -> Result<LifecycleState, LifecycleError> {
    registry.refresh(id)
}

/// True only for a ready server.
#[must_use]
pub fn is_runnable(registry: &LifecycleRegistry, id: &str) -> bool {
    registry.is_runnable(id)
}

/// Produce the safe persistence projection.
#[must_use]
pub fn persist(registry: &LifecycleRegistry) -> PersistedShape {
    registry.persist()
}

/// Restore a registry from a safe persistence projection.
pub fn restore(shape: &PersistedShape) -> Result<LifecycleRegistry, LifecycleError> {
    LifecycleRegistry::restore(shape)
}

//! Caller-owned scoped registry for secret-free integration descriptors.
//!
//! Pure in-memory projection: bounded names plus key/OAuth method metadata,
//! nested caller-owned scopes with last-scope-wins reads and strict LIFO
//! close. No credential material, no I/O, no threads, no clock, no network.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

/// Bound on distinct visible integration names.
pub const MAX_INTEGRATIONS: usize = 64;
/// Bound on methods stored per integration.
pub const MAX_METHODS_PER_INTEGRATION: usize = 8;
/// Bound on concurrently open scopes.
pub const MAX_SCOPES: usize = 16;
/// Bound on integration name length in bytes.
pub const MAX_NAME_LEN: usize = 64;
/// Bound on method id length in bytes.
pub const MAX_METHOD_ID_LEN: usize = 64;
/// Bound on method label length in bytes.
pub const MAX_LABEL_LEN: usize = 128;

/// Handle for one open scope. Counts up per registry starting at 1.
/// Scopes close in LIFO order only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeToken(pub u64);

/// Connection-method flavor. Carries no credential material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MethodKind {
    Key,
    OAuth,
}

/// One named method slot: id plus display label. No credential material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodMeta {
    pub kind: MethodKind,
    pub method_id: String,
    pub label: String,
}

/// One integration descriptor: name plus bounded method metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationDesc {
    pub name: String,
    pub methods: Vec<MethodMeta>,
}

/// Registry failure modes.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RegistryError {
    /// Name empty, too long, or outside the id charset.
    #[error("invalid integration name")]
    InvalidName,
    /// Method id/label invalid, or more than the per-integration bound.
    #[error("invalid method metadata")]
    InvalidMethod,
    /// Same name already present in the same scope.
    #[error("duplicate integration in scope")]
    Duplicate,
    /// Visible-name bound or per-scope entry bound reached.
    #[error("registry full")]
    Overflow,
    /// Unknown scope, non-innermost close, or scope-cap overflow.
    #[error("scope error")]
    Scope,
}

#[derive(Clone, Debug)]
struct ScopeLayer {
    handle: ScopeToken,
    entries: BTreeMap<String, IntegrationDesc>,
}

/// Bounded stack of caller-owned registration scopes.
#[derive(Clone, Debug)]
pub struct IntegrationRegistry {
    layers: Vec<ScopeLayer>,
    next: u64,
}

impl Default for IntegrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn valid_id(value: &str, max: usize) -> bool {
    if value.is_empty() || value.len() > max {
        return false;
    }
    let mut bytes = value.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

fn valid_label(value: &str) -> bool {
    value.len() <= MAX_LABEL_LEN && value.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

impl IntegrationRegistry {
    /// Empty registry. No I/O, no threads.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            next: 1,
        }
    }

    /// Open one scope and return its handle.
    pub fn open_scope(&mut self) -> Result<ScopeToken, RegistryError> {
        if self.layers.len() >= MAX_SCOPES {
            return Err(RegistryError::Scope);
        }
        let issued = ScopeToken(self.next);
        self.next += 1;
        self.layers.push(ScopeLayer {
            handle: issued,
            entries: BTreeMap::new(),
        });
        Ok(issued)
    }

    /// Register `desc` in an open scope.
    pub fn register(
        &mut self,
        scope: ScopeToken,
        desc: IntegrationDesc,
    ) -> Result<(), RegistryError> {
        if !valid_id(&desc.name, MAX_NAME_LEN) {
            return Err(RegistryError::InvalidName);
        }
        if desc.methods.len() > MAX_METHODS_PER_INTEGRATION {
            return Err(RegistryError::InvalidMethod);
        }
        for item in &desc.methods {
            if !valid_id(&item.method_id, MAX_METHOD_ID_LEN) || !valid_label(&item.label) {
                return Err(RegistryError::InvalidMethod);
            }
        }
        let index = self
            .layers
            .iter()
            .position(|layer| layer.handle == scope)
            .ok_or(RegistryError::Scope)?;
        if self.layers[index].entries.contains_key(&desc.name) {
            return Err(RegistryError::Duplicate);
        }
        if self.layers[index].entries.len() >= MAX_INTEGRATIONS {
            return Err(RegistryError::Overflow);
        }
        if !self.is_visible(&desc.name) && self.visible_count() >= MAX_INTEGRATIONS {
            return Err(RegistryError::Overflow);
        }
        self.layers[index]
            .entries
            .insert(desc.name.clone(), desc);
        Ok(())
    }

    /// Close the innermost scope and report how many entries it held.
    pub fn close_scope(&mut self, scope: ScopeToken) -> Result<u64, RegistryError> {
        match self.layers.last() {
            Some(top) if top.handle == scope => {
                let layer = self.layers.pop().expect("top layer checked above");
                Ok(layer.entries.len() as u64)
            }
            _ => Err(RegistryError::Scope),
        }
    }

    /// Innermost visible registration for `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&IntegrationDesc> {
        self.layers
            .iter()
            .rev()
            .find_map(|layer| layer.entries.get(name))
    }

    /// Visible registrations sorted by name ascending.
    #[must_use]
    pub fn list(&self) -> Vec<(&str, &IntegrationDesc)> {
        let mut merged: BTreeMap<&str, &IntegrationDesc> = BTreeMap::new();
        for layer in &self.layers {
            for (name, item) in &layer.entries {
                merged.insert(name.as_str(), item);
            }
        }
        merged.into_iter().collect()
    }

    fn is_visible(&self, name: &str) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.entries.contains_key(name))
    }

    fn visible_count(&self) -> usize {
        let mut names = BTreeSet::new();
        for layer in &self.layers {
            names.extend(layer.entries.keys());
        }
        names.len()
    }
}

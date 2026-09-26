#![forbid(unsafe_code)]
//! Plugin host types+registry only, no dynamic loading (mirrors TS checkout a0d9b6c;
//! `packages/tui/src/plugin/runtime.tsx:12-35` commands/status/slots state;
//! `packages/tui/src/plugin/runtime.tsx:37-57` PluginRuntimeCommands+emptyCommands;
//! `packages/tui/src/plugin/api.ts:11-37` createPluginRoutes revision bump;
//! `packages/plugin/src/tui.ts:551-558` TuiPluginStatus;
//! `packages/plugin/src/tui.ts:594-595,618-624` route.register/plugins list/activate;
//! `packages/tui/src/plugin/adapters.tsx:196-206,311-330` route/plugins passthrough).

use crate::plugin_slots::{SlotName, SlotRegistry, SlotError};

/// Max plugin commands (TS unbounded object; Rust bounded fail-closed).
pub const MAX_COMMANDS: usize = 256;
/// Max status entries (one per plugin command slot).
pub const MAX_PLUGINS: usize = 256;
/// Max id/label bytes (mirrors plugin_slots MAX_COMMAND_NAME/DESC).
pub const MAX_ID: usize = 128;
pub const MAX_LABEL: usize = 512;
/// Max stored errors per plugin + bytes each.
pub const MAX_ERRORS: usize = 8;
pub const MAX_ERROR_LEN: usize = 512;
/// Max route names tracked (revision still bumps fail-closed when full).
pub const MAX_ROUTES: usize = 256;
pub const MAX_ROUTE_LEN: usize = 256;

/// Command id+label (TS `TuiCommand` value/title side; types only, no callbacks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    pub id: String,
    pub label: String,
}

/// Readiness + bounded error strings (TS `TuiPluginStatus` enabled/active projection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStatus {
    pub ready: bool,
    pub errors: Vec<String>,
}

/// Route revision counter (TS `createSignal(0)` +1 on register/unregister, api.ts:19,29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RouteRevision(pub u64);

impl RouteRevision {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn bump(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    Full,
    BadId,
    TooLong,
    TooManyErrors,
    BadRoute,
    UnknownPlugin,
    Slot(SlotError),
}

impl core::fmt::Display for HostError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Full => write!(f, "plugin registry full"),
            Self::BadId => write!(f, "bad plugin id"),
            Self::TooLong => write!(f, "id/label too long"),
            Self::TooManyErrors => write!(f, "too many status errors"),
            Self::BadRoute => write!(f, "bad route name"),
            Self::UnknownPlugin => write!(f, "unknown plugin"),
            Self::Slot(e) => write!(f, "slot: {e}"),
        }
    }
}

impl std::error::Error for HostError {}

impl From<SlotError> for HostError {
    fn from(e: SlotError) -> Self {
        Self::Slot(e)
    }
}

fn check_id(id: &str) -> Result<(), HostError> {
    if id.is_empty() {
        return Err(HostError::BadId);
    }
    if id.len() > MAX_ID {
        return Err(HostError::TooLong);
    }
    Ok(())
}

/// Host registry: commands + status store + route revision + slot passthrough.
#[derive(Debug, Default)]
pub struct PluginHost {
    commands: Vec<PluginCommand>,
    statuses: std::collections::HashMap<String, PluginStatus>,
    routes: Vec<String>,
    revision: RouteRevision,
    slots: SlotRegistry,
}

impl PluginHost {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register command; id nonempty/bounded, label bounded, cap 256.
    pub fn register_command(&mut self, id: &str, label: &str) -> Result<(), HostError> {
        check_id(id)?;
        if label.len() > MAX_LABEL {
            return Err(HostError::TooLong);
        }
        if self.commands.len() >= MAX_COMMANDS {
            return Err(HostError::Full);
        }
        if self.commands.iter().any(|c| c.id == id) {
            return Err(HostError::BadId);
        }
        self.commands.push(PluginCommand { id: id.to_string(), label: label.to_string() });
        Ok(())
    }

    #[must_use]
    pub fn commands(&self) -> &[PluginCommand] {
        &self.commands
    }

    #[must_use]
    pub fn command(&self, id: &str) -> Option<&PluginCommand> {
        self.commands.iter().find(|c| c.id == id)
    }

    /// Register route name; bumps revision (TS api.ts register/unregister +1).
    pub fn register_route(&mut self, name: &str) -> Result<u64, HostError> {
        if name.is_empty() || name.len() > MAX_ROUTE_LEN {
            return Err(HostError::BadRoute);
        }
        if self.routes.len() >= MAX_ROUTES {
            return Err(HostError::Full);
        }
        self.routes.push(name.to_string());
        self.revision.bump();
        Ok(self.revision.get())
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision.0
    }

    /// Status store set (fail-closed on error count/len bounds).
    pub fn set_status(&mut self, id: &str, ready: bool, errors: &[String]) -> Result<(), HostError> {
        check_id(id)?;
        if self.statuses.len() >= MAX_PLUGINS && !self.statuses.contains_key(id) {
            return Err(HostError::Full);
        }
        if errors.len() > MAX_ERRORS {
            return Err(HostError::TooManyErrors);
        }
        for e in errors {
            if e.len() > MAX_ERROR_LEN {
                return Err(HostError::TooLong);
            }
        }
        self.statuses.insert(id.to_string(), PluginStatus { ready, errors: errors.to_vec() });
        Ok(())
    }

    #[must_use]
    pub fn get_status(&self, id: &str) -> Option<&PluginStatus> {
        self.statuses.get(id)
    }

    /// Slot passthrough (reuse, no redefine of SlotName/registry).
    pub fn register_slot(&mut self, plugin_id: &str, slot: SlotName) -> Result<usize, HostError> {
        Ok(self.slots.register(plugin_id, slot)?)
    }

    pub fn unregister_slot(&mut self, handle: usize) -> bool {
        self.slots.unregister(handle)
    }

    #[must_use]
    pub fn slots(&self) -> &SlotRegistry {
        &self.slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_command_ok() {
        let mut h = PluginHost::new();
        h.register_command("session.new", "New session").unwrap();
        assert_eq!(h.command("session.new").unwrap().label, "New session");
    }

    #[test]
    fn rejects_empty_and_long() {
        let mut h = PluginHost::new();
        assert_eq!(h.register_command("", "x"), Err(HostError::BadId));
        assert_eq!(h.register_command(&"i".repeat(200), "x"), Err(HostError::TooLong));
        assert_eq!(h.register_command("ok", &"l".repeat(600)), Err(HostError::TooLong));
    }

    #[test]
    fn rejects_duplicate_id() {
        let mut h = PluginHost::new();
        h.register_command("a", "A").unwrap();
        assert_eq!(h.register_command("a", "A2"), Err(HostError::BadId));
    }

    #[test]
    fn registry_full_at_256() {
        let mut h = PluginHost::new();
        for i in 0..MAX_COMMANDS {
            h.register_command(&format!("cmd.{i}"), "l").unwrap();
        }
        assert_eq!(h.register_command("extra", "l"), Err(HostError::Full));
    }

    #[test]
    fn revision_bumps_on_register() {
        let mut h = PluginHost::new();
        assert_eq!(h.revision(), 0);
        h.register_route("home").unwrap();
        h.register_route("session").unwrap();
        assert_eq!(h.revision(), 2);
        assert_eq!(h.register_route(""), Err(HostError::BadRoute));
    }

    #[test]
    fn status_set_get_roundtrip() {
        let mut h = PluginHost::new();
        h.set_status("p1", true, &["boom".to_string()]).unwrap();
        let s = h.get_status("p1").unwrap();
        assert!(s.ready);
        assert_eq!(s.errors, vec!["boom".to_string()]);
        assert!(h.get_status("missing").is_none());
    }

    #[test]
    fn status_rejects_bounds() {
        let mut h = PluginHost::new();
        let many = vec!["e".to_string(); MAX_ERRORS + 1];
        assert_eq!(h.set_status("p", false, &many), Err(HostError::TooManyErrors));
        assert_eq!(
            h.set_status("p", false, &["x".repeat(600)]),
            Err(HostError::TooLong)
        );
    }

    #[test]
    fn slot_passthrough_reuses_registry() {
        let mut h = PluginHost::new();
        let hd = h.register_slot("tips", SlotName::HomeFooter).unwrap();
        assert_eq!(h.slots().get(SlotName::HomeFooter).len(), 1);
        assert!(h.unregister_slot(hd));
        assert!(h.slots().get(SlotName::HomeFooter).is_empty());
    }
}

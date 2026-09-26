#![forbid(unsafe_code)]
//! Plugin runtime table (mirrors TS `packages/tui/src/plugin/runtime.tsx`
//! commands/status registry + `plugin/api.ts` route registry projection:
//! bounded name/enabled table, fail-closed caps).

/// Max name bytes per plugin (TS unbounded string; Rust bounded).
pub const MAX_NAME: usize = 64;
/// Max plugins in runtime table.
pub const MAX_PLUGINS: usize = 64;

/// Single plugin row: bounded name + enabled flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEntry {
    pub name: String,
    pub enabled: bool,
}

/// Bounded plugin registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginRuntime {
    pub plugins: Vec<PluginEntry>,
}

impl PluginRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Register a plugin as enabled. Errs on empty/dup/cap.
    pub fn register(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("plugin name empty".to_owned());
        }
        if name.len() > MAX_NAME {
            return Err("plugin name too long".to_owned());
        }
        if self.plugins.iter().any(|p| p.name == name) {
            return Err("duplicate plugin".to_owned());
        }
        if self.plugins.len() >= MAX_PLUGINS {
            return Err("plugin registry full".to_owned());
        }
        self.plugins.push(PluginEntry {
            name: name.to_owned(),
            enabled: true,
        });
        Ok(())
    }

    /// Toggle enabled flag. False when unknown.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        match self.plugins.iter_mut().find(|p| p.name == name) {
            Some(p) => {
                p.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// Names of enabled plugins, registration order.
    #[must_use]
    pub fn enabled_names(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_ok_enabled() {
        let mut r = PluginRuntime::new();
        assert!(r.register("git").is_ok());
        assert_eq!(r.enabled_names(), vec!["git".to_owned()]);
    }

    #[test]
    fn register_empty_errs() {
        let mut r = PluginRuntime::new();
        assert!(r.register("").is_err());
        assert!(r.register("   ").is_err());
    }

    #[test]
    fn register_dup_errs() {
        let mut r = PluginRuntime::new();
        assert!(r.register("git").is_ok());
        assert!(r.register("git").is_err());
    }

    #[test]
    fn register_cap_errs() {
        let mut r = PluginRuntime::new();
        for i in 0..MAX_PLUGINS {
            assert!(r.register(&format!("p{i}")).is_ok());
        }
        assert!(r.register("overflow").is_err());
    }

    #[test]
    fn toggle_ok() {
        let mut r = PluginRuntime::new();
        r.register("git").unwrap();
        assert!(r.set_enabled("git", false));
        assert!(r.enabled_names().is_empty());
        assert!(r.set_enabled("git", true));
        assert_eq!(r.enabled_names(), vec!["git".to_owned()]);
    }

    #[test]
    fn toggle_unknown_false() {
        let mut r = PluginRuntime::new();
        assert!(!r.set_enabled("nope", true));
    }
}

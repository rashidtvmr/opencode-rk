#![forbid(unsafe_code)]
//! Plugin slot names, registry, and legacy command shim (mirrors
//! `packages/plugin/src/tui.ts:455-486` host slot map, TS checkout a0d9b6c;
//! `packages/tui/src/plugin/slots.tsx:25-65` register/dispose;
//! `packages/tui/src/plugin/command-shim.ts:49-65` `toCommand` mapping and
//! `packages/tui/src/feature-plugins/builtins.ts:21-35` builtin plugin ids).

/// Max slot registrations (TS registry unbounded; Rust bounded fail-closed).
pub const MAX_SLOTS: usize = 64;
/// Max command name/description bytes (fail-closed; TS plain strings).
pub const MAX_COMMAND_NAME: usize = 128;
pub const MAX_COMMAND_DESC: usize = 512;

/// Host slot names from `TuiHostSlotMap` (`packages/plugin/src/tui.ts:455-486`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotName {
    App,
    AppBottom,
    HomeLogo,
    HomePrompt,
    HomePromptRight,
    SessionPrompt,
    SessionPromptRight,
    HomeBottom,
    HomeFooter,
    SidebarTitle,
    SidebarContent,
    SidebarFooter,
}

impl SlotName {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::AppBottom => "app_bottom",
            Self::HomeLogo => "home_logo",
            Self::HomePrompt => "home_prompt",
            Self::HomePromptRight => "home_prompt_right",
            Self::SessionPrompt => "session_prompt",
            Self::SessionPromptRight => "session_prompt_right",
            Self::HomeBottom => "home_bottom",
            Self::HomeFooter => "home_footer",
            Self::SidebarTitle => "sidebar_title",
            Self::SidebarContent => "sidebar_content",
            Self::SidebarFooter => "sidebar_footer",
        }
    }

    /// Parse a wire name; `None` for custom (non-host) slots.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "app" => Self::App,
            "app_bottom" => Self::AppBottom,
            "home_logo" => Self::HomeLogo,
            "home_prompt" => Self::HomePrompt,
            "home_prompt_right" => Self::HomePromptRight,
            "session_prompt" => Self::SessionPrompt,
            "session_prompt_right" => Self::SessionPromptRight,
            "home_bottom" => Self::HomeBottom,
            "home_footer" => Self::HomeFooter,
            "sidebar_title" => Self::SidebarTitle,
            "sidebar_content" => Self::SidebarContent,
            "sidebar_footer" => Self::SidebarFooter,
            _ => return None,
        })
    }
}

/// One slot registration (plugin id + slot); TS returns opaque unregister fn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotEntry {
    pub plugin_id: String,
    pub slot: SlotName,
}

/// Bounded slot registry (mirrors `registry.register(plugin)` in slots.tsx).
#[derive(Debug, Default, Clone)]
pub struct SlotRegistry {
    entries: Vec<SlotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotError {
    Full,
    BadPluginId,
}

impl core::fmt::Display for SlotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Full => write!(f, "slot registry full"),
            Self::BadPluginId => write!(f, "bad plugin id"),
        }
    }
}

impl std::error::Error for SlotError {}

impl SlotRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register; empty plugin id rejected (TS `isHostSlotPlugin` id check).
    pub fn register(&mut self, plugin_id: &str, slot: SlotName) -> Result<usize, SlotError> {
        if plugin_id.is_empty() || plugin_id.len() > MAX_COMMAND_NAME {
            return Err(SlotError::BadPluginId);
        }
        if self.entries.len() >= MAX_SLOTS {
            return Err(SlotError::Full);
        }
        self.entries.push(SlotEntry { plugin_id: plugin_id.to_string(), slot });
        Ok(self.entries.len() - 1)
    }

    /// Unregister by handle (mirrors TS returned closure).
    pub fn unregister(&mut self, handle: usize) -> bool {
        if handle < self.entries.len() {
            self.entries.remove(handle);
            true
        } else {
            false
        }
    }

    /// All entries for `slot`.
    #[must_use]
    pub fn get(&self, slot: SlotName) -> Vec<&SlotEntry> {
        self.entries.iter().filter(|e| e.slot == slot).collect()
    }
}

/// Legacy v1 command (mirrors `TuiCommand`, `packages/plugin/src/tui.ts:91-105`;
/// `toCommand` maps `value->name`, `title->desc` side, command-shim.ts:49-65).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    EmptyName,
    TooLong,
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "empty command name"),
            Self::TooLong => write!(f, "command name/description too long"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Validate a shimmed command (`value` nonempty, bounded).
pub fn validate_command(name: &str, description: &str) -> Result<PluginCommand, CommandError> {
    if name.is_empty() {
        return Err(CommandError::EmptyName);
    }
    if name.len() > MAX_COMMAND_NAME || description.len() > MAX_COMMAND_DESC {
        return Err(CommandError::TooLong);
    }
    Ok(PluginCommand { name: name.to_string(), description: description.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_roundtrip() {
        for (s, name) in [
            (SlotName::HomeFooter, "home_footer"),
            (SlotName::SidebarContent, "sidebar_content"),
            (SlotName::AppBottom, "app_bottom"),
        ] {
            assert_eq!(s.as_str(), name);
            assert_eq!(SlotName::parse(name), Some(s));
        }
        assert_eq!(SlotName::parse("custom_slot"), None);
    }

    #[test]
    fn register_get_unregister() {
        let mut r = SlotRegistry::new();
        let h = r.register("home-footer", SlotName::HomeFooter).unwrap();
        r.register("tips", SlotName::HomeFooter).unwrap();
        assert_eq!(r.get(SlotName::HomeFooter).len(), 2);
        assert!(r.get(SlotName::App).is_empty());
        assert!(r.unregister(h));
        assert_eq!(r.get(SlotName::HomeFooter).len(), 1);
        assert!(!r.unregister(99));
    }

    #[test]
    fn registry_rejects_bad_id_and_full() {
        let mut r = SlotRegistry::new();
        assert_eq!(r.register("", SlotName::App), Err(SlotError::BadPluginId));
        for i in 0..MAX_SLOTS {
            r.register(&format!("p{i}"), SlotName::App).unwrap();
        }
        assert_eq!(r.register("one-more", SlotName::App), Err(SlotError::Full));
    }

    #[test]
    fn command_shim_validate() {
        assert!(validate_command("", "d").is_err());
        let c = validate_command("session.new", "New session").unwrap();
        assert_eq!(c.name, "session.new");
        assert_eq!(validate_command(&"x".repeat(200), "d"), Err(CommandError::TooLong));
    }
}

#![forbid(unsafe_code)]
//! Legacy `api.command` shim registry (TS `command-shim.ts`).
//!
//! TS truth: v1 plugins register palette commands (`name/title/desc/run`);
//! this shim stores name+desc only and `invoke` returns `"ran <name>"`
//! instead of running a callback.
//!
//! `ponytail:` no callbacks, no keybinds; add only when a real caller needs them.

/// Max name chars (TS palette `value` short id).
pub const MAX_NAME_LEN: usize = 64;
/// Max desc chars (TS palette `description` short text).
pub const MAX_DESC_LEN: usize = 256;
/// Max registered commands (bounded palette).
pub const MAX_COMMANDS: usize = 64;

/// One shimmed palette command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimCommand {
    /// Command id (truncated to [`MAX_NAME_LEN`]).
    pub name: String,
    /// Short help text (truncated to [`MAX_DESC_LEN`]).
    pub desc: String,
}

/// Bounded command registry.
#[derive(Debug, Default)]
pub struct CommandShim {
    cmds: Vec<ShimCommand>,
}

impl CommandShim {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { cmds: Vec::new() }
    }

    /// Register command; errs on empty/dup/full, truncates overlong fields.
    pub fn register(&mut self, name: &str, desc: &str) -> Result<(), String> {
        let name = truncate(name, MAX_NAME_LEN);
        let desc = truncate(desc, MAX_DESC_LEN);
        if name.is_empty() {
            return Err("command name must not be empty".to_string());
        }
        if self.cmds.iter().any(|c| c.name == name) {
            return Err(format!("duplicate command: {name}"));
        }
        if self.cmds.len() >= MAX_COMMANDS {
            return Err(format!("command cap reached: {MAX_COMMANDS}"));
        }
        self.cmds.push(ShimCommand { name, desc });
        Ok(())
    }

    /// Run shimmed command; errs when missing, else `"ran <name>"`.
    pub fn invoke(&self, name: &str) -> Result<String, String> {
        self.cmds
            .iter()
            .find(|c| c.name == name)
            .map(|c| format!("ran {}", c.name))
            .ok_or_else(|| format!("unknown command: {name}"))
    }

    /// All registered commands.
    #[must_use]
    pub fn commands(&self) -> &[ShimCommand] {
        &self.cmds
    }
}

fn truncate(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        s.to_string()
    } else {
        s[..cap].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_ok() {
        let mut s = CommandShim::new();
        assert!(s.register("palette.show", "show palette").is_ok());
        assert_eq!(s.commands().len(), 1);
    }

    #[test]
    fn register_truncates() {
        let mut s = CommandShim::new();
        s.register(&"n".repeat(100), &"d".repeat(300)).unwrap();
        assert_eq!(s.commands()[0].name.len(), MAX_NAME_LEN);
        assert_eq!(s.commands()[0].desc.len(), MAX_DESC_LEN);
    }

    #[test]
    fn register_empty_errs() {
        let mut s = CommandShim::new();
        assert!(s.register("", "x").is_err());
        assert!(s.register("   ".trim(), "x").is_err());
    }

    #[test]
    fn register_dup_errs() {
        let mut s = CommandShim::new();
        s.register("a", "x").unwrap();
        assert!(s.register("a", "y").is_err());
    }

    #[test]
    fn register_cap_errs() {
        let mut s = CommandShim::new();
        for i in 0..MAX_COMMANDS {
            s.register(&format!("c{i}"), "d").unwrap();
        }
        assert!(s.register("overflow", "d").is_err());
    }

    #[test]
    fn invoke_ok() {
        let mut s = CommandShim::new();
        s.register("palette.show", "show").unwrap();
        assert_eq!(s.invoke("palette.show").unwrap(), "ran palette.show");
    }

    #[test]
    fn invoke_missing_errs() {
        let s = CommandShim::new();
        assert!(s.invoke("nope").is_err());
    }
}

#![forbid(unsafe_code)]
//! TUI API adapters + command shim (mirrors TS `adapters.tsx:173`
//! `createTuiApiAdapters` and `command-shim.ts:85` `createCommandShim`;
//! types only, no dynamic loading; bounded fail-closed like plugin_host).

/// Max adapters per batch (TS unbounded array; Rust bounded).
pub const MAX_ADAPTERS: usize = 32;
/// Max name/command/handler bytes (mirrors plugin_host MAX_ID style).
pub const MAX_NAME: usize = 64;
pub const MAX_COMMAND: usize = 64;
pub const MAX_HANDLER: usize = 64;

/// Named TUI API adapter (TS adapters.tsx adapter entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiAdapter {
    pub name: String,
    pub version: u32,
}

/// Command-to-handler shim (TS command-shim.ts entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandShim {
    pub command: String,
    pub handler: String,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Build adapters; skips empty names, truncates to 64, caps at 32.
#[must_use]
pub fn create_adapters(names: &[String]) -> Vec<TuiAdapter> {
    names
        .iter()
        .filter(|n| !n.is_empty())
        .take(MAX_ADAPTERS)
        .map(|n| TuiAdapter {
            name: truncate(n, MAX_NAME),
            version: 0,
        })
        .collect()
}

/// TS-parity alias for `createTuiApiAdapters`.
#[must_use]
pub fn create_tui_api_adapters(names: &[String]) -> Vec<TuiAdapter> {
    create_adapters(names)
}

/// Build shim; empty command/handler errs, overlong truncated to 64.
pub fn create_shim(command: &str, handler: &str) -> Result<CommandShim, String> {
    if command.is_empty() {
        return Err("empty command".to_string());
    }
    if handler.is_empty() {
        return Err("empty handler".to_string());
    }
    Ok(CommandShim {
        command: truncate(command, MAX_COMMAND),
        handler: truncate(handler, MAX_HANDLER),
    })
}

/// TS-parity alias for `createCommandShim`.
pub fn create_command_shim(command: &str, handler: &str) -> Result<CommandShim, String> {
    create_shim(command, handler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_cap_32() {
        let names: Vec<String> = (0..40).map(|i| format!("a{i}")).collect();
        let out = create_adapters(&names);
        assert_eq!(out.len(), MAX_ADAPTERS);
        assert_eq!(create_tui_api_adapters(&names).len(), MAX_ADAPTERS);
    }

    #[test]
    fn empty_names_skipped() {
        let names = vec!["".to_string(), "ok".to_string(), String::new()];
        let out = create_adapters(&names);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "ok");
    }

    #[test]
    fn shim_ok() {
        let s = create_shim("session.new", "handle_new").unwrap();
        assert_eq!(s.command, "session.new");
        assert_eq!(s.handler, "handle_new");
        assert!(create_command_shim("c", "h").is_ok());
    }

    #[test]
    fn shim_empty_errs() {
        assert!(create_shim("", "h").is_err());
        assert!(create_shim("c", "").is_err());
        assert!(create_shim("", "").is_err());
    }

    #[test]
    fn adapter_version_zero() {
        let out = create_adapters(&["x".to_string()]);
        assert_eq!(out[0].version, 0);
    }

    #[test]
    fn long_names_truncated_to_64() {
        let out = create_adapters(&["n".repeat(100)]);
        assert_eq!(out[0].name.len(), MAX_NAME);
        let s = create_shim(&"c".repeat(100), &"h".repeat(100)).unwrap();
        assert_eq!(s.command.len(), MAX_COMMAND);
        assert_eq!(s.handler.len(), MAX_HANDLER);
    }
}

//! Run command body: `/name args...` parse + render.
//!
//! TS ref: `run/footer.command.tsx` (absent in tree; spec-derived).

#![forbid(unsafe_code)]

/// Max chars kept in command name.
pub const MAX_NAME_LEN: usize = 64;
/// Max args kept per command.
pub const MAX_ARGS: usize = 16;
/// Max chars kept per arg.
pub const MAX_ARG_LEN: usize = 512;

/// Parsed `/command` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommand {
    /// Command name, leading `/` stripped, truncated to 64 chars.
    pub name: String,
    /// Args, truncated to 16 entries of 512 chars each.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<String>,
}

impl RunCommand {
    /// Parse `input` into a [`RunCommand`].
    ///
    /// Splits on ASCII whitespace; strips one leading `/`; empty/blank
    /// input errors. Caps applied (name 64, args 16 x 512).
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("empty command".to_string());
        }
        let body = trimmed.strip_prefix('/').unwrap_or(trimmed);
        if body.is_empty() {
            return Err("empty command".to_string());
        }
        let mut parts = body.split_whitespace();
        let raw_name = parts.next().ok_or_else(|| "empty command".to_string())?;
        let name: String = raw_name.chars().take(MAX_NAME_LEN).collect();
        let args: Vec<String> = parts
            .take(MAX_ARGS)
            .map(|a| a.chars().take(MAX_ARG_LEN).collect())
            .collect();
        Ok(Self {
            name,
            args,
            cwd: None,
        })
    }

    /// Render back to `/name args...` form.
    pub fn render(&self) -> String {
        if self.args.is_empty() {
            format!("/{}", self.name)
        } else {
            format!("/{} {}", self.name, self.args.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slash() {
        let c = RunCommand::parse("/build").unwrap();
        assert_eq!(c.name, "build");
        assert!(c.args.is_empty());
    }

    #[test]
    fn parse_args() {
        let c = RunCommand::parse("/run foo bar").unwrap();
        assert_eq!(c.name, "run");
        assert_eq!(c.args, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn empty_errs() {
        assert!(RunCommand::parse("").is_err());
        assert!(RunCommand::parse("   ").is_err());
        assert!(RunCommand::parse("/").is_err());
    }

    #[test]
    fn cap_truncates() {
        let long = "a".repeat(100);
        let c = RunCommand::parse(&format!("/{long}")).unwrap();
        assert_eq!(c.name.len(), MAX_NAME_LEN);
        let many = (0..32)
            .map(|i| format!("x{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let c = RunCommand::parse(&format!("/cmd {many}")).unwrap();
        assert_eq!(c.args.len(), MAX_ARGS);
        let big = "b".repeat(600);
        let c = RunCommand::parse(&format!("/cmd {big}")).unwrap();
        assert_eq!(c.args[0].len(), MAX_ARG_LEN);
    }

    #[test]
    fn render_roundtrip() {
        let c = RunCommand::parse("/deploy staging --force").unwrap();
        assert_eq!(c.render(), "/deploy staging --force");
        let bare = RunCommand::parse("test fast").unwrap();
        assert_eq!(bare.render(), "/test fast");
    }
}

/// Max registry entries.
pub const MAX_REGISTRY: usize = 64;
/// Max chars kept per [`CommandCall`] arg.
pub const MAX_CALL_ARG_LEN: usize = 256;

/// Registry of bare command names, cap 64.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    /// Registered names, each truncated to 64 chars.
    pub names: Vec<String>,
}

impl CommandRegistry {
    /// Register `name`; errs on empty/dup/full. Truncates to 64 chars.
    pub fn register(&mut self, name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        let body = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let clean: String = body.chars().take(MAX_NAME_LEN).collect();
        if clean.is_empty() {
            return Err("empty name".to_string());
        }
        if self.names.iter().any(|n| n == &clean) {
            return Err("duplicate command".to_string());
        }
        if self.names.len() >= MAX_REGISTRY {
            return Err("registry full".to_string());
        }
        self.names.push(clean);
        Ok(())
    }
}

/// Parsed `/name args...` call (strict leading `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCall {
    /// Command name, truncated to 64 chars.
    pub name: String,
    /// Args, truncated to 16 entries of 256 chars each.
    pub args: Vec<String>,
}

/// Parse `line` into a [`CommandCall`]; requires a leading `/`.
pub fn parse_call(line: &str) -> Result<CommandCall, String> {
    let trimmed = line.trim();
    let body = trimmed
        .strip_prefix('/')
        .ok_or_else(|| "missing leading slash".to_string())?;
    if body.is_empty() {
        return Err("empty command".to_string());
    }
    let mut parts = body.split_whitespace();
    let raw_name = parts.next().ok_or_else(|| "empty command".to_string())?;
    let name: String = raw_name.chars().take(MAX_NAME_LEN).collect();
    if name.is_empty() {
        return Err("empty command".to_string());
    }
    let args: Vec<String> = parts
        .take(MAX_ARGS)
        .map(|a| a.chars().take(MAX_CALL_ARG_LEN).collect())
        .collect();
    Ok(CommandCall { name, args })
}

/// One-line help for a call: `/name args...`.
pub fn help_line(call: &CommandCall) -> String {
    if call.args.is_empty() {
        format!("/{}", call.name)
    } else {
        format!("/{} {}", call.name, call.args.join(" "))
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn register_ok() {
        let mut r = CommandRegistry::default();
        r.register("build").unwrap();
        assert_eq!(r.names, vec!["build".to_string()]);
    }

    #[test]
    fn register_dup_errs() {
        let mut r = CommandRegistry::default();
        r.register("build").unwrap();
        assert!(r.register("build").is_err());
    }

    #[test]
    fn register_cap_errs() {
        let mut r = CommandRegistry::default();
        for i in 0..MAX_REGISTRY {
            r.register(&format!("cmd{i}")).unwrap();
        }
        assert!(r.register("overflow").is_err());
        let long = "a".repeat(100);
        let mut r2 = CommandRegistry::default();
        r2.register(&long).unwrap();
        assert_eq!(r2.names[0].len(), MAX_NAME_LEN);
        assert!(r2.register("").is_err());
    }

    #[test]
    fn parse_ok() {
        let c = parse_call("/run foo bar").unwrap();
        assert_eq!(c.name, "run");
        assert_eq!(c.args, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn parse_no_slash_errs() {
        assert!(parse_call("run foo").is_err());
        assert!(parse_call("").is_err());
        assert!(parse_call("/").is_err());
    }

    #[test]
    fn help_non_empty() {
        let c = parse_call("/deploy staging").unwrap();
        let h = help_line(&c);
        assert!(!h.is_empty());
        assert_eq!(h, "/deploy staging");
    }
}

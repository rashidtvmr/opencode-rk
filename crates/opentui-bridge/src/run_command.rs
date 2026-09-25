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

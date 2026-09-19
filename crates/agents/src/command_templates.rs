//! Slash-command template engine for custom commands.
//!
//! Parses command definitions (name, description, template body) from markdown
//! frontmatter and expands templates with positional substitution (`$ARGUMENTS`,
//! `$1`..`$N`), `@file` mentions to typed `FileRef` list, and `!shell` prefix to
//! typed `ShellPlan` without executing anything.
//!
//! Template size, argument count, file-ref count and description length are all
//! bounded. Output is deterministic. Definitions round-trip through
//! serde and frontmatter re-serialization.

#![forbid(unsafe_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

// ── Bounds ──────────────────────────────────────────────────────────────────

/// Maximum bytes for the template body.
pub const MAX_TEMPLATE_BYTES: usize = 64 * 1024;

/// Maximum bytes for the description field.
pub const MAX_DESCRIPTION_BYTES: usize = 512;

/// Maximum bytes for the command name.
pub const MAX_NAME_BYTES: usize = 128;

/// Maximum positional argument index ($1..$N).
pub const MAX_POSITIONAL_ARGS: usize = 64;

/// Maximum @file references per expansion.
pub const MAX_FILE_REFS: usize = 16;

// ── Types ───────────────────────────────────────────────────────────────────

/// A validated file reference from an `@file` mention.
///
/// Paths are validated to prevent traversal (no `..` components, must be
/// relative and within bounds). The path is stored as-is after validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileRef {
    /// Relative path, validated no traversal.
    pub path: String,
}

/// A typed shell command plan created from a `!shell` prefix.
///
/// The command is **never executed** by this module. It is a data-only plan
/// that a downstream permission broker may choose to execute after approval.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShellPlan {
    /// The shell command string (not executed here).
    pub command: String,
    /// Always true — shell commands require human approval.
    pub requires_approval: bool,
}

/// Errors during template parsing or expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// Template body exceeds MAX_TEMPLATE_BYTES.
    TemplateTooLarge { size: usize, max: usize },
    /// Description exceeds MAX_DESCRIPTION_BYTES.
    DescriptionTooLarge { size: usize, max: usize },
    /// Name exceeds MAX_NAME_BYTES.
    NameTooLarge { size: usize, max: usize },
    /// Name contains invalid characters (must be `[A-Za-z0-9][A-Za-z0-9._-]*`).
    InvalidName(String),
    /// Reference to an unknown template variable (e.g. `$UNKNOWN`).
    UnknownVariable(String),
    /// Too many positional arguments (exceeds MAX_POSITIONAL_ARGS).
    TooManyPositionalArgs { count: usize, max: usize },
    /// Too many @file references (exceeds MAX_FILE_REFS).
    TooManyFileRefs { count: usize, max: usize },
    /// @file path contains traversal (`..`) or is invalid.
    InvalidFilePath(String),
    /// Invalid frontmatter format.
    InvalidFrontmatter(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateTooLarge { size, max } => {
                write!(f, "template too large: {size} bytes, max {max}")
            }
            Self::DescriptionTooLarge { size, max } => {
                write!(f, "description too large: {size} bytes, max {max}")
            }
            Self::NameTooLarge { size, max } => {
                write!(f, "name too large: {size} bytes, max {max}")
            }
            Self::InvalidName(n) => write!(f, "invalid command name: {n:?}"),
            Self::UnknownVariable(v) => write!(f, "unknown template variable: ${v}"),
            Self::TooManyPositionalArgs { count, max } => {
                write!(f, "too many positional args: {count}, max {max}")
            }
            Self::TooManyFileRefs { count, max } => {
                write!(f, "too many file refs: {count}, max {max}")
            }
            Self::InvalidFilePath(p) => write!(f, "invalid file path: {p:?}"),
            Self::InvalidFrontmatter(msg) => write!(f, "invalid frontmatter: {msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

/// A parsed command definition (input to the template engine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDef {
    /// Command name: `[A-Za-z0-9][A-Za-z0-9._-]*`, 1..=128 bytes.
    pub name: String,
    /// Human description, 0..=512 bytes.
    pub description: String,
    /// Template body with `$ARGUMENTS`, `$1`..`$N`, `@file`, `!shell` directives.
    pub template: String,
}

/// The result of expanding a command template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandedCommand {
    /// The expanded text with positional substitutions applied.
    pub text: String,
    /// Any `@file` mentions resolved to validated FileRef entries.
    pub file_refs: Vec<FileRef>,
    /// If the template starts with `!shell`, a typed ShellPlan (never executed).
    pub shell_plan: Option<ShellPlan>,
}

// ── Parsing ─────────────────────────────────────────────────────────────────

/// Parse a command definition from markdown frontmatter format.
///
/// Expected format:
/// ```text
/// ---
/// name: my-command
/// description: Does something useful
/// ---
/// Template body with $ARGUMENTS and $1..$N placeholders.
/// ```
pub fn parse_command_def(raw: &str) -> Result<CommandDef, CommandError> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return Err(CommandError::InvalidFrontmatter(
            "missing opening ---".to_string(),
        ));
    }

    let after_open = &trimmed[3..];
    let end_idx = after_open
        .find("\n---")
        .ok_or_else(|| CommandError::InvalidFrontmatter("missing closing ---".to_string()))?;

    let block = &after_open[..end_idx];
    let body_start = 3 + end_idx + 4;
    let body_raw = if body_start < raw.len() {
        &raw[body_start..]
    } else {
        ""
    };
    let body = body_raw.strip_prefix('\n').unwrap_or(body_raw);

    let mut name = None;
    let mut description = None;

    for line in block.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                name = Some(val.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("description:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            description = Some(val.to_string());
        }
    }

    let name = name.ok_or_else(|| {
        CommandError::InvalidFrontmatter("missing 'name' field in frontmatter".to_string())
    })?;

    validate_name(&name)?;

    let description = description.unwrap_or_default();
    validate_description(&description)?;
    validate_template(body)?;

    Ok(CommandDef {
        name,
        description,
        template: body.to_string(),
    })
}

/// Validate a command name: `[A-Za-z0-9][A-Za-z0-9._-]*`, 1..=128 bytes.
pub fn validate_name(name: &str) -> Result<(), CommandError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(CommandError::NameTooLarge {
            size: name.len(),
            max: MAX_NAME_BYTES,
        });
    }
    if name.is_empty() {
        return Err(CommandError::InvalidName(name.to_string()));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err(CommandError::InvalidName(name.to_string()));
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && !matches!(c, '.' | '_' | '-') {
            return Err(CommandError::InvalidName(name.to_string()));
        }
    }
    Ok(())
}

/// Validate a description string: 0..=512 bytes.
pub fn validate_description(desc: &str) -> Result<(), CommandError> {
    if desc.len() > MAX_DESCRIPTION_BYTES {
        return Err(CommandError::DescriptionTooLarge {
            size: desc.len(),
            max: MAX_DESCRIPTION_BYTES,
        });
    }
    Ok(())
}

/// Validate a template body: 0..=64 KiB.
pub fn validate_template(template: &str) -> Result<(), CommandError> {
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(CommandError::TemplateTooLarge {
            size: template.len(),
            max: MAX_TEMPLATE_BYTES,
        });
    }
    Ok(())
}

// ── Expansion ───────────────────────────────────────────────────────────────

/// Expand a command template with the given positional arguments.
///
/// - `$ARGUMENTS` all args joined by space
/// - `$1`..`$N` positional (1-indexed)
/// - `@path` extracted as FileRef (validated, no traversal)
/// - `!shell cmd` produces ShellPlan (never executed)
/// - `$UNKNOWN` error
pub fn expand_template(
    def: &CommandDef,
    arguments: &[String],
) -> Result<ExpandedCommand, CommandError> {
    if arguments.len() > MAX_POSITIONAL_ARGS {
        return Err(CommandError::TooManyPositionalArgs {
            count: arguments.len(),
            max: MAX_POSITIONAL_ARGS,
        });
    }

    let all_args = arguments.join(" ");

    // Validate variable references
    validate_variables(&def.template, arguments.len())?;

    // Extract @file references
    let file_refs = extract_file_refs(&def.template)?;

    // Check shell prefix
    let trimmed = def.template.trim_start();
    let shell_plan = if trimmed.starts_with("!shell ") {
        let cmd = trimmed[7..].trim();
        Some(ShellPlan {
            command: cmd.to_string(),
            requires_approval: true,
        })
    } else {
        None
    };

    // Perform substitutions
    let mut result = def.template.clone();

    // Replace $ARGUMENTS
    result = result.replace("$ARGUMENTS", &all_args);

    // Replace $1..$N (1-indexed)
    for (i, arg) in arguments.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        result = result.replace(&placeholder, arg);
    }

    Ok(ExpandedCommand {
        text: result,
        file_refs,
        shell_plan,
    })
}

/// Validate that all $VARIABLE references in the template are known.
fn validate_variables(template: &str, arg_count: usize) -> Result<(), CommandError> {
    let mut i = 0;
    let bytes = template.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
            {
                end += 1;
            }
            let var_name = &template[start..end];

            if var_name.is_empty() {
                i = end;
                continue;
            }

            let is_valid = if var_name == "ARGUMENTS" {
                true
            } else if let Ok(n) = var_name.parse::<usize>() {
                n >= 1 && n <= arg_count && n <= MAX_POSITIONAL_ARGS
            } else {
                false
            };

            if !is_valid {
                return Err(CommandError::UnknownVariable(var_name.to_string()));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Extract @file references from the template, validating each path.
fn extract_file_refs(template: &str) -> Result<Vec<FileRef>, CommandError> {
    let mut refs = Vec::new();
    let mut i = 0;
    let bytes = template.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'@' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b' ' && bytes[end] != b'\n' {
                end += 1;
            }
            let path = &template[start..end];

            if !path.is_empty() {
                validate_file_path(path)?;
                refs.push(FileRef {
                    path: path.to_string(),
                });
            }

            if refs.len() > MAX_FILE_REFS {
                return Err(CommandError::TooManyFileRefs {
                    count: refs.len(),
                    max: MAX_FILE_REFS,
                });
            }

            i = end;
        } else {
            i += 1;
        }
    }

    Ok(refs)
}

/// Validate a file path: must be relative, no traversal.
fn validate_file_path(path: &str) -> Result<(), CommandError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.starts_with('/') {
        return Err(CommandError::InvalidFilePath(path.to_string()));
    }
    for component in path.split('/') {
        if component == ".." {
            return Err(CommandError::InvalidFilePath(path.to_string()));
        }
    }
    Ok(())
}

// ── Round-trip serialization (frontmatter) ──────────────────────────────────

/// Serialize a CommandDef back to markdown frontmatter format.
pub fn serialize_command_def(def: &CommandDef) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", def.name));
    if !def.description.is_empty() {
        out.push_str(&format!("description: {}\n", def.description));
    }
    out.push_str("---\n");
    out.push_str(&def.template);
    out
}

/// Round-trip: parse then serialize then parse should produce identical CommandDef.
pub fn round_trip(def: &CommandDef) -> Result<CommandDef, CommandError> {
    let serialized = serialize_command_def(def);
    parse_command_def(&serialized)
}

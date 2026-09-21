//! MCP server config: schema, bounded loader, merge precedence, URL guard.
//!
//! Step 1 of FIX-MCP-STUBS: standalone config types only. No process spawn,
//! no client wiring (see `mcp.rs` / `mcp_spawn.rs`, owned by other lanes).
#![forbid(unsafe_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Resource bounds for one MCP server config entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpConfigBounds {
    pub max_args: usize,
    pub max_args_bytes: usize,
    pub max_env: usize,
    pub max_env_bytes: usize,
    pub max_url_len: usize,
}

/// Validated MCP server config entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConfigSchema {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Doctor-style summary of a load attempt.
#[derive(Clone, Debug, Default)]
pub struct McpConfigStatus {
    pub valid: bool,
    pub errors: Vec<String>,
    /// Best-effort parse of the raw input (defaults on total failure).
    pub config: McpConfigSchema,
    /// Validated config, or default when invalid.
    pub effective: McpConfigSchema,
}

impl McpConfigStatus {
    pub fn from_result(
        result: &Result<McpConfigSchema, Vec<String>>,
        raw: &serde_json::Value,
    ) -> Self {
        match result {
            Ok(loaded) => McpConfigStatus {
                valid: true,
                errors: Vec::new(),
                config: loaded.clone(),
                effective: loaded.clone(),
            },
            Err(errors) => McpConfigStatus {
                valid: false,
                errors: errors.clone(),
                config: serde_json::from_value(raw.clone()).unwrap_or_default(),
                effective: McpConfigSchema::default(),
            },
        }
    }
}

/// Accept only `http` / `https` URLs (SSRF guard: reject ftp/file/data/...).
pub fn validate_url_scheme(url: &str) -> Result<(), String> {
    let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
    if scheme == "http" || scheme == "https" {
        Ok(())
    } else {
        Err(format!("url scheme rejected (want http/https): {url}"))
    }
}

/// Parse and bound-check a raw JSON config value.
pub fn load_config(
    raw: &serde_json::Value,
    bounds: &McpConfigBounds,
) -> Result<McpConfigSchema, Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    let command = raw.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if command.is_empty() {
        errors.push("command: missing or empty (must be a non-empty string)".to_owned());
    }

    let mut cfg: McpConfigSchema = match serde_json::from_value(raw.clone()) {
        Ok(cfg) => cfg,
        Err(e) => {
            errors.push(format!("config: invalid shape: {e}"));
            McpConfigSchema::default()
        }
    };
    // Keep the raw string form even if shape parse failed partway.
    if cfg.command.is_empty() && !command.is_empty() {
        cfg.command = command.to_owned();
    }

    if cfg.args.len() > bounds.max_args {
        errors.push(format!(
            "args: count {} exceeds max {}",
            cfg.args.len(),
            bounds.max_args
        ));
    }
    let args_bytes: usize = cfg.args.iter().map(|a| a.len()).sum();
    if args_bytes > bounds.max_args_bytes {
        errors.push(format!(
            "args: total bytes {args_bytes} exceeds max {}",
            bounds.max_args_bytes
        ));
    }

    if cfg.env.len() > bounds.max_env {
        errors.push(format!(
            "env: count {} exceeds max {}",
            cfg.env.len(),
            bounds.max_env
        ));
    }
    let env_bytes: usize = cfg.env.iter().map(|(k, v)| k.len() + v.len()).sum();
    if env_bytes > bounds.max_env_bytes {
        errors.push(format!(
            "env: total bytes {env_bytes} exceeds max {}",
            bounds.max_env_bytes
        ));
    }

    if let Some(url) = cfg.url.as_deref() {
        if url.len() > bounds.max_url_len {
            errors.push(format!(
                "url: length {} exceeds max {}",
                url.len(),
                bounds.max_url_len
            ));
        }
        if let Err(e) = validate_url_scheme(url) {
            errors.push(format!("url: {e}"));
        }
    }

    if errors.is_empty() {
        Ok(cfg)
    } else {
        Err(errors)
    }
}

/// Merge project + user configs. Each user field wins when non-empty;
/// empty user fields fall back to the project value.
pub fn merge_configs(project: &McpConfigSchema, user: &McpConfigSchema) -> McpConfigSchema {
    McpConfigSchema {
        command: if user.command.is_empty() {
            project.command.clone()
        } else {
            user.command.clone()
        },
        args: if user.args.is_empty() {
            project.args.clone()
        } else {
            user.args.clone()
        },
        env: if user.env.is_empty() {
            project.env.clone()
        } else {
            user.env.clone()
        },
        url: user.url.clone().or_else(|| project.url.clone()),
    }
}

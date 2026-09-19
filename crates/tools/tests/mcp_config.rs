// LANE-MCP-CONFIG: MCP server config schema, loader, merge precedence, and doctor status.

#[path = "../src/mcp_config.rs"]
mod mcp_config;

use mcp_config::{
    load_config, McpConfigBounds, McpConfigSchema, McpConfigStatus, merge_configs, validate_url_scheme,
};
use serde_json::json;

const DEFAULT_BOUNDS: McpConfigBounds = McpConfigBounds {
    max_args: 64,
    max_args_bytes: 4096,
    max_env: 32,
    max_env_bytes: 4096,
    max_url_len: 2048,
};

/// Helper: valid minimal config
fn valid_config() -> McpConfigSchema {
    McpConfigSchema {
        command: "/bin/echo".to_owned(),
        ..Default::default()
    }
}

/// Helper: valid config with all fields
fn full_config() -> McpConfigSchema {
    McpConfigSchema {
        command: "/usr/bin/python".to_owned(),
        args: vec!["-m".to_owned(), "http.server".to_owned()],
        env: [
            ("HOME".to_owned(), "/home/user".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
        ]
        .iter()
        .cloned()
        .collect(),
        url: Some("https://example.com/mcp".to_owned()),
    }
}

#[test]
fn config_schema_valid_minimal() {
    let cfg = valid_config();
    let result = load_config(&json!(cfg), &DEFAULT_BOUNDS);
    assert!(result.is_ok(), "valid minimal config should pass: {result:?}");
}

#[test]
fn config_schema_valid_full() {
    let cfg = full_config();
    let result = load_config(&json!(cfg), &DEFAULT_BOUNDS);
    assert!(result.is_ok(), "valid full config should pass: {result:?}");
}

#[test]
fn config_schema_missing_command_fails() {
    let cfg = json!({
        "args": ["--foo"],
        "env": {"HOME": "/home"},
    });
    let result = load_config(&cfg, &DEFAULT_BOUNDS);
    assert!(result.is_err(), "config missing command must fail");
    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("command")),
        "error must mention command: {errors:?}"
    );
}

#[test]
fn config_schema_args_count_bounds() {
    let cfg = json!({
        "command": "/bin/echo",
        "args": [format!("--arg{}", i).to_string(); 100],
    });
    let result = load_config(&cfg, &McpConfigBounds { max_args: 64, ..DEFAULT_BOUNDS });
    assert!(result.is_err(), "100 args must exceed max 64");
}

#[test]
fn config_schema_args_bytes_bounds() {
    let long_arg = " very long argument that exceeds the maximum allowed bytes total".to_owned();
    let cfg = json!({
        "command": "/bin/echo",
        "args": [long_arg.clone(); 10],
    });
    let result = load_config(&cfg, &McpConfigBounds { max_args_bytes: 50, ..DEFAULT_BOUNDS });
    assert!(result.is_err(), "args bytes must exceed max 50");
}

#[test]
fn config_schema_env_count_bounds() {
    let many_env: HashMap<String, String> = (0..=50)
        .map(|i| (format!("VAR_{i}"), format!("value_{i}")))
        .collect();
    let cfg = json!({
        "command": "/bin/echo",
        "env": many_env,
    });
    let result = load_config(&cfg, &McpConfigBounds { max_env: 32, ..DEFAULT_BOUNDS });
    assert!(result.is_err(), "50 env entries must exceed max 32");
}

#[test]
fn config_schema_env_bytes_bounds() {
    let big_val = " ".repeat(200);
    let many_env: HashMap<String, String> = (0..=5)
        .map(|i| (format!("VAR"), big_val.clone()))
        .collect();
    let cfg = json!({
        "command": "/bin/echo",
        "env": many_env,
    });
    let result = load_config(&cfg, &McpConfigBounds { max_env_bytes: 10, ..DEFAULT_BOUNDS });
    assert!(result.is_err(), "env bytes must exceed max 10");
}

#[test]
fn config_schema_url_length_bounds() {
    let long_url = "http://".to_owned() + &"a".repeat(3000);
    let cfg = json!({
        "command": "/bin/echo",
        "url": long_url,
    });
    let result = load_config(&cfg, &McpConfigBounds { max_url_len: 100, ..DEFAULT_BOUNDS });
    assert!(result.is_err(), "URL length must exceed max 100");
}

#[test]
fn config_schema_invalid_url_scheme() {
    let cfg = json!({
        "command": "/bin/echo",
        "url": "ftp://example.com/mcp",
    });
    let result = load_config(&cfg, &DEFAULT_BOUNDS);
    assert!(result.is_err(), "FTP scheme must be rejected");
}

#[test]
fn config_schema_traversal_partial() {
    // Config with only command, other fields use defaults
    let cfg = json!({
        "command": "/bin/true",
    });
    let result = load_config(&cfg, &DEFAULT_BOUNDS);
    assert!(result.is_ok(), "partial config with only command should be OK");
    let loaded = result.unwrap();
    assert_eq!(loaded.command, "/bin/true");
    assert!(loaded.args.is_empty());
    assert!(loaded.env.is_empty());
    assert!(loaded.url.is_none());
}

#[test]
fn config_schema_traversal_with_url() {
    let cfg = json!({
        "command": "/bin/echo",
        "url": "https://public-server.com/mcp",
    });
    let result = load_config(&cfg, &DEFAULT_BOUNDS);
    assert!(result.is_ok(), "config with valid URL should be OK");
    let loaded = result.unwrap();
    assert_eq!(loaded.url, Some("https://public-server.com/mcp".to_owned()));
}

#[test]
fn merge_precedence_user_over_project() {
    let project = McpConfigSchema {
        command: "/bin/echo".to_owned(),
        args: vec!["--project".to_owned()],
        ..Default::default()
    };
    let user = McpConfigSchema {
        command: "/usr/bin/python".to_owned(),
        args: vec!["--user".to_owned()],
        ..Default::default()
    };
    let merged = merge_configs(&project, &user);
    // User command overrides project command
    assert_eq!(merged.command, "/usr/bin/python", "user command should win");
    // User args override project args
    assert_eq!(merged.args, vec!["--user"], "user args should win");
    // Non-present fields use defaults
    assert!(merged.env.is_empty());
    assert!(merged.url.is_none());
}

#[test]
fn merge_precedence_project_fallback_when_user_missing() {
    let project = McpConfigSchema {
        command: "/bin/echo".to_owned(),
        args: vec!["--project".to_owned()],
        env: [("FROM".to_owned(), "project".to_owned())].iter().cloned().collect(),
        ..Default::default()
    };
    let user = McpConfigSchema::default();
    let merged = merge_configs(&project, &user);
    assert_eq!(merged.command, "/bin/echo", "project command used when user empty");
    assert_eq!(merged.args, vec!["--project"], "project args used when user empty");
    assert_eq!(
        merged.env.get("FROM"),
        Some(&"project".to_owned()),
        "project env used when user empty"
    );
}

#[test]
fn status_summary_struct() {
    let status = McpConfigStatus::default();
    assert!(!status.valid);
    assert!(status.errors.is_empty());
    assert!(status.config.command.is_empty());
    assert!(status.effective.command.is_empty());
}

#[test]
fn status_summary_with_errors() {
    let cfg = json!({
        "args": [],  // missing command
    });
    let result = load_config(&cfg, &DEFAULT_BOUNDS);
    let status = McpConfigStatus::from_result(&result, &cfg);
    assert!(!status.valid, "status should be invalid when command missing");
    assert!(!status.errors.is_empty(), "should have validation errors");
}

#[test]
fn ssrf_url_http_only() {
    let result = validate_url_scheme("http://example.com/mcp");
    assert!(result.is_ok(), "http scheme should be valid");
}

#[test]
fn ssrf_url_https_only() {
    let result = validate_url_scheme("https://example.com/mcp");
    assert!(result.is_ok(), "https scheme should be valid");
}

#[test]
fn ssrf_url_ftp_rejected() {
    let result = validate_url_scheme("ftp://example.com/mcp");
    assert!(result.is_err(), "ftp scheme should be rejected");
}

#[test]
fn ssrf_url_file_rejected() {
    let result = validate_url_scheme("file:///etc/passwd");
    assert!(result.is_err(), "file scheme should be rejected");
}

#[test]
fn ssrf_url_data_rejected() {
    let result = validate_url_scheme("data:text/plain,hello");
    assert!(result.is_err(), "data scheme should be rejected");
}
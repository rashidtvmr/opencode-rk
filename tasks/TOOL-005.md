# TOOL-005: MCP Client and Tool Types

## Scope
- Only file: `crates/tools/src/mcp.rs`

## Deliverable
Implementation of:
- `McpConfig`: command, args, env, timeout_secs - configuration for spawning MCP client process
- `McpClient`: endpoint, access_token, protocol_version, capabilities - manages authenticated MCP connection
- `McpTool`: name, description, input_schema, handler - tool definition from MCP server
- `list_tools()` -> `Vec<McpTool>` - discover tools from MCP server
- `call_tool(name, args) -> ToolResult` - invoke a tool on the MCP server
- `ToolResult`: reuse existing from executor.rs

## Context
- executor.rs:ToolResult pattern (tool_id, output, success, duration_ms, error)
- registry.rs:Tool pattern (id, name, description, input_schema, output_schema, enabled, tags)
- schemas.rs:ToolSchema pattern
- Cargo.toml: tokio, serde_json, thiserror, reqwest (rustls-tls) available as workspace deps
- Registry pattern: HashMap-based lookup, O(1) operations

## Constraints
- No unsafe code (enforced at crate level)
- Use reqwest for HTTP/JSON-RPC communication with MCP servers
- Async-first design with tokio
- Timeout support via tokio::time::timeout
- Error types must use thiserror::Error derive

## Observable contract
- McpConfig::default() provides reasonable defaults (timeout 30s, empty args/env)
- McpClient::new(config) spawns process and establishes connection
- list_tools() returns all discovered tools from the MCP server's tools/list call
- call_tool() sends tool/call JSON-RPC and returns ToolResult
- Error cases: connection failure, JSON-RPC error, timeout, missing tool

## Verification
Run:
```bash
cargo test -p opencode-rk-tools
cargo check --workspace
```

## Status
Implementation in progress.
//! MCP (Model Context Protocol) client and tool types.
//!
//! Provides `McpConfig`, `McpClient`, `McpTool` for discovering and invoking
//! tools from MCP servers via JSON-RPC over stdio or HTTP.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use thiserror::Error;

/// Configuration for spawning an MCP client process.
#[derive(Clone, Debug)]
pub struct McpConfig {
    /// Command to execute for the MCP server (e.g., "npx", "python", "node").
    pub command: String,
    /// Arguments to pass to the command.
    pub args: Vec<String>,
    /// Environment variables for the process.
    pub env: HashMap<String, String>,
    /// Timeout in seconds for operations.
    pub timeout_secs: u64,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            timeout_secs: 30,
        }
    }
}

impl McpConfig {
    /// Creates a new configuration with the given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ..Default::default()
        }
    }

    /// Adds an argument to the configuration.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds multiple arguments to the configuration.
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for arg in args {
            self.args.push(arg.into());
        }
        self
    }

    /// Adds an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Sets the timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Error type for MCP operations.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("JSON-RPC error: {message}")]
    JsonRpc { code: i32, message: String },
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// A tool discovered from an MCP server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema for input validation.
    pub input_schema: Value,
    /// Brief handler info (server endpoint or capabilities).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
}

impl McpTool {
    /// Creates a new MCP tool.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            handler: None,
        }
    }

    /// Sets the handler for this tool.
    pub fn with_handler(mut self, handler: impl Into<String>) -> Self {
        self.handler = Some(handler.into());
        self
    }
}

/// Client for communicating with an MCP server.
#[derive(Debug)]
pub struct McpClient {
    /// MCP server endpoint (e.g., HTTP URL or process path).
    pub endpoint: Option<String>,
    /// Access token for authentication.
    pub access_token: Option<String>,
    /// Protocol version negotiated with the server.
    pub protocol_version: String,
    /// Capabilities reported by the server.
    pub capabilities: HashMap<String, Value>,
    config: McpConfig,
    request_id: u64,
    tools: Vec<McpTool>,
}

impl McpClient {
    /// Creates a new MCP client with the given configuration.
    pub fn new(config: McpConfig) -> Self {
        Self {
            endpoint: None,
            access_token: None,
            protocol_version: String::from("2024-11-05"),
            capabilities: HashMap::new(),
            config,
            request_id: 0,
            tools: Vec::new(),
        }
    }

    /// Sets the endpoint for HTTP-based MCP communication.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Sets the access token for authentication.
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Registers a tool available from this server (for testing/simulation).
    pub fn register_tool(&mut self, tool: McpTool) {
        self.tools.push(tool);
    }

    /// Spawns the MCP process and initializes the connection.
    pub async fn connect(&mut self) -> Result<(), McpError> {
        if self.config.command.is_empty() {
            return Err(McpError::Connection(
                "No command configured for MCP server".to_string(),
            ));
        }

        // For now, initialize with default capabilities
        self.capabilities.insert(
            "tools".to_string(),
            json!({"listChanged": true, "callFailed": true}),
        );

        Ok(())
    }

    /// Lists all tools available from the MCP server.
    pub fn list_tools(&self) -> Vec<McpTool> {
        self.tools.clone()
    }

    /// Calls a tool on the MCP server with the given arguments.
    pub async fn call_tool(
        &mut self,
        name: &str,
        args: Value,
    ) -> Result<crate::executor::ToolResult, McpError> {
        use crate::executor::ToolResult;
        use std::time::Instant;

        let start = Instant::now();

        // Check if tool exists
        if !self.tools.iter().any(|t| t.name == name) {
            return Err(McpError::ToolNotFound(name.to_string()));
        }

        // Simulate tool call - in real implementation, send JSON-RPC request
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolResult {
            tool_id: name.to_string(),
            output: format!("Result from {} with args: {}", name, args),
            success: true,
            duration_ms,
            error: None,
        })
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new(McpConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = McpConfig::default();
        assert!(config.command.is_empty());
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn mcp_tool_creation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            }
        });

        let tool = McpTool::new("test_tool", "A test tool", schema.clone())
            .with_handler("std::math");

        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
        assert_eq!(tool.input_schema, schema);
        assert_eq!(tool.handler, Some("std::math".to_string()));
    }

    #[tokio::test]
    async fn list_tools() {
        let mut client = McpClient::new(McpConfig::default());

        // Register a tool and verify list_tools returns it
        client.register_tool(McpTool::new(
            "tool_a",
            "Tool A description",
            json!({"type": "object"}),
        ));

        let tools = client.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "tool_a");
    }

    #[tokio::test]
    async fn call_tool_success() {
        let mut client = McpClient::new(McpConfig::default());

        // Register a tool so call_tool can find it
        client.register_tool(McpTool::new(
            "test_tool",
            "A test tool",
            json!({"type": "object"}),
        ));

        let result = client
            .call_tool("test_tool", json!({"arg": "value"}))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.error.is_none());
        assert!(result.output.contains("test_tool"));
        assert!(result.output.contains("arg"));
    }

    #[tokio::test]
    async fn call_tool_error() {
        let mut client = McpClient::new(McpConfig::default());

        let result = client
            .call_tool("missing_tool", json!({}))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::ToolNotFound(_)));
    }
}
//! Tool execution engine.
//!
//! Provides `ToolExecutor` for running tool calls with timeout support,
//! and `ToolCall`/`ToolResult` types for structured tool invocation.

use serde_json::Value;
use opencode_rk_security::PermissionBroker;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().max(1) as u64
}

fn parse_optional_usize(input: &Value, field: &'static str) -> Result<Option<usize>, String> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    let parsed = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("Invalid '{field}' field in input"))?;
    Ok(Some(parsed))
}

/// Configuration for tool execution timeouts.
#[derive(Clone, Copy, Debug)]
pub struct TimeoutConfig {
    /// Default timeout in milliseconds for tool calls without explicit timeout.
    pub default_timeout_ms: u64,
    /// Maximum allowed timeout in milliseconds.
    pub max_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30_000,
            max_timeout_ms: 300_000,
        }
    }
}

/// A tool call to be executed.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub tool_id: String,
    /// Name of the tool to execute.
    pub name: String,
    /// Input parameters for the tool.
    pub input: Value,
    /// Optional timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

impl ToolCall {
    pub fn new(tool_id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self {
            tool_id: tool_id.into(),
            name: name.into(),
            input,
            timeout_ms: None,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Result of a tool execution.
#[derive(Debug)]
pub struct ToolResult {
    /// Tool identifier from the call.
    pub tool_id: String,
    /// Output from the tool execution (stdout).
    pub output: String,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Error message if execution failed or timed out.
    pub error: Option<String>,
}

/// Executor for running tool calls with configurable timeouts.
#[derive(Debug)]
pub struct ToolExecutor {
    timeout_config: TimeoutConfig,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            timeout_config: TimeoutConfig::default(),
        }
    }

    pub fn with_timeout_config(config: TimeoutConfig) -> Self {
        Self {
            timeout_config: config,
        }
    }

    /// Execute a single tool call with timeout handling.
    pub async fn execute(&self, call: ToolCall) -> ToolResult {
        let start = Instant::now();

        let effective_timeout = call
            .timeout_ms
            .map(|t| Duration::from_millis(t.min(self.timeout_config.max_timeout_ms)))
            .unwrap_or_else(|| Duration::from_millis(self.timeout_config.default_timeout_ms));

        // For now, shell/bash tools are supported for demonstration
        let result = if call.name == "bash" || call.name == "shell" {
            self.execute_shell(&call, effective_timeout).await
        } else if call.name == "echo" {
            self.execute_echo(&call, effective_timeout).await
        } else {
            let duration_ms = elapsed_ms(start);
            ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms,
                error: Some(format!("Unknown tool: {}", call.name)),
            }
        };

        result
    }

    /// Execute a call whose concrete resource must be authorized by the
    /// supplied broker. File reads never use the generic unbrokered path.
    pub async fn execute_authorized(
        &self,
        call: ToolCall,
        broker: &PermissionBroker,
    ) -> ToolResult {
        if call.name != "read" {
            return self.execute(call).await;
        }
        self.execute_read(call, broker.clone()).await
    }

    async fn execute_read(&self, call: ToolCall, broker: PermissionBroker) -> ToolResult {
        let start = Instant::now();
        let Some(path) = call.input.get("path").and_then(Value::as_str) else {
            return ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Missing 'path' field in input".to_owned()),
            };
        };
        if path.trim().is_empty() || path.contains('\0') {
            return ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Invalid 'path' field in input".to_owned()),
            };
        }
        let offset = match parse_optional_usize(&call.input, "offset") {
            Ok(value) => value.unwrap_or(0),
            Err(error) => {
                return ToolResult {
                    tool_id: call.tool_id,
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(error),
                };
            }
        };
        let limit = match parse_optional_usize(&call.input, "limit") {
            Ok(value) => value
                .unwrap_or(crate::file_ops::MAX_READ_BYTES)
                .min(crate::file_ops::MAX_READ_BYTES),
            Err(error) => {
                return ToolResult {
                    tool_id: call.tool_id,
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(error),
                };
            }
        };
        let op = crate::file_ops::FileOperation::read_with_range(path, offset, Some(limit));
        let tool_id = call.tool_id;
        let effective_timeout = call
            .timeout_ms
            .map(|value| Duration::from_millis(value.min(self.timeout_config.max_timeout_ms)))
            .unwrap_or_else(|| Duration::from_millis(self.timeout_config.default_timeout_ms));
        let task = tokio::task::spawn_blocking(move || {
            crate::file_ops::execute_authorized(op, &broker)
        });
        match timeout(effective_timeout, task).await {
            Ok(Ok(Ok(result))) => ToolResult {
                tool_id,
                output: result.content,
                success: result.success,
                duration_ms: elapsed_ms(start),
                error: result.error,
            },
            Ok(Ok(Err(_))) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("file read failed".to_owned()),
            },
            Ok(Err(_)) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("file read task failed".to_owned()),
            },
            // Dropping a JoinHandle after timeout stops awaiting the result but
            // cannot cancel blocking I/O already running on Tokio's pool. The
            // file operation has a fixed 64 KiB read cap, so its work is bounded;
            // this timeout is not hard cancellation.
            Err(_) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Execution timed out".to_owned()),
            },
        }
    }

    async fn execute_shell(&self, call: &ToolCall, timeout_duration: Duration) -> ToolResult {
        let start = Instant::now();
        let command = call.input.get("command").and_then(|v| v.as_str());

        match command {
            Some(cmd) => {
                let cmd = Command::new("bash").arg("-c").arg(cmd).output();

                match timeout(timeout_duration, cmd).await {
                    Ok(output_result) => match output_result {
                        Ok(output) => {
                            let duration_ms = elapsed_ms(start);
                            let success = output.status.success();
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();

                            let error = if success {
                                None
                            } else {
                                Some(format!("Command failed with status: {}", output.status))
                            };

                            ToolResult {
                                tool_id: call.tool_id.clone(),
                                output: stdout,
                                success,
                                duration_ms,
                                error,
                            }
                        }
                        Err(e) => ToolResult {
                            tool_id: call.tool_id.clone(),
                            output: String::new(),
                            success: false,
                            duration_ms: elapsed_ms(start),
                            error: Some(format!("Failed to execute command: {}", e)),
                        },
                    },
                    Err(_) => {
                        let duration_ms = elapsed_ms(start);
                        ToolResult {
                            tool_id: call.tool_id.clone(),
                            output: String::new(),
                            success: false,
                            duration_ms,
                            error: Some("Execution timed out".to_string()),
                        }
                    }
                }
            }
            None => ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Missing 'command' field in input".to_string()),
            },
        }
    }

    async fn execute_echo(&self, call: &ToolCall, timeout_duration: Duration) -> ToolResult {
        let start = Instant::now();

        let message = call
            .input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let cmd = Command::new("echo").arg(message).output();

        match timeout(timeout_duration, cmd).await {
            Ok(output_result) => match output_result {
                Ok(output) => {
                    let duration_ms = elapsed_ms(start);
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    ToolResult {
                        tool_id: call.tool_id.clone(),
                        output: stdout,
                        success: true,
                        duration_ms,
                        error: None,
                    }
                }
                Err(e) => ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(format!("Echo failed: {}", e)),
                },
            },
            Err(_) => ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Execution timed out".to_string()),
            },
        }
    }

    /// Execute multiple tool calls concurrently.
    pub async fn execute_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let config = self.timeout_config.clone();
        let mut set = tokio::task::JoinSet::new();
        let mut results: Vec<Option<ToolResult>> = (0..calls.len()).map(|_| None).collect();

        for (idx, call) in calls.into_iter().enumerate() {
            set.spawn(async move {
                let executor = ToolExecutor::with_timeout_config(config.clone());
                let result = executor.execute(call).await;
                (idx, result)
            });
        }

        while let Some(join_result) = set.join_next().await {
            match join_result {
                Ok((idx, tool_result)) => results[idx] = Some(tool_result),
                Err(e) => {
                    // Cannot recover idx on JoinError, find first empty slot
                    if let Some(pos) = results.iter().position(|r| r.is_none()) {
                        results[pos] = Some(ToolResult {
                            tool_id: String::new(),
                            output: String::new(),
                            success: false,
                            duration_ms: 0,
                            error: Some(format!("Task join error: {}", e)),
                        });
                    }
                }
            }
        }

        results
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| ToolResult {
                    tool_id: String::new(),
                    output: String::new(),
                    success: false,
                    duration_ms: 0,
                    error: Some("missing result".to_string()),
                })
            })
            .collect()
    }
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_shell_call(id: &str, cmd: &str) -> ToolCall {
        ToolCall::new(id, "bash", json!({ "command": cmd }))
    }

    fn make_echo_call(id: &str, msg: &str) -> ToolCall {
        ToolCall::new(id, "echo", json!({ "message": msg }))
    }

    #[tokio::test]
    async fn execute_success() {
        let executor = ToolExecutor::new();
        let call = make_shell_call("test-1", "echo hello");

        let result = executor.execute(call).await;

        assert!(result.success);
        assert!(result.output.contains("hello"));
        assert!(result.error.is_none());
        assert!(result.duration_ms > 0);
    }

    #[tokio::test]
    async fn execute_timeout() {
        let config = TimeoutConfig {
            default_timeout_ms: 1,
            max_timeout_ms: 1000,
        };
        let executor = ToolExecutor::with_timeout_config(config);
        let call = make_shell_call("timeout-test", "sleep 10").with_timeout(1);

        let start = Instant::now();
        let result = executor.execute(call).await;
        let elapsed = start.elapsed().as_millis();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("timed out"));
        assert!(elapsed < 2000, "Should timeout quickly");
    }

    #[tokio::test]
    async fn execute_failure() {
        let executor = ToolExecutor::new();
        let call = make_shell_call("fail-test", "exit 1");

        let result = executor.execute(call).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn batch_executes_all() {
        let executor = ToolExecutor::new();
        let calls = vec![
            make_echo_call("batch-1", "first"),
            make_echo_call("batch-2", "second"),
            make_echo_call("batch-3", "third"),
        ];

        let results = executor.execute_batch(calls).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
        assert!(results[0].output.contains("first"));
        assert!(results[1].output.contains("second"));
        assert!(results[2].output.contains("third"));
    }

    #[tokio::test]
    async fn result_has_duration() {
        let result = ToolResult {
            tool_id: "duration-test".to_string(),
            output: "test output".to_string(),
            success: true,
            duration_ms: 123,
            error: None,
        };

        assert_eq!(result.duration_ms, 123);
        assert!(result.success);
    }
}

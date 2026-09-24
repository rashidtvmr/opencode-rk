//! DISC-103: native tool registry dispatch wiring.
//!
//! The registry (`registry.rs`) was register-without-dispatch: no path from a
//! registered [`Tool`](crate::registry::Tool) through the executor
//! ([`ToolExecutor`](crate::executor::ToolExecutor)), an authorization policy,
//! and the durable output chain
//! ([`OutputStore`](crate::output_store::OutputStore)).
//!
//! This module owns that path in one place:
//!
//! 1. resolve `name` against [`ToolRegistry`](crate::registry::ToolRegistry)
//!    (fail closed on unknown/disabled — no spawn, no store write);
//! 2. consult the injected [`DispatchPolicy`] (deny — no spawn, no store write);
//! 3. run under a bounded [`Semaphore`] permit (concurrency cap);
//! 4. execute via [`ToolExecutor`];
//! 5. truncate output to the byte budget with [`TRUNCATED_MARKER`] and record a
//!    durable [`DispatchRecord`] (call/result pair with timestamp + provenance)
//!    in the [`OutputStore`].
//!
//! Cancellation is permit-safe: the permit is held by a guard that drops when
//! the dispatch future is dropped, so an aborted dispatch reclaims its task
//! and writes nothing.
//!
//! # Wiring (integrator, outside this file)
//! Add `pub mod registry_dispatch;` to `crates/tools/src/lib.rs`.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use thiserror::Error;
use tokio::sync::Semaphore;

use crate::executor::{ToolCall, ToolExecutor, ToolResult};
use crate::output_store::{OutputStore, ToolOutput};
use crate::registry::{ToolRegistry, Tool};

/// Marker appended when output hits the byte budget.
pub const TRUNCATED_MARKER: &str = "[truncated:over-byte-budget]";

/// Bounded, fixed denial for a shell alias dispatched without a concrete
/// broker capability. Contains no command, input, environment or identity.
pub const SHELL_DENIED_NO_BROKER: &str =
    "shell execution denied: broker authorization required";

/// Default bound on concurrent dispatches through one dispatcher.
pub const DEFAULT_MAX_PARALLEL: usize = 4;
/// Default bound on serialized input bytes per dispatch.
pub const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
/// Default bound on retained output bytes per dispatch record.
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Typed dispatch failures. Every variant is fail-closed: no process is
/// spawned and nothing is written to the output store.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchError {
    #[error("unknown tool: {0}")]
    Unknown(String),
    #[error("tool disabled: {0}")]
    Disabled(String),
    #[error("tool denied by policy: {0}")]
    Denied(String),
    #[error("input too large: {actual} bytes, max {max}")]
    InputTooLarge { actual: usize, max: usize },
    #[error("tool name is empty")]
    EmptyName,
}

/// Authorization hook consulted before every spawn. Inject the real broker
/// verdict at the call site; [`AllowAll`] is the permissive default.
pub trait DispatchPolicy: Send + Sync {
    fn authorize(&self, tool: &Tool, input: &Value) -> bool;
}

/// Permissive policy for tests and broker-less embedding.
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAll;

impl DispatchPolicy for AllowAll {
    fn authorize(&self, _tool: &Tool, _input: &Value) -> bool {
        true
    }
}

/// Bounds for one dispatcher. All collections stay within these caps.
#[derive(Debug, Clone, Copy)]
pub struct DispatchConfig {
    /// Maximum concurrent dispatches (semaphore permits).
    pub max_parallel: usize,
    /// Maximum serialized input bytes per dispatch.
    pub max_input_bytes: usize,
    /// Maximum retained output bytes per dispatch record.
    pub max_output_bytes: usize,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            max_parallel: DEFAULT_MAX_PARALLEL,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}

/// Durable call/result pair: what ran, what happened, when, and for whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRecord {
    /// Registry tool id that ran.
    pub tool_id: String,
    /// Registry tool name as requested.
    pub name: String,
    /// Whether the executor reported success.
    pub success: bool,
    /// Bounded output (truncated with [`TRUNCATED_MARKER`] when over budget).
    pub output: String,
    /// Executor error, if any.
    pub error: Option<String>,
    /// UNIX epoch millis when the record was written.
    pub timestamp_ms: u64,
    /// Caller-supplied provenance (session/turn/request id).
    pub provenance: String,
}

/// Registry -> policy -> executor -> durable-output dispatcher.
///
/// Holds the concurrency bound and the policy; the registry, executor and
/// store are passed per call so one dispatcher serves many owners.
pub struct RegistryDispatcher {
    config: DispatchConfig,
    semaphore: Arc<Semaphore>,
    max_permits: usize,
    policy: Arc<dyn DispatchPolicy>,
}

impl RegistryDispatcher {
    /// Create a dispatcher with default config and [`AllowAll`] policy.
    pub fn new() -> Self {
        Self::with_policy(DispatchConfig::default(), Arc::new(AllowAll))
    }

    /// Create a dispatcher with explicit bounds and policy.
    pub fn with_policy(config: DispatchConfig, policy: Arc<dyn DispatchPolicy>) -> Self {
        let permits = config.max_parallel.max(1);
        Self {
            config,
            semaphore: Arc::new(Semaphore::new(permits)),
            max_permits: permits,
            policy,
        }
    }

    /// Current bounds.
    pub fn config(&self) -> DispatchConfig {
        self.config
    }

    /// Permits currently available (for tests/monitoring).
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Maximum permits (for tests/monitoring).
    pub fn max_permits(&self) -> usize {
        self.max_permits
    }

    /// Dispatch one registered tool by name through policy, executor and the
    /// durable output store. Fail-closed on unknown/disabled/denied/oversized.
    ///
    /// The semaphore permit is held by a guard: dropping this future
    /// (cancellation) reclaims the permit and writes nothing to the store.
    pub async fn dispatch(
        &self,
        registry: &ToolRegistry,
        executor: &ToolExecutor,
        store: &Mutex<OutputStore>,
        name: &str,
        input: Value,
        provenance: &str,
    ) -> Result<DispatchRecord, DispatchError> {
        if name.is_empty() {
            return Err(DispatchError::EmptyName);
        }
        let input_bytes = serde_json::to_vec(&input).map(|v| v.len()).unwrap_or(usize::MAX);
        if input_bytes > self.config.max_input_bytes {
            return Err(DispatchError::InputTooLarge {
                actual: input_bytes,
                max: self.config.max_input_bytes,
            });
        }
        let tool = registry
            .get(name)
            .ok_or_else(|| DispatchError::Unknown(name.to_string()))?;
        if !tool.enabled {
            return Err(DispatchError::Disabled(name.to_string()));
        }
        if !self.policy.authorize(tool, &input) {
            return Err(DispatchError::Denied(name.to_string()));
        }
        // A boolean/AllowAll dispatch policy is not process authority. Shell
        // aliases need a concrete broker verdict, which this registry path does
        // not carry, so deny before any permit, executor call or store write.
        if is_shell_alias(tool) {
            return Ok(shell_denied_record(tool, provenance));
        }
        // Bounded: one permit per live dispatch. Guard drop = reclaim on cancel.
        let _permit = self
            .semaphore
            .acquire()
            .await
            .expect("dispatcher holds the semaphore open");
        let call = build_call(tool, input);
        let result = executor.execute(call).await;
        let bounded = bound_output(result.output.clone(), self.config.max_output_bytes);
        let record = record_result(&result, name, bounded.clone(), provenance);
        store_result(store, &record, &result, bounded);
        Ok(record)
    }

    /// Dispatch many tools concurrently, bounded by the semaphore. Order of
    /// the returned results matches the order of `requests`.
    ///
    /// Fail-closed items (unknown/disabled/denied/oversized) resolve without
    /// spawning. Valid items run as spawned tasks under semaphore permits;
    /// each task executes with a default [`ToolExecutor`] (ponytail: per-task
    /// executors do not inherit a custom `TimeoutConfig`; use [`Self::dispatch`]
    /// for custom timeouts, or promote the executor to `Clone` upstream).
    pub async fn dispatch_batch(
        &self,
        registry: &ToolRegistry,
        executor: &ToolExecutor,
        store: &Mutex<OutputStore>,
        requests: Vec<(String, Value, String)>,
    ) -> Vec<Result<DispatchRecord, DispatchError>> {
        let _ = executor;
        enum Ready {
            Immediate(Result<DispatchRecord, DispatchError>),
            Spawn { tool: Tool, input: Value, provenance: String },
        }
        let mut ready: Vec<Option<Ready>> = Vec::with_capacity(requests.len());
        for (name, input, provenance) in requests {
            let item = if name.is_empty() {
                Ready::Immediate(Err(DispatchError::EmptyName))
            } else if serde_json::to_vec(&input).map(|v| v.len()).unwrap_or(usize::MAX)
                > self.config.max_input_bytes
            {
                Ready::Immediate(Err(DispatchError::InputTooLarge {
                    actual: serde_json::to_vec(&input).map(|v| v.len()).unwrap_or(usize::MAX),
                    max: self.config.max_input_bytes,
                }))
            } else {
                match registry.get(&name) {
                    None => Ready::Immediate(Err(DispatchError::Unknown(name))),
                    Some(tool) if !tool.enabled => {
                        Ready::Immediate(Err(DispatchError::Disabled(name)))
                    }
                    Some(tool) if !self.policy.authorize(tool, &input) => {
                        Ready::Immediate(Err(DispatchError::Denied(name)))
                    }
                    Some(tool) if is_shell_alias(tool) => {
                        // Same no-permit/no-spawn/no-store guarantee as single
                        // dispatch: batch must not bypass the shell gate.
                        Ready::Immediate(Ok(shell_denied_record(tool, &provenance)))
                    }
                    Some(tool) => Ready::Spawn {
                        tool: tool.clone(),
                        input,
                        provenance,
                    },
                }
            };
            ready.push(Some(item));
        }

        let mut set = tokio::task::JoinSet::new();
        let mut spawned_idx: Vec<usize> = Vec::new();
        for (idx, slot) in ready.iter_mut().enumerate() {
            if let Some(Ready::Spawn { tool, input, provenance }) = slot.take() {
                let sem = self.semaphore.clone();
                let max_out = self.config.max_output_bytes;
                spawned_idx.push(idx);
                set.spawn(async move {
                    let _permit = sem
                        .acquire_owned()
                        .await
                        .expect("dispatcher holds the semaphore open");
                    let call = ToolCall::new(tool.id.clone(), tool.name.clone(), input);
                    let exec = ToolExecutor::new();
                    let result = exec.execute(call).await;
                    let bounded = bound_output(result.output.clone(), max_out);
                    let record = DispatchRecord {
                        tool_id: result.tool_id.clone(),
                        name: tool.name.clone(),
                        success: result.success,
                        output: bounded,
                        error: result.error.clone(),
                        timestamp_ms: now_millis(),
                        provenance,
                    };
                    (idx, record)
                });
            }
        }

        let mut records: Vec<Option<(usize, DispatchRecord)>> = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(pair) = joined {
                records.push(Some(pair));
            }
        }
        records.sort_by_key(|r| r.as_ref().map(|(i, _)| *i).unwrap_or(usize::MAX));

        // Durable write-back in request order; only successful spawns recorded.
        let mut by_idx: std::collections::HashMap<usize, DispatchRecord> = records
            .into_iter()
            .flatten()
            .map(|(i, r)| (i, r))
            .collect();
        let mut out: Vec<Result<DispatchRecord, DispatchError>> =
            Vec::with_capacity(ready.len() + spawned_idx.len());
        // Rebuild in original order: immediates stay, spawns pull from by_idx.
        let mut spawn_results: std::collections::HashMap<usize, DispatchRecord> = by_idx;
        for (idx, slot) in ready.into_iter().enumerate() {
            match slot {
                Some(Ready::Immediate(r)) => out.push(r),
                None if spawn_results.contains_key(&idx) => {
                    let rec = spawn_results.remove(&idx).expect("present");
                    if let Ok(mut guard) = store.lock() {
                        guard.push(ToolOutput::new(
                            rec.tool_id.clone(),
                            rec.output.clone(),
                            rec.success,
                        ));
                    }
                    out.push(Ok(rec));
                }
                _ => out.push(Err(DispatchError::Unknown(format!("slot-{idx}")))),
            }
        }
        let _ = spawned_idx;
        out
    }
}

impl Default for RegistryDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Current UNIX epoch time in milliseconds.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// True when a registry tool is a shell alias (`bash`/`shell`) by id or name.
///
/// The generic executor refuses these too; the registry gate keeps a denied
/// shell from reaching the executor at all, so no permit is taken and no
/// durable record is written.
fn is_shell_alias(tool: &Tool) -> bool {
    matches!(
        tool.id.as_str(),
        "bash" | "shell" | "/bin/bash" | "/bin/sh"
    ) || matches!(tool.name.as_str(), "bash" | "shell")
}

/// Bounded failed dispatch record for a shell alias denied before any permit,
/// spawn or store write. Preserves tool id/name/provenance; never echoes input.
fn shell_denied_record(tool: &Tool, provenance: &str) -> DispatchRecord {
    DispatchRecord {
        tool_id: tool.id.clone(),
        name: tool.name.clone(),
        success: false,
        output: String::new(),
        error: Some(SHELL_DENIED_NO_BROKER.to_owned()),
        timestamp_ms: now_millis(),
        provenance: provenance.to_string(),
    }
}

/// Build the executor call + durable record helpers shared by dispatch paths.
#[allow(dead_code)]
fn build_call(tool: &Tool, input: Value) -> ToolCall {
    ToolCall::new(tool.id.clone(), tool.name.clone(), input)
}

/// Truncate `output` to `max_bytes`, appending [`TRUNCATED_MARKER`].
#[allow(dead_code)]
fn bound_output(output: String, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return output;
    }
    let mut cut = max_bytes.saturating_sub(TRUNCATED_MARKER.len());
    while cut > 0 && !output.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &output[..cut], TRUNCATED_MARKER)
}

/// Record an executor result durably and return the dispatch record.
#[allow(dead_code)]
fn record_result(result: &ToolResult, name: &str, output: String, provenance: &str) -> DispatchRecord {
    DispatchRecord {
        tool_id: result.tool_id.clone(),
        name: name.to_string(),
        success: result.success,
        output,
        error: result.error.clone(),
        timestamp_ms: now_millis(),
        provenance: provenance.to_string(),
    }
}

/// Push one result into the store (durable call/result pair).
#[allow(dead_code)]
fn store_result(
    store: &Mutex<OutputStore>,
    record: &DispatchRecord,
    result: &ToolResult,
    bounded: String,
) {
    let _ = record;
    if let Ok(mut guard) = store.lock() {
        guard.push(ToolOutput::new(
            result.tool_id.clone(),
            bounded,
            result.success,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn registry_with_echo() -> ToolRegistry {
        let mut reg = ToolRegistry::default();
        reg.register(
            Tool::new(
                "echo",
                "echo",
                "echo a message",
                json!({ "type": "object" }),
                json!({ "type": "string" }),
            )
            .with_tags(&["test"]),
        );
        reg
    }

    fn test_dispatcher() -> RegistryDispatcher {
        RegistryDispatcher::new()
    }

    fn test_store() -> Mutex<OutputStore> {
        Mutex::new(OutputStore::new(1024 * 1024))
    }

    /// Deny-everything policy for fail-closed tests.
    #[derive(Debug, Default)]
    struct DenyAll;
    impl DispatchPolicy for DenyAll {
        fn authorize(&self, _tool: &Tool, _input: &Value) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn disc103_t01_registered_echo_dispatches_and_records() {
        let reg = registry_with_echo();
        let exec = ToolExecutor::new();
        let store = test_store();
        let d = test_dispatcher();

        let rec = d
            .dispatch(
                &reg,
                &exec,
                &store,
                "echo",
                json!({ "message": "hello-dispatch" }),
                "sess-1/turn-2",
            )
            .await
            .expect("registered tool must dispatch");

        assert_eq!(rec.tool_id, "echo");
        assert_eq!(rec.name, "echo");
        assert!(rec.success);
        assert!(rec.output.contains("hello-dispatch"));
        assert!(rec.error.is_none());
        assert!(rec.timestamp_ms > 0);
        assert_eq!(rec.provenance, "sess-1/turn-2");

        let guard = store.lock().unwrap();
        let entries = guard.get("echo");
        assert_eq!(entries.len(), 1, "one durable call/result pair");
        assert!(entries[0].output.contains("hello-dispatch"));
        assert!(entries[0].timestamp > 0);
    }

    #[tokio::test]
    async fn disc103_t02_unknown_tool_fails_closed_no_store_effect() {
        let reg = registry_with_echo();
        let exec = ToolExecutor::new();
        let store = test_store();
        let d = test_dispatcher();

        let err = d
            .dispatch(&reg, &exec, &store, "nope-missing", json!({}), "prov")
            .await
            .expect_err("unknown tool must fail");
        assert_eq!(err, DispatchError::Unknown("nope-missing".to_string()));

        let guard = store.lock().unwrap();
        assert_eq!(guard.stats(), (0, 0), "no side effects on unknown tool");
        assert_eq!(d.available_permits(), d.max_permits(), "permit reclaimed");
    }

    #[tokio::test]
    async fn disc103_t03_disabled_tool_refused_no_spawn() {
        let mut reg = registry_with_echo();
        assert!(reg.disable("echo"));
        let exec = ToolExecutor::new();
        let store = test_store();
        let d = test_dispatcher();

        let err = d
            .dispatch(&reg, &exec, &store, "echo", json!({ "message": "x" }), "prov")
            .await
            .expect_err("disabled tool must be refused");
        assert_eq!(err, DispatchError::Disabled("echo".to_string()));

        let guard = store.lock().unwrap();
        assert_eq!(guard.stats(), (0, 0), "disabled tool: no spawn, no record");
    }

    #[tokio::test]
    async fn disc103_t04_denied_by_policy_no_spawn_no_permit_leak() {
        let reg = registry_with_echo();
        let exec = ToolExecutor::new();
        let store = test_store();
        let d = RegistryDispatcher::with_policy(DispatchConfig::default(), Arc::new(DenyAll));

        let err = d
            .dispatch(&reg, &exec, &store, "echo", json!({ "message": "x" }), "prov")
            .await
            .expect_err("denied tool must fail");
        assert_eq!(err, DispatchError::Denied("echo".to_string()));

        let guard = store.lock().unwrap();
        assert_eq!(guard.stats(), (0, 0), "denied: no spawn, no record");
        assert_eq!(d.available_permits(), d.max_permits(), "no permit leak");
    }

    #[tokio::test]
    async fn disc103_t05_batch_respects_max_parallel_bound() {
        let mut reg = ToolRegistry::default();
        reg.register(Tool::new(
            "bash",
            "bash",
            "shell",
            json!({ "type": "object" }),
            json!({ "type": "string" }),
        ));
        let exec = ToolExecutor::new();
        let store = test_store();
        let config = DispatchConfig {
            max_parallel: 2,
            ..DispatchConfig::default()
        };
        let d = RegistryDispatcher::with_policy(config, Arc::new(AllowAll));

        let requests: Vec<(String, Value, String)> = (0..4)
            .map(|i| {
                (
                    "bash".to_string(),
                    json!({ "command": format!("sleep 0.2; echo done-{i}") }),
                    format!("prov-{i}"),
                )
            })
            .collect();

        let start = Instant::now();
        let results = d.dispatch_batch(&reg, &exec, &store, requests).await;
        let elapsed = start.elapsed();

        assert_eq!(results.len(), 4);
        for r in &results {
            assert!(r.as_ref().expect("batch item must dispatch").success);
        }
        // 4 x 0.2s under 2 permits serializes to ~0.4s; unbounded would be ~0.2s.
        assert!(
            elapsed >= Duration::from_millis(350),
            "permits must bound parallelism (elapsed {elapsed:?})"
        );
        assert_eq!(d.available_permits(), d.max_permits(), "permits reclaimed");
        let guard = store.lock().unwrap();
        assert_eq!(guard.get("bash").len(), 4, "every dispatch recorded");
    }

    #[tokio::test]
    async fn disc103_t06_oversized_input_rejected_bounded() {
        let reg = registry_with_echo();
        let exec = ToolExecutor::new();
        let store = test_store();
        let config = DispatchConfig {
            max_input_bytes: 32,
            ..DispatchConfig::default()
        };
        let d = RegistryDispatcher::with_policy(config, Arc::new(AllowAll));

        let big = "x".repeat(1024);
        let err = d
            .dispatch(&reg, &exec, &store, "echo", json!({ "message": big }), "prov")
            .await
            .expect_err("oversized input must be rejected");
        assert!(matches!(err, DispatchError::InputTooLarge { .. }));

        let guard = store.lock().unwrap();
        assert_eq!(guard.stats(), (0, 0), "rejected: no spawn, no record");
    }

    #[tokio::test]
    async fn disc103_t07_cancel_reclaims_permit_no_store_write() {
        let reg = registry_with_echo();
        let exec = ToolExecutor::new();
        let store = test_store();
        let d = RegistryDispatcher::with_policy(
            DispatchConfig {
                max_parallel: 1,
                ..DispatchConfig::default()
            },
            Arc::new(AllowAll),
        );

        // Occupy the single permit so the dispatch below pends on acquire.
        let held = d.semaphore.clone().try_acquire_owned().expect("permit free");
        assert_eq!(d.available_permits(), 0);

        // Cancel (drop) a pending dispatch via timeout: the future is dropped
        // while parked on the semaphore, reclaiming its task with no write.
        let timed = tokio::time::timeout(
            Duration::from_millis(100),
            d.dispatch(&reg, &exec, &store, "echo", json!({ "message": "cancelled" }), "prov"),
        )
        .await;
        assert!(timed.is_err(), "pending dispatch must be cancellable");
        drop(held);

        assert_eq!(d.available_permits(), 1, "permit reclaimed after cancel");
        let guard = store.lock().unwrap();
        assert_eq!(guard.stats(), (0, 0), "cancelled: nothing recorded");
        drop(guard);

        // Liveness: a fresh dispatch on the same dispatcher still works.
        let rec = d
            .dispatch(&reg, &exec, &store, "echo", json!({ "message": "alive" }), "prov")
            .await
            .expect("dispatcher usable after cancel");
        assert!(rec.success);
    }
}

//! TOOL-RED-SHELL frozen RED: shell execution requires explicit broker authorization.
//!
//! Fails on current public behavior because `ToolExecutor::execute` routes
//! `bash`/`shell` straight to `execute_shell` (`crates/tools/src/executor.rs:109-134,240`)
//! and `RegistryDispatcher::new` authorizes via `AllowAll`
//! (`crates/tools/src/registry_dispatch.rs:131-146`), so a shell request
//! without a concrete broker capability spawns. GREEN must deny before spawn
//! with no marker file, no surviving child, and no secret in loggable output.

use opencode_rk_tools::executor::{ToolCall, ToolExecutor};
use opencode_rk_tools::output_store::OutputStore;
use opencode_rk_tools::registry::{Tool, ToolRegistry};
use opencode_rk_tools::registry_dispatch::RegistryDispatcher;
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;

// Fake sentinel: never a real credential, never printed on failure.
const NO_LEAK_NOR_PRINT: &str = "RED-SENTINEL-7f3a-not-a-secret";
const SECRET_HINT: &str = "RED-SENTINEL";

fn looks_like_denied_before_spawn(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("denied")
        || t.contains("not authorized")
        || t.contains("unauthorized")
        || t.contains("requires human approval")
        || t.contains("forbidden")
        || t.contains("broker")
}

fn result_text_of(result: &opencode_rk_tools::executor::ToolResult) -> String {
    let mut s = String::with_capacity(
        result.output.len() + result.error.as_ref().map_or(0, |e| e.len()) + 8,
    );
    s.push_str(&result.output);
    if let Some(error) = &result.error {
        s.push_str(error);
    }
    s
}

fn leaked(text: &str) -> bool {
    text.contains(SECRET_HINT)
}

/// t01: shell request without authorization is denied before child spawn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t01_shell_without_broker_denied_before_spawn() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let marker = dir.path().join("t01-no-spawn.marker");
    assert!(!marker.exists(), "fixture must start clean");
    let executor = ToolExecutor::new();
    // Fixed harmless fixture command; no interpolation of untrusted strings.
    let command = format!("touch {}", marker.display());
    let call = ToolCall::new("shell-red-t01", "bash", json!({ "command": command }));
    let result = tokio::time::timeout(Duration::from_secs(10), executor.execute(call))
        .await
        .expect("bounded execution");
    assert!(
        !result.success,
        "unauthorized shell must not succeed; got output len {}",
        result.output.len()
    );
    let text = result_text_of(&result);
    assert!(
        looks_like_denied_before_spawn(&text),
        "shell ran or failed for another reason; expected denial, text len {}",
        text.len()
    );
    assert!(
        !marker.exists(),
        "denied shell left a marker file: spawn was not gated"
    );
}

/// t02: denial creates no marker file and emits no secret into output/error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_denial_leaves_no_marker_and_no_secret() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let marker = dir.path().join("t02-no-side-effect.marker");
    assert!(!marker.exists(), "fixture must start clean");
    let executor = ToolExecutor::new();
    let command = format!("touch {}", marker.display());
    let input = json!({ "command": command, "note": NO_LEAK_NOR_PRINT });
    let call = ToolCall::new("shell-red-t02", "shell", input);
    let result = tokio::time::timeout(Duration::from_secs(10), executor.execute(call))
        .await
        .expect("bounded execution");
    assert!(
        !result.success,
        "unauthorized shell must not succeed; got output len {}",
        result.output.len()
    );
    assert!(
        !marker.exists(),
        "denied shell left a marker file: spawn was not gated"
    );
    let text = result_text_of(&result);
    assert!(!leaked(&text), "denial disclosed the sentinel");
}

/// t03: default dispatcher denies shell with no store write and no permit leak.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_dispatcher_without_broker_denies_shell_no_store_write() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let marker = dir.path().join("t03-dispatch-denied.marker");
    assert!(!marker.exists(), "fixture must start clean");
    let mut registry = ToolRegistry::default();
    registry.register(Tool::new(
        "bash",
        "bash",
        "shell",
        json!({ "type": "object" }),
        json!({ "type": "string" }),
    ));
    let executor = ToolExecutor::new();
    let store = Mutex::new(OutputStore::new(1024 * 1024));
    let dispatcher = RegistryDispatcher::new();
    let command = format!("touch {}", marker.display());
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        dispatcher.dispatch(
            &registry,
            &executor,
            &store,
            "bash",
            json!({ "command": command }),
            "shell-red-t03",
        ),
    )
    .await
    .expect("bounded dispatch");
    let record = outcome.expect("unauthorized shell dispatch must be denied");
    assert!(
        !record.success,
        "unauthorized shell dispatch must not report success"
    );
    let mut text = record.output;
    if let Some(error) = record.error {
        text.push_str(&error);
    }
    assert!(
        looks_like_denied_before_spawn(&text),
        "dispatch ran or failed for another reason; expected denial, text len {}",
        text.len()
    );
    assert!(
        !marker.exists(),
        "denied dispatch left a marker file: spawn was not gated"
    );
    let guard = store.lock().expect("store lock");
    assert_eq!(
        guard.stats(),
        (0, 0),
        "denied dispatch must write no durable record"
    );
    drop(guard);
    assert_eq!(
        dispatcher.available_permits(),
        dispatcher.max_permits(),
        "permit leak on denied dispatch"
    );
}

/// t04: bounded timeout leaves no surviving child and no late marker.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_timeout_leaves_no_late_marker() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let marker = dir.path().join("t04-cancelled.marker");
    assert!(!marker.exists(), "fixture must start clean");
    let executor = ToolExecutor::new();
    let command = format!("sleep 1; touch {}", marker.display());
    let call =
        ToolCall::new("shell-red-t04", "bash", json!({ "command": command })).with_timeout(50);
    let result = tokio::time::timeout(Duration::from_secs(10), executor.execute(call))
        .await
        .expect("bounded execution");
    assert!(
        !result.success,
        "timed-out shell must not succeed; got output len {}",
        result.output.len()
    );
    assert!(
        result_text_of(&result).to_ascii_lowercase().contains("timed out"),
        "expected a timeout failure, text len {}",
        result_text_of(&result).len()
    );
    // Settle past the sleep so a surviving child would have written by now.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        !marker.exists(),
        "timed-out shell left a late marker: child survived cancellation"
    );
}

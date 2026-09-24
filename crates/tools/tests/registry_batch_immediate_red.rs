//! TOOL-SHELL-BATCH-IMMEDIATE frozen RED: `dispatch_batch` must preserve every
//! `Ready::Immediate` outcome instead of discarding it.
//!
//! Defect (source evidence at the RED base revision, `registry_dispatch.rs`):
//! the spawn-extraction loop does `if let Some(Ready::Spawn {..}) = slot.take()`
//! for EVERY slot, so every non-`Spawn` (`Immediate`) item is dropped to `None`;
//! the rebuild loop then maps `None`-without-spawn-result to
//! `Err(DispatchError::Unknown(format!("slot-{idx}")))`.
//!
//! Result: unknown / disabled / denied / oversized / empty-name fail-closed
//! items and shell-denied records all resolve as a misleading
//! `Unknown("slot-i")`, breaking ordered parity with single `dispatch`.
//!
//! GREEN must map each batch item to exactly the single-dispatch outcome:
//! - shell denied retains the fixed denial record (`SHELL_DENIED_NO_BROKER`),
//!   never a fake success (no unbrokered shell authority in batch either);
//! - unknown retains the original requested tool id;
//! - disabled / oversized / empty-name retain their exact error;
//! - order matches `requests`; fail-closed items take no permit and write no
//!   durable record.

use opencode_rk_tools::executor::ToolExecutor;
use opencode_rk_tools::output_store::OutputStore;
use opencode_rk_tools::registry::{Tool, ToolRegistry};
use opencode_rk_tools::registry_dispatch::{
    AllowAll, DispatchConfig, DispatchError, DispatchPolicy, DispatchRecord, RegistryDispatcher,
    SHELL_DENIED_NO_BROKER,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

const OVERSIZED_MAX_INPUT_BYTES: usize = 128;

/// Registry with the tool ids used by the batch fixtures. `disabled-echo`
/// starts registered then disabled by [`fixture_registry`].
fn fixture_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::default();
    for (id, name) in [
        ("echo", "echo"),
        ("disabled-echo", "disabled-echo"),
        ("bash", "bash"),
    ] {
        reg.register(Tool::new(
            id,
            name,
            "batch immediate-red fixture",
            json!({ "type": "object" }),
            json!({ "type": "string" }),
        ));
    }
    assert!(
        reg.disable("disabled-echo"),
        "fixture tool must exist to disable"
    );
    reg
}

fn fixture_dispatcher() -> RegistryDispatcher {
    RegistryDispatcher::with_policy(
        DispatchConfig {
            max_input_bytes: OVERSIZED_MAX_INPUT_BYTES,
            ..DispatchConfig::default()
        },
        Arc::new(AllowAll),
    )
}

fn fixture_store() -> Mutex<OutputStore> {
    Mutex::new(OutputStore::new(1024 * 1024))
}

/// Deny-everything policy for the policy-denied batch case.
#[derive(Debug, Default)]
struct DenyAll;
impl DispatchPolicy for DenyAll {
    fn authorize(&self, _tool: &Tool, _input: &Value) -> bool {
        false
    }
}

/// Comparable projection of a record (timestamps are not deterministic).
fn fields(rec: &DispatchRecord) -> (String, String, bool, String, Option<String>, String) {
    (
        rec.tool_id.clone(),
        rec.name.clone(),
        rec.success,
        rec.output.clone(),
        rec.error.clone(),
        rec.provenance.clone(),
    )
}

fn oversized_input() -> Value {
    json!({ "message": "x".repeat(4096) })
}

/// t01: an unknown batch item retains the original tool id, not `slot-i`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t01_batch_unknown_retains_requested_id() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let store = fixture_store();
    let d = fixture_dispatcher();

    let results = d
        .dispatch_batch(
            &reg,
            &exec,
            &store,
            vec![(
                "missing-tool".to_string(),
                json!({}),
                "prov-unknown".to_string(),
            )],
        )
        .await;

    assert_eq!(results.len(), 1);
    match &results[0] {
        Err(DispatchError::Unknown(id)) => assert_eq!(
            id, "missing-tool",
            "unknown batch item must retain the requested tool id, not slot-i"
        ),
        other => panic!("unknown batch item must be Unknown(requested id), got {other:?}"),
    }
    let guard = store.lock().unwrap();
    assert_eq!(guard.stats(), (0, 0), "unknown item: no store write");
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "unknown item: no permit lost"
    );
}

/// t02: a disabled batch item retains its exact `Disabled` error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t02_batch_disabled_retains_exact_error() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let store = fixture_store();
    let d = fixture_dispatcher();

    let results = d
        .dispatch_batch(
            &reg,
            &exec,
            &store,
            vec![(
                "disabled-echo".to_string(),
                json!({ "message": "x" }),
                "prov-disabled".to_string(),
            )],
        )
        .await;

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0],
        Err(DispatchError::Disabled("disabled-echo".to_string())),
        "disabled batch item must retain its exact error"
    );
    let guard = store.lock().unwrap();
    assert_eq!(guard.stats(), (0, 0), "disabled item: no store write");
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "disabled item: no permit lost"
    );
}

/// t03: an oversized batch item retains `InputTooLarge` with the real bounds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t03_batch_oversized_retains_input_too_large() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let store = fixture_store();
    let d = fixture_dispatcher();

    let results = d
        .dispatch_batch(
            &reg,
            &exec,
            &store,
            vec![(
                "echo".to_string(),
                oversized_input(),
                "prov-oversized".to_string(),
            )],
        )
        .await;

    assert_eq!(results.len(), 1);
    match &results[0] {
        Err(DispatchError::InputTooLarge { max, .. }) => assert_eq!(
            *max, OVERSIZED_MAX_INPUT_BYTES,
            "oversized batch item must report the configured max"
        ),
        other => panic!("oversized batch item must be InputTooLarge, got {other:?}"),
    }
    let guard = store.lock().unwrap();
    assert_eq!(guard.stats(), (0, 0), "oversized item: no store write");
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "oversized item: no permit lost"
    );
}

/// t04: a shell batch item retains the fixed denial record (never fake success).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t04_batch_shell_denied_retains_fixed_record() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let store = fixture_store();
    let d = fixture_dispatcher();

    let results = d
        .dispatch_batch(
            &reg,
            &exec,
            &store,
            vec![(
                "bash".to_string(),
                json!({ "command": "echo hi" }),
                "prov-shell".to_string(),
            )],
        )
        .await;

    assert_eq!(results.len(), 1);
    let rec = results[0]
        .as_ref()
        .expect("shell denial is a bounded record, not an error");
    assert_eq!(rec.tool_id, "bash");
    assert_eq!(rec.name, "bash");
    assert!(
        !rec.success,
        "batch unbrokered shell must not report success"
    );
    assert!(
        rec.output.is_empty(),
        "shell denial must not retain command output"
    );
    assert_eq!(
        rec.error.as_deref(),
        Some(SHELL_DENIED_NO_BROKER),
        "shell denial must keep the fixed broker-required text"
    );
    assert_eq!(rec.provenance, "prov-shell");
    // No unbrokered shell authority in batch: nothing spawned, nothing stored.
    let guard = store.lock().unwrap();
    assert_eq!(guard.stats(), (0, 0), "shell denial: no store write");
    assert!(guard.get("bash").is_empty(), "shell denial: no bash record");
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "shell denial: no permit lost"
    );
}

/// t05: order-preserving parity with single dispatch across all fail-closed
/// and denied outcomes plus a valid item.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t05_batch_ordered_parity_with_single_dispatch() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let d = fixture_dispatcher();
    let batch_store = fixture_store();

    let requests: Vec<(String, Value, String)> = vec![
        (
            "missing-tool".to_string(),
            json!({}),
            "prov-unknown".to_string(),
        ),
        (
            "disabled-echo".to_string(),
            json!({ "message": "x" }),
            "prov-disabled".to_string(),
        ),
        (
            "echo".to_string(),
            oversized_input(),
            "prov-oversized".to_string(),
        ),
        (
            "bash".to_string(),
            json!({ "command": "echo hi" }),
            "prov-shell".to_string(),
        ),
        (
            "echo".to_string(),
            json!({ "message": "ok" }),
            "prov-echo".to_string(),
        ),
        (String::new(), json!({}), "prov-empty".to_string()),
    ];

    let batch = d
        .dispatch_batch(&reg, &exec, &batch_store, requests.clone())
        .await;
    assert_eq!(
        batch.len(),
        requests.len(),
        "one result per request, in order"
    );

    for (idx, (name, input, provenance)) in requests.iter().enumerate() {
        let single_store = fixture_store();
        let single = d
            .dispatch(&reg, &exec, &single_store, name, input.clone(), provenance)
            .await;
        match (&batch[idx], &single) {
            (Ok(b), Ok(s)) => assert_eq!(
                fields(b),
                fields(s),
                "batch[{}] Ok record must equal single dispatch",
                idx
            ),
            (Err(b), Err(s)) => assert_eq!(b, s, "batch[{}] Err must equal single dispatch", idx),
            (b, s) => panic!("batch[{idx}] outcome {b:?} must match single dispatch {s:?}"),
        }
    }

    // Fail-closed items leave no durable trace; only the valid echo is stored.
    let guard = batch_store.lock().unwrap();
    assert_eq!(guard.stats().0, 1, "only the valid echo item is recorded");
    assert_eq!(guard.get("echo").len(), 1);
    assert!(guard.get("bash").is_empty(), "shell denial: no bash record");
    assert!(guard.get("missing-tool").is_empty());
    assert!(guard.get("disabled-echo").is_empty());
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "batch must reclaim every permit"
    );
}

/// t06: policy-denied and empty-name items keep their exact errors and produce
/// no permit or store side effects.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tsbi_t06_batch_denied_and_empty_name_exact_no_side_effects() {
    let reg = fixture_registry();
    let exec = ToolExecutor::new();
    let store = fixture_store();
    let d = RegistryDispatcher::with_policy(DispatchConfig::default(), Arc::new(DenyAll));

    let results = d
        .dispatch_batch(
            &reg,
            &exec,
            &store,
            vec![
                (
                    "echo".to_string(),
                    json!({ "message": "x" }),
                    "prov-denied".to_string(),
                ),
                (String::new(), json!({}), "prov-empty".to_string()),
            ],
        )
        .await;

    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0],
        Err(DispatchError::Denied("echo".to_string())),
        "policy-denied batch item must be Denied, not Unknown(slot-i)"
    );
    assert_eq!(
        results[1],
        Err(DispatchError::EmptyName),
        "empty-name batch item must be EmptyName, not Unknown(slot-i)"
    );
    let guard = store.lock().unwrap();
    assert_eq!(guard.stats(), (0, 0), "denied/empty items: no store write");
    drop(guard);
    assert_eq!(
        d.available_permits(),
        d.max_permits(),
        "denied/empty: no permit lost"
    );
}

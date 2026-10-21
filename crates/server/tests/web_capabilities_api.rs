//! Native tool advertisement over `/api/capabilities` (default = actual tools).
//!
//! Contract, derived from the shipped registry and turn allowlist (seven real
//! turn tools plus the legacy registered `file` inspector):
//! - `OPENCODE_RK_TURN_TOOLS` unset (the normal operator default): all eight
//!   built-in ids appear sorted; the seven real turn tools are honest
//!   `available_for_web_turn=true`; the legacy registered `file` id stays
//!   `false` because the turn adapter does not dispatch it.
//! - Explicit `OPENCODE_RK_TURN_TOOLS=""`: every built-in id is denied, the
//!   command never dispatches, and no id is blanket-flipped to true.
//! - Explicit `OPENCODE_RK_TURN_TOOLS="bash"`: an explicit list narrows —
//!   only `bash` is turn-enabled and every other registered id reports false.
//! - Naming an unsupported registered id (`file`) never widens the list.
//! - The web adapter sections stay pinned unavailable.
//!
//! Env is process-global, so one test function drives all scenarios and a
//! static mutex serializes the var against sibling tests in this binary.
#![forbid(unsafe_code)]

use std::{
    env,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

/// All eight registered built-in ids, in the sorted wire order.
const BUILTIN_IDS: [&str; 8] = [
    "bash", "edit", "file", "glob", "grep", "read", "shell", "write",
];
/// The ids the turn adapter actually dispatches (legacy `file` excluded).
const TURN_IDS: [&str; 7] = ["bash", "edit", "glob", "grep", "read", "shell", "write"];
const TURN_ALLOWLIST_KEY: &str = "OPENCODE_RK_TURN_TOOLS";

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Mutex::default).lock().unwrap()
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn unset(key: &'static str) -> Self {
        let previous = env::var(key).ok();
        env::remove_var(key);
        Self { key, previous }
    }
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            env::set_var(self.key, previous);
        } else {
            env::remove_var(self.key);
        }
    }
}

async fn get_capabilities() -> Value {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn tools_of(value: &Value) -> Vec<Value> {
    value["tools"].as_array().expect("tools array").to_vec()
}

fn ids_of(value: &Value) -> Vec<&str> {
    value["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["id"].as_str().expect("tool id"))
        .collect()
}

fn find_tool<'a>(tools: &'a [Value], id: &str) -> &'a Value {
    tools
        .iter()
        .find(|tool| tool["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("registry must advertise {id}"))
}

/// Sorted deterministic ids, honest per-tool advertisement, and the pinned
/// unavailable web adapters shared by every scenario.
fn assert_sorted_ids(value: &Value) {
    assert_eq!(ids_of(value), BUILTIN_IDS);
}

fn assert_adapter_sections_unavailable(value: &Value) {
    assert_eq!(value["attachments"]["draft_ingest"], true);
    assert_eq!(value["attachments"]["available_for_web_turn"], false);
    assert_eq!(value["search"]["available"], false);
    assert_eq!(value["deep_research"]["available"], false);
    assert_eq!(value["voice"]["available"], false);
    assert_eq!(value["plugins"]["available_for_web_turn"], false);
    assert_eq!(value["approvals"]["available_for_web_turn"], false);
}

fn map_flags(value: &Value) -> Vec<(&'static str, bool)> {
    let tools = tools_of(value);
    BUILTIN_IDS
        .iter()
        .map(|id| {
            (
                *id,
                find_tool(&tools, id)["available_for_web_turn"]
                    .as_bool()
                    .unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn web_012_t01_daemon_reports_real_tools_and_unavailable_web_adapters() {
    let _lock = env_lock();
    let _ambient = EnvGuard::unset(TURN_ALLOWLIST_KEY);

    // Default (unset): the seven real turn tools are advertised executable;
    // the legacy registered `file` id must stay false, not blanket-true.
    let value = get_capabilities().await;
    assert_sorted_ids(&value);
    let tools = tools_of(&value);
    for id in BUILTIN_IDS {
        let tool = find_tool(&tools, id);
        assert_eq!(tool["enabled"], true, "{id} must stay enabled");
        let reason = tool["reason"].as_str().expect("reason string");
        assert!(!reason.is_empty(), "{id} reason must be non-empty");
        assert!(
            reason.contains("turn adapter"),
            "{id} reason must name the turn-adapter boundary"
        );
    }
    assert_eq!(
        map_flags(&value),
        BUILTIN_IDS
            .into_iter()
            .map(|id| (id, TURN_IDS.contains(&id)))
            .collect::<Vec<_>>(),
        "default advertisement must be honest per registry id"
    );
    assert_adapter_sections_unavailable(&value);

    // Explicit empty: the durable deny list (Some(""), not unset) disables
    // every advertised id without widening anything to true.
    let _deny = EnvGuard::set(TURN_ALLOWLIST_KEY, "");
    let denied = get_capabilities().await;
    assert_sorted_ids(&denied);
    for (id, enabled) in map_flags(&denied) {
        assert!(!enabled, "{id} must be denied under the empty allowlist");
        assert!(
            find_tool(&tools_of(&denied), id)["reason"]
                .as_str()
                .unwrap()
                .contains("turn adapter"),
            "{id} denial reason must name the turn-adapter boundary"
        );
    }
    assert_adapter_sections_unavailable(&denied);
    drop(_deny);

    // Explicit list narrows honestly.
    let _allow = EnvGuard::set(TURN_ALLOWLIST_KEY, "bash");
    let allowed = get_capabilities().await;
    assert_sorted_ids(&allowed);
    assert_eq!(
        map_flags(&allowed),
        BUILTIN_IDS
            .into_iter()
            .map(|id| (id, id == "bash"))
            .collect::<Vec<_>>(),
        "explicit allowlist must narrow, not widen or blanket"
    );
    assert_adapter_sections_unavailable(&allowed);
    drop(_allow);

    // The legacy registered `file` id is never dispatchable even if named.
    let _named = EnvGuard::set(TURN_ALLOWLIST_KEY, "file");
    let named = get_capabilities().await;
    assert_eq!(
        find_tool(&tools_of(&named), "file")["available_for_web_turn"],
        false
    );
    for (id, enabled) in map_flags(&named) {
        assert!(!enabled, "{id} must stay denied when only 'file' is named");
    }
}

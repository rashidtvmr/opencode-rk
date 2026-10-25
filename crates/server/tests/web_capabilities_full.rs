//! Pinned/whole-response `/api/capabilities` contract.
//!
//! Registry truth: ten built-in ids emitted sorted (bash, edit, file, glob,
//! grep, read, shell, webfetch, websearch, write). Default operator env
//! (`OPENCODE_RK_TURN_TOOLS` unset) must mark the nine real turn tools true
//! and keep the legacy registered `file` inspector false; env-dependent scenarios are serialized
//! with a process mutex, and env-independent scenarios never touch the var.
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

/// All built-in ids in the sorted wire order, including legacy `file`.
const BUILTIN_IDS: [&str; 10] = [
    "bash", "edit", "file", "glob", "grep", "read", "shell", "webfetch", "websearch", "write",
];
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

async fn get_capabilities(uri: &str) -> Value {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn web_cap_full_t01_available_flags_pinned() {
    let _lock = env_lock();
    let _ambient = EnvGuard::unset(TURN_ALLOWLIST_KEY);
    let value = get_capabilities("/api/capabilities").await;
    assert_eq!(value["search"]["available"], false);
    assert_eq!(value["deep_research"]["available"], false);
    assert_eq!(value["voice"]["available"], false);
    assert_eq!(value["attachments"]["draft_ingest"], true);
    assert_eq!(value["attachments"]["available_for_web_turn"], false);
    assert_eq!(value["plugins"]["available_for_web_turn"], false);
    assert_eq!(value["approvals"]["available_for_web_turn"], false);
    for tool in value["tools"].as_array().unwrap() {
        let id = tool["id"].as_str().expect("tool id");
        // Default advertisement: real turn tools enabled, legacy file not.
        assert_eq!(
            tool["available_for_web_turn"],
            id != "file",
            "{id} flag must be honest for the shipped default"
        );
    }
    assert_eq!(value["artifacts"]["available"], true);
    assert_eq!(value["artifacts"]["editing_available"], true);
    assert_eq!(value["artifacts"]["run_available"], false);
    assert_eq!(value["artifacts"]["apply_available"], false);
}

#[tokio::test]
async fn web_cap_full_t02_reasons_non_empty() {
    let value = get_capabilities("/api/capabilities").await;
    for key in ["search", "deep_research", "voice", "artifacts"] {
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
    for key in ["plugins", "approvals", "attachments"] {
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
    for tool in value["tools"].as_array().unwrap() {
        let reason = tool["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "tool reason must be non-empty");
    }
}

#[tokio::test]
async fn web_cap_full_t03_unknown_scope_never_available() {
    let value = get_capabilities("/api/capabilities").await;
    assert!(value.get("unknown_scope").is_none());
    let scoped = get_capabilities("/api/capabilities?scope=unknown_scope").await;
    assert!(scoped.get("unknown_scope").is_none());
    assert_eq!(scoped["search"]["available"], false);
    assert_eq!(scoped["deep_research"]["available"], false);
    assert_eq!(scoped["voice"]["available"], false);
    assert_eq!(scoped["attachments"]["available_for_web_turn"], false);
    assert_eq!(scoped["plugins"]["available_for_web_turn"], false);
    assert_eq!(scoped["approvals"]["available_for_web_turn"], false);
}

#[tokio::test]
async fn web_cap_full_t04_surfaces_pinned() {
    let _lock = env_lock();
    let _ambient = EnvGuard::unset(TURN_ALLOWLIST_KEY);
    let value = get_capabilities("/api/capabilities").await;
    let ids = value["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, BUILTIN_IDS);
    for tool in value["tools"].as_array().unwrap() {
        let id = tool["id"].as_str().expect("tool id");
        assert_eq!(tool["enabled"], true);
        // Shipped default: legacy file id never advertised as dispatchable.
        assert_eq!(
            tool["available_for_web_turn"],
            id != "file",
            "{id} must be honestly advertised for the shipped default"
        );
        assert!(
            tool["reason"].as_str().unwrap().contains("turn adapter"),
            "tool reason must name the turn-adapter boundary"
        );
    }
    assert!(
        value["plugins"]["reason"]
            .as_str()
            .unwrap()
            .contains("plugin host"),
        "plugins reason must name the plugin-host gap"
    );
    assert!(
        value["approvals"]["reason"]
            .as_str()
            .unwrap()
            .contains("approval"),
        "approvals reason must name the approval gap"
    );
    assert!(
        value["attachments"]["reason"]
            .as_str()
            .unwrap()
            .contains("attachment"),
        "attachments reason must name the attachment gap"
    );
}

#[tokio::test]
async fn web_cap_full_t05_response_deterministic() {
    let _lock = env_lock();
    let _ambient = EnvGuard::unset(TURN_ALLOWLIST_KEY);
    let first = get_capabilities("/api/capabilities").await;
    let second = get_capabilities("/api/capabilities").await;
    assert_eq!(first, second);
    let ids = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "tools must be emitted in sorted id order");
}

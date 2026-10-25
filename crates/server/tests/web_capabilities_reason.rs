//! Reason honesty for `/api/capabilities` under the shipped default.
//!
//! Every capability/tool entry must carry a non-empty, honest reason that names
//! the turn-adapter boundary. Default (`OPENCODE_RK_TURN_TOOLS` unset) is the
//! operator default: the nine real turn tools (bash, shell, read, write, edit,
//! grep, glob, webfetch, websearch) are advertised true while the legacy
//! registered `file` inspector stays non-dispatchable. An explicit env value is
//! not required to make the reasons honest - never over-constrain a token that
//! is not the wire contract.
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

/// Turn-dispatchable registry ids in sorted order; legacy `file` excluded.
const TURN_IDS: [&str; 9] = [
    "bash", "edit", "glob", "grep", "read", "shell", "webfetch", "websearch", "write",
];
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

fn file_entry<'a>(value: &'a Value) -> &'a Value {
    value["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|tool| tool["id"].as_str() == Some("file"))
        .expect("legacy registered file id must be listed")
}

#[tokio::test]
async fn web_013_reason_unavailable_entries_carry_non_empty_reasons() {
    let _lock = env_lock();
    let _ambient = EnvGuard::unset(TURN_ALLOWLIST_KEY);
    let value = get_capabilities("/api/capabilities").await;
    for key in ["search", "deep_research", "voice"] {
        assert_eq!(value[key]["available"], false);
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
    // Honesty in both directions: exact registry inventory, sorted; every
    // reason names the turn-adapter boundary; default advertisement is truthful.
    let tools = value["tools"].as_array().expect("tools array");
    let ids: Vec<_> = tools
        .iter()
        .map(|tool| tool["id"].as_str().expect("tool id"))
        .collect();
    assert_eq!(ids, BUILTIN_IDS);
    for tool in tools {
        let id = tool["id"].as_str().expect("tool id");
        let reason = tool["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{id} reason must be non-empty");
        assert!(
            reason.contains("turn adapter"),
            "{id} reason must name the turn-adapter boundary"
        );
    }
    let expected_true: Vec<&str> = TURN_IDS.to_vec();
    for tool in tools {
        let id = tool["id"].as_str().expect("tool id");
        let turn = tool["available_for_web_turn"].as_bool().expect("bool");
        assert_eq!(
            turn,
            expected_true.contains(&id),
            "{id} default advertisement must be honest"
        );
    }
    assert_eq!(file_entry(&value)["available_for_web_turn"], false);
    for key in ["plugins", "approvals", "attachments"] {
        assert_eq!(value[key]["available_for_web_turn"], false);
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
}

#[tokio::test]
async fn web_013_reason_names_adapter_boundary() {
    let value = get_capabilities("/api/capabilities").await;
    assert!(
        value["search"]["reason"].as_str().unwrap().contains("grep"),
        "search reason must name the grep boundary"
    );
    assert!(
        value["deep_research"]["reason"]
            .as_str()
            .unwrap()
            .contains("citation"),
        "research reason must name the citation gap"
    );
    let voice = value["voice"]["reason"].as_str().unwrap();
    assert!(
        voice.contains("transcription") || voice.contains("audio"),
        "voice reason must name the audio gap"
    );
    // Non-dispatchable ids keep an honest boundary reason under the default.
    assert!(
        file_entry(&value)["reason"]
            .as_str()
            .unwrap()
            .contains("turn adapter"),
        "file must name the turn-adapter boundary"
    );
}

#[tokio::test]
async fn web_013_unknown_scope_is_not_silently_available() {
    let value = get_capabilities("/api/capabilities").await;
    assert!(value.get("unknown_scope").is_none());
    assert!(value["search"].get("available_for_web_turn").is_none());
    let scoped = get_capabilities("/api/capabilities?scope=unknown_scope").await;
    assert_eq!(scoped["search"]["available"], false);
    assert_eq!(scoped["deep_research"]["available"], false);
    assert_eq!(scoped["voice"]["available"], false);
}

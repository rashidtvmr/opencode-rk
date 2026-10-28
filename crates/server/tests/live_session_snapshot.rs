//! LIVE-SNAPSHOT-CONTRACT (v3): authenticated GET /api/sessions/live returns
//! an atomic, bounded projection of the live RunOwner registry carrying only
//! stable identity and lifecycle booleans. Secrets seeded into a genuine
//! pending permission (process args, cwd, and the resources list real callers
//! embed) must never appear in the serialized route body, while
//! `hasPendingPermission` stays true on the wire for the correct session and
//! flips to false after cancellation. No real turn, provider, or network is
//! exercised.
//!
//! Isolation audit: every `crate::server/tests/*.rs` file compiles to its
//! own test binary, so this file's registry state (process-local statics)
//! can never interleave with the lib unit tests or with other integration
//! binaries; the process-global admission registry and `ACTIVE_TURNS` are
//! unique to this process. Every test in this file that admits an owner
//! holds the file-local `SNAPSHOT_HTTP_LOCK`, so within this binary all
//! permit acquisition and registry assertions are serialized. The genuine
//! pending test additionally scopes `OPENCODE_RK_PERMISSION_RULES` to a
//! disposable never-written temp path (restored on drop): its `install_pending`
//! call resolves a known-empty rule set and the real user preference file is
//! never read.
#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::SessionId;
use opencode_rk_security::{OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_server::{
    AppState,
    daemon_auth::DaemonAuth,
    router_with_auth,
    run_owner::{PermissionRequest, PermissionSource, TurnExecutionPermit},
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::{TempDir, tempdir};
use tower::ServiceExt;

/// Serializes every test in this file (they all need exclusive admission).
static SNAPSHOT_HTTP_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();

async fn lock() -> tokio::sync::MutexGuard<'static, ()> {
    SNAPSHOT_HTTP_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

/// Scoped rules-path fixture. While held, `permission_rules::path()` resolves
/// to a disposable, never-written temp file, so rule loads see a known-empty
/// set and the real user preference file is never read. The previous value
/// is restored on drop. Safe under parallel tests because this binary's
/// tests are serialized by `SNAPSHOT_HTTP_LOCK`, so no sibling test reads
/// the variable concurrently.
struct ScopedRulesPath {
    previous: Option<std::ffi::OsString>,
    _disposable: tempfile::TempDir,
}

impl ScopedRulesPath {
    fn install() -> Self {
        let disposable = tempdir().expect("disposable rules fixture dir");
        let previous = std::env::var_os("OPENCODE_RK_PERMISSION_RULES");
        std::env::set_var(
            "OPENCODE_RK_PERMISSION_RULES",
            disposable.path().join("never-written-rules.json"),
        );
        Self {
            previous,
            _disposable: disposable,
        }
    }
}

impl Drop for ScopedRulesPath {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var("OPENCODE_RK_PERMISSION_RULES", value),
            None => std::env::remove_var("OPENCODE_RK_PERMISSION_RULES"),
        }
    }
}

fn build_app(auth: DaemonAuth) -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    let sessions = SessionService::new(Arc::new(storage));
    let app = router_with_auth(
        AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    );
    (app, dir)
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request")
}

async fn body_json(response: axum::response::Response) -> (String, Value) {
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let raw = String::from_utf8(body.to_vec()).expect("utf8 body");
    let value: Value = serde_json::from_str(&raw).expect("json");
    (raw, value)
}

/// The wire shape of one live-rail entry is exactly these five keys.
const TAB_KEYS: [&str; 5] = [
    "sessionID",
    "runID",
    "admitted",
    "cancelled",
    "hasPendingPermission",
];

fn assert_entry_shape(entry: &Value) {
    let object = entry.as_object().expect("entry is an object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = TAB_KEYS;
    expected.sort_unstable();
    assert_eq!(keys, expected, "live route entries must stay boolean-only");
}

#[tokio::test]
async fn live_snapshot_missing_bearer_is_401() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let (app, _dir) = build_app(auth);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/live")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn live_snapshot_wrong_bearer_is_403() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let (app, _dir) = build_app(auth);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/live")
                .header("Authorization", "Bearer 0".repeat(64))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn live_snapshot_no_owners_returns_empty_array() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let token = auth.token().to_owned();
    let (app, _dir) = build_app(auth);
    let response = app
        .oneshot(get("/api/sessions/live", &token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (_, value) = body_json(response).await;
    assert!(
        value["data"].as_array().expect("data array").is_empty(),
        "no admitted owners must yield an empty live snapshot"
    );
}

#[tokio::test]
async fn live_snapshot_authed_two_admitted_rows() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let token = auth.token().to_owned();
    let _first = TurnExecutionPermit::acquire(SessionId::new()).expect("root capacity");
    let _second = TurnExecutionPermit::acquire(SessionId::new()).expect("root capacity");
    let (app, _dir) = build_app(auth);
    let response = app
        .oneshot(get("/api/sessions/live", &token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (_, value) = body_json(response).await;
    let data = value["data"].as_array().expect("data array");
    assert_eq!(data.len(), 2, "two admitted owners must appear");
    for entry in data {
        assert_eq!(entry["admitted"].as_bool(), Some(true));
        assert_eq!(entry["cancelled"].as_bool(), Some(false));
        assert!(entry["sessionID"].as_str().is_some());
        assert!(entry["runID"].as_u64().is_some());
        assert_eq!(
            entry["hasPendingPermission"].as_bool(),
            Some(false),
            "fresh owners carry no pending permission"
        );
        assert_entry_shape(entry);
    }
}

#[tokio::test]
async fn live_snapshot_empty_after_permit_drop_authed() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let token = auth.token().to_owned();
    {
        let _permit = TurnExecutionPermit::acquire(SessionId::new()).expect("root capacity");
        let (app, _dir) = build_app(auth.clone());
        let response = app
            .oneshot(get("/api/sessions/live", &token))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let (_, value) = body_json(response).await;
        assert_eq!(value["data"].as_array().expect("data array").len(), 1);
    }
    let (app, _dir) = build_app(auth);
    let response = app
        .oneshot(get("/api/sessions/live", &token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (_, value) = body_json(response).await;
    assert!(
        value["data"].as_array().expect("data array").is_empty(),
        "dropped permit must be absent from the live snapshot"
    );
}

/// Genuine pending-permission leak guard through the real authenticated
/// route. A unique secret is seeded into a real `OperationIntent` (process
/// args + cwd) and, mirroring real call sites, into the request's
/// `resources` as the serialized intent, installed via the legitimate
/// `install_pending` entry point. The route body must report
/// `hasPendingPermission == true` for the exact session while omitting every
/// seeded secret, and must flip the boolean to false after cancellation.
#[tokio::test]
async fn live_snapshot_authed_pending_secret_omitted_bool_true_then_false() {
    let _lock = lock().await;
    let auth = DaemonAuth::mint().expect("mint");
    let token = auth.token().to_owned();
    let route_secret = "route-live-fake-secret-4d9be21c";
    let route_secret_cwd = "/tmp/route-live-fake-path-51c7aa9";

    let permit = TurnExecutionPermit::acquire(SessionId::new()).expect("root capacity");
    let owner = permit.owner();
    let session_id = owner.session_id.to_string();

    let intent = OperationIntent::Process {
        program: "cat".to_owned(),
        args: vec!["read".to_owned(), route_secret.to_owned()],
        cwd: std::path::PathBuf::from(route_secret_cwd),
    };
    let request = PermissionRequest {
        id: "per_route_fixture".to_owned(),
        session_id: session_id.clone(),
        action: "process: cat".to_owned(),
        resources: vec![serde_json::to_string(&intent).expect("intent serializes")],
        source: PermissionSource {
            kind: "tool",
            message_id: "m-route-snapshot".to_owned(),
            call_id: "c-route-snapshot".to_owned(),
        },
    };
    // Known-empty rule set via the disposable scoped path: no persisted rule
    // can match the ad-hoc action "process: cat", so the genuine request is
    // stored pending rather than auto-resolved, deterministically and without
    // touching the user's real preference file.
    let _rules = ScopedRulesPath::install();
    let _receiver = owner
        .install_pending(
            request,
            intent,
            &PermissionBroker::new(SecurityPolicy::lean_default("/")),
        )
        .expect("pending request must be stored for an unmatched action");

    // Positive path: boolean true on the wire with the secret payload absent.
    let (app, _dir_one) = build_app(auth.clone());
    let response = app
        .oneshot(get("/api/sessions/live", &token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (raw, value) = body_json(response).await;
    let entry = value["data"]
        .as_array()
        .expect("data array")
        .iter()
        .find(|entry| entry["sessionID"].as_str() == Some(session_id.as_str()))
        .expect("admitted pending owner must appear in the live route");
    assert_eq!(
        entry["hasPendingPermission"].as_bool(),
        Some(true),
        "genuine pending must read true through the route"
    );
    assert_eq!(entry["admitted"].as_bool(), Some(true));
    assert_eq!(entry["cancelled"].as_bool(), Some(false));
    assert_entry_shape(entry);
    assert!(
        !raw.contains(route_secret),
        "route body must omit the seeded secret"
    );
    assert!(
        !raw.contains(route_secret_cwd),
        "route body must omit the seeded secret cwd"
    );
    assert!(
        !raw.contains("resources"),
        "route body must never carry a resources key"
    );

    // Negative path: cancel clears the pending permission; the owner stays
    // registered until its permit drops, now with the boolean false.
    owner.cancel();
    let (app, _dir_two) = build_app(auth);
    let response = app
        .oneshot(get("/api/sessions/live", &token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let (_, value) = body_json(response).await;
    let entry = value["data"]
        .as_array()
        .expect("data array")
        .iter()
        .find(|entry| entry["sessionID"].as_str() == Some(session_id.as_str()))
        .expect("owner remains registered until the permit drops");
    assert_eq!(
        entry["hasPendingPermission"].as_bool(),
        Some(false),
        "cancel must flip the pending boolean to false"
    );
    assert_eq!(entry["admitted"].as_bool(), Some(true));
    drop(permit);
}

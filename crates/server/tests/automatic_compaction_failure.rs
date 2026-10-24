//! Failed or interrupted automatic compaction must leave durable context intact.
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, routing::post, Json};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use tower::ServiceExt;

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Environment {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(values.iter().map(|(key, value)| {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            (*key, previous)
        }).collect())
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, previous) in &self.0 {
            match previous {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task { fn drop(&mut self) { self.0.abort(); } }

#[tokio::test]
async fn provider_failure_and_interruption_leave_history_and_checkpoint_unchanged() {
    for mode in ["provider-failure", "interrupt", "concurrent-append"] {
        let interrupt = mode == "interrupt";
        let concurrent_append = mode == "concurrent-append";
        let directory = tempfile::tempdir().unwrap();
        let sessions = SessionService::new(Arc::new(Storage::open_in_memory(directory.path().join("blobs")).unwrap()));
        let session = sessions.create_in_directory("Compaction failure", Some(directory.path().to_owned())).await.unwrap();
        sessions.append_text(session.id, MessageRole::User, "Durable question").await.unwrap();
        sessions.append_text(session.id, MessageRole::Assistant, "Durable answer").await.unwrap();
        let before = sessions.messages(session.id, 10).await.unwrap();
        let (seen_tx, mut seen_rx) = tokio::sync::mpsc::channel::<Value>(1);
        let release = Arc::new(tokio::sync::Notify::new());
        let handler_release = release.clone();
        let regular_calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = regular_calls.clone();
        let provider = axum::Router::new()
            .route("/responses/compact", post(move |Json(body): Json<Value>| {
                let seen = seen_tx.clone();
                let release = handler_release.clone();
                async move {
                    seen.send(body).await.unwrap();
                    if interrupt || concurrent_append { release.notified().await; }
                    if concurrent_append {
                        return (StatusCode::OK, Json(json!({
                            "id":"stale-window", "object":"response.compaction", "created_at":1,
                            "output":[{"type":"compaction","id":"stale-item","encrypted_content":"stale-opaque-state"}]
                        })));
                    }
                    (StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"fixture compaction rejected"}})))
                }
            }))
            .route("/responses", post(move || {
                handler_calls.fetch_add(1, Ordering::SeqCst);
                async { Json(json!({"output":[]})) }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let _task = Task(tokio::spawn(async move { axum::serve(listener, provider).await.unwrap(); }));
        let _environment = Environment::set(&[
            ("OPENAI_BASE_URL", format!("http://{address}")),
            ("OPENAI_API_KEY", "disposable-failure-fixture".into()),
            ("OPENCODE_RK_HOME", directory.path().to_string_lossy().into_owned()),
            ("OPENCODE_RK_COMPACTION__ENABLED", "true".into()),
            ("OPENCODE_RK_COMPACTION__STRATEGY", "native".into()),
            ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "1".into()),
        ]);
        let app = router(AppState { sessions:sessions.clone(), catalog:Arc::new(Catalog::default()) });
        let request = Request::builder().method("POST")
            .uri(format!("/api/sessions/{}/turns", session.id)).header("content-type", "application/json")
            .body(Body::from(json!({"model":"openai/fixture","text":"Must not append on failed preflight"}).to_string())).unwrap();
        let turn_app = app.clone();
        let turn = tokio::spawn(async move { turn_app.oneshot(request).await.unwrap() });
        let compact = tokio::time::timeout(Duration::from_secs(2), seen_rx.recv()).await.unwrap().unwrap();
        assert_eq!(compact["input"].as_array().unwrap().len(), 2);
        if interrupt {
            let response = tokio::time::timeout(Duration::from_secs(2), app.clone().oneshot(
                Request::builder().method("POST").uri(format!("/api/session/{}/interrupt", session.id))
                    .body(Body::empty()).unwrap()
            )).await.unwrap().unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            release.notify_waiters();
        }
        if concurrent_append {
            let response = app.oneshot(Request::builder().method("POST")
                .uri(format!("/api/sessions/{}/messages", session.id)).header("content-type", "application/json")
                .body(Body::from(json!({"text":"Concurrent external message"}).to_string())).unwrap())
                .await.unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            release.notify_waiters();
        }
        let response = tokio::time::timeout(Duration::from_secs(2), turn).await.unwrap().unwrap();
        assert!(!response.status().is_success());
        let _ = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let after = sessions.messages(session.id, 10).await.unwrap();
        if concurrent_append {
            assert_eq!(after.len(), before.len() + 1);
            assert_eq!(&after[..before.len()], before.as_slice());
            assert!(serde_json::to_string(&after.last().unwrap().body).unwrap().contains("Concurrent external message"));
        } else {
            assert_eq!(after, before);
        }
        assert!(sessions.compaction_checkpoint(session.id).await.unwrap().is_none());
        assert_eq!(regular_calls.load(Ordering::SeqCst), 0);
    }
}

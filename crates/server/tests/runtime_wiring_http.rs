use std::sync::Arc;

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_providers::registry::ProviderRegistry;
use opencode_rk_server::{
    router, runtime_wiring::RuntimeWiring, app_runtime::EnginePolicy, AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use opencode_rk_tools::registry::ToolRegistry;
use serde_json::json;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

#[tokio::test]
async fn http_turn_stream_borrows_daemon_runtime_permits() {
    let dir = tempdir().expect("temporary fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    let session = sessions.create("runtime permit test").await.expect("session");
    let sessions_after = sessions.clone();
    let runtime = RuntimeWiring::with_sessions(
        ProviderRegistry::new(),
        sessions.clone(),
        ToolRegistry::new(),
        EnginePolicy::default_deny(),
    );

    let available = runtime.available_permits();
    assert!(available > 0);
    assert!(available <= 2);
    runtime.register_session(session.id).expect("runtime session");
    let (events, _receiver) = mpsc::channel(1);
    let client = runtime.attach(events).expect("runtime client");
    let mut held = Vec::with_capacity(available);
    for _ in 0..available {
        let (_, permit) = runtime
            .submit_headless(
                client,
                session.id,
                opencode_rk_contracts::AgentId::new(),
                "openai/test",
            )
            .expect("shared permit");
        held.push(permit);
    }
    assert_eq!(runtime.available_permits(), 0);

    // Preserve the two-field compatibility surface while supplying the daemon
    // runtime through an Axum extension.
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    })
    .layer(Extension(runtime.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns/stream", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "text": "",
                        "model": "openai/test",
                        "reasoning_effort": "none"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(runtime.available_permits(), 0);
    assert!(sessions_after
        .messages(session.id, 10)
        .await
        .expect("messages")
        .is_empty());
    drop(held);
    assert_eq!(runtime.available_permits(), available);
}

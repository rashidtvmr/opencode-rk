//! TAB-21: `POST /api/sessions/{id}/view` records the observed idle transition
//! as the monotone `viewedAt` watermark (`session.ts:61-69`, `projector.ts:619`)
//! and returns `204 No Content`; `get()` serializes the fact.
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn view_sets_monotone_watermark() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let post = |uri: String, value: Value| {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap()
    };

    let response = app
        .clone()
        .oneshot(post(
            "/api/sessions".into(),
            json!({"title": "view", "directory": dir.path()}),
        ))
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let id = value["session"]["id"].as_str().unwrap().to_owned();

    // `session.ts:62-68`: the route is a no-op while the session has no idle
    // transition to acknowledge, so establish one first - exactly the fact the
    // projector writes on a terminal execution (`projector.ts:423-447`).
    let idle = opencode_rk_contracts::Timestamp::from_epoch_millis(1_700_000_000_000).unwrap();
    sessions
        .mark_idle(id.parse().unwrap(), idle)
        .await
        .expect("record the idle transition");

    // A NEWER idle transition advances the watermark and returns 204.
    let response = app
        .clone()
        .oneshot(post(
            format!("/api/sessions/{id}/view"),
            json!({"idle": 1_700_000_000_000i64}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let fetched: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        fetched["session"]["viewed_at"],
        json!("2023-11-14T22:13:20Z")
    );

    // A STALE observation never regresses the watermark.
    let response = app
        .clone()
        .oneshot(post(
            format!("/api/sessions/{id}/view"),
            json!({"idle": 1_600_000_000_000i64}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let fetched: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        fetched["session"]["viewed_at"],
        json!("2023-11-14T22:13:20Z")
    );
}

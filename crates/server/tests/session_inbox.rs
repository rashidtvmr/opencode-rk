//! MSG-009: the session inbox routes.
//!
//! Faithful to the pinned upstream (`packages/server/src/handlers/session.ts:519-548`,
//! `packages/core/src/session/inbox.ts:246-272`):
//! - `GET /api/sessions/:id/inbox` lists items ordered by enqueue sequence, 404
//!   when the session is unknown.
//! - `PATCH /api/sessions/:id/inbox/:inboxID` changes delivery unconditionally
//!   (upstream has no single-steer rule), 404 for an unknown session and 409 for
//!   an unknown item (`mutatePending` → `InboxConflictError`).
//! - `DELETE /api/sessions/:id/inbox/:inboxID` cancels and is an idempotent 204
//!   no-op once the row is gone.
//! Each mutation publishes its `session.inbox.*` event.
use std::{str::FromStr, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{Delivery, SessionId};
use opencode_rk_server::{
    event_bus::{EventBus, ServerEvent, Subscription},
    router_with_events, AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

fn app_with_sessions() -> (axum::Router, Arc<Storage>, Subscription) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let events = EventBus::new(64);
    let sub = events.subscribe(None);
    let app = router_with_events(
        AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        },
        None,
        events,
    );
    // The tempdir is leaked into the process for the lifetime of the test; the
    // in-memory storage keeps the blobs dir alive via its own handle.
    std::mem::forget(dir);
    (app, storage, sub)
}

async fn create_session(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": title }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        response.status().is_success(),
        "create failed: {}",
        response.status()
    );
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    body["session"]["id"].as_str().unwrap().to_owned()
}

async fn inbox_status(app: &axum::Router, id: &str) -> (StatusCode, Vec<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{id}/inbox"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    if status != StatusCode::OK {
        return (status, Vec::new());
    }
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let ids = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_owned())
        .collect();
    (status, ids)
}

async fn patch_delivery(app: &axum::Router, id: &str, inbox_id: &str, delivery: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/sessions/{id}/inbox/{inbox_id}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "delivery": delivery }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn delete_inbox(app: &axum::Router, id: &str, inbox_id: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sessions/{id}/inbox/{inbox_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

fn take_events(sub: &mut Subscription) -> Vec<ServerEvent> {
    let mut out = Vec::new();
    while let Ok(event) = sub.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn msg009_inbox_routes() {
    let (app, storage, mut sub) = app_with_sessions();
    let id = create_session(&app, "inbox").await;
    let sid = SessionId::from_str(&id).unwrap();

    // There is no HTTP enqueue route (upstream enqueues through the prompt/steer
    // flow), so seed the store directly and exercise the read/mutate routes.
    let first = storage
        .enqueue_inbox(sid, "user", json!({ "text": "one" }), Delivery::Steer)
        .unwrap();
    let second = storage
        .enqueue_inbox(sid, "user", json!({ "text": "two" }), Delivery::Queue)
        .unwrap();
    let _ = take_events(&mut sub);

    // GET lists both, ordered by enqueue sequence.
    assert_eq!(
        inbox_status(&app, &id).await,
        (StatusCode::OK, vec![first.id.clone(), second.id.clone()])
    );

    // PATCH steer -> queue succeeds even though `first` was the only steer row
    // (upstream has no single-steer rule), and publishes the change.
    assert_eq!(
        patch_delivery(&app, &id, &first.id, "queue").await,
        StatusCode::NO_CONTENT
    );
    let events = take_events(&mut sub);
    assert!(
        events.iter().any(|e| matches!(
            e,
            ServerEvent::InboxDeliveryChanged { session, inbox_id, delivery }
                if *session == sid && inbox_id == &first.id && *delivery == Delivery::Queue
        )),
        "expected InboxDeliveryChanged, got {events:?}"
    );
    assert_eq!(
        inbox_status(&app, &id).await,
        (StatusCode::OK, vec![first.id.clone(), second.id.clone()])
    );

    // PATCH an unknown item is a 409 conflict (`mutatePending` -> InboxConflictError).
    assert_eq!(
        patch_delivery(&app, &id, "does-not-exist", "queue").await,
        StatusCode::CONFLICT
    );

    // DELETE removes the row and publishes the cancellation.
    assert_eq!(
        delete_inbox(&app, &id, &second.id).await,
        StatusCode::NO_CONTENT
    );
    let events = take_events(&mut sub);
    assert!(
        events.iter().any(|e| matches!(
            e,
            ServerEvent::InboxCancelled { session, inbox_id }
                if *session == sid && inbox_id == &second.id
        )),
        "expected InboxCancelled, got {events:?}"
    );
    assert_eq!(
        inbox_status(&app, &id).await,
        (StatusCode::OK, vec![first.id.clone()])
    );

    // DELETE is an idempotent no-op once the row is gone.
    assert_eq!(
        delete_inbox(&app, &id, &second.id).await,
        StatusCode::NO_CONTENT
    );
    // ... and it publishes nothing (upstream `projectCancelled` only emits on a
    // real delete).
    assert!(
        take_events(&mut sub).is_empty(),
        "a repeat cancel must not publish InboxCancelled"
    );
    assert_eq!(
        inbox_status(&app, &id).await,
        (StatusCode::OK, vec![first.id.clone()])
    );

    // An unknown session is a 404, not an empty list.
    let unknown = SessionId::new().to_string();
    assert_eq!(
        inbox_status(&app, &unknown).await.0,
        StatusCode::NOT_FOUND
    );
}

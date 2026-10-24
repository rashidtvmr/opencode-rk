use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    daemon_auth::DaemonAuth,
    event_bus::{EventBus, EventKind, ServerEvent},
    router_with_events, AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn send(
    app: &axum::Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    value: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(value.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn session_todo_events_are_post_commit_and_rejected_writes_are_silent() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let auth = DaemonAuth::mint().unwrap();
    let events = EventBus::new(8);
    let mut subscription = events.subscribe(Some(EventKind::TodoUpdated));
    let app = router_with_events(
        AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth.clone()),
        events,
    );
    let (status, created) = send(
        &app,
        Some(auth.token()),
        "POST",
        "/api/sessions",
        json!({"title":"Todo event"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["session"]["id"].as_str().unwrap();
    let path = format!("/api/sessions/{id}/todo");
    let todo =
        json!({"id":"one","content":"Commit before publish","status":"pending","priority":"high"});
    assert_eq!(
        send(&app, None, "PUT", &path, json!({"todos":[todo.clone()]}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, None, "GET", &path, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(subscription.try_recv().is_err());
    assert_eq!(
        send(
            &app,
            Some(auth.token()),
            "PUT",
            &path,
            json!({"todos":[todo.clone()]})
        )
        .await
        .0,
        StatusCode::OK
    );
    let event = subscription
        .try_recv()
        .expect("event published before the API returns");
    let ServerEvent::TodoUpdated { session, todos } = event else {
        panic!("wrong event")
    };
    assert_eq!(session.to_string(), id);
    assert_eq!(serde_json::to_value(&todos).unwrap(), json!([todo]));
    // Read through the actual storage-backed service after observing the event.
    assert_eq!(sessions.todos(session).await.unwrap(), todos);
    assert!(
        subscription.try_recv().is_err(),
        "exactly one successful publication"
    );
    assert!(send(
        &app,
        Some(auth.token()),
        "PUT",
        &path,
        json!({"todos":[{"id":"bad","content":"","status":"pending","priority":"low"}]})
    )
    .await
    .0
    .is_client_error());
    assert!(
        subscription.try_recv().is_err(),
        "invalid replacement must not publish"
    );
    assert_eq!(sessions.todos(session).await.unwrap(), todos);
    let mut oversized = Vec::new();
    for index in 0..20 {
        let mut item = todos[0].clone();
        item.id = index.to_string();
        item.content = "x".repeat(4096);
        oversized.push(item);
    }
    // Even internal producers cannot bypass the event aggregate bound.
    let internal_bus = EventBus::new(1);
    let mut internal_subscription = internal_bus.subscribe(None);
    assert_eq!(
        internal_bus.publish(ServerEvent::TodoUpdated {
            session,
            todos: oversized
        }),
        Err(opencode_rk_server::event_bus::BusError::InvalidPayload)
    );
    assert!(internal_subscription.try_recv().is_err());
}

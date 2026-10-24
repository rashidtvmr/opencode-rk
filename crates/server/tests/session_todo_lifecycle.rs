//! Public-route contracts for ordered, durable, session-scoped todo replacement.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

fn app(root: &std::path::Path) -> axum::Router {
    router(AppState {
        sessions: SessionService::new(Arc::new(Storage::open(StoragePaths::under(root)).unwrap())),
        catalog: Arc::new(Catalog::default()),
    })
}

async fn request(app: &axum::Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn session(app: &axum::Router) -> String {
    let (status, body) = request(
        app,
        "POST",
        "/api/sessions",
        json!({"title":"Todo lifecycle"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["session"]["id"].as_str().unwrap().to_owned()
}

fn todos() -> Value {
    json!({"todos":[
        {"id":"second","content":"Inspect real payload","status":"in_progress","priority":"high"},
        {"id":"first","content":"Retain persisted ordering","status":"pending","priority":"medium"}
    ]})
}

#[tokio::test]
async fn session_todo_replacement_preserves_order_isolation_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let app1 = app(root.path());
    let first = session(&app1).await;
    let other = session(&app1).await;
    let path = format!("/api/sessions/{first}/todo");
    let expected = todos();
    let (status, body) = request(&app1, "PUT", &path, expected.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, expected);
    assert_eq!(
        request(
            &app1,
            "GET",
            &format!("/api/sessions/{other}/todo"),
            Value::Null
        )
        .await,
        (StatusCode::OK, json!({"todos":[]}))
    );
    drop(app1);
    let reopened = app(root.path());
    assert_eq!(
        request(&reopened, "GET", &path, Value::Null).await,
        (StatusCode::OK, expected)
    );
    assert_eq!(
        request(&reopened, "PUT", &path, json!({"todos":[]})).await,
        (StatusCode::OK, json!({"todos":[]}))
    );
    assert_eq!(
        request(&reopened, "GET", &path, Value::Null).await,
        (StatusCode::OK, json!({"todos":[]}))
    );
}

#[tokio::test]
async fn session_todo_invalid_replacement_does_not_delete_existing_items() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let id = session(&app).await;
    let path = format!("/api/sessions/{id}/todo");
    let original = todos();
    assert_eq!(
        request(&app, "PUT", &path, original.clone()).await.0,
        StatusCode::OK
    );
    for invalid in [
        json!({"todos":[{"id":"x","content":"Bad status","status":"invented","priority":"high"}]}),
        json!({"todos":[{"id":"x","content":"x".repeat(4097),"status":"pending","priority":"low"}]}),
        json!({"todos":[{"id":"dup","content":"One","status":"pending","priority":"low"},
                         {"id":"dup","content":"Two","status":"pending","priority":"low"}]}),
        json!({"todos":(0..20).map(|i| json!({"id":i.to_string(),"content":"x".repeat(4096),
            "status":"pending","priority":"medium"})).collect::<Vec<_>>()}),
    ] {
        assert!(request(&app, "PUT", &path, invalid)
            .await
            .0
            .is_client_error());
        assert_eq!(
            request(&app, "GET", &path, Value::Null).await,
            (StatusCode::OK, original.clone())
        );
    }
}

#[tokio::test]
async fn session_todo_unknown_session_is_not_an_empty_success() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let path = "/api/sessions/00000000-0000-0000-0000-000000000001/todo";
    assert_eq!(
        request(&app, "GET", path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "PUT", path, todos()).await.0,
        StatusCode::NOT_FOUND
    );
}

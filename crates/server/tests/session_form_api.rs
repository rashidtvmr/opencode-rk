//! FORM server surface: the session-scoped form lifecycle routes publish
//! `form.created` / `form.replied` / `form.cancelled` and enforce the
//! pending-only, session-scoped contract from upstream `Form.Service`
//! (`packages/core/src/form.ts:96-160`).
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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn form_body() -> Value {
    json!({
        "title": "Questions",
        "fields": [{
            "key": "q0",
            "type": "string",
            "title": "Pick",
            "description": "Which?",
            "custom": true,
            "options": [{"value": "A", "label": "A", "description": "first"}]
        }]
    })
}

#[tokio::test]
async fn session_form_lifecycle_is_session_scoped_and_publishes_events() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let auth = DaemonAuth::mint().unwrap();
    let events = EventBus::new(8);
    let mut created = events.subscribe(Some(EventKind::FormCreated));
    let mut replied = events.subscribe(Some(EventKind::FormReplied));
    let mut cancelled = events.subscribe(Some(EventKind::FormCancelled));
    let app = router_with_events(
        AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth.clone()),
        events,
    );

    let (status, created_session) = send(
        &app,
        Some(auth.token()),
        "POST",
        "/api/sessions",
        json!({"title": "Forms"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session = created_session["session"]["id"].as_str().unwrap().to_owned();
    let (status, other_session) = send(
        &app,
        Some(auth.token()),
        "POST",
        "/api/sessions",
        json!({"title": "Other"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let other = other_session["session"]["id"].as_str().unwrap().to_owned();

    let list_path = format!("/api/session/{session}/form");
    // Unauthenticated access is rejected and publishes nothing.
    assert_eq!(
        send(&app, None, "GET", &list_path, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(created.try_recv().is_err());

    // Create: the form is returned and `form.created` is published before the
    // response settles.
    let (status, body) = send(&app, Some(auth.token()), "POST", &list_path, form_body()).await;
    assert_eq!(status, StatusCode::OK);
    let id = body["data"]["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("frm_"), "form id namespace: {id}");
    assert_eq!(body["data"]["sessionId"], json!(session));
    let event = created.try_recv().expect("form.created published");
    let ServerEvent::FormCreated { form } = event else {
        panic!("wrong event")
    };
    assert_eq!(form.id.as_str(), id);
    assert_eq!(form.fields[0].key, "q0");

    // List returns only this session's pending form.
    let (status, listed) = send(&app, Some(auth.token()), "GET", &list_path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    assert_eq!(listed["data"][0]["id"], json!(id));

    // A different session cannot see it.
    let other_path = format!("/api/session/{other}/form");
    let (_, other_list) = send(&app, Some(auth.token()), "GET", &other_path, Value::Null).await;
    assert!(other_list["data"].as_array().unwrap().is_empty());

    // Single get is session-scoped.
    let detail_path = format!("/api/session/{session}/form/{id}");
    let (status, detail) = send(&app, Some(auth.token()), "GET", &detail_path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["data"]["state"]["status"], json!("pending"));
    assert_eq!(
        send(&app, Some(auth.token()), "GET", &format!("/api/session/{other}/form/{id}"), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // Unknown field is rejected and does not settle or publish.
    let reply_path = format!("/api/session/{session}/form/{id}/reply");
    assert_eq!(
        send(&app, Some(auth.token()), "POST", &reply_path, json!({"answer": {"nope": "x"}}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert!(replied.try_recv().is_err());

    // A valid reply settles once and publishes `form.replied`.
    assert_eq!(
        send(&app, Some(auth.token()), "POST", &reply_path, json!({"answer": {"q0": "A"}}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let event = replied.try_recv().expect("form.replied published");
    let ServerEvent::FormReplied { id: event_id, session_id, answer } = event else {
        panic!("wrong event")
    };
    assert_eq!(event_id.as_str(), id);
    assert_eq!(session_id, session);
    assert_eq!(answer.get("q0"), Some(&json!("A")));
    let (_, settled) = send(&app, Some(auth.token()), "GET", &detail_path, Value::Null).await;
    assert_eq!(settled["data"]["state"]["status"], json!("answered"));
    // A second reply conflicts and publishes nothing new.
    assert_eq!(
        send(&app, Some(auth.token()), "POST", &reply_path, json!({"answer": {"q0": "B"}}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert!(replied.try_recv().is_err());
    // Settled forms drop out of the list.
    let (_, listed) = send(&app, Some(auth.token()), "GET", &list_path, Value::Null).await;
    assert!(listed["data"].as_array().unwrap().is_empty());

    // A fresh form can be cancelled with a message; `form.cancelled` publishes.
    let (_, body) = send(&app, Some(auth.token()), "POST", &list_path, form_body()).await;
    let cancel_id = body["data"]["id"].as_str().unwrap().to_owned();
    let cancel_path = format!("/api/session/{session}/form/{cancel_id}?message=nobody");
    assert_eq!(
        send(&app, Some(auth.token()), "DELETE", &cancel_path, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    let event = cancelled.try_recv().expect("form.cancelled published");
    let ServerEvent::FormCancelled { id: event_id, session_id } = event else {
        panic!("wrong event")
    };
    assert_eq!(event_id.as_str(), cancel_id);
    assert_eq!(session_id, session);
    let (_, cancelled_detail) =
        send(&app, Some(auth.token()), "GET", &format!("/api/session/{session}/form/{cancel_id}"), Value::Null).await;
    assert_eq!(cancelled_detail["data"]["state"]["status"], json!("cancelled"));
    assert_eq!(cancelled_detail["data"]["state"]["message"], json!("nobody"));

    // Unknown form id is a 404, not a silent success.
    assert_eq!(
        send(&app, Some(auth.token()), "GET", &format!("/api/session/{session}/form/frm_missing"), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

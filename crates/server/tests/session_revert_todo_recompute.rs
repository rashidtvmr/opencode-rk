//! SIDE-10: staging a revert recomputes the effective todo list from typed tool
//! history (latest completed `todowrite` before the boundary) and persists it,
//! so a subsequent GET reflects the reverted state, not the stored verbatim row.
use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, Timestamp};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths, ToolCallAdmission, ToolRound};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn send(app: &axum::Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Admit one completed `todowrite` round and return the display message id,
/// which is the row a revert boundary must point at to exclude this call.
async fn append_todowrite(
    sessions: &SessionService,
    session: opencode_rk_contracts::SessionId,
    turn: MessageId,
    ordinal: u32,
    arguments: &str,
) -> MessageId {
    let display = MessageId::new();
    let round = ToolRound {
        round_id: format!("round-{turn}-{ordinal}"),
        session_id: session,
        turn_message_id: turn,
        round_ordinal: ordinal,
        expected_pairs: 1,
    };
    sessions
        .admit_tool_round(
            round.clone(),
            vec![ToolCallAdmission {
                call_id: format!("call-{ordinal}"),
                name: "todowrite".to_owned(),
                arguments: arguments.to_owned(),
                message: MessageRecord {
                    id: display,
                    session_id: session,
                    role: MessageRole::Tool,
                    body: PayloadRef::inline("[todowrite] pending").unwrap(),
                    created_at: Timestamp::now(),
                },
            }],
        )
        .await
        .unwrap();
    sessions
        .persist_tool_output(round, 0, "ok".to_owned(), display)
        .await
        .unwrap();
    display
}

#[tokio::test]
async fn staged_revert_recomputes_and_persists_derived_todos() {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open(StoragePaths::under(root.path())).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    let (_, body) = send(&app, "POST", "/api/sessions", json!({"title":"t"})).await;
    let id = body["session"]["id"].as_str().unwrap().to_owned();
    let session = id.parse::<opencode_rk_contracts::SessionId>().unwrap();
    let turn = sessions
        .append_text(session, MessageRole::User, "work".to_owned())
        .await
        .unwrap();

    // Two completed todowrite calls. The revert boundary will sit at the second.
    let first = r#"{"todos":[{"id":"a","content":"First","status":"pending","priority":"high"}]}"#;
    let second = r#"{"todos":[{"id":"b","content":"Second","status":"in_progress","priority":"low"}]}"#;
    append_todowrite(&sessions, session, turn.id, 0, first).await;
    let boundary = append_todowrite(&sessions, session, turn.id, 1, second).await;

    // Pre-seed the stored table with the post-revert list; the recompute must
    // overwrite it with the boundary-derived list ("a").
    let path = format!("/api/sessions/{id}/todo");
    let stored = json!({"todos":[{"id":"b","content":"Second","status":"in_progress","priority":"low"}]});
    assert_eq!(
        send(&app, "PUT", &path, stored).await.0,
        StatusCode::OK
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/sessions/{id}/revert/stage"),
        json!({"messageID": boundary.to_string()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, derived) = send(&app, "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(derived["todos"][0]["id"], json!("a"));
    assert_eq!(derived["todos"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn todo_get_uses_latest_completed_tool_input_over_stale_storage() {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open(StoragePaths::under(root.path())).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let (_, body) = send(&app, "POST", "/api/sessions", json!({"title":"t"})).await;
    let id = body["session"]["id"].as_str().unwrap().to_owned();
    let session = id.parse::<opencode_rk_contracts::SessionId>().unwrap();
    let turn = sessions
        .append_text(session, MessageRole::User, "work".to_owned())
        .await
        .unwrap();
    append_todowrite(
        &sessions,
        session,
        turn.id,
        0,
        r#"{"todos":[{"id":"derived","content":"Latest tool state","status":"in_progress","priority":"high"}]}"#,
    )
    .await;
    let path = format!("/api/sessions/{id}/todo");
    assert_eq!(
        send(
            &app,
            "PUT",
            &path,
            json!({"todos":[{"id":"stale","content":"Stale stored state","status":"pending","priority":"low"}]}),
        )
        .await
        .0,
        StatusCode::OK
    );

    let (status, body) = send(&app, "GET", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["todos"][0]["id"], json!("derived"));
}

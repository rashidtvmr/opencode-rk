//! Public-route contracts for ordered, durable, session-scoped todo replacement.
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

fn parsed_todos(value: &Value) -> Vec<opencode_rk_storage::session_todos::Todo> {
    serde_json::from_value(value["todos"].clone()).unwrap()
}

/// Admit one completed `todowrite` round so GET /todo has a derived baseline
/// (latest completed `todowrite` input), mirroring `session_revert_todo_recompute`.
async fn append_todowrite(
    sessions: &SessionService,
    session: opencode_rk_contracts::SessionId,
    turn: MessageId,
    ordinal: u32,
    arguments: &str,
) {
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
                    files: Vec::new(),
                    skills: Vec::new(),
                },
            }],
        )
        .await
        .unwrap();
    sessions
        .persist_tool_output(round, 0, "ok".to_owned(), display)
        .await
        .unwrap();
}

#[tokio::test]
async fn session_todo_replacement_preserves_order_isolation_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open(StoragePaths::under(root.path())).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app1 = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let first = session(&app1).await;
    let other = session(&app1).await;
    let path = format!("/api/sessions/{first}/todo");
    let expected = todos();
    // GET /todo derives from the latest completed `todowrite` input, so seed
    // one whose input equals the replacement; the PUT then stores verbatim rows
    // while GET stays on the derived baseline.
    let sid = first.parse::<opencode_rk_contracts::SessionId>().unwrap();
    let turn = sessions
        .append_text(sid, MessageRole::User, "work".to_owned())
        .await
        .unwrap();
    append_todowrite(&sessions, sid, turn.id, 0, &expected.to_string()).await;
    let (status, body) = request(&app1, "PUT", &path, expected.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, expected);
    // The replacement landed verbatim and in order in the stored rows (the
    // layer PUT actually writes), even though GET derives from tool history.
    assert_eq!(sessions.todos(sid).await.unwrap(), parsed_todos(&expected));
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
    let reopened = router(AppState {
        sessions: SessionService::new(Arc::new(
            Storage::open(StoragePaths::under(root.path())).unwrap(),
        )),
        catalog: Arc::new(Catalog::default()),
    });
    // Restart: the derived list survives because the completed `todowrite`
    // history is durable.
    assert_eq!(
        request(&reopened, "GET", &path, Value::Null).await,
        (StatusCode::OK, expected.clone())
    );
    assert_eq!(
        request(&reopened, "PUT", &path, json!({"todos":[]})).await,
        (StatusCode::OK, json!({"todos":[]}))
    );
    // The empty replacement is stored (raw view), but GET still derives the
    // latest completed `todowrite` input, per the upstream contract.
    assert!(sessions.todos(sid).await.unwrap().is_empty());
    assert_eq!(
        request(&reopened, "GET", &path, Value::Null).await,
        (StatusCode::OK, expected)
    );
}

#[tokio::test]
async fn session_todo_invalid_replacement_does_not_delete_existing_items() {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open(StoragePaths::under(root.path())).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let id = session(&app).await;
    let path = format!("/api/sessions/{id}/todo");
    let original = todos();
    // Seed a completed `todowrite` so the derived GET baseline is `original`;
    // the invalid-replacement promise holds at the storage layer PUT writes
    // (rejected payloads never touch the stored rows) and on the derived view.
    let sid = id.parse::<opencode_rk_contracts::SessionId>().unwrap();
    let turn = sessions
        .append_text(sid, MessageRole::User, "work".to_owned())
        .await
        .unwrap();
    append_todowrite(&sessions, sid, turn.id, 0, &original.to_string()).await;
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
        // Stored rows are untouched by the rejected PUT...
        assert_eq!(sessions.todos(sid).await.unwrap(), parsed_todos(&original));
        // ...and the derived GET baseline is unchanged too.
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

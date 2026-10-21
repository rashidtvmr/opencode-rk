use std::sync::Arc;
use axum::{body::Body, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn retry_guard_sees_admissions_after_display_history_first_page() {
    use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, Timestamp};
    use opencode_rk_sessions::{ToolRound, ToolCallAdmission};
    let dir = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let session = sessions.create("later tool admission").await.unwrap();
    sessions.append_text(session.id,MessageRole::User,"earlier turn").await.unwrap();
    let turn = sessions.append_text(session.id,MessageRole::User,"later turn").await.unwrap();
    let round = ToolRound { round_id:"later-round".into(),session_id:session.id,turn_message_id:turn.id,round_ordinal:0,expected_pairs:1 };
    sessions.admit_tool_round(round,vec![ToolCallAdmission { call_id:"later-call".into(),name:"read".into(),arguments:"{\"path\":\"probe.txt\"}".into(),
        message:MessageRecord { id:MessageId::new(),session_id:session.id,role:MessageRole::Tool,body:PayloadRef::inline("pending").unwrap(),created_at:Timestamp::now() } }]).await.unwrap();
    assert!(sessions.display_tool_history(session.id,1).await.unwrap().is_empty(),"display pagination must not be used as a replay authority");
    assert!(sessions.has_tool_admissions(session.id).await.unwrap(),"pending later-turn admissions must forbid blind retry");
}

#[tokio::test]
async fn child_controls_validate_parent_and_never_invent_a_running_child() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent", Some(dir.path().into())).await.unwrap();
    let foreign = sessions.create("unrelated").await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(Request::builder().uri(format!("/api/sessions/{}/children", parent.id)).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(value["children"], json!([]));
    for operation in ["stop", "retry", "continue"] {
        let response = app.clone().oneshot(Request::builder().method("POST")
            .uri(format!("/api/sessions/{}/children/{}/{operation}", parent.id, foreign.id))
            .header("content-type", "application/json")
            .body(Body::from(json!({"text":"must never run"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{operation}");
    }
    assert!(sessions.messages(foreign.id, 100).await.unwrap().is_empty());
}

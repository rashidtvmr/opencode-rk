//! 004-B: context selection is an HTTP-visible projection, not destructive history.
#![forbid(unsafe_code)]

use std::{env, sync::Arc, time::Duration};

use axum::{body::Body, extract::State, http::Request, routing::post, Json, Router};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{
    MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, Timestamp,
};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::{HistoryItem, SessionService, ToolCallAdmission, ToolRound};
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn new(values: &[(&'static str, String)]) -> Self {
        let mut old = Vec::new();
        for (key, value) in values {
            old.push((*key, env::var_os(key)));
            env::set_var(key, value);
        }
        Self(old)
    }
    fn clear(&mut self, key: &'static str) {
        self.0.push((key, env::var_os(key)));
        env::remove_var(key);
    }
    fn set(&mut self, key: &'static str, value: &str) {
        self.0.push((key, env::var_os(key)));
        env::set_var(key, value);
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..).rev() {
            match value {
                Some(v) => env::set_var(key, v),
                None => env::remove_var(key),
            }
        }
    }
}

struct ProviderTask(tokio::task::JoinHandle<()>);
impl Drop for ProviderTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn provider(State(tx): State<mpsc::Sender<Value>>, Json(body): Json<Value>) -> Json<Value> {
    let is_summary = body["input"][0]["content"]
        .as_str()
        .is_some_and(|content| content.contains("Summarize the older conversation"));
    tx.send(body).await.expect("provider channel");
    Json(
        json!({"id":"fixture-response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text": if is_summary { "SUMMARY" } else { "FIXTURE_REPLY" }}]}]}),
    )
}

fn input_has_exact(input: &[Value], content: &str) -> bool {
    input
        .iter()
        .any(|item| item["content"].as_str() == Some(content))
}

fn record(session_id: SessionId, role: MessageRole, text: &str) -> MessageRecord {
    MessageRecord {
        id: MessageId::new(),
        session_id,
        role,
        body: PayloadRef::inline(text).unwrap(),
        created_at: Timestamp::now(),
    }
}

async fn fixture(
    root: &std::path::Path,
) -> (
    SessionService,
    Router,
    mpsc::Receiver<Value>,
    ProviderTask,
    EnvGuard,
) {
    let (tx, rx) = mpsc::channel(8);
    let provider = Router::new()
        .route("/v1/responses", post(provider))
        .with_state(tx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = ProviderTask(tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    }));
    let mut env = EnvGuard::new(&[
        ("HOME", root.display().to_string()),
        ("OPENCODE_RK_HOME", root.join("home").display().to_string()),
        (
            "OPENCODE_RK_CONFIG_DIR",
            root.join("config").display().to_string(),
        ),
        ("OPENAI_BASE_URL", format!("http://{address}/v1")),
        ("OPENAI_API_KEY", "disposable-context-key".into()),
    ]);
    for key in [
        "OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS",
        "OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS",
        "OPENCODE_RK_COMPACTION__STRATEGY",
    ] {
        env.clear(key);
    }
    env.clear("OPENCODE_RK_COMPACTION__ENABLED");
    env::set_var("OPENCODE_RK_COMPACTION__ENABLED", "false");
    let sessions = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.join("state"))).unwrap(),
    ));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    (sessions, app, rx, task, env)
}

async fn next(rx: &mut mpsc::Receiver<Value>) -> Value {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn selection_excludes_http_items_but_preserves_all_durable_messages() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "CURRENT_INSTRUCTIONS").unwrap();
    let (sessions, app, mut rx, _task, _env) = fixture(root.path()).await;
    let session = sessions
        .create_in_directory("selection", Some(root.path().into()))
        .await
        .unwrap();
    let mut ids = Vec::new();
    for i in 0..102 {
        ids.push(
            sessions
                .append_text(
                    session.id,
                    if i % 2 == 0 {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    },
                    format!("DROP{i} item_id={i}"),
                )
                .await
                .unwrap()
                .id,
        );
    }
    for id in ids.iter().take(4) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/sessions/{}/context/exclude", session.id))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"item_id":id.to_string()}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response.status().is_success());
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"LAST_NEW_TURN"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    let request = next(&mut rx).await;
    let input = request["input"].as_array().unwrap();
    for marker in [
        "DROP0 item_id=0",
        "DROP1 item_id=1",
        "DROP2 item_id=2",
        "DROP3 item_id=3",
    ] {
        assert!(
            !input_has_exact(input, marker),
            "excluded marker leaked: {marker}"
        );
    }
    assert!(input_has_exact(input, "DROP101 item_id=101"));
    assert!(input_has_exact(input, "LAST_NEW_TURN"));
    assert!(input.iter().any(|item| item["role"] == "system"
        && item["content"]
            .as_str()
            .is_some_and(|s| s.contains("CURRENT_INSTRUCTIONS"))));
    assert_eq!(sessions.messages(session.id, 200).await.unwrap().len(), 104);
}

#[tokio::test]
async fn excluded_display_row_does_not_drop_completed_typed_tool_pair() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let (sessions, app, mut rx, _task, _env) = fixture(root.path()).await;
    let session = sessions.create("typed projection").await.unwrap();
    let user = sessions
        .append_text(session.id, MessageRole::User, "typed round")
        .await
        .unwrap();
    let mut admissions = Vec::new();
    let mut displays = Vec::new();
    for pair in 0..2u64 {
        let display = record(
            session.id,
            MessageRole::Tool,
            &format!("display pair {pair}"),
        );
        admissions.push(ToolCallAdmission {
            call_id: format!("call-{pair}"),
            name: "read".into(),
            arguments: format!(r#"{{"path":"p{pair}"}}"#),
            message: display.clone(),
        });
        displays.push(display);
    }
    let round = ToolRound {
        round_id: format!("round-{}", user.id),
        session_id: session.id,
        turn_message_id: user.id,
        round_ordinal: 0,
        expected_pairs: 2,
    };
    sessions
        .admit_tool_round(round.clone(), admissions)
        .await
        .unwrap();
    for (pair, display) in displays.iter().enumerate() {
        sessions
            .persist_tool_output(
                round.clone(),
                pair as u64,
                format!("output-{pair}"),
                display.id,
            )
            .await
            .unwrap();
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"item_id":displays[0].id.to_string()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"typed continuation"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    let request = next(&mut rx).await;
    let text = request.to_string();
    assert!(!text.contains("call-0"));
    assert!(!text.contains("output-0"));
    assert!(text.contains("call-1"));
    assert!(text.contains("output-1"));
    drop(app);
    let reopened = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.path().join("state"))).unwrap(),
    ));
    let app = router(AppState {
        sessions: reopened.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let restarted = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"after restart"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(restarted.status(), axum::http::StatusCode::CREATED);
    let restarted_request = next(&mut rx).await;
    assert!(
        restarted_request.to_string().contains("call-1")
            && restarted_request.to_string().contains("output-1")
    );
    let history = sessions
        .bounded_history(session.id, 200, 256 * 1024)
        .await
        .unwrap();
    assert!(
        history
            .iter()
            .filter(|item| matches!(item, HistoryItem::Tool(_)))
            .count()
            >= 4
    );
}

#[tokio::test]
async fn fallback_summary_excludes_old_history_and_next_turn_keeps_tail() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let (sessions, app, mut rx, _task, mut env) = fixture(root.path()).await;
    let session = sessions.create("fallback").await.unwrap();
    env.set("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1");
    let mut old_ids = Vec::new();
    for i in 0..4 {
        old_ids.push(
            sessions
                .append_text(
                    session.id,
                    MessageRole::User,
                    format!("OLD_MARKER_{i}_USER"),
                )
                .await
                .unwrap()
                .id,
        );
        old_ids.push(
            sessions
                .append_text(
                    session.id,
                    MessageRole::Assistant,
                    format!("OLD_MARKER_{i}_ASSISTANT"),
                )
                .await
                .unwrap()
                .id,
        );
    }
    sessions
        .append_text(session.id, MessageRole::User, "RECENT_TAIL")
        .await
        .unwrap();
    for id in old_ids[0..2].iter() {
        let excluded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/sessions/{}/context/exclude", session.id))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"item_id": id.to_string()}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(excluded.status(), axum::http::StatusCode::OK);
    }
    sessions
        .append_text(session.id, MessageRole::Assistant, "RECENT_REPLY")
        .await
        .unwrap();
    let compact = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/compact", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","strategy":"fallback"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact.status(), axum::http::StatusCode::OK);
    let summary_request = next(&mut rx).await;
    let summary_input = summary_request["input"].as_array().unwrap();
    assert!(!summary_request.to_string().contains("OLD_MARKER_0_USER"));
    assert!(!summary_request
        .to_string()
        .contains("OLD_MARKER_0_ASSISTANT"));
    assert!(summary_request.to_string().contains("OLD_MARKER_2_USER"));
    assert!(summary_input[0]["content"]
        .as_str()
        .unwrap()
        .contains("OLD_MARKER_2"));
    let turn = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"AFTER_SUMMARY"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), axum::http::StatusCode::CREATED);
    let request = next(&mut rx).await;
    let text = request.to_string();
    assert!(
        text.contains("AFTER_SUMMARY") && text.contains("RECENT_TAIL") && text.contains("SUMMARY")
    );
    let next_input = request["input"].as_array().unwrap();
    assert!(!input_has_exact(next_input, "OLD_MARKER_0_USER"));
    assert!(!input_has_exact(next_input, "OLD_MARKER_0_ASSISTANT"));
    assert!(!input_has_exact(next_input, "OLD_MARKER_1_USER"));
    assert!(!input_has_exact(next_input, "OLD_MARKER_1_ASSISTANT"));
    assert_eq!(sessions.messages(session.id, 100).await.unwrap().len(), 12);
}

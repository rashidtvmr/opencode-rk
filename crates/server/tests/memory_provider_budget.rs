//! Automatic native compaction must preserve durable history while bounding provider input.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::post,
    Json,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{env, sync::Arc, time::Duration};
use tower::ServiceExt;

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Environment {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(
            values
                .iter()
                .map(|(key, value)| {
                    let old = env::var_os(key);
                    env::set_var(key, value);
                    (*key, old)
                })
                .collect(),
        )
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, old) in &self.0 {
            if let Some(old) = old {
                env::set_var(key, old);
            } else {
                env::remove_var(key);
            }
        }
    }
}
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn instruction_item_is_reserved_before_bounding_full_history() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "BUDGET_INSTRUCTION_SENTINEL").unwrap();
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let summary = sessions
        .create_in_directory("Budget", Some(root.path().to_owned()))
        .await
        .unwrap();
    for index in 0..100 {
        sessions
            .append_text(
                summary.id,
                if index % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                format!("Earlier message {index}"),
            )
            .await
            .unwrap();
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(4);
    let compact_tx = tx.clone();
    let response_tx = tx.clone();
    let provider = axum::Router::new()
        .route("/responses/compact", post(move |Json(body): Json<Value>| {
            let tx = compact_tx.clone();
            async move {
                tx.send(json!({"endpoint":"compact","body":body})).await.unwrap();
                Json(json!({
                    "id":"resp_budget_compaction",
                    "object":"response.compaction",
                    "created_at":1764967971,
                    "usage":{"input_tokens":100,"output_tokens":1},
                    "output":[
                        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"retained summary"}],"future_message_metadata":{"fixture":true}},
                        {"type":"compaction","id":"cmp_budget","encrypted_content":"opaque-budget-compaction","future_compaction_metadata":{"fixture":true}}
                    ]
                }))
            }
        }))
        .route("/responses", post(move |Json(body): Json<Value>| {
            let tx = response_tx.clone(); async move {
                tx.send(json!({"endpoint":"responses","body":body})).await.unwrap();
                Json(json!({"id":"resp_budget","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Budget answer"}]}]}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _task = Task(tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    }));
    let _env = Environment::set(&[
        ("OPENAI_BASE_URL", format!("http://{address}")),
        ("OPENAI_API_KEY", "budget-fixture-key".into()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "true".into()),
        ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "16".into()),
        ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "2".into()),
        ("OPENCODE_RK_COMPACTION__STRATEGY", "native".into()),
    ]);
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", summary.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"Newest request"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "instructions must not make an otherwise bounded history invalid: {}",
        String::from_utf8_lossy(&bytes)
    );
    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first["endpoint"], "compact");
    let compact_input = first["body"]["input"].as_array().unwrap();
    assert_eq!(compact_input.len(), 100, "native compaction must receive all old messages");
    for (index, item) in compact_input.iter().enumerate() {
        assert_eq!(item["content"], format!("Earlier message {index}"));
    }
    assert!(compact_input.iter().all(|item| item["content"] != "Newest request"));
    assert!(compact_input.iter().all(|item| item["role"] != "system"));

    let final_request = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_request["endpoint"], "responses");
    let input = final_request["body"]["input"].as_array().unwrap();
    assert!(input.len() <= opencode_rk_providers::responses::MAX_RESPONSES_INPUT_MESSAGES);
    assert!(input.iter().any(|item| item["role"] == "system"
        && item["content"]
            .as_str()
            .is_some_and(|text| text.contains("BUDGET_INSTRUCTION_SENTINEL"))));
    assert!(input.iter().any(|item| item["content"] == "Newest request"));
    assert!(input.iter().any(|item| item["type"] == "compaction"
        && item["encrypted_content"] == "opaque-budget-compaction"));
    assert!(input.iter().any(|item| {
        item["future_message_metadata"]["fixture"] == true
            && item["content"][0]["text"] == "retained summary"
    }));
    assert!(!input.iter().any(|item| item["content"] == "Earlier message 0"));
    assert!(input.iter().any(|item| {
        item["type"] == "compaction"
            && item["future_compaction_metadata"]["fixture"] == true
            && item["encrypted_content"] == "opaque-budget-compaction"
    }));
    assert_eq!(
        sessions.messages(summary.id, 200).await.unwrap().len(),
        102,
        "request bounding must preserve durable history"
    );

    let durable_after_first = sessions.messages(summary.id, 200).await.unwrap();
    let first_assistant = durable_after_first.last().unwrap();
    let first_assistant_id = first_assistant.id;
    assert_eq!(first_assistant_id, durable_after_first[101].id);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", summary.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"Second newest request"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_status = second.status();
    let second_bytes = to_bytes(second.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(
        second_status,
        StatusCode::CREATED,
        "native checkpoint must remain usable on the next turn: {}",
        String::from_utf8_lossy(&second_bytes)
    );

    let second_compact = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_compact["endpoint"], "compact");
    let second_compact_input = second_compact["body"]["input"].as_array().unwrap();
    assert_eq!(second_compact_input.len(), 4);
    let prior_canonical: Vec<&Value> = input
        .iter()
        .filter(|item| item["type"] == "compaction" || item["future_message_metadata"]["fixture"] == true)
        .collect();
    assert_eq!(prior_canonical.len(), 2);
    assert_eq!(second_compact_input[0], *prior_canonical[0]);
    assert_eq!(second_compact_input[1], *prior_canonical[1]);
    assert_eq!(second_compact_input[2]["content"], "Newest request");
    assert_eq!(second_compact_input[3]["content"], "Budget answer");
    assert_eq!(
        second_compact_input
            .iter()
            .filter(|item| item["content"] == "Newest request")
            .count(),
        1
    );
    assert!(second_compact_input
        .iter()
        .all(|item| item["content"] != "Second newest request"));
    assert!(second_compact_input.iter().all(|item| item["content"] != "Earlier message 0"));
    assert!(second_compact_input.iter().all(|item| item["content"] != "BUDGET_INSTRUCTION_SENTINEL"));

    let second_response = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_response["endpoint"], "responses");
    let second_input = second_response["body"]["input"].as_array().unwrap();
    assert!(second_input.iter().any(|item| item["content"] == "Second newest request"));
    assert!(second_input.iter().any(|item| item["role"] == "system"
        && item["content"].as_str().is_some_and(|text| text.contains("BUDGET_INSTRUCTION_SENTINEL"))));
    assert!(!second_input.iter().any(|item| item["content"] == "Newest request"));
    assert!(!second_input.iter().any(|item| item["content"] == "Budget answer"));

    let durable_after_second = sessions.messages(summary.id, 200).await.unwrap();
    assert_eq!(durable_after_second.len(), 104);
    assert_eq!(durable_after_second[101].id, first_assistant_id);
    let checkpoint = sessions.compaction_checkpoint(summary.id).await.unwrap().unwrap();
    assert_eq!(checkpoint.boundary_message_id, first_assistant_id);
    assert!(durable_after_second.iter().any(|message| {
        serde_json::to_string(&message.body)
            .unwrap()
            .contains("Second newest request")
    }));
}

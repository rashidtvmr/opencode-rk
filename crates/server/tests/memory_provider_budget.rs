//! Instructions and typed history must share the provider's request budget.
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
use std::{env, sync::Arc};
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
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(1);
    let provider = axum::Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let tx = tx.clone(); async move {
            tx.send(body).await.unwrap();
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
    ]);
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
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
    let captured = rx.recv().await.unwrap();
    let input = captured["input"].as_array().unwrap();
    assert!(input.len() <= opencode_rk_providers::responses::MAX_RESPONSES_INPUT_MESSAGES);
    assert!(input.iter().any(|item| item["role"] == "system"
        && item["content"]
            .as_str()
            .is_some_and(|text| text.contains("BUDGET_INSTRUCTION_SENTINEL"))));
    assert!(input.iter().any(|item| item["content"] == "Newest request"));
    assert_eq!(
        sessions.messages(summary.id, 200).await.unwrap().len(),
        102,
        "request bounding must preserve durable history"
    );
}

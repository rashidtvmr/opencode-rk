//! Instruction selection must affect the actual outbound provider request.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::post,
    Json,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{env, sync::Arc};
use tower::ServiceExt;

struct EnvGuard(&'static str, Option<String>);
impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = env::var(key).ok();
        env::set_var(key, value);
        Self(key, old)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.1 {
            env::set_var(self.0, value);
        } else {
            env::remove_var(self.0);
        }
    }
}
struct ServerTask(tokio::task::JoinHandle<()>);
impl Drop for ServerTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn memory_selection_changes_outbound_system_prompt_without_changing_sources_or_history() {
    let root = tempfile::tempdir().unwrap();
    let instruction = root.path().join("AGENTS.md");
    const SENTINEL: &str = "MEMORY_SOURCE_SENTINEL_A329C";
    std::fs::write(&instruction, SENTINEL).unwrap();
    let storage = Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap());
    let sessions = SessionService::new(storage);
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(3);
    let provider = axum::Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let tx = tx.clone();
        async move {
            tx.send(body).await.unwrap();
            Json(json!({"id":"resp_memory", "status":"completed", "output":[
                {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Fixture answer"}]}
            ]}))
        }
    })).layer(axum::extract::DefaultBodyLimit::max(512 * 1024));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _task = ServerTask(tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    }));
    let _base = EnvGuard::set("OPENAI_BASE_URL", &format!("http://{address}"));
    let _key = EnvGuard::set("OPENAI_API_KEY", "disposable-fixture-key");
    let (status, created) = send(
        &app,
        "POST",
        "/api/sessions",
        json!({"title":"Memory payload", "directory": root.path()}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["session"]["id"].as_str().unwrap();
    let turn_path = format!("/api/sessions/{id}/turns");
    for (index, included) in [true, false, true].into_iter().enumerate() {
        if index > 0 {
            let action = if included { "include" } else { "exclude" };
            assert_eq!(
                send(
                    &app,
                    "POST",
                    &format!("/api/sessions/{id}/memory/{action}"),
                    json!({"path":instruction})
                )
                .await
                .0,
                StatusCode::OK
            );
        }
        assert_eq!(
            send(
                &app,
                "POST",
                &turn_path,
                json!({"model":"openai/fixture-model","text":format!("User turn {index}")})
            )
            .await
            .0,
            StatusCode::CREATED
        );
        let captured = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let system = captured["input"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["role"] == "system")
            .map(Value::to_string)
            .collect::<String>();
        assert_eq!(
            system.contains(SENTINEL),
            included,
            "instruction selection must change the actual system input, round {index}"
        );
        assert_eq!(std::fs::read_to_string(&instruction).unwrap(), SENTINEL);
    }
    let session_id = id.parse().unwrap();
    let history = sessions.messages(session_id, 100).await.unwrap();
    assert_eq!(
        history.len(),
        6,
        "all three user/assistant turns remain durable"
    );
}

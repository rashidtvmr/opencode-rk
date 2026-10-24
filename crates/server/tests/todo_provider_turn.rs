//! A model-issued todo update must persist and return through the typed tool loop.
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, routing::post, Json};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{env, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use tower::ServiceExt;

struct EnvGuard(&'static str, Option<String>);
impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value);
        Self(key, previous)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.1 { env::set_var(self.0, value); }
        else { env::remove_var(self.0); }
    }
}
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task { fn drop(&mut self) { self.0.abort(); } }

async fn request(app: &axum::Router, method: &str, path: &str, body: Value) -> (StatusCode, Vec<u8>) {
    let response = app.clone().oneshot(Request::builder().method(method).uri(path)
        .header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = tokio::time::timeout(Duration::from_secs(10), to_bytes(response.into_body(), 256 * 1024)).await.unwrap().unwrap();
    (status, bytes.to_vec())
}

#[tokio::test]
async fn model_todo_write_is_persisted_and_returned_to_provider_with_policy_denial_having_no_effect() {
    let root = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap()));
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let count = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(4);
    let todos = json!([{"id":"first","content":"Verify persisted tasks","status":"in_progress","priority":"high"}]);
    let expected = todos.clone();
    let provider = axum::Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let tx = tx.clone(); let count = count.clone(); let todos = todos.clone();
        async move {
            tx.send(body).await.unwrap();
            let event = if count.fetch_add(1, Ordering::SeqCst) % 2 == 0 {
                json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_todo","call_id":"call_todo","name":"todowrite","arguments":json!({"todos":todos}).to_string()}})
            } else {
                json!({"type":"response.output_text.delta","delta":"Tasks updated"})
            };
            let completed = json!({"type":"response.completed","response":{"id":"resp_todo","status":"completed"}});
            ([("content-type", "text/event-stream")], format!("data: {event}\n\ndata: {completed}\n\n"))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _task = Task(tokio::spawn(async move { axum::serve(listener, provider).await.unwrap(); }));
    let _key = EnvGuard::set("OPENAI_API_KEY", "todo-fixture-key");
    let _base = EnvGuard::set("OPENAI_BASE_URL", &format!("http://{address}"));
    for permitted in [true, false] {
        let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", if permitted { "todowrite" } else { "" });
        let (status, created) = request(&app, "POST", "/api/sessions", json!({"title":"Todo turn","directory":root.path()})).await;
        assert_eq!(status, StatusCode::CREATED);
        let created: Value = serde_json::from_slice(&created).unwrap();
        let id = created["session"]["id"].as_str().unwrap();
        let (status, stream) = request(&app, "POST", &format!("/api/sessions/{id}/turns/stream"), json!({"model":"openai/fixture","text":"Track the task"})).await;
        assert!(status.is_success(), "{}", String::from_utf8_lossy(&stream));
        let first = rx.recv().await.unwrap();
        let second = rx.recv().await.unwrap();
        let advertised = first["tools"].as_array().is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "todowrite"));
        assert_eq!(advertised, permitted, "actual provider definitions must follow turn policy");
        let persisted = sessions.todos(id.parse().unwrap()).await.unwrap();
        if permitted {
            assert_eq!(serde_json::to_value(&persisted).unwrap(), expected);
            assert!(second["input"].as_array().unwrap().iter().any(|item| item["type"] == "function_call_output" && item["call_id"] == "call_todo" && item["output"].as_str().is_some_and(|output| output.contains("Verify persisted tasks"))));
        } else {
            assert!(persisted.is_empty(), "unadvertised model calls cannot mutate todo state");
        }
    }
}

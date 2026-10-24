//! 004-D: native compaction is a durable checkpoint for a later turn.
//!
//! This deliberately uses a real HTTP Responses fixture.  The endpoint is
//! expected to be RED until the server wires the already-implemented native
//! provider transport into sessions.

use std::{env, sync::{atomic::{AtomicUsize, Ordering}, Arc}, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        let old = values
            .iter()
            .map(|(key, value)| {
                let previous = env::var_os(key);
                env::set_var(key, value);
                (*key, previous)
            })
            .collect();
        Self(old)
    }
}

#[tokio::test]
async fn native_compaction_streaming_turn_uses_checkpoint_and_pairs_todowrite_continuation() {
    let _environment_lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "NATIVE_STREAM_AGENTS_SENTINEL").unwrap();
    let compacted = json!({"type":"compaction","id":"cmp_stream_1","created_at":1735000010,"encrypted_content":"opaque-stream-encrypted-marker","unknown_nested_metadata":{"stream":"preserve-exactly"}});
    let retained = json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"stream retained context"}],"unknown_metadata":{"stream_fixture":true}});
    let todos = json!([{"id":"stream-todo","content":"Preserve streamed checkpoint","status":"in_progress","priority":"high"}]);
    let (tx, mut rx) = mpsc::channel::<Value>(4);
    let calls = Arc::new(AtomicUsize::new(0));
    let c_tx = tx.clone(); let c_item = compacted.clone(); let r_item = retained.clone();
    let s_tx = tx.clone(); let s_calls = calls.clone();
    let provider = Router::new()
        .route("/responses/compact", post(move |Json(body): Json<Value>| {
            let tx = c_tx.clone(); let c = c_item.clone(); let r = r_item.clone();
            async move {
                tx.send(body).await.unwrap();
                Json(json!({"id":"resp_stream_compact","object":"response.compaction","created_at":1735000011,"output":[r,c]}))
            }
        }))
        .route("/responses", post(move |Json(body): Json<Value>| {
            let tx = s_tx.clone(); let calls = s_calls.clone(); let todos = todos.clone();
            async move {
                tx.send(body).await.unwrap();
                let event = if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_stream_todo","call_id":"call_stream_todo","name":"todowrite","arguments":json!({"todos":todos}).to_string()}})
                } else {
                    json!({"type":"response.output_text.delta","delta":"Stream continued after checkpoint"})
                };
                let done = json!({"type":"response.completed","response":{"id":"resp_stream_turn","status":"completed"}});
                ([("content-type", "text/event-stream")], format!("data: {event}\n\ndata: {done}\n\n"))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let address = listener.local_addr().unwrap();
    let _task = Task(tokio::spawn(async move { axum::serve(listener, provider).await.unwrap(); }));
    let _env = EnvGuard::set(&[("OPENAI_BASE_URL",format!("http://{address}")),("OPENAI_API_KEY","disposable-stream-key".into()),("OPENCODE_RK_HOME",root.path().to_string_lossy().into_owned()),("OPENCODE_RK_TURN_TOOLS","todowrite".into()),("OPENCODE_RK_PERMISSION_RULES",root.path().join("no-rules.json").to_string_lossy().into_owned())]);
    let paths = StoragePaths::under(root.path().join("storage")); let sessions = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let session = sessions.create_in_directory("native streaming compaction", Some(root.path().to_owned())).await.unwrap();
    for (role,text) in [(MessageRole::User,"stream old user one"),(MessageRole::Assistant,"stream old assistant one"),(MessageRole::User,"stream old user two"),(MessageRole::Assistant,"stream old assistant two")] { sessions.append_text(session.id,role,text).await.unwrap(); }
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let compact = app.clone().oneshot(Request::builder().method("POST").uri(format!("/api/sessions/{}/compact",session.id)).header("content-type","application/json").body(Body::from(json!({"model":"openai/fixture","strategy":"native"}).to_string())).unwrap()).await.unwrap(); assert_eq!(compact.status(),StatusCode::OK); rx.recv().await.unwrap();
    let stream = app.oneshot(Request::builder().method("POST").uri(format!("/api/sessions/{}/turns/stream",session.id)).header("content-type","application/json").body(Body::from(json!({"model":"openai/fixture","text":"stream new user"}).to_string())).unwrap()).await.unwrap(); assert!(stream.status().is_success()); let body = to_bytes(stream.into_body(),256*1024).await.unwrap(); let visible = String::from_utf8_lossy(&body); assert!(visible.contains("Stream continued after checkpoint")); assert!(!visible.contains("opaque-stream-encrypted-marker"));
    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    let second = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    for request in [&first,&second] { let input=request["input"].as_array().unwrap(); assert!(input.iter().any(|x|x==&retained)); assert!(input.iter().any(|x|x==&compacted)); assert!(input.iter().any(|x|x["content"]=="stream new user")); assert!(input.iter().any(|x|x["role"]=="system"&&x["content"].as_str().unwrap().contains("NATIVE_STREAM_AGENTS_SENTINEL"))); assert!(!input.iter().any(|x|x.to_string().contains("stream old user one"))); }
    let second_input=second["input"].as_array().unwrap(); assert!(second_input.iter().any(|x|x["type"]=="function_call"&&x["call_id"]=="call_stream_todo")); assert!(second_input.iter().any(|x|x["type"]=="function_call_output"&&x["call_id"]=="call_stream_todo")); assert_eq!(sessions.todos(session.id).await.unwrap().len(),1);
    let messages=sessions.messages(session.id,50).await.unwrap(); for text in ["stream old user one","stream old assistant one","stream old user two","stream old assistant two","stream new user","Stream continued after checkpoint"] { assert!(messages.iter().any(|m|serde_json::to_string(&m.body).unwrap().contains(text))); }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
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

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 128 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
async fn native_compaction_survives_restart_and_continues_without_duplicate_history() {
    let _environment_lock = ENV_LOCK.lock().await;
    let root = tempdir().expect("disposable fixture root");
    std::fs::write(root.path().join("AGENTS.md"), "NATIVE_COMPACTION_AGENTS_SENTINEL")
        .unwrap();
    let paths = StoragePaths::under(root.path().join("storage"));

    let compacted_item = json!({
        "type": "compaction",
        "id": "cmp_fixture_1",
        "created_at": 1_735_000_000,
        "encrypted_content": "opaque-encrypted-content-\u{0000}-byte-exact",
        "unknown_nested_metadata": {"preserve": [true, 7, "byte-exact"]}
    });
    let retained_item = json!({
        "type": "message",
        "role": "assistant",
        "content": [{"type": "output_text", "text": "retained canonical context"}],
        "unknown_metadata": {"fixture": "must-survive"}
    });
    let (request_tx, mut request_rx) = mpsc::channel::<Value>(4);
    let compacted_item_for_provider = compacted_item.clone();
    let retained_item_for_provider = retained_item.clone();
    let compact_request_tx = request_tx.clone();
    let continuation_request_tx = request_tx.clone();
    let provider = Router::new()
        .route(
            "/responses/compact",
            post(move |Json(body): Json<Value>| {
                let request_tx = compact_request_tx.clone();
                let compacted_item = compacted_item_for_provider.clone();
                let retained_item = retained_item_for_provider.clone();
                async move {
                    request_tx.send(body).await.unwrap();
                    Json(json!({
                        "id": "resp_compact_fixture",
                        "object": "response.compaction",
                        "created_at": 1_735_000_001,
                        "output": [retained_item, compacted_item]
                    }))
                }
            }),
        )
        .route(
            "/responses",
            post(move |Json(body): Json<Value>| {
                let request_tx = continuation_request_tx.clone();
                async move {
                    request_tx.send(body).await.unwrap();
                    Json(json!({
                        "id": "resp_continue_fixture",
                        "status": "completed",
                        "output": [{"type":"message","role":"assistant","content":[{"type":"output_text","text":"continued after native compaction"}]}]
                    }))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _provider_task = Task(tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    }));
    let _env = EnvGuard::set(&[
        ("OPENAI_BASE_URL", format!("http://{address}")),
        ("OPENAI_API_KEY", "disposable-fixture-key".to_owned()),
        ("OPENCODE_RK_HOME", root.path().to_string_lossy().into_owned()),
        (
            "OPENCODE_RK_PERMISSION_RULES",
            root.path().join("no-permission-rules.json").to_string_lossy().into_owned(),
        ),
    ]);

    let storage = Storage::open(paths.clone()).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let session = sessions
        .create_in_directory("native compaction", Some(root.path().to_owned()))
        .await
        .unwrap();
    let session_id = session.id;
    for (role, text) in [
        (MessageRole::User, "old user one"),
        (MessageRole::Assistant, "old assistant one"),
        (MessageRole::User, "old user two"),
        (MessageRole::Assistant, "old assistant two"),
    ] {
        sessions.append_text(session_id, role, text).await.unwrap();
    }

    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let compact = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/compact"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","strategy":"native"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact.status(), StatusCode::OK);
    let compact_body = to_bytes(compact.into_body(), 128 * 1024).await.unwrap();
    let compact_json: Value = serde_json::from_slice(&compact_body).unwrap();
    assert!(!compact_json.to_string().contains("opaque-encrypted-content"));
    assert!(compact_json.get("encrypted_content").is_none());
    let compact_request = tokio::time::timeout(Duration::from_secs(2), request_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(compact_request.to_string().contains("old user one"));
    assert!(compact_request.to_string().contains("old assistant two"));

    drop(app);
    let reopened = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let app = router(AppState {
        sessions: reopened.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/turns"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","text":"new user after restart"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    let visible_response = response_json(turn).await;
    assert_eq!(visible_response["assistant_message"]["body"]["text"], "continued after native compaction");
    assert!(!visible_response.to_string().contains("opaque-encrypted-content"));
    let continuation_request = tokio::time::timeout(Duration::from_secs(2), request_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let input = continuation_request["input"].as_array().unwrap();
    assert!(input.iter().any(|item| item == &retained_item));
    assert!(input.iter().any(|item| item == &compacted_item));
    let retained_position = input.iter().position(|item| item == &retained_item).unwrap();
    let compacted_position = input.iter().position(|item| item == &compacted_item).unwrap();
    let newest_position = input.iter().position(|item| item["content"] == "new user after restart").unwrap();
    assert!(retained_position < compacted_position && compacted_position < newest_position);
    assert!(input.iter().any(|item| item["content"] == "new user after restart"));
    assert!(input.iter().any(|item| item["role"] == "system" && item["content"].as_str().unwrap().contains("NATIVE_COMPACTION_AGENTS_SENTINEL")));
    assert!(!input.iter().any(|item| item.to_string().contains("old user one")));
    assert!(!input.iter().any(|item| item.to_string().contains("old assistant two")));

    let messages = reopened.messages(session_id, 50).await.unwrap();
    assert_eq!(messages.len(), 6);
    assert!(!serde_json::to_string(&messages).unwrap().contains("opaque-encrypted-content"));
    for text in ["old user one", "old assistant one", "old user two", "old assistant two", "new user after restart", "continued after native compaction"] {
        assert!(messages.iter().any(|message| serde_json::to_string(&message.body).unwrap().contains(text)), "missing durable text: {text}");
    }
}

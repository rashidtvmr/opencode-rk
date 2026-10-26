use std::{
    env,
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    (
        router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        }),
        dir,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            env::set_var(self.key, previous);
        } else {
            env::remove_var(self.key);
        }
    }
}

fn spawn_openai_fixture_with_usage(usage: Option<Value>) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");

    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected_len = None;

        loop {
            let read = stream.read(&mut buffer).expect("read provider request");
            assert!(
                read > 0,
                "provider request ended before the full body arrived"
            );
            request.extend_from_slice(&buffer[..read]);

            if expected_len.is_none() {
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(str::parse::<usize>)
                        })
                        .transpose()
                        .expect("valid content length")
                        .unwrap_or(0);
                    expected_len = Some(header_end + 4 + content_length);
                }
            }

            if expected_len.is_some_and(|len| request.len() >= len) {
                break;
            }
            assert!(
                request.len() <= 128 * 1024,
                "provider request exceeded fixture bound"
            );
        }

        let mut response = json!({
            "id": "resp_fixture",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture assistant reply"}]
            }]
        });
        if let Some(usage) = usage {
            response["usage"] = usage;
        }
        let response = response.to_string();
        let wire = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response.len(),
            response
        );
        stream
            .write_all(wire.as_bytes())
            .expect("write provider response");

        String::from_utf8(request).expect("provider request is utf-8")
    });

    (format!("http://{address}/v1"), task)
}

fn spawn_openai_fixture() -> (String, thread::JoinHandle<String>) {
    spawn_openai_fixture_with_usage(None)
}

#[tokio::test(flavor = "current_thread")]
async fn session_turn_executes_provider_and_persists_both_messages() {
    let (provider_base, provider_request) = spawn_openai_fixture();
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let (app, _dir) = app();

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"Turn integration"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await;
    let session_id = created["session"]["id"].as_str().expect("session id");

    let turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/turns"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "text": "Hello agent",
                        "model": "openai/gpt-5.6",
                        "reasoning_effort": "high"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    let turn_body = json_body(turn).await;
    assert_eq!(turn_body["user_message"]["role"], "user");
    assert_eq!(turn_body["user_message"]["body"]["text"], "Hello agent");
    assert_eq!(turn_body["assistant_message"]["role"], "assistant");
    assert_eq!(
        turn_body["assistant_message"]["body"]["text"],
        "Fixture assistant reply"
    );

    let request = provider_request.join().expect("provider fixture completed");
    assert!(request.starts_with("POST /v1/responses HTTP/1.1\r\n"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer fixture-secret\r\n"));
    let (_, provider_body) = request.split_once("\r\n\r\n").expect("request body");
    let provider_json: Value = serde_json::from_str(provider_body).expect("provider json");
    assert_eq!(provider_json["model"], "gpt-5.6");
    assert_eq!(provider_json["reasoning"]["effort"], "high");
    assert_eq!(provider_json["input"][0]["role"], "user");
    assert_eq!(provider_json["input"][0]["content"], "Hello agent");

    let messages = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(messages.status(), StatusCode::OK);
    let messages = json_body(messages).await;
    assert_eq!(messages["messages"].as_array().unwrap().len(), 2);
    assert_eq!(messages["messages"][0]["role"], "user");
    assert_eq!(messages["messages"][1]["role"], "assistant");
    assert_eq!(
        messages["messages"][1]["body"]["text"],
        "Fixture assistant reply"
    );
}


#[tokio::test(flavor = "current_thread")]
async fn nonstream_turn_publishes_provider_usage_memory_and_unknown_when_absent() {
    let root = tempdir().expect("memory fixture root");
    let instruction = root.path().join("AGENTS.md");
    std::fs::write(&instruction, "NONSTREAM_ACCOUNTING_MEMORY_SENTINEL").unwrap();
    let storage = Storage::open_in_memory(root.path().join("blobs")).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let session = sessions
        .create_in_directory("Non-stream accounting", Some(root.path().to_owned()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let (provider_base, provider_request) = spawn_openai_fixture_with_usage(Some(json!({
        "input_tokens": 120,
        "output_tokens": 30,
        "input_tokens_details": {"cached_tokens": 40},
        "output_tokens_details": {"reasoning_tokens": 10}
    })));
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let session_id = session.id.to_string();
    let turn = app.clone().oneshot(Request::builder()
        .method("POST")
        .uri(format!("/api/sessions/{session_id}/turns"))
        .header("content-type", "application/json")
        .body(Body::from(json!({"text":"account this", "model":"openai/gpt-5.6"}).to_string()))
        .unwrap()).await.unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    provider_request.join().unwrap();
    let context = app.clone().oneshot(Request::builder()
        .uri(format!("/api/sessions/{session_id}/context"))
        .body(Body::empty()).unwrap()).await.unwrap();
    let report = json_body(context).await["usage_report"].clone();
    assert_eq!(report["reported_used_tokens"], 120);
    assert_eq!(report["memory_files"].as_array().unwrap().len(), 1);
    assert_eq!(report["memory_files"][0], instruction.canonicalize().unwrap().to_string_lossy().as_ref());
    assert_eq!(report["mcp_tools"].as_array().unwrap().len(), 0);
    let category = |id: &str| report["categories"].as_array().unwrap().iter()
        .find(|category| category["id"] == id).unwrap().clone();
    assert_eq!(category("system_tools")["tokens"], 0);
    assert_eq!(category("mcp_server_instructions")["tokens"], 0);
    assert_eq!(category("memory_files")["tokens"].is_number(), true);
    assert_eq!(category("free_space")["provenance"], "derived_from_reported_usage");
    assert_eq!(report["coverage"]["complete"], false);
    assert!(report["coverage"]["gaps"].as_array().unwrap().iter()
        .any(|gap| gap == "Skills are not exposed as a distinct assembled-request source"));

    let missing_base = spawn_openai_fixture_with_usage(None);
    let _missing_base = EnvGuard::set("OPENAI_BASE_URL", &missing_base.0);
    let missing_root = tempdir().expect("missing-usage fixture root");
    let missing_storage = Storage::open_in_memory(missing_root.path().join("blobs")).unwrap();
    let missing_sessions = SessionService::new(Arc::new(missing_storage));
    let missing_session = missing_sessions
        .create_in_directory("Missing usage", Some(missing_root.path().to_owned()))
        .await
        .unwrap();
    let missing_id = missing_session.id.to_string();
    let missing_app = router(AppState {
        sessions: missing_sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let turn = missing_app.clone().oneshot(Request::builder()
        .method("POST").uri(format!("/api/sessions/{missing_id}/turns"))
        .header("content-type", "application/json")
        .body(Body::from(json!({"text":"unknown usage", "model":"openai/gpt-5.6"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    missing_base.1.join().unwrap();
    let context = missing_app.oneshot(Request::builder()
        .uri(format!("/api/sessions/{missing_id}/context")).body(Body::empty()).unwrap()).await.unwrap();
    let report = json_body(context).await["usage_report"].clone();
    assert_eq!(report["reported_used_tokens"], Value::Null);
    let free_space = report["categories"].as_array().unwrap().iter()
        .find(|category| category["id"] == "free_space").unwrap();
    assert_eq!(free_space["tokens"], Value::Null);
    assert_eq!(free_space["provenance"], "unreported");
}

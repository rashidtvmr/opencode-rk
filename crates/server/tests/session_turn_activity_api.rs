use std::{
    env,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, OnceLock},
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

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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

fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).expect("read provider request");
        assert!(read > 0, "provider request ended early");
        request.extend_from_slice(&buffer[..read]);
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
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
            return request;
        }
        assert!(
            request.len() <= 128 * 1024,
            "provider request exceeded fixture bound"
        );
    }
}

fn spawn_openai_fixture(events: &'static [u8]) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_request(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .expect("write fixture headers");
        stream.write_all(events).expect("write fixture events");
        stream.flush().expect("flush fixture events");
        String::from_utf8(request).expect("utf-8 provider request")
    });
    (format!("http://{address}/v1"), task)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 128 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

async fn create_session(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"WEB-009 activity"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

fn decode_ndjson(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("ndjson event"))
        .collect()
}

async fn stream_turn(app: &axum::Router, session_id: &str) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/turns/stream"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "text":"Explain the result",
                        "model":"openai/gpt-5.6",
                        "reasoning_effort":"high"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded turn stream");
    decode_ndjson(&bytes)
}

#[tokio::test]
async fn web_009_t01_provider_reasoning_summary_streams_and_survives_reload() {
    let _env_guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    const EVENTS: &[u8] = concat!(
        "event: response.reasoning_summary_part.added\n",
        "data: {\"type\":\"response.reasoning_summary_part.added\",\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"\"}}\n\n",
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"delta\":\"Checked the \"}\n\n",
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"delta\":\"relevant constraints.\"}\n\n",
        "event: response.reasoning_summary_text.done\n",
        "data: {\"type\":\"response.reasoning_summary_text.done\",\"item_id\":\"rs_1\",\"output_index\":0,\"summary_index\":0,\"text\":\"Checked the relevant constraints.\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Final answer\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_activity\",\"status\":\"completed\"}}\n\n"
    )
    .as_bytes();

    let (provider_base, provider_request) = spawn_openai_fixture(EVENTS);
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let (app, _dir) = app();
    let session_id = create_session(&app).await;

    let events = stream_turn(&app, &session_id).await;
    assert_eq!(events[0]["type"], "user_message");
    assert_eq!(
        events[1],
        json!({"type":"reasoning_summary_delta","delta":"Checked the "})
    );
    assert_eq!(
        events[2],
        json!({"type":"reasoning_summary_delta","delta":"relevant constraints."})
    );
    assert_eq!(events[3]["type"], "assistant_delta");
    assert_eq!(events[4]["type"], "assistant_message");
    assert_eq!(
        events[4]["reasoning_summary"],
        "Checked the relevant constraints."
    );
    let assistant_id = events[4]["message"]["id"].as_str().expect("assistant id");

    let request = provider_request.join().expect("provider fixture completed");
    let (_, body) = request.split_once("\r\n\r\n").expect("provider body");
    let body: Value = serde_json::from_str(body).expect("provider json");
    assert_eq!(body["reasoning"]["effort"], "high");
    assert_eq!(body["reasoning"]["summary"], "auto");

    let activity = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/activity?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(activity.status(), StatusCode::OK);
    let activity = json_body(activity).await;
    assert_eq!(
        activity,
        json!({
            "activity":[{
                "message_id":assistant_id,
                "reasoning_summary":"Checked the relevant constraints."
            }]
        })
    );
}

#[tokio::test]
async fn web_009_t02_unsupported_structured_provider_activity_fails_closed() {
    let _env_guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    const EVENTS: &[u8] = concat!(
        "event: response.output_text.annotation.added\n",
        "data: {\"type\":\"response.output_text.annotation.added\",\"annotation_index\":0,\"annotation\":{\"type\":\"url_citation\",\"url\":\"https://example.test\",\"title\":\"Example\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"must not persist\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_unsupported\",\"status\":\"completed\"}}\n\n"
    )
    .as_bytes();

    let (provider_base, provider_request) = spawn_openai_fixture(EVENTS);
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let (app, _dir) = app();
    let session_id = create_session(&app).await;

    let events = stream_turn(&app, &session_id).await;
    assert_eq!(events[0]["type"], "user_message");
    assert_eq!(events[1]["type"], "error");
    assert_eq!(events[1]["code"], "bad_gateway");
    assert!(events[1]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("unsupported provider stream event"));
    assert!(events
        .iter()
        .all(|event| event["type"] != "assistant_message"));

    let messages = app
        .clone()
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
    assert_eq!(messages["messages"].as_array().unwrap().len(), 1);
    assert_eq!(messages["messages"][0]["role"], "user");

    provider_request.join().expect("provider fixture completed");
}

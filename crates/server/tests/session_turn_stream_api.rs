use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
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

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
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
            break;
        }
        assert!(
            request.len() <= 128 * 1024,
            "provider request exceeded fixture bound"
        );
    }

    request
}

fn spawn_streaming_openai_fixture() -> (String, mpsc::Sender<()>, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    let (release_tx, release_rx) = mpsc::channel();

    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_request(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n",
            )
            .expect("write provider headers");

        stream
            .write_all(
                b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"del",
            )
            .expect("write split provider event prefix");
        stream.flush().expect("flush split provider prefix");
        thread::sleep(Duration::from_millis(25));
        stream
            .write_all(b"ta\":\"Fixture \"}\n\n")
            .expect("write split provider event suffix");
        stream.flush().expect("flush first provider delta");

        release_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("client observed first delta before provider completed");

        stream
            .write_all(
                b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"assistant reply\"}\n\n",
            )
            .expect("write second provider delta");
        stream
            .write_all(
                b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_fixture\",\"status\":\"completed\"}}\n\n",
            )
            .expect("write provider completion");
        stream.flush().expect("flush provider completion");

        String::from_utf8(request).expect("provider request is utf-8")
    });

    (format!("http://{address}/v1"), release_tx, task)
}

fn spawn_failed_openai_fixture() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");

    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let _request = read_request(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"fixture stream failed\"}}}\n\n",
            )
            .expect("write provider failure event");
        stream.flush().expect("flush provider failure event");
    });

    (format!("http://{address}/v1"), task)
}

async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind native server fixture");
    let address = listener.local_addr().expect("native server address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("native server fixture");
    });
    (address, task)
}

fn stream_client(
    address: SocketAddr,
    session_id: String,
    release_provider: Option<mpsc::Sender<()>>,
) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).expect("connect native server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set native response timeout");
    let body = json!({
        "text": "Hello streaming agent",
        "model": "openai/gpt-5.6",
        "reasoning_effort": "high"
    })
    .to_string();
    let request = format!(
        "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: {address}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .expect("write native streaming request");
    stream.flush().expect("flush native streaming request");

    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut released = false;
    loop {
        let read = stream
            .read(&mut buffer)
            .expect("read native streaming response");
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..read]);

        if !released
            && response
                .windows(b"\"type\":\"assistant_delta\"".len())
                .any(|window| window == b"\"type\":\"assistant_delta\"")
            && response
                .windows(b"Fixture ".len())
                .any(|window| window == b"Fixture ")
        {
            if let Some(sender) = release_provider.as_ref() {
                sender
                    .send(())
                    .expect("release provider after first client delta");
            }
            released = true;
        }

        if response
            .windows(b"\"type\":\"assistant_message\"".len())
            .any(|window| window == b"\"type\":\"assistant_message\"")
            || response
                .windows(b"\"type\":\"error\"".len())
                .any(|window| window == b"\"type\":\"error\"")
        {
            break;
        }
    }
    response
}

fn response_events(response: &[u8]) -> Vec<Value> {
    let text = String::from_utf8_lossy(response);
    let (_, body) = text.split_once("\r\n\r\n").expect("native HTTP body");
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .collect()
}

async fn create_session(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":title}).to_string()))
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

#[tokio::test]
async fn session_turn_stream_forwards_deltas_before_completion_and_persists_once() {
    let (provider_base, release_provider, provider_request) = spawn_streaming_openai_fixture();
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let (app, _dir) = app();
    let session_id = create_session(&app, "Streaming integration").await;
    let query_app = app.clone();
    let (address, server) = spawn_http(app).await;

    let client_session_id = session_id.clone();
    let response = tokio::task::spawn_blocking(move || {
        stream_client(address, client_session_id, Some(release_provider))
    })
    .await
    .expect("stream client task");

    let response_text = String::from_utf8_lossy(&response);
    assert!(response_text.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(response_text
        .to_ascii_lowercase()
        .contains("content-type: application/x-ndjson"));
    let events = response_events(&response);
    assert_eq!(
        events.len(),
        4,
        "expected user, two deltas, and final assistant"
    );
    assert_eq!(events[0]["type"], "user_message");
    assert_eq!(events[0]["message"]["role"], "user");
    assert_eq!(
        events[0]["message"]["body"]["text"],
        "Hello streaming agent"
    );
    assert_eq!(
        events[1],
        json!({"type":"assistant_delta","delta":"Fixture "})
    );
    assert_eq!(
        events[2],
        json!({"type":"assistant_delta","delta":"assistant reply"})
    );
    assert_eq!(events[3]["type"], "assistant_message");
    assert_eq!(events[3]["message"]["role"], "assistant");
    assert_eq!(
        events[3]["message"]["body"]["text"],
        "Fixture assistant reply"
    );

    let request = provider_request.join().expect("provider fixture completed");
    assert!(request.starts_with("POST /v1/responses HTTP/1.1\r\n"));
    let (_, provider_body) = request.split_once("\r\n\r\n").expect("request body");
    let provider_json: Value = serde_json::from_str(provider_body).expect("provider json");
    assert_eq!(provider_json["model"], "gpt-5.6");
    assert_eq!(provider_json["reasoning"]["effort"], "high");
    assert_eq!(provider_json["stream"], true);

    let messages = query_app
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

    server.abort();
}

#[tokio::test]
async fn session_turn_stream_reports_provider_failure_without_persisting_assistant() {
    let (provider_base, provider_task) = spawn_failed_openai_fixture();
    let _base_url = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
    let (app, _dir) = app();
    let session_id = create_session(&app, "Streaming failure").await;
    let query_app = app.clone();
    let (address, server) = spawn_http(app).await;

    let client_session_id = session_id.clone();
    let response =
        tokio::task::spawn_blocking(move || stream_client(address, client_session_id, None))
            .await
            .expect("stream client task");

    let response_text = String::from_utf8_lossy(&response);
    assert!(response_text.starts_with("HTTP/1.1 201 Created\r\n"));
    let events = response_events(&response);
    assert_eq!(events[0]["type"], "user_message");
    assert_eq!(events[1]["type"], "error");
    assert_eq!(events[1]["code"], "bad_gateway");
    assert!(events[1]["message"]
        .as_str()
        .expect("stream error message")
        .contains("fixture stream failed"));

    provider_task.join().expect("provider fixture completed");
    let messages = query_app
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

    server.abort();
}

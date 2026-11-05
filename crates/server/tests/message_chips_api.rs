//! MSG-005: a create-turn request's file/skill references resolve to
//! message-level chips and surface on the user message from `/messages`. An
//! unknown attachment id is a client error, never a fabricated chip.

use std::{
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
use tempfile::TempDir;
use tower::ServiceExt;

/// `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `HOME` and `XDG_DATA_HOME` are
/// process-global; hold one lock and point credential lookup at a disposable
/// directory so no persisted auth shadows the fixture key.
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvFixture {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: TempDir,
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvFixture {
    fn new(overrides: &[(&'static str, String)]) -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let mut saved = Vec::new();
        for (key, value) in [
            ("HOME", dir.path().to_string_lossy().into_owned()),
            ("XDG_DATA_HOME", dir.path().to_string_lossy().into_owned()),
        ]
        .into_iter()
        .chain(overrides.iter().map(|(k, v)| (*k, v.clone())))
        {
            saved.push((key, std::env::var_os(key)));
            std::env::set_var(key, value);
        }
        Self {
            _lock: lock,
            _dir: dir,
            saved,
        }
    }
}

impl Drop for EnvFixture {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn spawn_provider() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected = None;
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "provider request ended early");
            request.extend_from_slice(&buffer[..read]);
            if expected.is_none() {
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(str::parse::<usize>)
                        })
                        .transpose()
                        .unwrap()
                        .unwrap_or(0);
                    expected = Some(end + 4 + length);
                }
            }
            if expected.is_some_and(|len| request.len() >= len) {
                break;
            }
        }
        let response = json!({
            "id": "resp_chips",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "ok"}]
            }],
            "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2},
        })
        .to_string();
        let wire = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response.len(),
            response
        );
        stream.write_all(wire.as_bytes()).unwrap();
    });
    (format!("http://{address}/v1"), handle)
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn get(app: &axum::Router, uri: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn app(sessions: SessionService) -> axum::Router {
    router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test(flavor = "current_thread")]
async fn create_turn_resolves_file_and_skill_chips_onto_the_user_message() {
    let (base_url, provider) = spawn_provider();
    let _env = EnvFixture::new(&[
        ("OPENAI_BASE_URL", base_url),
        ("OPENAI_API_KEY", "fixture-secret".into()),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = app(sessions);

    let (status, created) = post(&app, "/api/sessions", json!({"title": "chips"})).await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = created["session"]["id"].as_str().unwrap().to_owned();

    // Stage one durable draft attachment the turn will reference by id.
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/attachments?name=notes.txt"))
                .header("content-type", "text/plain")
                .body(Body::from("hello attachment"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::CREATED);
    let upload: Value =
        serde_json::from_slice(&to_bytes(upload.into_body(), 32 * 1024).await.unwrap()).unwrap();
    let attachment_id = upload["attachment"]["id"].as_str().unwrap().to_owned();

    let (status, turn) = post(
        &app,
        &format!("/api/sessions/{session_id}/turns"),
        json!({
            "text": "please look",
            "model": "openai/gpt-5.6",
            "files": [attachment_id],
            "skills": [
                {"id": "skill-commit", "name": "commit"},
                {"id": "skill-commit", "name": "commit"},
            ],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{turn}");
    provider.join().unwrap();

    let body = get(&app, &format!("/api/sessions/{session_id}/messages?limit=50")).await;
    let user = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == json!("user"))
        .expect("user message");
    let files = user["files"].as_array().expect("file chips");
    assert_eq!(files.len(), 1, "one referenced attachment -> one chip");
    assert_eq!(files[0]["name"], json!("notes.txt"));
    assert_eq!(files[0]["mime"], json!("text/plain"));
    assert!(
        files[0]["hash"].as_str().is_some_and(|hash| !hash.is_empty()),
        "the chip carries the attachment content hash"
    );
    assert_eq!(
        user["skills"],
        json!([{"id": "skill-commit", "name": "commit"}]),
        "skills are deduped by id"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_attachment_id_is_rejected_without_fabricating_a_chip() {
    let (base_url, provider) = spawn_provider();
    let _env = EnvFixture::new(&[
        ("OPENAI_BASE_URL", base_url),
        ("OPENAI_API_KEY", "fixture-secret".into()),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = app(sessions);

    let (status, created) = post(&app, "/api/sessions", json!({"title": "bad chip"})).await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = created["session"]["id"].as_str().unwrap().to_owned();

    let (status, _) = post(
        &app,
        &format!("/api/sessions/{session_id}/turns"),
        json!({
            "text": "please look",
            "model": "openai/gpt-5.6",
            "files": ["01J0000000000000000000000Z"],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // The provider must never have been contacted for a rejected request.
    drop(provider);
}

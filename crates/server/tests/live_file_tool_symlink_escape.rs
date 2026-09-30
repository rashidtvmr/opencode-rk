//! RED lane: live file writes must not follow a symlink outside the workspace.
//!
//! The fixture deliberately places the lexical write path below the broker's
//! current-directory root while its Unix symlink target is outside that root.
//! A passing implementation denies before opening the target and reports that
//! denial through both live stream and bounded provider feedback.
#![forbid(unsafe_code)]

#[cfg(unix)]
mod unix {
    use std::{
        env, fs,
        io::{Read, Write},
        net::{SocketAddr, TcpListener, TcpStream},
        os::unix::fs::symlink,
        path::Path,
        sync::Arc,
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
    use tempfile::{tempdir, TempDir};
    use tower::ServiceExt;

    const SENTINEL: &[u8] = b"outside-sentinel-exact-bytes\n";
    const WRITE_CONTENT: &str = "symlink-target-must-not-change";

    fn build_app() -> (axum::Router, TempDir) {
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

    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn set(path: &Path) -> Self {
            let original = env::current_dir().expect("current directory");
            env::set_current_dir(path).expect("set fixture cwd");
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original).expect("restore current directory");
        }
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected_len = None;
        loop {
            let read = stream.read(&mut buffer).expect("read provider request");
            assert!(read > 0, "provider request ended early");
            request.extend_from_slice(&buffer[..read]);
            if expected_len.is_none() {
                if let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
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
                request.len() <= 256 * 1024,
                "provider request exceeded fixture bound"
            );
        }
        request
    }

    fn respond_sse(stream: &mut TcpStream, events: &[&str]) {
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .expect("write provider headers");
        for event in events {
            stream
                .write_all(event.as_bytes())
                .expect("write provider event");
        }
        stream.flush().expect("flush provider events");
    }

    fn round_one_events(path: &Path) -> Vec<String> {
        let args = serde_json::to_string(&json!({
            "path": path.to_string_lossy(),
            "content": WRITE_CONTENT,
        }))
        .expect("write arguments");
        let item = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "id": "fc_symlink_write_1",
                "call_id": "call_symlink_write_1",
                "name": "write",
                "arguments": args,
            }
        });
        vec![
            format!("event: response.output_item.done\ndata: {item}\n\n"),
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_symlink_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
        ]
    }

    fn round_two_events() -> Vec<String> {
        vec![
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Symlink write denied\"}\n\n".to_owned(),
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_symlink_r2\",\"status\":\"completed\"}}\n\n".to_owned(),
        ]
    }

    fn spawn_scripted_provider(
        rounds: Vec<Vec<String>>,
    ) -> (String, thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
        let address = listener.local_addr().expect("provider fixture address");
        let task = thread::spawn(move || {
            let mut bodies = Vec::new();
            for events in &rounds {
                let (mut stream, _) = listener.accept().expect("accept provider request");
                let request = read_request(&mut stream);
                let header_end = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .expect("provider request header terminator");
                bodies.push(
                    serde_json::from_slice(&request[header_end + 4..]).expect("provider JSON"),
                );
                respond_sse(
                    &mut stream,
                    &events.iter().map(String::as_str).collect::<Vec<_>>(),
                );
            }
            bodies
        });
        (format!("http://{address}/v1"), task)
    }

    async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind server");
        let address = listener.local_addr().expect("server address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        (address, task)
    }

    fn stream_turn(address: SocketAddr, session_id: String) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).expect("connect server");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("read timeout");
        let body = json!({
            "text": "write the fixture through the live tool",
            "model": "openai/gpt-5.6",
            "reasoning_effort": "high",
        })
        .to_string();
        let request = format!(
            "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).expect("write request");
        stream.flush().expect("flush request");
        let mut response = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = stream.read(&mut buffer).expect("read response");
            if read == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..read]);
            assert!(
                response.len() <= 512 * 1024,
                "live response exceeded fixture bound"
            );
        }
        response
    }

    fn ndjson_events(response: &[u8]) -> Vec<Value> {
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("response header terminator");
        std::str::from_utf8(&response[header_end + 4..])
            .expect("utf-8 stream body")
            .lines()
            .filter_map(|line| {
                let start = line.find('{')?;
                serde_json::from_str(&line[start..]).ok()
            })
            .collect()
    }

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("bounded response body");
        serde_json::from_slice(&bytes).expect("JSON body")
    }

    async fn create_session(app: &axum::Router) -> String {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "title": "symlink escape" }).to_string()))
                    .expect("session request"),
            )
            .await
            .expect("create session");
        assert_eq!(response.status(), StatusCode::CREATED);
        json_body(response).await["session"]["id"]
            .as_str()
            .expect("session id")
            .to_owned()
    }

    async fn fetch_messages(app: &axum::Router, session_id: &str) -> Vec<Value> {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sessions/{session_id}/messages?limit=50"))
                    .body(Body::empty())
                    .expect("messages request"),
            )
            .await
            .expect("fetch messages");
        assert_eq!(response.status(), StatusCode::OK);
        json_body(response).await["messages"]
            .as_array()
            .expect("messages array")
            .clone()
    }

    #[tokio::test]
    async fn live_write_symlink_escape_denied_before_target_io() {
        let cwd = tempdir().expect("unique writable cwd");
        let outside = tempdir().expect("outside disposable directory");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, SENTINEL).expect("seed outside sentinel");
        let _cwd_guard = CwdGuard::set(cwd.path());
        // macOS may spell the same temp directory as `/var` or `/private/var`.
        // Use the spelling returned by current_dir so the broker's lexical root
        // and the fixture target are identical strings.
        let cwd_path = env::current_dir().expect("fixture cwd");

        let result = async {
            let target = cwd_path.join("workspace").join("target.txt");
            fs::create_dir_all(target.parent().expect("target parent"))
                .expect("create lexical parent");
            symlink(&sentinel, &target).expect("create target symlink");

            let _api_key = EnvGuard::set("OPENAI_API_KEY", "fixture-secret");
            let _tools = EnvGuard::set("OPENCODE_RK_TURN_TOOLS", "write");
            let (app, _storage_dir) = build_app();
            let (provider_base, provider_task) =
                spawn_scripted_provider(vec![round_one_events(&target), round_two_events()]);
            let _base = EnvGuard::set("OPENAI_BASE_URL", &provider_base);
            let session_id = create_session(&app).await;
            let (address, server) = spawn_http(app.clone()).await;
            let stream_session = session_id.clone();
            let response =
                tokio::task::spawn_blocking(move || stream_turn(address, stream_session))
                    .await
                    .expect("stream task");
            server.abort();

            assert_eq!(fs::read(&sentinel).expect("read sentinel"), SENTINEL);
            let events = ndjson_events(&response);
            let output = events
                .iter()
                .find(|event| event["type"] == "tool_output")
                .and_then(|event| event["output"].as_str())
                .expect("write tool output");
            assert!(
                output.contains("denied")
                    || output.contains("symlink")
                    || output.contains("approval"),
                "symlink denial must be truthful, got {output:?}"
            );
            assert!(!output.contains("Successfully wrote"));
            assert!(!events.iter().any(|event| {
                let text = event.to_string();
                text.contains(WRITE_CONTENT) || text.contains("outside-sentinel-exact-bytes")
            }));

            let bodies = provider_task.join().expect("provider fixture finished");
            assert_eq!(
                bodies.len(),
                2,
                "denial permits exactly one bounded feedback round"
            );
            let feedback = bodies[1]["input"]
                .as_array()
                .expect("round two input")
                .iter()
                .find(|item| item["type"] == "function_call_output")
                .expect("denial feedback");
            let feedback_text = feedback["output"].as_str().unwrap_or_default();
            assert!(
                feedback_text.contains("denied")
                    || feedback_text.contains("symlink")
                    || feedback_text.contains("approval"),
                "provider feedback must describe denial, got {feedback_text:?}"
            );
            assert!(!feedback_text.contains(WRITE_CONTENT));

            let messages = fetch_messages(&app, &session_id).await;
            assert!(!messages.iter().any(|message| {
                let text = message.to_string();
                text.contains(WRITE_CONTENT) || text.contains("outside-sentinel-exact-bytes")
            }));
        }
        .await;

        result
    }
}

#[cfg(not(unix))]
#[test]
fn symlink_escape_test_deferred_on_non_unix() {}

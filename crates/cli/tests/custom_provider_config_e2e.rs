#![forbid(unsafe_code)]

//! Installed serve/API journey for project-configured OpenAI-compatible providers.

use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const IO_TIMEOUT: Duration = Duration::from_secs(2);
const JOURNEY_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_HTTP_BYTES: usize = 256 * 1024;
const API_TOKEN: &str = "provider-fallback-secret-test-only";
const CUSTOM_HEADER: &str = "x-acme-config: header-from-project-config";
const CUSTOM_BODY_KEY: &str = "acme_extension";
const CUSTOM_BODY_VALUE: &str = "body-from-project-config";
const WIRE_MODEL: &str = "wire-model-2026";

fn opencode_rk_bin() -> String {
    std::env::var_os("OPENCODE_RK_BIN")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())
        .expect("OPENCODE_RK_BIN must name the built opencode-rk binary")
        .to_string_lossy()
        .into_owned()
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestHome {
    data: PathBuf,
    project: PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("rk-{}-{id}", std::process::id()));
        let data = root.join("data");
        let project = root.join("project");
        fs::create_dir_all(&data).expect("create disposable data dir");
        fs::create_dir_all(&project).expect("create disposable project dir");
        Self { data, project }
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        if let Some(root) = self.data.parent() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn spawn(home: &TestHome, listen: SocketAddr) -> Self {
        let child = Command::new(opencode_rk_bin())
            .env_clear()
            .env("OPENCODE_RK_HOME", &home.data)
            // Deliberately leave ACME_PRIMARY_KEY unset. Only the second ordered
            // candidate exists in this child process.
            .env("ACME_FALLBACK_KEY", API_TOKEN)
            .current_dir(&home.project)
            .args(["serve", "--listen", &listen.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start installed opencode-rk serve");
        Self(child)
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct ProviderFixture {
    address: SocketAddr,
    task: Option<thread::JoinHandle<Result<String, String>>>,
}

impl ProviderFixture {
    fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback provider fixture");
        let address = listener.local_addr().expect("provider fixture address");
        listener
            .set_nonblocking(true)
            .expect("make fixture accept bounded");
        let task = thread::spawn(move || serve_one_provider_request(listener));
        Self {
            address,
            task: Some(task),
        }
    }

    fn join(&mut self) -> Result<String, String> {
        self.task
            .take()
            .expect("provider fixture joined once")
            .join()
            .map_err(|_| "provider fixture thread panicked".to_owned())?
    }
}

impl Drop for ProviderFixture {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

fn free_loopback() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve daemon loopback port");
    listener.local_addr().expect("daemon address")
}

fn write_project_config(home: &TestHome, provider_base: &str) {
    let config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {
            "acme": {
                "name": "Acme fixture",
                "npm": "@ai-sdk/openai-compatible",
                "env": ["ACME_PRIMARY_KEY", "ACME_FALLBACK_KEY"],
                "options": {
                    "baseURL": provider_base,
                    "headers": {"x-acme-config": "header-from-project-config"},
                    "body": {"acme_extension": CUSTOM_BODY_VALUE}
                },
                "models": {
                    "model-key": {
                        "name": "Acme model",
                        "id": WIRE_MODEL
                    }
                }
            }
        }
    });
    fs::write(
        home.project.join("opencode.json"),
        serde_json::to_vec(&config).expect("serialize project config"),
    )
    .expect("write project config");
}

fn wait_for_bearer(home: &TestHome, address: SocketAddr) -> String {
    let descriptor = home.data.join("runtime/backend.json");
    let deadline = Instant::now() + JOURNEY_TIMEOUT;
    loop {
        if let Ok(bytes) = fs::read(&descriptor) {
            if bytes.len() <= 8 * 1024 {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if value["http_origin"].as_str() == Some(&format!("http://{address}")) {
                        if let Some(token) = value["auth_token"].as_str() {
                            return token.to_owned();
                        }
                    }
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "serve did not publish authenticated descriptor"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn api_request(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    bearer: &str,
) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let body = body.unwrap_or("");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Type: application/json\r\nAuthorization: Bearer {bearer}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()?;
    read_bounded(&mut stream)
}

fn read_bounded(reader: &mut impl Read) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "HTTP fixture response exceeded 256 KiB",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn response_body(response: &str) -> &str {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .expect("HTTP response has header terminator")
}

fn wait_for_health(address: SocketAddr) {
    let deadline = Instant::now() + JOURNEY_TIMEOUT;
    loop {
        if let Ok(response) = api_request(address, "GET", "/health", None, "") {
            if response.starts_with("HTTP/1.1 200") {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "serve did not become healthy at {address}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn serve_one_provider_request(listener: TcpListener) -> Result<String, String> {
    let deadline = Instant::now() + JOURNEY_TIMEOUT;
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("provider fixture received no configured turn request".to_owned());
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(format!("provider fixture accept failed: {error}")),
        }
    };
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("provider read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("provider write timeout: {error}"))?;
    let request = read_request(&mut stream)?;
    let reply = serde_json::json!({
        "id": "resp_custom_provider_fixture",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "custom provider persisted reply"}]
        }]
    })
    .to_string();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    )
    .map_err(|error| format!("write fixture response: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("flush fixture response: {error}"))?;

    // One request is the whole provider contract; reject metadata/discovery or
    // retry traffic rather than silently accepting extra calls.
    let extra_deadline = Instant::now() + Duration::from_millis(150);
    while Instant::now() < extra_deadline {
        match listener.accept() {
            Ok(_) => return Err("unexpected additional provider/discovery request".to_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(format!("provider fixture accept failed: {error}")),
        }
    }
    Ok(request)
}

fn read_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("read provider request: {error}"))?;
        if read == 0 {
            return Err("provider request ended before content length".to_owned());
        }
        if bytes.len().saturating_add(read) > MAX_HTTP_BYTES {
            return Err("provider request exceeded 256 KiB".to_owned());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(str::trim)
                        .map(str::parse::<usize>)
                })
                .transpose()
                .map_err(|error| format!("invalid provider content length: {error}"))?
                .ok_or_else(|| "provider request omitted content length".to_owned())?;
            if content_length > MAX_HTTP_BYTES - (header_end + 4) {
                return Err("provider request body exceeded 256 KiB".to_owned());
            }
            if bytes.len() >= header_end + 4 + content_length {
                return String::from_utf8(bytes)
                    .map_err(|error| format!("provider request was not UTF-8: {error}"));
            }
        }
    }
}

fn assert_contains_no_secret(response: &str) {
    assert!(
        !response.contains(API_TOKEN),
        "provider credential leaked into daemon API response"
    );
}

#[test]
fn configured_custom_provider_catalog_turn_and_persistence() {
    let home = TestHome::new();
    let mut provider = ProviderFixture::spawn();
    let provider_base = format!("http://{}/v1", provider.address);
    write_project_config(&home, &provider_base);
    let daemon_address = free_loopback();
    let _daemon = ChildGuard::spawn(&home, daemon_address);
    let bearer = wait_for_bearer(&home, daemon_address);
    wait_for_health(daemon_address);

    let catalog = api_request(daemon_address, "GET", "/api/models", None, &bearer)
        .expect("authenticated provider catalog request");
    assert!(
        catalog.starts_with("HTTP/1.1 200"),
        "models endpoint status"
    );
    assert_contains_no_secret(&catalog);
    let catalog_json: serde_json::Value =
        serde_json::from_str(response_body(&catalog)).expect("valid model catalog JSON");
    assert!(
        catalog_json["models"]
            .as_array()
            .is_some_and(|models| models.iter().any(|model| {
                model["provider_id"] == "acme" && model["model_id"] == "model-key"
            })),
        "project provider/model key must appear in live /api/models catalog"
    );

    let created = api_request(
        daemon_address,
        "POST",
        "/api/sessions",
        Some(r#"{"title":"custom-provider-config-e2e"}"#),
        &bearer,
    )
    .expect("create live session");
    assert!(created.starts_with("HTTP/1.1 201"), "session create status");
    assert_contains_no_secret(&created);
    let created_json: serde_json::Value =
        serde_json::from_str(response_body(&created)).expect("valid create session response");
    let session_id = created_json["session"]["id"]
        .as_str()
        .expect("created session ID");
    let prompt = "respond using configured acme provider";
    let turn_body = serde_json::json!({
        "text": prompt,
        "model": "acme/model-key",
        "reasoning_effort": "medium"
    })
    .to_string();
    let turn_path = format!("/api/sessions/{session_id}/turns");
    let turn = api_request(
        daemon_address,
        "POST",
        &turn_path,
        Some(&turn_body),
        &bearer,
    )
    .expect("submit configured provider turn");
    assert!(
        turn.starts_with("HTTP/1.1 201"),
        "turn failed at daemon API"
    );
    assert_contains_no_secret(&turn);

    let transcript_path = format!("/api/sessions/{session_id}/messages?limit=20");
    let transcript = api_request(daemon_address, "GET", &transcript_path, None, &bearer)
        .expect("read persisted session transcript");
    assert!(transcript.starts_with("HTTP/1.1 200"), "transcript status");
    assert_contains_no_secret(&transcript);
    let transcript_json: serde_json::Value =
        serde_json::from_str(response_body(&transcript)).expect("valid persisted transcript");
    let messages = transcript_json["messages"]
        .as_array()
        .expect("persisted messages array");
    assert!(
        messages
            .iter()
            .any(|message| message["body"]["text"] == prompt),
        "persisted transcript must include submitted prompt"
    );
    assert!(
        messages
            .iter()
            .any(|message| { message["body"]["text"] == "custom provider persisted reply" }),
        "persisted transcript must include assistant provider reply"
    );

    let request = provider
        .join()
        .expect("exactly one bounded loopback provider request");
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .expect("provider request header boundary");
    assert!(
        request.starts_with("POST /v1/responses HTTP/1.1\r\n"),
        "configured baseURL must target the compatible Responses endpoint"
    );
    assert!(
        headers.lines().any(|line| {
            line.to_ascii_lowercase()
                .starts_with("authorization: bearer ")
                && line
                    .split_once(':')
                    .is_some_and(|(_, value)| value.trim() == format!("Bearer {API_TOKEN}"))
        }),
        "only configured second ordered env credential must authenticate the request"
    );
    assert!(
        headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case(CUSTOM_HEADER)),
        "configured custom request header must be forwarded"
    );
    let provider_json: serde_json::Value =
        serde_json::from_str(body.trim_end_matches('\0')).expect("valid provider request JSON");
    assert_eq!(provider_json["model"], WIRE_MODEL, "wire model ID mapping");
    assert_eq!(
        provider_json[CUSTOM_BODY_KEY], CUSTOM_BODY_VALUE,
        "configured custom request body must be forwarded"
    );
    assert_eq!(provider_json["input"][0]["content"][0]["text"], prompt);
}

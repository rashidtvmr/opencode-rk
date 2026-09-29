//! PROV-030 external black-box candidate v3.
//! Source-only: not registered, compiled, frozen, or runtime-verified.
#![forbid(unsafe_code)]

use std::{
    fs::File,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tempfile::TempDir;

const PERSISTED_KEY: &str = "synthetic-prov030-persisted-key";
const AMBIENT_KEY: &str = "synthetic-prov030-ambient-key";
const MAX_WIRE_BYTES: usize = 256 * 1024;
const MAX_DESCRIPTOR_BYTES: u64 = 8 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(8);
const READY_TIMEOUT: Duration = Duration::from_secs(12);
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn checked_add(a: usize, b: usize) -> io::Result<usize> {
    a.checked_add(b).ok_or_else(|| invalid("HTTP length overflow"))
}

fn read_more(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    deadline: Instant,
    cap: usize,
) -> io::Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "HTTP deadline"));
    }
    stream.set_read_timeout(Some(remaining))?;
    let mut chunk = [0u8; 4096];
    let count = stream.read(&mut chunk)?;
    if count == 0 {
        return Err(invalid("incomplete HTTP message"));
    }
    let new_len = checked_add(bytes.len(), count)?;
    if new_len > cap {
        return Err(invalid("HTTP wire cap exceeded"));
    }
    bytes.extend_from_slice(&chunk[..count]);
    Ok(())
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes.get(start..)?.windows(2).position(|w| w == b"\r\n").map(|n| start + n)
}

/// Read one bounded HTTP message and return headers plus decoded body. Exactly
/// one Content-Length or a strict chunked transfer is required. Chunk trailers
/// are accepted only as bounded `field: value` lines terminated by an empty line.
fn read_http_message(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut raw = Vec::new();
    let header_end = loop {
        if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        read_more(stream, &mut raw, deadline, MAX_WIRE_BYTES)?;
    };
    let header_text = std::str::from_utf8(&raw[..header_end]).map_err(|_| invalid("HTTP headers"))?;
    let mut content_length = None;
    let mut transfer_encoding = None;
    for line in header_text.lines().skip(1) {
        let (name, value) = line.split_once(':').ok_or_else(|| invalid("malformed header"))?;
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(invalid("duplicate Content-Length"));
            }
            content_length = Some(value.trim().parse::<usize>().map_err(|_| invalid("bad Content-Length"))?);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if transfer_encoding.replace(value.trim().to_owned()).is_some() {
                return Err(invalid("duplicate Transfer-Encoding"));
            }
        }
    }
    let chunked = match transfer_encoding.as_deref() {
        None => false,
        Some(value) => {
            let codings: Vec<_> = value.split(',').map(str::trim).collect();
            if codings.len() != 1 || !codings[0].eq_ignore_ascii_case("chunked") {
                return Err(invalid("unsupported transfer coding"));
            }
            true
        }
    };
    if chunked == content_length.is_some() {
        return Err(invalid("ambiguous or missing HTTP framing"));
    }
    let body_start = checked_add(header_end, 4)?;
    if let Some(length) = content_length {
        let total = checked_add(body_start, length)?;
        if total > MAX_WIRE_BYTES {
            return Err(invalid("declared body exceeds cap"));
        }
        while raw.len() < total {
            read_more(stream, &mut raw, deadline, MAX_WIRE_BYTES)?;
        }
        raw.truncate(total);
        return Ok(raw);
    }

    let mut cursor = body_start;
    let mut decoded = Vec::new();
    loop {
        let line_end = loop {
            if let Some(end) = find_crlf(&raw, cursor) {
                break end;
            }
            read_more(stream, &mut raw, deadline, MAX_WIRE_BYTES)?;
        };
        let size_text = std::str::from_utf8(&raw[cursor..line_end])
            .map_err(|_| invalid("chunk size"))?;
        let size_text = size_text.split(';').next().ok_or_else(|| invalid("chunk extension"))?;
        let size = usize::from_str_radix(size_text, 16).map_err(|_| invalid("chunk size"))?;
        cursor = checked_add(line_end, 2)?;
        if size == 0 {
            loop {
                let trailer_end = loop {
                    if let Some(end) = find_crlf(&raw, cursor) {
                        break end;
                    }
                    read_more(stream, &mut raw, deadline, MAX_WIRE_BYTES)?;
                };
                if trailer_end == cursor {
                    cursor = checked_add(trailer_end, 2)?;
                    break;
                }
                let trailer = std::str::from_utf8(&raw[cursor..trailer_end])
                    .map_err(|_| invalid("chunk trailer"))?;
                let (name, _) = trailer.split_once(':').ok_or_else(|| invalid("chunk trailer"))?;
                if name.trim().is_empty() {
                    return Err(invalid("empty chunk trailer name"));
                }
                cursor = checked_add(trailer_end, 2)?;
            }
            break;
        }
        if size > MAX_WIRE_BYTES || decoded.len() > MAX_WIRE_BYTES - size {
            return Err(invalid("decoded chunk cap"));
        }
        let data_end = checked_add(cursor, size)?;
        let framed_end = checked_add(data_end, 2)?;
        while raw.len() < framed_end {
            read_more(stream, &mut raw, deadline, MAX_WIRE_BYTES)?;
        }
        decoded.extend_from_slice(&raw[cursor..data_end]);
        if &raw[data_end..framed_end] != b"\r\n" {
            return Err(invalid("chunk terminator"));
        }
        cursor = framed_end;
    }
    let mut output = raw[..body_start].to_vec();
    output.extend_from_slice(&decoded);
    Ok(output)
}

fn response_parts(raw: &[u8]) -> (u16, &[u8]) {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("response headers");
    let status = String::from_utf8_lossy(&raw[..end])
        .lines().next().expect("status line").split_whitespace().nth(1)
        .expect("status code").parse().expect("numeric status");
    (status, &raw[end + 4..])
}

fn authorization(req: &[u8]) -> Result<Option<String>, &'static str> {
    let end = req.windows(4).position(|w| w == b"\r\n\r\n").ok_or("provider headers")?;
    let mut found = None;
    for line in String::from_utf8_lossy(&req[..end]).lines().skip(1) {
        let (name, value) = line.split_once(':').ok_or("provider header")?;
        if name.eq_ignore_ascii_case("authorization") {
            if found.is_some() { return Err("duplicate authorization"); }
            let mut fields = value.split_ascii_whitespace();
            if !fields.next().is_some_and(|scheme| scheme.eq_ignore_ascii_case("bearer")) {
                return Err("authorization scheme");
            }
            let token = fields.next().ok_or("authorization token")?;
            if fields.next().is_some() { return Err("authorization fields"); }
            found = Some(token.to_owned());
        }
    }
    Ok(found)
}

struct Reply { body: Vec<u8>, content_type: &'static str }
struct Provider { url: String, cancel: Option<mpsc::Sender<()>>, join: Option<thread::JoinHandle<Vec<Vec<u8>>>> }

impl Provider {
    fn new(rounds: Vec<(Reply, Reply)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("provider bind");
        listener.set_nonblocking(true).expect("provider nonblocking");
        let address = listener.local_addr().expect("provider address");
        let (cancel, stop) = mpsc::channel();
        let join = thread::spawn(move || {
            let mut captures = Vec::new();
            for (normal, streaming) in rounds {
                let request_deadline = Instant::now() + READY_TIMEOUT;
                let (mut socket, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            if stop.recv_timeout(Duration::from_millis(25)).is_ok() { return captures; }
                            if Instant::now() >= request_deadline { return captures; }
                        }
                        Err(_) => return captures,
                    }
                };
                let Ok(request) = read_http_message(&mut socket) else { return captures };
                let is_stream = String::from_utf8_lossy(&request).contains("\"stream\":true");
                let reply = if is_stream { streaming } else { normal };
                let header = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    reply.content_type, reply.body.len()
                );
                if socket.write_all(header.as_bytes()).and_then(|_| socket.write_all(&reply.body)).is_err() {
                    return captures;
                }
                captures.push(request);
            }
            captures
        });
        Self { url: format!("http://{address}/v1"), cancel: Some(cancel), join: Some(join) }
    }

    fn finish(mut self) -> Vec<Vec<u8>> {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        self.join.take().expect("provider owner").join().expect("provider thread")
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() { let _ = cancel.send(()); }
        if let Some(join) = self.join.take() { let _ = join.join(); }
    }
}

fn normal_reply(text: &str) -> Reply {
    Reply { body: serde_json::to_vec(&json!({"id":"resp030","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}]})).expect("fixture JSON"), content_type: "application/json" }
}

fn stream_reply(text: &str) -> Reply {
    let body = format!(
        "event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":{}}}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp030\",\"status\":\"completed\"}}}}\n\n",
        serde_json::to_string(text).expect("fixture text")
    );
    Reply { body: body.into_bytes(), content_type: "text/event-stream" }
}

fn stream_write_reply() -> Reply {
    Reply { body: b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc030\",\"call_id\":\"call030\",\"name\":\"write\",\"arguments\":\"{\\\"path\\\":\\\"marker-prov030.txt\\\",\\\"content\\\":\\\"PROV030-WRITE-7\\\",\\\"append\\\":false}\"}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp030\",\"status\":\"completed\"}}\n\n".to_vec(), content_type: "text/event-stream" }
}

struct ChildGuard { child: Option<Child> }
impl Drop for ChildGuard {
    fn drop(&mut self) { if let Some(mut child) = self.child.take() { let _ = child.kill(); let _ = child.wait(); } }
}

struct Server { child: Child, origin: String, bearer: String, _home: TempDir }
impl Server {
    fn new(provider: &str, ambient: Option<&str>, records: Option<Value>, project: &Path) -> Self {
        let binary = PathBuf::from(std::env::var_os("OC2_TEST_BINARY").expect("OC2_TEST_BINARY required"));
        assert!(binary.is_file(), "OC2_TEST_BINARY is not a file");
        let home = tempfile::tempdir().expect("disposable HOME");
        let xdg = home.path().join("xdg");
        let data = home.path().join("rk-state");
        if let Some(records) = records {
            let directory = xdg.join("opencode");
            std::fs::create_dir_all(&directory).expect("auth directory");
            let path = directory.join("auth.json");
            std::fs::write(&path, serde_json::to_vec(&records).expect("auth JSON")).expect("auth fixture");
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("auth mode");
            }
        }
        let _environment_lock = ENV_LOCK.lock().expect("environment lock");
        let mut command = Command::new(binary);
        command.env_clear()
            .env("HOME", home.path()).env("USERPROFILE", home.path())
            .env("XDG_DATA_HOME", &xdg).env("OPENCODE_RK_HOME", &data)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("OPENAI_BASE_URL", provider)
            .env("OPENCODE_RK_TURN_TOOLS", "write")
            .current_dir(project).arg("--data-dir").arg(&data)
            .arg("serve").arg("--listen").arg("127.0.0.1:0")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(key) = ambient { command.env("OPENAI_API_KEY", key); }
        let child = command.spawn().expect("spawn daemon");
        let mut guard = ChildGuard { child: Some(child) };
        drop(_environment_lock);

        let descriptor = data.join("runtime/backend.json");
        let deadline = Instant::now() + READY_TIMEOUT;
        let (origin, bearer) = loop {
            if let Some(child) = guard.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() { panic!("daemon exited before readiness: {status}"); }
            }
            if let Ok(metadata) = std::fs::metadata(&descriptor) {
                if metadata.len() <= MAX_DESCRIPTOR_BYTES {
                    let mut file = File::open(&descriptor).expect("descriptor open");
                    let mut bytes = Vec::new();
                    file.take(MAX_DESCRIPTOR_BYTES + 1).read_to_end(&mut bytes).expect("descriptor read");
                    if bytes.len() <= MAX_DESCRIPTOR_BYTES {
                        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                            if let (Some(origin), Some(token)) = (value["http_origin"].as_str(), value["auth_token"].as_str()) {
                                if !origin.is_empty() && !token.is_empty() { break (origin.to_owned(), token.to_owned()); }
                            }
                        }
                    }
                }
            }
            assert!(Instant::now() < deadline, "bounded daemon readiness");
            thread::yield_now();
        };
        Self { child: guard.child.take().expect("child ownership"), origin, bearer, _home: home }
    }
}
impl Drop for Server { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }

fn request(server: &Server, method: &str, path: &str, body: Value) -> (u16, Vec<u8>) {
    let address: SocketAddr = server.origin.strip_prefix("http://").expect("loopback origin").parse().expect("origin address");
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT).expect("daemon connect");
    let body = serde_json::to_vec(&body).expect("request JSON");
    let header = format!("{method} {path} HTTP/1.1\r\nhost: {address}\r\nauthorization: Bearer {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", server.bearer, body.len());
    stream.write_all(header.as_bytes()).expect("daemon headers");
    stream.write_all(&body).expect("daemon body");
    let response = read_http_message(&mut stream).expect("bounded daemon response");
    let (status, body) = response_parts(&response);
    (status, body.to_vec())
}

fn records() -> Value {
    json!({"openai":{"type":"api","key":PERSISTED_KEY},"example-ext":{"type":"api","key":"valid-sibling"},"malformed":{"type":"api","key":7}})
}
fn no_secret(bytes: &[u8], bearer: &str) {
    let text = String::from_utf8_lossy(bytes);
    assert!(!text.contains(PERSISTED_KEY));
    assert!(!text.contains(AMBIENT_KEY));
    assert!(!text.contains(bearer), "response leaked daemon bearer");
}

fn assert_no_daemon_bearer(request: &[u8], bearer: &str) {
    assert!(!request.windows(bearer.len()).any(|window| window == bearer.as_bytes()), "provider request leaked daemon bearer");
}
fn assert_auth(request: &[u8], expected: &str) {
    let actual = authorization(request).expect("authorization parse");
    assert!(actual.as_deref() == Some(expected), "provider Authorization credential mismatch");
}
fn assert_no_auth(request: &[u8]) {
    let actual = authorization(request).expect("authorization parse");
    assert!(actual.is_none(), "unexpected provider Authorization field");
}
fn session(server: &Server, title: &str) -> String { let (status, body) = request(server, "POST", "/api/sessions", json!({"title":title})); assert_eq!(status, 201); serde_json::from_slice::<Value>(&body).expect("session JSON")["session"]["id"].as_str().expect("session id").to_owned() }
fn turn(text: &str) -> Value { json!({"text":text,"model":"openai/gpt-5.6","reasoning_effort":"high"}) }
fn terminal_success(body: &[u8], text: &str) { let value = String::from_utf8_lossy(body); assert!(value.lines().any(|line| serde_json::from_str::<Value>(line).ok().is_some_and(|event| event["type"] == "assistant_message" && event["stop_reason"] == "completed" && event["message"]["body"]["text"] == text))); assert!(!value.contains("\"type\":\"error\"")); }

#[test]
fn ambient_only_nonstream_control_succeeds() {
    let provider = Provider::new(vec![(normal_reply("ambient"), stream_reply("ambient"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, Some(AMBIENT_KEY), None, project.path());
    let id = session(&server, "ambient nonstream");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns"), turn("ambient"));
    assert_eq!(status, 201); assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["assistant_message"]["body"]["text"], "ambient"); no_secret(&body, &server.bearer);
    let captures = provider.finish(); assert_eq!(captures.len(), 1); assert_auth(&captures[0], AMBIENT_KEY); assert_no_daemon_bearer(&captures[0], &server.bearer);
}

#[test]
fn ambient_only_stream_write_control_succeeds() {
    let provider = Provider::new(vec![(normal_reply("unused"), stream_write_reply()), (normal_reply("unused"), stream_reply("ambient write done"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, Some(AMBIENT_KEY), None, project.path());
    let id = session(&server, "ambient stream write");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns/stream"), turn("write marker"));
    assert_eq!(status, 201); terminal_success(&body, "ambient write done"); no_secret(&body, &server.bearer);
    assert_eq!(std::fs::read_to_string(project.path().join("marker-prov030.txt")).expect("write effect"), "PROV030-WRITE-7");
    let captures = provider.finish(); assert_eq!(captures.len(), 2); for capture in &captures { assert_auth(capture, AMBIENT_KEY); assert_no_daemon_bearer(capture, &server.bearer); }
    let second = String::from_utf8_lossy(&captures[1]); assert!(second.contains("call030")); assert!(second.contains("write success"));
}

#[test]
fn persisted_valid_auth_with_malformed_sibling_succeeds() {
    let provider = Provider::new(vec![(normal_reply("persisted"), stream_reply("persisted"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, None, Some(records()), project.path());
    let id = session(&server, "persisted");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns"), turn("persisted"));
    assert_eq!(status, 201); no_secret(&body, &server.bearer);
    let captures = provider.finish(); assert_eq!(captures.len(), 1); assert_auth(&captures[0], PERSISTED_KEY); assert_no_daemon_bearer(&captures[0], &server.bearer);
}

#[test]
fn both_present_uses_persisted_precedence() {
    let provider = Provider::new(vec![(normal_reply("precedence"), stream_reply("precedence"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, Some(AMBIENT_KEY), Some(records()), project.path());
    let id = session(&server, "precedence");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns"), turn("precedence"));
    assert_eq!(status, 201); assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["assistant_message"]["body"]["text"], "precedence"); no_secret(&body, &server.bearer);
    let captures = provider.finish(); assert_eq!(captures.len(), 1); assert_auth(&captures[0], PERSISTED_KEY); assert_no_daemon_bearer(&captures[0], &server.bearer); assert!(!String::from_utf8_lossy(&captures[0]).contains(AMBIENT_KEY));
}

#[test]
fn no_usable_auth_makes_zero_provider_requests() {
    let provider = Provider::new(vec![(normal_reply("must not run"), stream_reply("must not run"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, None, Some(json!({"malformed":{"type":"api","key":7}})), project.path());
    let id = session(&server, "no auth");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns"), turn("no auth"));
    assert_ne!(status, 201); no_secret(&body, &server.bearer); let captures = provider.finish(); assert!(captures.is_empty());
}

#[test]
fn persisted_stream_write_followup_has_effect_call_identity_and_terminal_success() {
    let provider = Provider::new(vec![(normal_reply("unused"), stream_write_reply()), (normal_reply("unused"), stream_reply("persisted write done"))]);
    let project = tempfile::tempdir().expect("project");
    let server = Server::new(&provider.url, None, Some(records()), project.path());
    let id = session(&server, "persisted stream write");
    let (status, body) = request(&server, "POST", &format!("/api/sessions/{id}/turns/stream"), turn("write marker"));
    assert_eq!(status, 201); terminal_success(&body, "persisted write done"); no_secret(&body, &server.bearer);
    assert_eq!(std::fs::read_to_string(project.path().join("marker-prov030.txt")).expect("write effect"), "PROV030-WRITE-7");
    let captures = provider.finish(); assert_eq!(captures.len(), 2); for capture in &captures { assert_auth(capture, PERSISTED_KEY); assert_no_daemon_bearer(capture, &server.bearer); }
    let second = String::from_utf8_lossy(&captures[1]); assert!(second.contains("call030")); assert!(second.contains("write success"));
}

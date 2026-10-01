//! APP-012 installed restart/resume RED.
//!
//! This is deliberately std-only: the independent runner supplies an exact
//! installed `oc2` through `OC2_TEST_BINARY`. No development binary is built
//! implicitly and no user database is touched.
extern crate serde_json;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const MAX_BODY: usize = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);

fn read_request(mut stream: &TcpStream) -> Vec<u8> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 { break; }
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() <= MAX_BODY, "fixture request exceeded bound");
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length = headers.lines().find_map(|line| {
                line.to_ascii_lowercase().strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            }).unwrap_or(0);
            if bytes.len() >= end + 4 + length { break; }
        }
    }
    bytes
}

fn request_json(request: &[u8]) -> serde_json::Value {
    let header_end = request.windows(4).position(|w| w == b"\r\n\r\n").expect("provider headers");
    serde_json::from_slice(&request[header_end + 4..]).expect("provider JSON body")
}

fn assert_typed_transcript(request: &[u8], target: &Path, original_prompt: &str) {
    let value = request_json(request);
    let input = value.get("input").and_then(serde_json::Value::as_array).expect("Responses input array");
    let mut call = None;
    let mut output = None;
    let mut original = false;
    for item in input {
        if item.get("type").and_then(serde_json::Value::as_str) == Some("function_call") {
            assert_eq!(item.get("call_id").and_then(serde_json::Value::as_str), Some("call-human"));
            assert_eq!(item.get("name").and_then(serde_json::Value::as_str), Some("write"));
            let args = item.get("arguments").and_then(serde_json::Value::as_str).expect("function arguments");
            let args: serde_json::Value = serde_json::from_str(args).expect("typed write arguments");
            assert_eq!(args.get("path").and_then(serde_json::Value::as_str), target.to_str());
            assert_eq!(args.get("content").and_then(serde_json::Value::as_str), Some("must-not-write"));
            call = Some(());
        } else if item.get("type").and_then(serde_json::Value::as_str) == Some("function_call_output") {
            assert_eq!(item.get("call_id").and_then(serde_json::Value::as_str), Some("call-human"));
            assert!(item.get("output").and_then(serde_json::Value::as_str).unwrap_or("").contains("requires human approval"));
            output = Some(());
        }
        if item.get("role").and_then(serde_json::Value::as_str) == Some("user")
            && item.get("content").and_then(serde_json::Value::as_str) == Some(original_prompt) { original = true; }
    }
    assert!(call.is_some(), "typed function_call missing");
    assert!(output.is_some(), "typed function_call_output missing");
    assert!(original, "typed original user message missing");
}

fn assert_initial_input(request: &[u8], original_prompt: &str) {
    let value = request_json(request);
    let input = value.get("input").and_then(serde_json::Value::as_array).expect("Responses input array");
    assert!(input.iter().any(|item| item.get("role").and_then(serde_json::Value::as_str) == Some("user")
        && item.get("content").and_then(serde_json::Value::as_str) == Some(original_prompt)), "initial typed user input missing");
}

fn sse(events: &[&str]) -> String {
    let body = events.join("");
    format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body)
}

fn fixture(target: &Path) -> (String, mpsc::Receiver<Vec<u8>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(false).unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::sync_channel(3);
    let path = target.display().to_string().replace('"', "\\\"");
    let join = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        for round in 0..3 {
            let deadline = Instant::now() + Duration::from_secs(8);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
                    Err(error) => panic!("bounded provider accept failed: {error}"),
                }
            };
            let request = read_request(&stream);
            sender.send(request).unwrap();
            let wire = if round == 0 {
                sse(&[
                    &format!("event: response.output_item.done\ndata: {{\"type\":\"response.output_item.done\",\"item\":{{\"type\":\"function_call\",\"id\":\"fc1\",\"call_id\":\"call-human\",\"name\":\"write\",\"arguments\":\"{{\\\"path\\\":\\\"{path}\\\",\\\"content\\\":\\\"must-not-write\\\"}}\"}}}}\n\n"),
                    "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"status\":\"completed\"}}\n\n",
                ])
            } else if round == 1 {
                sse(&[
                    "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"denied\"}\n\n",
                    "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r2\",\"status\":\"completed\"}}\n\n",
                ])
            } else {
                sse(&[
                    "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"resumed-after-restart\"}\n\n",
                    "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r3\",\"status\":\"completed\"}}\n\n",
                ])
            };
            stream.write_all(wire.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/v1"), receiver, join)
}

struct DaemonGuard { child: Option<std::process::Child> }
impl DaemonGuard {
    fn new(child: std::process::Child) -> Self { Self { child: Some(child) } }
    fn stop(&mut self) { if let Some(mut child) = self.child.take() { let _ = child.kill(); let _ = child.wait(); } }
}
impl Drop for DaemonGuard {
    fn drop(&mut self) { self.stop(); }
}

fn binary() -> PathBuf {
    let path = PathBuf::from(std::env::var_os("OC2_TEST_BINARY").expect("OC2_TEST_BINARY"));
    assert!(path.is_file() && executable(&path), "installed binary is not executable: {}", path.display());
    path
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}
#[cfg(not(unix))]
fn executable(path: &Path) -> bool { path.is_file() }

fn spawn_server(binary: &Path, home: &Path, provider: &str, port: u16) -> std::process::Child {
    let mut child = Command::new(binary);
    child.env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("OPENCODE_RK_HOME", home)
        .env("OPENCODE_RK_TURN_TOOLS", "write")
        .env("OPENAI_API_KEY", "synthetic-fixture-key")
        .env("OPENAI_BASE_URL", provider)
        .args(["serve", "--listen", &format!("127.0.0.1:{port}")])
        .current_dir(home.join("project"))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("spawn installed oc2 serve")
}

fn http(origin: &str, token: &str, method: &str, path: &str, body: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(origin).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let request = format!("{method} {path} HTTP/1.1\r\nhost: localhost\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new(); stream.read_to_end(&mut response).unwrap();
    let status = String::from_utf8_lossy(&response).split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, response)
}

fn bearer(home: &Path) -> String {
    let path = home.join("runtime/backend.json");
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(start) = text.find("\"auth_token\":\"") {
                let rest = &text[start + 14..];
                if let Some(end) = rest.find('"') { return rest[..end].to_owned(); }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("bounded descriptor read failed")
}

fn session_id(response: &[u8]) -> String {
    let text = String::from_utf8_lossy(response);
    let json = text.split("\r\n\r\n").nth(1).unwrap();
    let marker = "\"id\":\"";
    let start = json.find(marker).unwrap() + marker.len();
    json[start..].split('"').next().unwrap().to_owned()
}

#[test]
fn installed_second_turn_survives_restart_without_approving_write() {
    let binary = binary();
    let home = std::env::temp_dir().join(format!("app012-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let project = home.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let target = project.join("denied.txt");
    let before = b"unchanged";
    std::fs::write(&target, before).unwrap();
    let (provider, requests, server) = fixture(&target);
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let origin = format!("127.0.0.1:{port}");
    let mut daemon = DaemonGuard::new(spawn_server(&binary, &home, &provider, port));
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if TcpStream::connect(&origin).is_ok() { break; }
        thread::sleep(Duration::from_millis(50));
    }
    let mut token = bearer(&home);
    let (status, created) = http(&origin, &token, "POST", "/api/sessions", r#"{"title":"APP-012 restart"}"#);
    assert_eq!(status, 201, "session creation failed: {:?}", String::from_utf8_lossy(&created));
    let session = session_id(&created);
    let body = r#"{"text":"inspect the disposable fixture","model":"openai/gpt-5.6","reasoning_effort":"high"}"#;
    let (first_status, first_response) = http(&origin, &token, "POST", &format!("/api/sessions/{session}/turns/stream"), body);
    assert_eq!(first_status, 201, "first stream failed: {:?}", String::from_utf8_lossy(&first_response));
    assert!(first_response.windows(b"call-human".len()).any(|w| w == b"call-human"), "first stream omitted typed call id");
    let first_request = requests.recv_timeout(TIMEOUT).expect("initial provider request");
    assert_initial_input(&first_request, "inspect the disposable fixture");
    let followup_request = requests.recv_timeout(TIMEOUT).expect("tool follow-up provider request");
    assert_typed_transcript(&followup_request, &target, "inspect the disposable fixture");
    daemon.stop();
    let mut daemon = DaemonGuard::new(spawn_server(&binary, &home, &provider, port));
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if TcpStream::connect(&origin).is_ok() { break; }
        thread::sleep(Duration::from_millis(50));
    }
    let second_body = r#"{"text":"continue after restart","model":"openai/gpt-5.6","reasoning_effort":"high"}"#;
    token = bearer(&home);
    let (second_status, second) = http(&origin, &token, "POST", &format!("/api/sessions/{session}/turns/stream"), second_body);
    assert_eq!(second_status, 201, "restart second turn must succeed, got {:?}", String::from_utf8_lossy(&second));
    daemon.stop();
    let third_request = if second_status == 201 { Some(requests.recv_timeout(TIMEOUT).expect("third provider request after restart")) } else { None };
    server.join().unwrap();
    if let Some(third_request) = third_request { assert_typed_transcript(&third_request, &target, "inspect the disposable fixture"); }
    assert!(followup_request.windows(b"call-human".len()).any(|w| w == b"call-human"), "tool follow-up lost call identity");
    assert_eq!(std::fs::read(&target).unwrap(), before, "RequireHuman write mutated the fixture");
    let _ = std::fs::remove_dir_all(home);
}

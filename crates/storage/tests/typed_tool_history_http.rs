//! DB-022 registered RED: real installed HTTP/provider typed tool-history writer.
//!
//! This deliberately uses the installed binary and a loopback-only provider.  It
//! is not a storage unit test and must not be replaced by inserting typed rows
//! directly: the rows are evidence that the HTTP turn and broker reached the
//! real writer.
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const LIMIT: usize = 256 * 1024;
const DEADLINE: Duration = Duration::from_secs(20);

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct ProviderGuard {
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl Drop for ProviderGuard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn installed_binary() -> PathBuf {
    let path = std::env::var_os("OC2_TEST_BINARY")
        .map(PathBuf::from)
        .expect("OC2_TEST_BINARY must name the installed fixture binary; refusing a skip");
    assert!(path.is_file(), "installed binary missing: {}", path.display());
    path
}

fn read_http(stream: &mut TcpStream) -> Vec<u8> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut result = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut expected = None;
    while result.len() <= LIMIT {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 { break; }
        result.extend_from_slice(&chunk[..n]);
        if let Some(end) = result.windows(4).position(|w| w == b"\r\n\r\n") {
            if expected.is_none() {
                let headers = String::from_utf8_lossy(&result[..end]);
                expected = headers.lines().find_map(|line| {
                    line.to_ascii_lowercase().strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .map(|n| end + 4 + n)
                });
            }
            if expected.is_some_and(|n| result.len() >= n) { break; }
        }
    }
    assert!(result.len() <= LIMIT, "fixture HTTP body exceeded bound");
    result
}

fn sse(events: &[String]) -> Vec<u8> {
    let body = events.concat();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(), body
    ).into_bytes()
}

fn provider_round(call_ids: &[(&str, &str, &str)], output: &str) -> Vec<u8> {
    let mut events = Vec::new();
    for (id, name, args) in call_ids {
        let item = json!({
            "type": "response.output_item.done",
            "item": {"type":"function_call", "id":format!("fc-{id}"),
              "call_id":id, "name":name, "arguments":args}
        });
        events.push(format!("event: response.output_item.done\ndata: {item}\n\n"));
    }
    let completed = json!({"type":"response.completed", "response":{"id":"fixture-response","status":"completed"}});
    events.push(format!("event: response.completed\ndata: {completed}\n\n"));
    if !output.is_empty() {
        events.push(format!("event: response.output_text.delta\ndata: {}\n\n", json!({"delta":output})));
    }
    sse(&events)
}

fn spawn_provider(rounds: Vec<Vec<u8>>) -> (String, Receiver<Vec<u8>>, ProviderGuard) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let thread_stopping = Arc::clone(&stopping);
    let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::sync_channel(4);
    let join = thread::spawn(move || {
        for response in rounds {
            let deadline = Instant::now() + DEADLINE;
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
                        && !thread_stopping.load(Ordering::Acquire)
                        && Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) if thread_stopping.load(Ordering::Acquire) => return,
                    Err(error) => panic!("bounded provider accept failed: {error}"),
                }
            };
            let request = read_http(&mut stream);
            tx.send(request).expect("provider capture receiver");
            stream.write_all(&response).expect("provider response");
        }
    });
    (format!("http://{address}/v1"), rx, ProviderGuard { stopping, join: Some(join) })
}

fn request(origin: &str, token: &str, method: &str, path: &str, body: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(origin).expect("connect daemon");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let wire = format!(
        "{method} {path} HTTP/1.1\r\nhost: localhost\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(wire.as_bytes()).unwrap();
    let response = read_http(&mut stream);
    let status = String::from_utf8_lossy(&response).split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, response)
}

fn wait_token(home: &Path) -> String {
    let path = home.join("runtime/backend.json");
    let deadline = Instant::now() + DEADLINE;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if let Some(token) = value.get("auth_token").and_then(Value::as_str) {
                    return token.to_owned();
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon did not publish bounded auth descriptor");
}

fn json_body(response: &[u8]) -> Value {
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    serde_json::from_slice(&response[split + 4..]).unwrap()
}

fn find_state_db(root: &Path) -> PathBuf {
    // The installed daemon owns this data root; do not inspect any host DB.
    for candidate in [root.join("state.db"), root.join("runtime/state.db"), root.join("data/state.db")] {
        if candidate.is_file() { return candidate; }
    }
    panic!("installed fixture did not create a disposable state.db under {}", root.display())
}

#[test]
fn installed_http_tool_writer_preserves_typed_round_and_rolls_back_failed_pair() {
    let binary = installed_binary();
    let home = TempDir::new().unwrap();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let target = project.join("typed-output.txt");
    let args = json!({"path":target,"content":"fixture\u{0}unicode"}).to_string();
    let first = provider_round(&[("call-α", "write", &args)], "first-output");
    let first_followup = provider_round(&[], "followup-output");
    let fault_args = json!({"path":target,"content":"fault"}).to_string();
    let second = provider_round(&[("fault-call", "write", &fault_args)], "fault-output");
    let (provider, requests, _provider_guard) = spawn_provider(vec![first, first_followup, second]);
    let child = Command::new(binary)
        .env_clear().env("HOME", home.path()).env("PATH", "/usr/bin:/bin")
        .env("LANG", "C").env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_TURN_TOOLS", "write")
        .env("OPENAI_API_KEY", "fixture-key").env("OPENAI_BASE_URL", provider)
        .arg("serve").arg("--listen").arg("127.0.0.1:0")
        .current_dir(&project).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("spawn installed oc2 serve");
    let _daemon = ChildGuard(Some(child));
    let token = wait_token(home.path());
    let descriptor = std::fs::read_to_string(home.path().join("runtime/backend.json"))
        .expect("fixture daemon descriptor");
    let descriptor: Value = serde_json::from_str(&descriptor).expect("valid daemon descriptor");
    let origin = descriptor.get("http_origin").and_then(Value::as_str)
        .and_then(|origin| origin.strip_prefix("http://"))
        .map(str::to_owned).expect("fixture daemon loopback origin");
    let (status, created) = request(&origin, &token, "POST", "/api/sessions", r#"{"title":"DB-022"}"#);
    assert_eq!(status, 201, "session creation: {}", String::from_utf8_lossy(&created));
    let session = json_body(&created)["session"]["id"].as_str().unwrap().to_owned();
    let body = r#"{"text":"perform typed fixture","model":"openai/gpt-5.6","reasoning_effort":"high"}"#;
    let (status, response) = request(&origin, &token, "POST", &format!("/api/sessions/{session}/turns/stream"), body);
    assert_eq!(status, 201, "turn failed: {}", String::from_utf8_lossy(&response));
    let provider_request = requests.recv_timeout(DEADLINE).expect("real provider request");
    assert!(!provider_request.is_empty(), "initial provider request was not captured");

    let db = find_state_db(home.path());
    let connection = Connection::open(db).unwrap();
    let tables: String = connection.query_row(
        "SELECT group_concat(name, ',') FROM sqlite_master WHERE type='table' AND name IN ('tool_rounds','typed_tool_records')",
        [], |row| row.get(0)).unwrap_or_default();
    assert!(tables.contains("tool_rounds") && tables.contains("typed_tool_records"), "DB-022 RED: typed schema is absent; DB-021/DB-019 must add it before rollback phase");
    let (call_id, name, payload, byte_len): (String, String, String, i64) = connection.query_row(
        "SELECT call_id,name,payload,byte_len FROM typed_tool_records WHERE kind='call' ORDER BY pair_index LIMIT 1", [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
    assert_eq!(call_id, "call-α");
    assert_eq!(name, "write");
    assert_eq!(payload, args);
    assert_eq!(byte_len, payload.len() as i64);
    let (output_id, output_name, output_payload, output_len): (String, String, String, i64) = connection.query_row(
        "SELECT call_id,name,payload,byte_len FROM typed_tool_records WHERE kind='output' ORDER BY pair_index LIMIT 1", [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
    assert_eq!(output_id, "call-α");
    assert_eq!(output_name, "write");
    assert!(output_payload.contains("fixture"), "delivered write output was not retained exactly: {output_payload:?}");
    assert_eq!(output_len, output_payload.len() as i64);
    let count: i64 = connection.query_row("SELECT count(*) FROM typed_tool_records", [], |row| row.get(0)).unwrap();
    let first_tool_messages: i64 = connection.query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |row| row.get(0)).unwrap();
    assert_eq!(first_tool_messages, 1, "first typed pair must retain its ordinary Tool message");

    connection.execute_batch("CREATE TRIGGER db022_abort BEFORE INSERT ON typed_tool_records BEGIN SELECT RAISE(ABORT, 'DB-022 fixture fault'); END;").unwrap();
    let (failed_status, _) = request(&origin, &token, "POST", &format!("/api/sessions/{session}/turns/stream"), body);
    assert_ne!(failed_status, 201, "faulted typed write must fail closed");
    let after: i64 = connection.query_row("SELECT count(*) FROM typed_tool_records", [], |row| row.get(0)).unwrap();
    assert_eq!(after, count, "failed pair leaked typed rows");
    let second_request = requests.recv_timeout(DEADLINE).expect("faulted turn's initial provider request");
    assert!(!second_request.is_empty(), "faulted turn's initial provider request was not accounted for");
    assert!(requests.try_recv().is_err(), "provider received a request after the deterministic fault request");
    let second_tool_messages: i64 = connection.query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |row| row.get(0)).unwrap();
    assert_eq!(second_tool_messages, first_tool_messages, "failed pair leaked ordinary Tool message");
}

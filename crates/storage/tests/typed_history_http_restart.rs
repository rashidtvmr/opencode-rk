//! G4 RED contract: real HTTP turns persist typed tool history across restart.
//!
//! The positive path never inserts typed rows directly. SQLite is inspected only
//! after the installed daemon has executed the provider function call through its
//! broker/tool path. All processes, sockets, and databases are disposable.
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
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
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl Drop for ProviderGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn binary() -> PathBuf {
    let path = std::env::var_os("OC2_TEST_BINARY")
        .map(PathBuf::from)
        .expect("OC2_TEST_BINARY must name the installed fixture binary");
    assert!(
        path.is_file(),
        "installed binary missing: {}",
        path.display()
    );
    path
}

fn read_http(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut end = None;
    let mut chunked = false;
    while bytes.len() <= LIMIT {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if end.is_none() {
            if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..pos]);
                chunked = headers.lines().any(|line| {
                    line.to_ascii_lowercase()
                        .contains("transfer-encoding: chunked")
                });
                let length = headers.lines().find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                });
                end = length.map(|n| pos + 4 + n);
            }
        }
        if chunked && bytes.windows(7).any(|w| w == b"\r\n0\r\n\r\n") {
            break;
        }
        if end.map_or(false, |n| bytes.len() >= n) {
            break;
        }
    }
    assert!(bytes.len() <= LIMIT, "HTTP wire exceeded bound");
    bytes
}

fn sse(items: Vec<Value>) -> Vec<u8> {
    let mut body = String::new();
    for item in items {
        body.push_str(&format!(
            "event: response.output_item.done\ndata: {}\n\n",
            item
        ));
    }
    body.push_str("event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"fixture\",\"status\":\"completed\"}}\n\n");
    body.push_str("event: response.output_text.delta\ndata: {\"delta\":\"fixture-output\"}\n\n");
    format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).into_bytes()
}

fn round(id: &str, name: &str, args: &str) -> Vec<u8> {
    sse(vec![
        json!({"type":"response.output_item.done", "item":{"type":"function_call", "id":format!("fc-{id}"), "call_id":id, "name":name, "arguments":args}}),
    ])
}

fn provider(responses: Vec<Vec<u8>>) -> (String, Receiver<Vec<u8>>, ProviderGuard) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::sync_channel(8);
    let join = thread::spawn(move || {
        for response in responses {
            let deadline = Instant::now() + DEADLINE;
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        stream
                            .set_nonblocking(false)
                            .expect("accepted provider stream must be blocking");
                        break (stream, peer);
                    }
                    Err(error)
                        if (error.kind() == std::io::ErrorKind::WouldBlock
                            || error.raw_os_error() == Some(35))
                            && !thread_stop.load(Ordering::Acquire)
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) if thread_stop.load(Ordering::Acquire) => return,
                    Err(_error) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("bounded provider accept failed: {error}"),
                }
            };
            let request = read_http(&mut stream);
            assert!(
                !request.is_empty(),
                "provider accepted an empty request; refusing to consume its response"
            );
            tx.send(request).unwrap();
            stream.write_all(&response).unwrap();
        }
    });
    (
        format!("http://{address}/v1"),
        rx,
        ProviderGuard {
            stop,
            join: Some(join),
        },
    )
}

fn spawn(binary: &Path, home: &Path, project: &Path, provider: &str) -> ChildGuard {
    let child = Command::new(binary)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("OPENCODE_RK_HOME", home)
        .env("OPENCODE_RK_TURN_TOOLS", "write")
        .env("OPENAI_API_KEY", "fixture-key")
        .env("OPENAI_BASE_URL", provider)
        .arg("serve")
        .arg("--listen")
        .arg("127.0.0.1:0")
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn oc2");
    ChildGuard(Some(child))
}

fn token(home: &Path) -> String {
    let deadline = Instant::now() + DEADLINE;
    let path = home.join("runtime/backend.json");
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if let Some(token) = value.get("auth_token").and_then(Value::as_str) {
                    return token.into();
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("daemon descriptor timeout")
}

fn descriptor(home: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(home.join("runtime/backend.json")).unwrap())
        .unwrap()
}

fn token_after_restart(home: &Path, old_pid: u64) -> String {
    let deadline = Instant::now() + DEADLINE;
    let path = home.join("runtime/backend.json");
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if value
                    .get("pid")
                    .and_then(Value::as_u64)
                    .is_some_and(|pid| pid != old_pid)
                {
                    if let Some(token) = value.get("auth_token").and_then(Value::as_str) {
                        return token.into();
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("new daemon descriptor timeout")
}

fn origin(home: &Path) -> String {
    let value: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join("runtime/backend.json")).unwrap())
            .unwrap();
    value["http_origin"]
        .as_str()
        .unwrap()
        .trim_start_matches("http://")
        .to_owned()
}

fn request(origin: &str, token: &str, method: &str, path: &str, body: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(origin).unwrap();
    let wire = format!("{method} {path} HTTP/1.1\r\nhost: localhost\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    stream.write_all(wire.as_bytes()).unwrap();
    read_http(&mut stream)
}

fn status(response: &[u8]) -> u16 {
    String::from_utf8_lossy(response)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}
fn body(response: &[u8]) -> &[u8] {
    let p = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("HTTP headers");
    &response[p + 4..]
}
fn decoded_body(response: &[u8]) -> Vec<u8> {
    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("HTTP headers");
    let headers = String::from_utf8_lossy(&response[..header_end]).to_ascii_lowercase();
    let raw = body(response);
    if !headers.contains("transfer-encoding: chunked") {
        return raw.to_vec();
    }
    let mut cursor = 0;
    let mut decoded = Vec::new();
    while cursor < raw.len() {
        let line_end = raw[cursor..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .expect("chunk size line")
            + cursor;
        let size = usize::from_str_radix(
            std::str::from_utf8(&raw[cursor..line_end])
                .expect("chunk size UTF-8")
                .trim(),
            16,
        )
        .expect("chunk size");
        cursor = line_end + 2;
        if size == 0 {
            break;
        }
        assert!(
            size <= LIMIT && decoded.len() + size <= LIMIT,
            "decoded HTTP body exceeded bound"
        );
        assert!(cursor + size + 2 <= raw.len(), "truncated HTTP chunk");
        decoded.extend_from_slice(&raw[cursor..cursor + size]);
        cursor += size;
        assert_eq!(&raw[cursor..cursor + 2], b"\r\n", "chunk terminator");
        cursor += 2;
    }
    decoded
}
fn json_body(response: &[u8]) -> Value {
    serde_json::from_slice(&decoded_body(response)).expect("JSON response body")
}
fn request_json(request: &[u8]) -> Value {
    let text = std::str::from_utf8(request).expect("provider request UTF-8");
    let split = text.find("\r\n\r\n").expect("provider request headers");
    serde_json::from_str(&text[split + 4..]).expect("provider request JSON")
}
fn json_text(value: &Value) -> String {
    serde_json::to_string(value).expect("serialize provider JSON")
}
fn input_items(request: &[u8]) -> Vec<Value> {
    request_json(request)
        .get("input")
        .and_then(Value::as_array)
        .cloned()
        .expect("provider input array")
}
fn assert_typed_pair(request: &[u8], call_id: &str, arguments: &str) {
    let items = input_items(request);
    let call = items
        .iter()
        .enumerate()
        .find(|(_, item)| {
            item.get("type").and_then(Value::as_str) == Some("function_call")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("typed function call");
    let output = items
        .iter()
        .enumerate()
        .find(|(_, item)| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("typed function output");
    assert!(call.0 < output.0, "typed call precedes its output");
    assert_eq!(call.1.get("name").and_then(Value::as_str), Some("write"));
    assert_eq!(
        call.1.get("arguments").and_then(Value::as_str),
        Some(arguments)
    );
    assert_eq!(
        output.1.get("output").and_then(Value::as_str),
        Some("write success")
    );
    assert_eq!(
        items
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            )
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            )
            .count(),
        1
    );
}
fn db(home: &Path) -> PathBuf {
    fn visit(dir: &Path) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).ok()? {
            let path = entry.ok()?.path();
            if path.is_dir() {
                if let Some(found) = visit(&path) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "state.db" || n == "database.db")
            {
                return Some(path);
            }
        }
        None
    }
    visit(home).unwrap_or_else(|| panic!("no disposable daemon database under {}", home.display()))
}

#[test]
fn typed_history_http_tool_continuation_second_turn_and_restart() {
    let home = TempDir::new().unwrap();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = std::fs::canonicalize(&project).expect("canonical project");
    let target = project.join("safe.txt");
    let args = json!({"path":target,"content":"safe content"}).to_string();
    let (provider_url, requests, _provider_guard) = provider(vec![
        round("call-1", "write", &args),
        sse(vec![]),
        round("call-2", "write", &args),
        sse(vec![]),
        sse(vec![]),
    ]);
    let bin = binary();
    let mut daemon = spawn(&bin, home.path(), &project, &provider_url);
    let auth = token(home.path());
    let addr = origin(home.path());
    let created = request(&addr, &auth, "POST", "/api/sessions", r#"{"title":"G4"}"#);
    assert_eq!(status(&created), 201);
    let session = json_body(&created)["session"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let turn =
        r#"{"text":"write the safe file","model":"openai/gpt-5.6","reasoning_effort":"high"}"#;
    let first = request(
        &addr,
        &auth,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        turn,
    );
    assert_eq!(
        status(&first),
        201,
        "first turn: {}",
        String::from_utf8_lossy(&first)
    );
    let initial = requests
        .recv_timeout(DEADLINE)
        .expect("initial provider request");
    assert!(!initial.is_empty());
    let initial_json = request_json(&initial);
    assert!(json_text(&initial_json).contains("write the safe file"));
    let continuation = requests.recv_timeout(DEADLINE).unwrap();
    assert_typed_pair(&continuation, "call-1", &args);
    assert_eq!(
        std::fs::read_to_string(&target).expect("safe write output"),
        "safe content"
    );
    let connection = Connection::open(db(home.path())).unwrap();
    let rows: Vec<(String, String, String, String, i64)> = {
        let mut stmt = connection
            .prepare(
                "SELECT kind,call_id,name,payload,byte_len FROM typed_tool_records ORDER BY rowid",
            )
            .expect("typed schema");
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .expect("typed rows query")
        .map(|x| x.expect("typed row"))
        .collect()
    };
    assert_eq!(
        rows,
        vec![
            (
                "call".into(),
                "call-1".into(),
                "write".into(),
                args.clone(),
                args.len() as i64
            ),
            (
                "output".into(),
                "call-1".into(),
                "write".into(),
                "write success".into(),
                "write success".len() as i64
            )
        ]
    );
    let tool_messages: i64 = connection
        .query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |r| {
            r.get(0)
        })
        .expect("tool message count");
    assert_eq!(tool_messages, 1);
    let second = request(
        &addr,
        &auth,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        r#"{"text":"confirm it","model":"openai/gpt-5.6","reasoning_effort":"high"}"#,
    );
    assert_eq!(status(&second), 201);
    let second_initial = requests
        .recv_timeout(DEADLINE)
        .expect("second turn provider request");
    let second_followup = requests
        .recv_timeout(DEADLINE)
        .expect("second turn continuation");
    let second_text = json_text(&request_json(&second_initial));
    assert!(second_text.contains("confirm it"));
    assert_typed_pair(&second_followup, "call-2", &args);
    let old_pid = descriptor(home.path())["pid"].as_u64().unwrap();
    daemon.0.as_mut().unwrap().kill().unwrap();
    daemon.0.as_mut().unwrap().wait().unwrap();
    daemon.0 = None;
    let _restarted = spawn(&bin, home.path(), &project, &provider_url);
    let auth2 = token_after_restart(home.path(), old_pid);
    let addr2 = origin(home.path());
    let resumed = request(
        &addr2,
        &auth2,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        r#"{"text":"after restart","model":"openai/gpt-5.6"}"#,
    );
    assert_eq!(status(&resumed), 201);
    let replay = requests
        .recv_timeout(DEADLINE)
        .expect("restart provider request");
    assert_typed_pair(&replay, "call-1", &args);
    assert_typed_pair(&replay, "call-2", &args);
    assert_eq!(
        input_items(&replay)
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("call_id").and_then(Value::as_str) == Some("call-1")
            )
            .count(),
        1
    );
    assert_eq!(
        input_items(&replay)
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some("call-1")
            )
            .count(),
        1
    );
}

#[test]
fn typed_history_http_fault_is_transactional_and_late_error_is_ndjson() {
    let home = TempDir::new().unwrap();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = std::fs::canonicalize(&project).expect("canonical project");
    let target = project.join("fault.txt");
    let args = json!({"path":target,"content":"fault"}).to_string();
    let (provider_url, requests, _guard) = provider(vec![round("fault-call", "write", &args)]);
    let bin = binary();
    let _daemon = spawn(&bin, home.path(), &project, &provider_url);
    let auth = token(home.path());
    let addr = origin(home.path());
    let created = request(
        &addr,
        &auth,
        "POST",
        "/api/sessions",
        r#"{"title":"fault"}"#,
    );
    assert_eq!(status(&created), 201);
    let session = json_body(&created)["session"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let database = db(home.path());
    let fault_db = Connection::open(&database).unwrap();
    fault_db.execute_batch("CREATE TRIGGER g4_abort BEFORE INSERT ON typed_tool_records BEGIN SELECT RAISE(ABORT, 'G4 deterministic fault'); END;").unwrap();
    drop(fault_db);
    let response = request(
        &addr,
        &auth,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        r#"{"text":"fault","model":"openai/gpt-5.6","reasoning_effort":"high"}"#,
    );
    let first_request = requests
        .recv_timeout(DEADLINE)
        .expect("fault provider request");
    assert!(json_text(&request_json(&first_request)).contains("fault"));
    assert_eq!(
        status(&response),
        201,
        "late stream error must retain 201 headers"
    );
    let decoded_response = decoded_body(&response);
    let response_text = std::str::from_utf8(&decoded_response).expect("NDJSON UTF-8");
    let records: Vec<Value> = response_text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                Some(serde_json::from_str(line).expect("terminal NDJSON record"))
            }
        })
        .collect();
    let final_error = records.last().expect("terminal NDJSON error record");
    assert_eq!(
        final_error.get("type").and_then(Value::as_str),
        Some("error")
    );
    assert_eq!(
        final_error.get("code").and_then(Value::as_str),
        Some("internal_error")
    );
    assert!(
        requests.recv_timeout(Duration::from_millis(100)).is_err(),
        "fault produced provider continuation"
    );
    let connection = Connection::open(database).unwrap();
    let typed: i64 = connection
        .query_row("SELECT count(*) FROM typed_tool_records", [], |r| r.get(0))
        .expect("typed row count");
    let tools: i64 = connection
        .query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |r| {
            r.get(0)
        })
        .expect("tool row count");
    assert_eq!(typed, 0);
    assert_eq!(tools, 0);
    assert!(!target.exists());
}

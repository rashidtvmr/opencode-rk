//! APP-016 supplemental RED: a failed first typed pair in a provider batch must
//! stop dispatch before the second broker-authorized write.
//!
//! This uses the verifier-provided installed daemon, authenticated public HTTP
//! turn API, loopback Responses fixture, and the daemon's own file-backed SQLite
//! database. No Storage/server mocks or fabricated typed rows are used.
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const HTTP_LIMIT: usize = 256 * 1024;
const PROVIDER_LIMIT: usize = 256 * 1024;
const DEADLINE: Duration = Duration::from_secs(20);
const SOCKET_TIMEOUT: Duration = Duration::from_secs(8);

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

/// Own and join the bounded HTTP client worker even if an assertion unwinds.
struct HttpTask(Option<JoinHandle<(u16, Vec<u8>)>>);
impl HttpTask {
    fn join(mut self) -> (u16, Vec<u8>) {
        self.0.take().expect("owned HTTP task").join().expect("HTTP worker panicked")
    }

    fn is_finished(&self) -> bool {
        self.0.as_ref().expect("owned HTTP task").is_finished()
    }
}
impl Drop for HttpTask {
    fn drop(&mut self) {
        if let Some(join) = self.0.take() {
            let _ = join.join();
        }
    }
}

fn installed_binary() -> PathBuf {
    let path = std::env::var_os("OC2_TEST_BINARY")
        .map(PathBuf::from)
        .expect("OC2_TEST_BINARY must name the verifier-provided installed fixture; refusing a skip");
    assert!(path.is_file(), "verifier-provided installed binary is missing");
    path
}

/// Read a bounded HTTP request through its complete Content-Length body.
fn read_provider_request(stream: &mut TcpStream) -> Vec<u8> {
    let deadline = Instant::now() + DEADLINE;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_total = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "provider request exceeded its absolute fixture deadline");
        stream.set_read_timeout(Some(remaining.min(SOCKET_TIMEOUT))).expect("provider read timeout");
        let count = stream.read(&mut buffer).expect("read provider request");
        if count == 0 {
            break;
        }
        assert!(bytes.len() <= PROVIDER_LIMIT.saturating_sub(count), "provider request exceeded fixture bound");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            if expected_total.is_none() {
                let headers = std::str::from_utf8(&bytes[..header_end]).expect("provider headers are ASCII/UTF-8");
                let content_length = headers.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("valid provider Content-Length"))
                }).expect("provider request has Content-Length");
                assert!(content_length <= PROVIDER_LIMIT, "provider request body exceeds fixture bound");
                expected_total = Some(header_end.checked_add(4 + content_length).expect("request length arithmetic"));
            }
            if bytes.len() >= expected_total.expect("header parsed") {
                assert_eq!(bytes.len(), expected_total.expect("header parsed"), "provider sent bytes beyond Content-Length");
                return bytes;
            }
        }
        assert!(bytes.len() <= PROVIDER_LIMIT, "provider request exceeded fixture bound");
    }
    panic!("provider closed before a complete bounded request")
}

/// Provider peer waits for an explicit response permit after reporting each
/// captured request. This lets the test install its SQLite fault trigger before
/// the provider can release the requested tool batch.
fn spawn_provider() -> (String, Receiver<Vec<u8>>, SyncSender<Vec<u8>>, ProviderGuard) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback provider");
    let address = listener.local_addr().expect("provider address");
    listener.set_nonblocking(true).expect("nonblocking provider listener");
    let stopping = Arc::new(AtomicBool::new(false));
    let thread_stopping = Arc::clone(&stopping);
    let (request_tx, request_rx) = mpsc::sync_channel::<Vec<u8>>(8);
    let (response_tx, response_rx) = mpsc::sync_channel::<Vec<u8>>(4);
    let join = thread::spawn(move || {
        let accept_deadline = Instant::now() + DEADLINE;
        loop {
            if thread_stopping.load(Ordering::Acquire) {
                return;
            }
            assert!(Instant::now() < accept_deadline, "provider accept exceeded its absolute fixture deadline");
            let accepted = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("bounded loopback provider accept failed: {error}"),
            };
            let (mut stream, _) = accepted;
            let request = read_provider_request(&mut stream);
            request_tx.send(request).expect("test owns provider request receiver");
            let response = loop {
                if thread_stopping.load(Ordering::Acquire) {
                    return;
                }
                match response_rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(response) => break response,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            };
            stream.write_all(&response).expect("write scripted provider response");
            stream.flush().expect("flush scripted provider response");
        }
    });
    (
        format!("http://{address}/v1"),
        request_rx,
        response_tx,
        ProviderGuard { stopping, join: Some(join) },
    )
}

fn provider_round(calls: &[(&str, &str, &str)], text: &str) -> Vec<u8> {
    let mut events = Vec::new();
    for (call_id, name, arguments) in calls {
        let event = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "id": format!("fixture-item-{call_id}"),
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
            }
        });
        events.push(format!("event: response.output_item.done\ndata: {event}\n\n"));
    }
    if !text.is_empty() {
        let delta = json!({"delta": text});
        events.push(format!("event: response.output_text.delta\ndata: {delta}\n\n"));
    }
    // The pinned Responses parser marks itself exhausted at response.completed;
    // therefore any text delta must precede this terminal provider event.
    let completed = json!({
        "type": "response.completed",
        "response": {"id": "same-batch-fixture", "status": "completed"}
    });
    events.push(format!("event: response.completed\ndata: {completed}\n\n"));
    let body = events.concat();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(), body
    ).into_bytes()
}

fn read_http_response(stream: &mut TcpStream) -> Vec<u8> {
    let deadline = Instant::now() + DEADLINE;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "HTTP response exceeded its absolute fixture deadline");
        stream.set_read_timeout(Some(remaining.min(SOCKET_TIMEOUT))).expect("daemon read timeout");
        let count = stream.read(&mut buffer).expect("read daemon response");
        if count == 0 {
            break;
        }
        assert!(bytes.len() <= HTTP_LIMIT.saturating_sub(count), "daemon response exceeded fixture bound");
        bytes.extend_from_slice(&buffer[..count]);
    }
    bytes
}

fn request(origin: &str, token: &str, method: &str, path: &str, body: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(origin).expect("connect to fixture daemon");
    stream.set_read_timeout(Some(SOCKET_TIMEOUT)).expect("daemon client timeout");
    stream.set_write_timeout(Some(SOCKET_TIMEOUT)).expect("daemon client write timeout");
    let wire = format!(
        "{method} {path} HTTP/1.1\r\nhost: localhost\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(wire.as_bytes()).expect("write daemon request");
    stream.flush().expect("flush daemon request");
    let response = read_http_response(&mut stream);
    let status = std::str::from_utf8(&response)
        .expect("HTTP status/header bytes are UTF-8")
        .split_whitespace()
        .nth(1)
        .expect("HTTP response status")
        .parse::<u16>()
        .expect("numeric HTTP status");
    (status, response)
}

fn spawn_request(origin: String, token: String, path: String, body: String) -> HttpTask {
    HttpTask(Some(thread::spawn(move || request(&origin, &token, "POST", &path, &body))))
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
        thread::sleep(Duration::from_millis(25));
    }
    panic!("fixture daemon did not publish its bounded auth descriptor")
}

fn response_body(response: &[u8]) -> (&str, &[u8]) {
    let split = response.windows(4).position(|window| window == b"\r\n\r\n").expect("HTTP header terminator");
    let headers = std::str::from_utf8(&response[..split]).expect("HTTP headers UTF-8");
    (headers, &response[split + 4..])
}

fn json_body(response: &[u8]) -> Value {
    let (_, body) = response_body(response);
    serde_json::from_slice(body).expect("JSON HTTP response body")
}

/// Decode the HTTP/1.1 chunked body emitted for the stream route, requiring a
/// complete terminator and no trailing bytes. Non-chunked content-length bodies
/// are also supported for a bounded fixture diagnostic.
fn decoded_body(response: &[u8]) -> Vec<u8> {
    let (headers, wire_body) = response_body(response);
    let chunked = headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.split(',').any(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
        })
    });
    if !chunked {
        if let Some(length) = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().expect("valid response Content-Length"))
        }) {
            assert!(length <= HTTP_LIMIT, "response body exceeds fixture bound");
            assert_eq!(wire_body.len(), length, "response Content-Length mismatch");
        }
        return wire_body.to_vec();
    }

    let mut decoded = Vec::new();
    let mut cursor = 0usize;
    loop {
        let line_end = wire_body.get(cursor..)
            .and_then(|tail| tail.windows(2).position(|window| window == b"\r\n"))
            .expect("chunk size line terminator");
        let line = std::str::from_utf8(&wire_body[cursor..cursor + line_end]).expect("ASCII chunk size");
        let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
            .expect("valid chunk size");
        cursor = cursor.checked_add(line_end + 2).expect("chunk cursor arithmetic");
        if size == 0 {
            if wire_body.get(cursor..).is_some_and(|tail| tail.starts_with(b"\r\n")) {
                cursor += 2;
            } else {
                let end = wire_body.get(cursor..)
                    .and_then(|tail| tail.windows(4).position(|window| window == b"\r\n\r\n"))
                    .expect("end of chunk trailers");
                cursor = cursor.checked_add(end + 4).expect("trailer cursor arithmetic");
            }
            assert_eq!(cursor, wire_body.len(), "bytes after terminal HTTP chunk");
            return decoded;
        }
        assert!(decoded.len() <= HTTP_LIMIT.saturating_sub(size), "decoded stream exceeded fixture bound");
        let end = cursor.checked_add(size).expect("chunk length arithmetic");
        let framed_end = end.checked_add(2).expect("chunk framing arithmetic");
        assert!(framed_end <= wire_body.len(), "truncated HTTP chunk");
        assert_eq!(wire_body.get(end..framed_end), Some(&b"\r\n"[..]), "chunk data terminator");
        decoded.extend_from_slice(&wire_body[cursor..end]);
        cursor = framed_end;
    }
}

fn ndjson_events(response: &[u8]) -> Vec<Value> {
    let bytes = decoded_body(response);
    assert!(!bytes.is_empty() && bytes.ends_with(b"\n"), "stream must end in a complete LF-terminated NDJSON record");
    bytes[..bytes.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| {
            assert!(!line.is_empty(), "stream contains an empty NDJSON record");
            serde_json::from_slice(line).expect("valid NDJSON event")
        })
        .collect()
}

fn state_db(root: &Path) -> PathBuf {
    let path = root.join("state.db");
    assert!(path.is_file(), "fixture daemon must create its owned root/state.db");
    path
}

fn assert_loopback_origin(descriptor: &Value) -> String {
    let origin = descriptor.get("http_origin").and_then(Value::as_str).expect("daemon HTTP origin");
    let authority = origin.strip_prefix("http://").expect("plain HTTP loopback fixture");
    assert!(authority.starts_with("127.0.0.1:"), "fixture daemon origin must be loopback");
    authority.to_owned()
}

fn wait_provider_request(requests: &Receiver<Vec<u8>>) -> Vec<u8> {
    let request = requests.recv_timeout(DEADLINE).expect("expected real provider request");
    assert!(!request.is_empty(), "provider request fixture must be nonempty");
    request
}

fn send_provider_response(responses: &SyncSender<Vec<u8>>, response: Vec<u8>) {
    responses.send(response).expect("provider fixture remains owned and ready");
}

#[test]
fn first_pair_persistence_failure_stops_second_call_in_the_same_provider_batch() {
    let binary = installed_binary();
    let home = TempDir::new().expect("disposable fixture root");
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).expect("create synthetic project");

    let control_path = project.join("control-write.txt");
    let a_path = project.join("write-a.txt");
    let b_path = project.join("write-b.txt");
    let control_args = json!({"path": control_path, "content": "positive control effect"}).to_string();
    let a_args = json!({"path": a_path, "content": "effect A completed before persistence"}).to_string();
    let b_args = json!({"path": b_path, "content": "effect B must not run"}).to_string();
    let (provider, provider_requests, provider_responses, _provider_guard) = spawn_provider();

    let child = Command::new(binary)
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_TURN_TOOLS", "write")
        .env("OPENAI_API_KEY", "fixture-only-key")
        .env("OPENAI_BASE_URL", provider)
        .arg("serve")
        .arg("--listen")
        .arg("127.0.0.1:0")
        .current_dir(&project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn verifier-provided installed daemon");
    let _daemon_guard = ChildGuard(Some(child));

    let token = wait_token(home.path());
    let descriptor_text = std::fs::read_to_string(home.path().join("runtime/backend.json"))
        .expect("read disposable daemon descriptor");
    let descriptor: Value = serde_json::from_str(&descriptor_text).expect("decode daemon descriptor");
    let origin = assert_loopback_origin(&descriptor);
    let (status, created) = request(&origin, &token, "POST", "/api/sessions", r#"{"title":"same-batch stop fixture"}"#);
    assert_eq!(status, 201, "create fixture session");
    let session_id = json_body(&created)["session"]["id"].as_str().expect("session id").to_owned();
    let turn_body = r#"{"text":"perform authorized fixture writes","model":"openai/gpt-5.6","reasoning_effort":"high"}"#.to_owned();

    // Positive control: the same installed daemon, policy broker, FileTool,
    // provider protocol, and HTTP stream perform a real authorized write.
    let control_task = spawn_request(
        origin.clone(), token.clone(),
        format!("/api/sessions/{session_id}/turns/stream"), turn_body.clone(),
    );
    wait_provider_request(&provider_requests);
    send_provider_response(&provider_responses, provider_round(&[("control-call", "write", &control_args)], ""));
    wait_provider_request(&provider_requests);
    send_provider_response(&provider_responses, provider_round(&[], "positive control complete"));
    let (control_status, control_response) = control_task.join();
    assert_eq!(control_status, 201, "positive-control stream keeps HTTP 201");
    let control_events = ndjson_events(&control_response);
    assert!(control_events.iter().any(|event| event["type"] == "tool_output" && event["call_id"] == "control-call"));
    let control_assistant = control_events.iter()
        .find(|event| event["type"] == "assistant_message")
        .expect("positive control must reach real assistant completion");
    assert!(control_assistant["message"]["body"]["text"].as_str() == Some("positive control complete"),
        "positive control assistant text did not match the scripted constant");
    assert_eq!(std::fs::read_to_string(&control_path).expect("positive control file effect"), "positive control effect");

    let database = state_db(home.path());
    let connection = Connection::open(&database).expect("open same daemon-owned file-backed state.db");
    connection.busy_timeout(Duration::from_secs(3)).expect("bounded SQLite lock wait");
    let schema_objects: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('tool_rounds','typed_tool_records')",
        [], |row| row.get(0),
    ).expect("inspect actual typed schema");
    assert_eq!(schema_objects, 2,
        "fixture prerequisite: storage-enabled source must provide actual tool_rounds and typed_tool_records; schema absence is not the intended same-batch RED");
    let baseline_tool_messages: i64 = connection.query_row(
        "SELECT count(*) FROM messages WHERE session_id=?1 AND role='tool'",
        [&session_id], |row| row.get(0),
    ).expect("baseline Tool rows");
    let baseline_typed_rows: i64 = connection.query_row(
        "SELECT count(*) FROM typed_tool_records",
        [], |row| row.get(0),
    ).expect("baseline typed rows");

    // A is selected by actual schema columns; B is deliberately not matched.
    // Provider request handoff is gated, so this trigger exists before A or B
    // arguments can be delivered to the daemon.
    connection.execute_batch(
        "CREATE TRIGGER same_batch_abort_a BEFORE INSERT ON typed_tool_records \
         WHEN NEW.call_id='same-batch-A' BEGIN SELECT RAISE(ABORT, 'same-batch A persistence fault'); END;",
    ).expect("install selective first-pair abort trigger on daemon schema");
    let trigger_count: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='same_batch_abort_a'",
        [], |row| row.get(0),
    ).expect("confirm fixture trigger");
    assert_eq!(trigger_count, 1, "fault injection must be active before provider response release");

    let failed_task = spawn_request(
        origin.clone(), token.clone(),
        format!("/api/sessions/{session_id}/turns/stream"), turn_body,
    );
    wait_provider_request(&provider_requests);
    send_provider_response(
        &provider_responses,
        provider_round(&[("same-batch-A", "write", &a_args), ("same-batch-B", "write", &b_args)], ""),
    );

    // A second provider request is not expected after the first typed-pair
    // persistence error. If the legacy untyped path asks for a follow-up, answer
    // it so the client stream can terminate and report the actual outcome rather
    // than hanging the test on a bad implementation.
    let followup_deadline = Instant::now() + DEADLINE;
    let mut observed_followups = 0usize;
    while !failed_task.is_finished() && Instant::now() < followup_deadline {
        match provider_requests.recv_timeout(Duration::from_millis(25)) {
            Ok(request) => {
                assert!(!request.is_empty(), "unexpected follow-up provider request must be captured");
                observed_followups = observed_followups.checked_add(1).expect("bounded follow-up count");
                assert!(observed_followups <= 4, "provider follow-up count exceeds fixture bound");
                // Answer only to drive an incorrect implementation to a
                // terminal event; the nonzero count remains an assertion
                // failure after the owned HTTP request has completed.
                send_provider_response(&provider_responses, provider_round(&[], "unexpected follow-up completed"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("provider fixture disconnected before turn ended"),
        }
    }
    assert!(failed_task.is_finished(), "stream request exceeded bounded turn deadline");
    let (failed_status, failed_response) = failed_task.join();
    assert_eq!(failed_status, 201, "late stream persistence faults retain committed HTTP status");

    // The tool effect precedes typed persistence: A is expected to remain. B is
    // the decisive regression observation and must never be compensated away.
    assert_eq!(std::fs::read_to_string(&a_path).expect("A effect is allowed to remain after its later persistence failure"), "effect A completed before persistence");
    let b_effect = std::fs::read_to_string(&b_path).ok();
    assert!(b_effect.is_none(),
        "B was dispatched after A's selective persistence fault; observed B side effect: {b_effect:?}");

    let events = ndjson_events(&failed_response);
    let a_output_index = events.iter().position(|event| {
        event["type"] == "tool_output" && event["call_id"] == "same-batch-A"
    }).expect("A's output event is emitted before its pair persistence attempt");
    let terminal = events.last().expect("failed turn has terminal stream event");
    assert_eq!(terminal["type"], "error", "persistence failure is terminal stream error");
    assert_eq!(terminal["code"], "internal_error", "typed persistence error code");
    assert!(events.iter().position(|event| event["type"] == "error").is_some_and(|index| index > a_output_index),
        "A output must precede terminal persistence error");
    assert!(!events.iter().any(|event| event["type"] == "tool_output" && event["call_id"] == "same-batch-B"),
        "B must not emit a tool output after A persistence failure");
    assert!(!events.iter().any(|event| event["type"] == "assistant_message"),
        "failed tool-pair persistence is not successful assistant completion");
    assert_eq!(observed_followups, 0, "provider received a later request after A pair persistence failure");
    assert!(matches!(provider_requests.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "no provider request may arrive after the failed turn body terminates");

    let tool_messages_after: i64 = connection.query_row(
        "SELECT count(*) FROM messages WHERE session_id=?1 AND role='tool'",
        [&session_id], |row| row.get(0),
    ).expect("post-fault Tool row count");
    let typed_rows_after: i64 = connection.query_row(
        "SELECT count(*) FROM typed_tool_records",
        [], |row| row.get(0),
    ).expect("post-fault typed row count");
    assert_eq!(tool_messages_after, baseline_tool_messages,
        "failed A pair's Tool message must roll back atomically");
    assert_eq!(typed_rows_after, baseline_typed_rows,
        "failed A typed call/output records must roll back atomically");
    let leaked_batch_rows: i64 = connection.query_row(
        "SELECT count(*) FROM typed_tool_records WHERE call_id IN ('same-batch-A','same-batch-B')",
        [], |row| row.get(0),
    ).expect("query batch typed records");
    assert_eq!(leaked_batch_rows, 0, "failed pair or undispatched B must have no typed records");
    drop(connection);
}

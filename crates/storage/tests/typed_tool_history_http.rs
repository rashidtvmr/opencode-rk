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
    let mut result = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut expected = None;
    while result.len() <= LIMIT {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        result.extend_from_slice(&chunk[..n]);
        if let Some(end) = result.windows(4).position(|w| w == b"\r\n\r\n") {
            if expected.is_none() {
                let headers = String::from_utf8_lossy(&result[..end]);
                expected = headers.lines().find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .map(|n| end + 4 + n)
                });
            }
            if expected.is_some_and(|n| result.len() >= n) {
                break;
            }
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
        events.push(format!(
            "event: response.output_item.done\ndata: {item}\n\n"
        ));
    }
    let completed = json!({"type":"response.completed", "response":{"id":"fixture-response","status":"completed"}});
    events.push(format!("event: response.completed\ndata: {completed}\n\n"));
    if !output.is_empty() {
        events.push(format!(
            "event: response.output_text.delta\ndata: {}\n\n",
            json!({"delta":output})
        ));
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
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && !thread_stopping.load(Ordering::Acquire)
                            && Instant::now() < deadline =>
                    {
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
    (
        format!("http://{address}/v1"),
        rx,
        ProviderGuard {
            stopping,
            join: Some(join),
        },
    )
}

fn request(origin: &str, token: &str, method: &str, path: &str, body: &str) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(origin).expect("connect daemon");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let wire = format!(
        "{method} {path} HTTP/1.1\r\nhost: localhost\r\nauthorization: Bearer {token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(wire.as_bytes()).unwrap();
    let response = read_http(&mut stream);
    let status = String::from_utf8_lossy(&response)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
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

fn provider_json(request: &[u8]) -> Value {
    let split = request
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("provider request headers");
    serde_json::from_slice(&request[split + 4..]).expect("provider request JSON")
}

fn assert_typed_pair(request: &[u8], call_id: &str, arguments: &str) {
    let provider = provider_json(request);
    let input = provider
        .get("input")
        .and_then(Value::as_array)
        .expect("provider input array");
    let call = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("typed function call");
    let output = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("typed function output");
    assert!(call < output, "typed call precedes its output");
    assert_eq!(
        input[call].get("name").and_then(Value::as_str),
        Some("write")
    );
    assert_eq!(
        input[call].get("arguments").and_then(Value::as_str),
        Some(arguments)
    );
    assert_eq!(
        input[output].get("output").and_then(Value::as_str),
        Some("write success")
    );
    assert_eq!(
        input
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            )
            .count(),
        1
    );
    assert_eq!(
        input
            .iter()
            .filter(
                |item| item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
            )
            .count(),
        1
    );
}

fn decoded_chunked_body(response: &[u8]) -> Vec<u8> {
    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("HTTP headers");
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let wire = &response[header_end + 4..];
    if !headers.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
        })
    }) {
        return wire.to_vec();
    }
    let mut decoded = Vec::new();
    let mut cursor = 0usize;
    loop {
        let line_end = wire
            .get(cursor..)
            .and_then(|rest| rest.windows(2).position(|w| w == b"\r\n"))
            .map(|end| cursor + end)
            .expect("chunk size CRLF");
        let size_line = std::str::from_utf8(&wire[cursor..line_end])
            .expect("chunk size UTF-8");
        // Strip optional chunk extensions (e.g. ";ext=val") per RFC 7230 §4.1.
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .unwrap_or_else(|_| panic!("invalid chunk size {size_text:?}"));
        cursor = cursor
            .checked_add(line_end - cursor + 2)
            .expect("chunk cursor overflow");
        if size == 0 {
            // Zero-length chunk terminates the body. Consume optional trailers:
            // either a bare CRLF (no trailers) or trailer fields terminated by
            // a final CRLFCRLF sequence.
            if wire.get(cursor..).is_some_and(|rest| rest.starts_with(b"\r\n")) {
                cursor += 2;
            } else {
                let trailer_end = wire
                    .get(cursor..)
                    .and_then(|rest| rest.windows(4).position(|w| w == b"\r\n\r\n"))
                    .expect("missing end of chunk trailers after terminal zero-chunk");
                cursor = cursor
                    .checked_add(trailer_end + 4)
                    .expect("trailer cursor overflow");
            }
            // After terminal chunk + optional trailers, no bytes may follow.
            assert_eq!(
                cursor,
                wire.len(),
                "trailing bytes after terminal HTTP chunk framing"
            );
            return decoded;
        }
        let end = cursor.checked_add(size).expect("chunk length overflow");
        let framed_end = end.checked_add(2).expect("chunk framing overflow");
        assert!(
            framed_end <= wire.len(),
            "truncated chunk data or missing chunk CRLF"
        );
        assert_eq!(&wire[end..framed_end], b"\r\n", "chunk terminator");
        decoded.extend_from_slice(&wire[cursor..end]);
        cursor = framed_end;
        assert!(decoded.len() <= LIMIT, "decoded HTTP body exceeded bound");
    }
}

/// Focused helper-level unit cases for chunked decoder framing contracts.
/// These exercise the `decoded_chunked_body` parser directly with synthetic
/// HTTP responses; they do not start a server or daemon.
#[cfg(test)]
mod chunked_decoder_framing {
    use super::*;

    fn fake_chunked_http_response(body_wire: &[u8]) -> Vec<u8> {
        let mut resp = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
        resp.extend_from_slice(body_wire);
        resp
    }

    #[test]
    fn valid_terminal_chunk_with_no_trailers() {
        let wire = fake_chunked_http_response(b"5\r\nhello\r\n0\r\n\r\n");
        let decoded = decoded_chunked_body(&wire);
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn valid_terminal_chunk_with_optional_trailers() {
        let wire =
            fake_chunked_http_response(b"5\r\nhello\r\n0\r\nX-Trailer: val\r\n\r\n");
        let decoded = decoded_chunked_body(&wire);
        assert_eq!(decoded, b"hello");
    }

    #[test]
    #[should_panic(expected = "trailing bytes after terminal HTTP chunk framing")]
    fn reject_trailing_bytes_after_terminal_chunk() {
        let wire = fake_chunked_http_response(b"5\r\nhello\r\n0\r\n\r\ngarbage");
        let _ = decoded_chunked_body(&wire);
    }

    #[test]
    #[should_panic(expected = "truncated chunk data or missing chunk CRLF")]
    fn reject_truncated_terminal_chunk_data() {
        // Claims 5 bytes but only provides 3 before EOF.
        let wire = fake_chunked_http_response(b"5\r\nhel");
        let _ = decoded_chunked_body(&wire);
    }

    #[test]
    #[should_panic(expected = "trailing bytes after terminal HTTP chunk framing")]
    fn reject_trailing_garbage_after_trailers() {
        let wire = fake_chunked_http_response(
            b"5\r\nhello\r\n0\r\nX-Trailer: v\r\n\r\nextra",
        );
        let _ = decoded_chunked_body(&wire);
    }

    #[test]
    fn valid_chunk_extensions_are_stripped_before_size_parse() {
        // RFC 7230 §4.1: chunk-size may be followed by optional chunk-ext.
        let wire = fake_chunked_http_response(b"5;ext=val\r\nhello\r\n0\r\n\r\n");
        let decoded = decoded_chunked_body(&wire);
        assert_eq!(decoded, b"hello");
    }

    #[test]
    #[should_panic(expected = "missing end of chunk trailers after terminal zero-chunk")]
    fn reject_truncated_zero_terminal_missing_trailer_termination() {
        // Zero-chunk present but no trailer-terminating CRLFCRLF and no bare CRLF.
        let wire = fake_chunked_http_response(b"5\r\nhello\r\n0\r\nX-Trailer: incomplete");
        let _ = decoded_chunked_body(&wire);
    }
}

fn find_state_db(root: &Path) -> PathBuf {
    // The installed daemon owns this data root; do not inspect any host DB.
    for candidate in [
        root.join("state.db"),
        root.join("runtime/state.db"),
        root.join("data/state.db"),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "installed fixture did not create a disposable state.db under {}",
        root.display()
    )
}

#[test]
fn installed_http_tool_writer_preserves_typed_round_and_rolls_back_failed_pair() {
    let binary = installed_binary();
    let home = TempDir::new().unwrap();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = std::fs::canonicalize(&project).expect("canonical disposable project");
    let target = project.join("typed-output.txt");
    let args = json!({"path":target,"content":"fixture\u{0}unicode"}).to_string();
    let first = provider_round(&[("call-α", "write", &args)], "first-output");
    let first_followup = provider_round(&[], "followup-output");
    let fault_target = project.join("fault-output.txt");
    let fault_args = json!({"path":fault_target,"content":"fault"}).to_string();
    let second = provider_round(&[("fault-call", "write", &fault_args)], "fault-output");
    let (provider, requests, _provider_guard) = spawn_provider(vec![first, first_followup, second]);
    let child = Command::new(binary)
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("OPENCODE_RK_HOME", home.path())
        .env("OPENCODE_RK_TURN_TOOLS", "write")
        .env("OPENAI_API_KEY", "fixture-key")
        .env("OPENAI_BASE_URL", provider)
        .arg("serve")
        .arg("--listen")
        .arg("127.0.0.1:0")
        .current_dir(&project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn installed oc2 serve");
    let _daemon = ChildGuard(Some(child));
    let token = wait_token(home.path());
    let descriptor = std::fs::read_to_string(home.path().join("runtime/backend.json"))
        .expect("fixture daemon descriptor");
    let descriptor: Value = serde_json::from_str(&descriptor).expect("valid daemon descriptor");
    let origin = descriptor
        .get("http_origin")
        .and_then(Value::as_str)
        .and_then(|origin| origin.strip_prefix("http://"))
        .map(str::to_owned)
        .expect("fixture daemon loopback origin");
    let (status, created) = request(
        &origin,
        &token,
        "POST",
        "/api/sessions",
        r#"{"title":"DB-022"}"#,
    );
    assert_eq!(
        status,
        201,
        "session creation: {}",
        String::from_utf8_lossy(&created)
    );
    let session = json_body(&created)["session"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let body =
        r#"{"text":"perform typed fixture","model":"openai/gpt-5.6","reasoning_effort":"high"}"#;
    let (status, response) = request(
        &origin,
        &token,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        body,
    );
    assert_eq!(
        status,
        201,
        "turn failed: {}",
        String::from_utf8_lossy(&response)
    );
    let provider_request = requests
        .recv_timeout(DEADLINE)
        .expect("real provider request");
    assert!(
        !provider_request.is_empty(),
        "initial provider request was not captured"
    );
    let first_followup_request = requests
        .recv_timeout(DEADLINE)
        .expect("first turn provider continuation");
    assert_typed_pair(&first_followup_request, "call-α", &args);

    let db = find_state_db(home.path());
    let connection = Connection::open(db).unwrap();
    let tables: String = connection.query_row(
        "SELECT group_concat(name, ',') FROM sqlite_master WHERE type='table' AND name IN ('tool_rounds','typed_tool_records')",
        [], |row| row.get(0)).unwrap_or_default();
    assert!(
        tables.contains("tool_rounds") && tables.contains("typed_tool_records"),
        "DB-022 RED: typed schema is absent; DB-021/DB-019 must add it before rollback phase"
    );
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
    assert_eq!(
        output_payload, "write success",
        "delivered write output must not disclose path/content"
    );
    assert_eq!(output_len, output_payload.len() as i64);
    let count: i64 = connection
        .query_row("SELECT count(*) FROM typed_tool_records", [], |row| {
            row.get(0)
        })
        .unwrap();
    let first_tool_messages: i64 = connection
        .query_row(
            "SELECT count(*) FROM messages WHERE role='tool'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        first_tool_messages, 1,
        "first typed pair must retain its ordinary Tool message"
    );

    connection.execute_batch("CREATE TRIGGER db022_abort BEFORE INSERT ON typed_tool_records BEGIN SELECT RAISE(ABORT, 'DB-022 fixture fault'); END;").unwrap();
    let (failed_status, failed_response) = request(
        &origin,
        &token,
        "POST",
        &format!("/api/sessions/{session}/turns/stream"),
        body,
    );
    assert_eq!(
        failed_status, 201,
        "streaming late persistence faults retain committed HTTP status"
    );
    // Original tip predicate: the real installed daemon's failed stream response
    // must use chunked transfer-encoding. Assert on the actual HTTP headers before
    // decoding, independent of the permissive helper fallback for synthetic fixtures.
    let failed_head_end = failed_response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("faulted stream HTTP headers");
    let failed_headers = String::from_utf8_lossy(&failed_response[..failed_head_end]);
    let failed_is_chunked = failed_headers.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("transfer-encoding")
                    && value
                        .split(',')
                        .any(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
            })
    });
    assert!(
        failed_is_chunked,
        "real installed failed stream must use chunked transfer-encoding: {failed_headers:?}"
    );
    let failed_body = decoded_chunked_body(&failed_response);
    assert!(
        !failed_body.is_empty() && failed_body.ends_with(b"\n"),
        "faulted stream must contain complete NDJSON records"
    );
    let events: Vec<Value> = failed_body[..failed_body.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice(line).expect("faulted stream contains valid NDJSON"))
        .collect();
    assert!(
        !events.is_empty(),
        "faulted stream must contain at least one NDJSON event"
    );
    assert!(
        events
            .iter()
            .all(|event| event.get("type").and_then(Value::as_str).is_some()),
        "every parsed NDJSON event must carry a string type field: {events:?}"
    );
    let error_event = events.last().expect("nonempty event list checked above");
    assert_eq!(
        error_event.get("type").and_then(Value::as_str),
        Some("error")
    );
    assert_eq!(
        error_event.get("code").and_then(Value::as_str),
        Some("internal_error")
    );
    assert!(error_event
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|message| !message.is_empty()));
    let after: i64 = connection
        .query_row("SELECT count(*) FROM typed_tool_records", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(after, count, "failed pair leaked typed rows");
    let second_request = requests
        .recv_timeout(DEADLINE)
        .expect("faulted turn's initial provider request");
    assert!(
        !second_request.is_empty(),
        "faulted turn's initial provider request was not accounted for"
    );
    assert!(
        requests.try_recv().is_err(),
        "provider received a request after the deterministic fault request"
    );
    let second_tool_messages: i64 = connection
        .query_row(
            "SELECT count(*) FROM messages WHERE role='tool'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        second_tool_messages, first_tool_messages,
        "failed pair leaked ordinary Tool message"
    );
    assert!(
        !fault_target.exists(),
        "failed pair produced a file side effect"
    );
}

//! Bounded automatic transient retries on the initial Responses HTTP request.
//!
//! Contract under test (Phase-1 missing requirement "eligible transient
//! exponential auto retries"):
//! - HTTP 429 and 503 responses on the initial request are retried with a
//!   bounded deterministic exponential backoff (200ms, 400ms, cap 3 attempts);
//! - every retry sends a byte-identical request payload because nothing —
//!   no stream, no tool call, no output — became observable between attempts;
//! - terminal client errors (HTTP 400) are never retried;
//! - once a 2xx response carries usable headers, any later failure inside the
//!   stream is a stream error, never a retried request.

use std::sync::Arc;
use std::time::{Duration, Instant};

use opencode_rk_providers::{
    config::ProviderConfig,
    responses::{OpenAiResponsesClient, ResponsesInput, ResponsesItem, ResponsesRole, ResponsesTool},
};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex as AsyncMutex;

/// One request body exactly as the fixture server received it, plus the
/// arrival time used to prove backoff ordering.
/// Owned by the spawned server task: the listener lives inside the task, so
/// the test only ever touches the address, the channel of bodies and the
/// handle used to keep the server alive for the whole test.
struct Fixture {
    address: String,
    bodies: Arc<AsyncMutex<Vec<(String, Instant)>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn requests(&self) -> Vec<(String, Instant)> {
        self.bodies.lock().await.clone()
    }
}

/// Serial accept loop. Attempts are served in the canned order; extra
/// connections receive an immediate empty close (never a 2xx), so an
/// over-eager retry shows up as a failed read, not a success.
async fn spawn_fixture(canned: Vec<Vec<u8>>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let bodies: Arc<AsyncMutex<Vec<(String, Instant)>>> = Arc::default();
    let task_bodies = Arc::clone(&bodies);
    let server = tokio::spawn(async move {
        let canned = canned.into_iter();
        for response in canned {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let body = read_request(&mut socket).await;
            let bodies = Arc::clone(&task_bodies);
            let arrived = Instant::now();
            bodies.lock().await.push((body, arrived));
            if socket.write_all(&response).await.is_err() {
                break;
            }
            let _ = socket.shutdown().await;
        }
    });
    Fixture { address, bodies, server }
}

/// Bounded request-body read: enough headers to find `Content-Length`, then
/// exactly that many body bytes. No polling sleep — one async `read` at a
/// time on a connected socket (never `WouldBlock` on a fresh `TcpStream`).
async fn read_request(socket: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut header_end = None;
    loop {
        let read = match tokio::io::AsyncReadExt::read(socket, &mut chunk).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(_) => break,
        };
        raw.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&raw);
        if header_end.is_none() {
            header_end = text.find("\r\n\r\n").map(|position| position + 4);
        }
        if let Some(header_end) = header_end {
            let headers = text[..header_end].to_ascii_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if raw.len() >= header_end + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&raw).into_owned()
}

/// JSON body of the recorded request (after the blank header line).
fn request_body(record: &str) -> String {
    match record.find("\r\n\r\n") {
        Some(position) => record[position + 4..].to_string(),
        None => String::new(),
    }
}

fn error_response(status_line: &str, retry_after: Option<&str>, message: &str) -> Vec<u8> {
    let body = format!("{{\"error\":{{\"message\":\"{message}\"}}}}");
    let retry_after = retry_after
        .map(|value| format!("Retry-After: {value}\r\n"))
        .unwrap_or_default();
    format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\n{retry_after}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// `stream: true` success: a completed SSE stream with usage.
fn stream_completed_response() -> Vec<u8> {
    let body = "event: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":2}}}\r\n\r\n";
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// `stream: true` failure *after* usable headers: the response failed event
/// arrives mid-stream, so recovery is a stream error, never a request retry.
fn stream_failed_response() -> Vec<u8> {
    let body = "event: response.failed\r\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"stream aborted\"}}}\r\n\r\n";
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// Non-streaming (`buffered`) success: a completed Responses envelope.
fn buffered_completed_response() -> Vec<u8> {
    let body = "{\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"recovered after retries\"}]}]}";
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn fixture_client(base_url: String) -> OpenAiResponsesClient {
    OpenAiResponsesClient::from_config(
        ProviderConfig {
            provider_id: "fixture".into(),
            base_url,
            api_key_env: "UNUSED_FIXTURE_KEY".into(),
            timeout_secs: 3,
            max_tokens: 256,
            temperature: 0.7,
        },
        None,
    )
    .unwrap()
}

fn buffered_usage_response(usage: Option<&str>) -> Vec<u8> {
    let body = match usage {
        Some(usage) => format!(
            "{{\"status\":\"completed\",\"usage\":{},\"output\":[{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"fixture answer\"}}]}}]}}",
            usage
        ),
        None => "{\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"fixture answer\"}]}]}".to_owned(),
    };
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[tokio::test(flavor = "current_thread")]
async fn nonstreaming_usage_api_preserves_inclusive_input_and_unknown_subsets() {
    let fixture = spawn_fixture(vec![
        buffered_usage_response(Some("{\"input_tokens\":125000,\"output_tokens\":32,\"input_tokens_details\":{\"cached_tokens\":120000}}")),
        buffered_usage_response(None),
    ])
    .await;
    let client = fixture_client(format!("http://{}", fixture.address));
    let items = vec![ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "hello".into(),
    }];

    let reported = client
        .create_with_items_and_usage("fixture-model", "", &items)
        .await
        .unwrap();
    assert_eq!(reported.text, "fixture answer");
    let usage = reported.usage.unwrap();
    assert_eq!(usage.input_tokens, 125000);
    assert_eq!(usage.output_tokens, 32);
    assert_eq!(usage.cached_tokens, Some(120000));
    assert_eq!(usage.reasoning_tokens, None);

    let unreported = client
        .create_with_items_and_usage("fixture-model", "", &items)
        .await
        .unwrap();
    assert_eq!(unreported.text, "fixture answer");
    assert_eq!(unreported.usage, None);
    wait_for_requests(&fixture.bodies, 2).await;
    fixture.server.abort();
}

fn fixture_turn() -> (Vec<ResponsesItem>, Vec<ResponsesTool>) {
    let items = vec![
        ResponsesItem::Text {
            role: ResponsesRole::System,
            content: "You are a retry fixture.".into(),
        },
        ResponsesItem::Text {
            role: ResponsesRole::User,
            content: "turn input".into(),
        },
    ];
    let tools = vec![ResponsesTool::function(
        "fixture_tool",
        "fixture-only tool",
        serde_json::json!({"type":"object","properties":{}}),
    )];
    (items, tools)
}

/// Poll until the fixture has recorded `count` request bodies.
async fn wait_for_requests(bodies: &Arc<AsyncMutex<Vec<(String, Instant)>>>, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if bodies.lock().await.len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("fixture recorded {} of {count} requests", bodies.lock().await.len());
}

#[tokio::test(flavor = "current_thread")]
async fn transient_429_then_503_then_200_retries_bounded_with_identical_payload() {
    let fixture = spawn_fixture(vec![
        error_response("429 Too Many Requests", None, "slow down"),
        error_response("503 Service Unavailable", None, "overloaded"),
        stream_completed_response(),
    ])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        client.stream_with_tools("fixture-model", "high", &items, &tools),
    )
    .await
    .expect("retry loop stays bounded")
    .expect("transient 429/503 are retried automatically");

    // Consume the whole stream: retry only covers the initial HTTP request;
    // everything after the 2xx headers is ordinary stream consumption.
    let mut events = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(5), stream.next_event())
            .await
            .expect("stream event must not time out")
            .expect("stream event must not error after the 2xx headers");
        match next {
            Some(event) => events.push(event),
            None => break,
        }
    }
    assert_eq!(events.len(), 1, "one terminal completion event");

    wait_for_requests(&fixture.bodies, 3).await;
    let record = fixture.requests().await;
    assert_eq!(record.len(), 3, "initial + exactly two bounded retries");

    let first_body = request_body(&record[0].0);
    for (index, (raw, _)) in record.iter().enumerate().skip(1) {
        assert_eq!(request_body(raw), first_body, "retry {index} payload identical");
    }
    let payload: serde_json::Value = serde_json::from_str(&first_body).unwrap();
    assert_eq!(payload["model"], "fixture-model");
    assert_eq!(payload["input"][0]["content"], "You are a retry fixture.");
    assert_eq!(payload["input"][1]["content"], "turn input");
    assert_eq!(payload["tools"][0]["name"], "fixture_tool");
    assert_eq!(payload["stream"], true);
    assert_eq!(payload["reasoning"]["effort"], "high");

    // Deterministic exponential backoff (no jitter): 200ms then 400ms.
    let first_gap = (record[1].1 - record[0].1).as_millis();
    let second_gap = (record[2].1 - record[1].1).as_millis();
    assert!(first_gap >= 170, "first backoff {first_gap}ms < 170ms");
    assert!(second_gap >= 350, "second backoff {second_gap}ms < 350ms");

    fixture.server.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn terminal_400_is_never_retried() {
    let fixture = spawn_fixture(vec![error_response(
        "400 Bad Request",
        None,
        "bad request",
    )])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        client.stream_with_tools("fixture-model", "high", &items, &tools),
    )
    .await
    .expect("terminal errors answer promptly");
    let error = match outcome {
        Err(error) => error,
        Ok(_) => panic!("400 must not produce a stream"),
    };

    match &error {
        opencode_rk_providers::responses::ResponsesError::Upstream { status, message } => {
            assert_eq!(*status, 400);
            assert_eq!(message, "bad request");
        }
        other => panic!("expected typed upstream 400 error, got {other:?}"),
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fixture.requests().await.len(), 1, "no retry after 400");

    fixture.server.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn failure_after_2xx_headers_is_a_stream_error_never_retried() {
    let fixture = spawn_fixture(vec![stream_failed_response()]).await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        client.stream_with_tools("fixture-model", "high", &items, &tools),
    )
    .await
    .expect("admission stays bounded")
    .expect("2xx headers admit the stream");

    let error = tokio::time::timeout(Duration::from_secs(5), stream.next_event())
        .await
        .expect("stream failure answers promptly")
        .expect_err("response.failed after headers is a stream error");
    assert!(
        matches!(
            &error,
            opencode_rk_providers::responses::ResponsesError::StreamFailed { code, message }
                if code == "unknown" && message == "stream aborted"
        ),
        "expected typed stream failure, got {error:?}"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fixture.requests().await.len(), 1, "no retry after 2xx headers");

    fixture.server.abort();
}

/// Buffered (non-streaming) client shares the initial-send retry: one
/// transient 429 then a completed JSON envelope.
#[tokio::test(flavor = "current_thread")]
async fn buffered_create_retries_transient_then_succeeds() {
    let fixture = spawn_fixture(vec![
        error_response("429 Too Many Requests", None, "slow down"),
        buffered_completed_response(),
    ])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let input = vec![ResponsesInput::new(ResponsesRole::User, "turn input")];

    let text = tokio::time::timeout(
        Duration::from_secs(5),
        client.create("fixture-model", "high", &input),
    )
    .await
    .expect("retry loop stays bounded")
    .expect("buffered turn recovers from transient 429");
    assert_eq!(text, "recovered after retries");

    wait_for_requests(&fixture.bodies, 2).await;
    let record = fixture.requests().await;
    assert_eq!(record.len(), 2, "one bounded retry");
    assert_eq!(request_body(&record[0].0), request_body(&record[1].0));
    assert!((record[1].1 - record[0].1).as_millis() >= 170, "first backoff >= 170ms");

    fixture.server.abort();
}

/// All attempts hit transient statuses: bounded 3 attempts, byte-identical
/// payloads, and the returned upstream error carries the attempts count —
/// the upstream `error.message` only, never the surrounding body.
#[tokio::test(flavor = "current_thread")]
async fn transient_exhaustion_reports_attempt_count() {
    let fixture = spawn_fixture(vec![
        error_response("429 Too Many Requests", None, "slow down"),
        error_response("503 Service Unavailable", None, "overloaded"),
        error_response("429 Too Many Requests", None, "still slow"),
    ])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        client.stream_with_tools("fixture-model", "high", &items, &tools),
    )
    .await
    .expect("exhaustion answers promptly");
    let error = match outcome {
        Err(error) => error,
        Ok(_) => panic!("exhausted transient attempts must not produce a stream"),
    };
    match &error {
        opencode_rk_providers::responses::ResponsesError::Upstream { status, message } => {
            assert_eq!(*status, 429, "last attempt status is reported");
            assert!(
                message.contains("(retry exhausted after 3 attempts)"),
                "attempts count missing from error: {message}"
            );
            assert!(
                message.contains("still slow"),
                "final upstream message preserved: {message}"
            );
            assert!(
                !message.contains("slow down") && !message.contains("overloaded"),
                "only the final attempt message is reported, not earlier bodies: {message}"
            );
        }
        other => panic!("expected typed upstream error, got {other:?}"),
    }

    wait_for_requests(&fixture.bodies, 3).await;
    let record = fixture.requests().await;
    assert_eq!(record.len(), 3, "initial + two bounded retries");
    for pair in record.windows(2) {
        assert_eq!(request_body(&pair[0].0), request_body(&pair[1].0), "identical retry payload");
    }
    assert!((record[1].1 - record[0].1).as_millis() >= 170, "first backoff >= 170ms");
    assert!((record[2].1 - record[1].1).as_millis() >= 350, "second backoff >= 350ms");

    fixture.server.abort();
}

/// Cancel the caller's future between attempts: the pending backoff sleep and
/// any subsequent HTTP request are dropped, so no third attempt is recorded.
#[tokio::test(flavor = "current_thread")]
async fn dropped_caller_future_stops_backoff_and_http() {
    let fixture = spawn_fixture(vec![
        error_response("429 Too Many Requests", None, "slow down"),
        error_response("429 Too Many Requests", None, "slow down again"),
        error_response("429 Too Many Requests", None, "still slow"),
    ])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let task = tokio::spawn(async move {
        let _stream = client
            .stream_with_tools("fixture-model", "high", &items, &tools)
            .await;
    });

    wait_for_requests(&fixture.bodies, 2).await;
    task.abort();

    // The third attempt would fire within ~400ms of backoff if cancellation
    // failed; a 1s quiescence window catches it deterministically.
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(fixture.requests().await.len(), 2, "cancel stops attempt 3");

    fixture.server.abort();
}

/// `Retry-After` seconds are honored when larger than the exponential step
/// but always capped near 2s, so a huge header cannot block the caller.
#[tokio::test(flavor = "current_thread")]
async fn retry_after_is_honored_then_capped() {
    let fixture = spawn_fixture(vec![
        error_response("429 Too Many Requests", Some("1"), "slow down a second"),
        error_response("429 Too Many Requests", Some("9999"), "please wait hours"),
        stream_completed_response(),
    ])
    .await;
    let client = fixture_client(format!("http://{}/v1", fixture.address));
    let (items, tools) = fixture_turn();

    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        client.stream_with_tools("fixture-model", "high", &items, &tools),
    )
    .await
    .expect("Retry-After keeps the retry loop bounded")
    .expect("transient 429 with Retry-After is retried automatically");

    let mut events = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(5), stream.next_event())
            .await
            .expect("stream event must not time out")
            .expect("stream event must not error after the 2xx headers");
        match next {
            Some(event) => events.push(event),
            None => break,
        }
    }
    assert_eq!(events.len(), 1, "one terminal completion event");

    wait_for_requests(&fixture.bodies, 3).await;
    let record = fixture.requests().await;
    assert_eq!(record.len(), 3, "initial + two bounded retries");
    for pair in record.windows(2) {
        assert_eq!(request_body(&pair[0].0), request_body(&pair[1].0), "identical retry payload");
    }
    let first_gap = (record[1].1 - record[0].1).as_millis();
    let second_gap = (record[2].1 - record[1].1).as_millis();
    assert!(first_gap >= 900, "Retry-After: 1 wins over the 200ms step, got {first_gap}ms");
    assert!(
        second_gap >= 1800,
        "Retry-After: 9999 clamps toward the ~2s cap, got {second_gap}ms"
    );
    assert!(
        second_gap <= 2600,
        "Retry-After: 9999 must not block for hours, got {second_gap}ms"
    );

    fixture.server.abort();
}

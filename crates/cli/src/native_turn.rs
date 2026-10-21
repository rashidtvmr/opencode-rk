//! Bounded, owned NDJSON adapter for native streaming turns.
#![cfg(feature = "native")]

use reqwest::redirect::Policy;
use serde_json::Value;
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::sync::mpsc;

const MAX_FRAME: usize = 1024 * 1024;
const MAX_TEXT: usize = 64 * 1024;
const MAX_TOTAL: usize = 256 * 1024;

#[derive(Debug)]
pub enum Event {
    User,
    Delta(String),
    ReasoningDelta(String),
    ReasoningFinal(String),
    ToolCall {
        call_id: String,
        name: String,
        arguments: Value,
    },
    ToolOutput {
        call_id: String,
        name: String,
        output: String,
        error: Option<String>,
    },
    Assistant(String),
    Error(String),
}

pub struct TurnWorker {
    rx: mpsc::Receiver<Event>,
    task: tokio::task::JoinHandle<()>,
}

impl TurnWorker {
    pub fn start_shell(
        origin: &str,
        session: &str,
        auth: &str,
        command: &str,
        workdir: Option<&str>,
    ) -> Result<Self, String> {
        Self::start_path(
            origin,
            session,
            auth,
            "/shell/stream",
            serde_json::json!({"command": command, "workdir": workdir}),
        )
    }
    pub fn start(
        origin: &str,
        session: &str,
        auth: &str,
        text: &str,
        model: &str,
    ) -> Result<Self, String> {
        Self::start_path(
            origin,
            session,
            auth,
            "/turns/stream",
            serde_json::json!({"text": text, "model": model, "reasoning_effort": "high"}),
        )
    }

    fn start_path(
        origin: &str,
        session: &str,
        auth: &str,
        path: &str,
        body: Value,
    ) -> Result<Self, String> {
        crate::native_debug::event(
            "turn.start",
            serde_json::json!({"session":session,"path":path,"body_bytes":body.to_string().len()}),
        );
        if !crate::daemon_client::is_wellformed_token(auth) {
            return Err("missing validated daemon token".into());
        }
        let authority = origin
            .strip_prefix("http://")
            .ok_or("native turns require http loopback")?;
        if authority.contains('/')
            || authority.contains('@')
            || authority.contains('?')
            || authority.contains('#')
        {
            return Err("invalid daemon origin".into());
        }
        let addresses: Vec<_> = authority
            .to_socket_addrs()
            .map_err(|_| "invalid daemon origin")?
            .collect();
        if addresses.is_empty() || !addresses.iter().all(|address| address.ip().is_loopback()) {
            return Err("native turns require a loopback daemon origin".into());
        }
        let url = format!("http://{authority}/api/sessions/{session}{path}");
        let header = crate::daemon_client::authorization_header(auth);
        let (tx, rx) = mpsc::channel(128);
        let task = tokio::spawn(async move {
            let result = async {
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(Policy::none())
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(300))
                    .build()
                    .map_err(|e| e.to_string())?;
                let mut response = client
                    .post(url)
                    .header(reqwest::header::AUTHORIZATION, header)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                crate::native_debug::event("turn.http_status", serde_json::json!({"status":response.status().as_u16()}));
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let mut bytes = Vec::new();
                    while let Some(chunk) = response.chunk().await.map_err(|_| "error response interrupted")? {
                        if bytes.len() + chunk.len() > 16 * 1024 { break; }
                        bytes.extend_from_slice(&chunk);
                    }
                    return Err(provider_error_summary(status, &bytes));
                }
                let mut carry = Vec::new();
                let mut total = 0;
                let mut retained: usize = 0;
                let mut terminal = false;
                while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                    total += chunk.len();
                    if total > MAX_TOTAL {
                        return Err("turn stream exceeded output budget".into());
                    }
                    carry.extend_from_slice(&chunk);
                    if carry.len() > MAX_FRAME {
                        return Err("turn frame exceeded limit".into());
                    }
                    while let Some(pos) = carry.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = carry.drain(..=pos).collect();
                        if line.len() <= 1 {
                            continue;
                        }
                        let value: Value = serde_json::from_slice(&line[..line.len() - 1])
                            .map_err(|e| format!("invalid turn event: {e}"))?;
                        crate::native_debug::event("turn.frame", serde_json::json!({"type":journal_frame_type(&value),"bytes":line.len()}));
                        if value["type"]=="assistant_message" {
                            if let Some(summary)=value.get("reasoning_summary").and_then(Value::as_str).filter(|summary|!summary.is_empty()) {
                                retained=retained.saturating_add(summary.len());
                                if retained>MAX_TEXT {return Err("retained text limit exceeded".into());}
                                tx.send(Event::ReasoningFinal(summary.to_owned())).await.map_err(|_|"UI closed".to_owned())?;
                            }
                        }
                        if let Some(event) = decode(value, &mut retained)? {
                            if matches!(event, Event::Assistant(_) | Event::Error(_)) {
                                terminal = true;
                            }
                            tx.send(event).await.map_err(|_| "UI closed".to_string())?;
                        }
                    }
                }
                if !carry.is_empty() {
                    return Err("turn stream ended with incomplete event".into());
                }
                if !terminal {
                    return Err("turn stream ended without a terminal assistant event".into());
                }
                Ok::<(), String>(())
            }
            .await;
            crate::native_debug::event(
                "turn.stream_closed",
                serde_json::json!({"ok":result.is_ok(),"error_category":result.as_ref().err().map(|error| if error.contains("HTTP") { "http_status" } else if error.contains("incomplete") || error.contains("terminal") { "truncated_stream" } else if error.contains("limit") || error.contains("budget") { "resource_limit" } else { "transport_or_protocol" })}),
            );
            if let Err(error) = result {
                let _ = tx.send(Event::Error(error)).await;
            }
        });
        Ok(Self { rx, task })
    }
    pub fn try_next(&mut self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
    pub fn is_finished(&self) -> bool {
        self.task.is_finished() && self.rx.is_empty()
    }
}

fn decode(value: Value, retained: &mut usize) -> Result<Option<Event>, String> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or("event missing type")?;
    if kind == "tool_call" {
        let bytes = value
            .get("arguments")
            .map(|arguments| arguments.to_string().len())
            .unwrap_or(0);
        *retained = retained.saturating_add(bytes);
        if *retained > MAX_TEXT {
            return Err("retained text limit exceeded".into());
        }
    }
    let field = |key: &str, retained: &mut usize| -> Result<String, String> {
        let text = value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{kind} missing {key}"))?;
        *retained += text.len();
        if *retained > MAX_TEXT {
            return Err("retained text limit exceeded".into());
        }
        Ok(text.to_owned())
    };
    Ok(Some(match kind {
        "user_message" => Event::User,
        "assistant_delta" => Event::Delta(field("delta", retained)?),
        "tool_call" => Event::ToolCall {
            call_id: field_optional(&value, "call_id", retained),
            name: field("name", retained)?,
            arguments: match value.get("arguments") {
                Some(Value::String(encoded)) => serde_json::from_str(encoded)
                    .map_err(|_| "tool call arguments are not valid JSON")?,
                Some(arguments) => arguments.clone(),
                None => Value::Object(Default::default()),
            },
        },
        "tool_output" => Event::ToolOutput {
            call_id: field_optional(&value, "call_id", retained),
            name: field("name", retained)?,
            output: field("output", retained)?,
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    (value.get("is_error").and_then(Value::as_bool) == Some(true)
                        || value.get("success").and_then(Value::as_bool) == Some(false))
                    .then(|| {
                        value
                            .get("output")
                            .and_then(Value::as_str)
                            .unwrap_or("Tool failed")
                            .to_owned()
                    })
                }),
        },
        "assistant_message" => {
            let text = value
                .pointer("/message/body/text")
                .and_then(Value::as_str)
                .ok_or("assistant missing text")?;
            *retained += text.len();
            if *retained > MAX_TEXT {
                return Err("retained text limit exceeded".into());
            }
            Event::Assistant(text.to_owned())
        }
        "error" => {
            Event::Error(field("message", retained).unwrap_or_else(|_| "turn failed".into()))
        }
        "reasoning_summary_delta" => Event::ReasoningDelta(field("delta", retained)?),
        "reasoning_summary" => Event::ReasoningFinal(field("text", retained)?),
        other => return Err(format!("unsupported event {other}")),
    }))
}

fn field_optional(value: &Value, key: &str, retained: &mut usize) -> String {
    let value = value.get(key).and_then(Value::as_str).unwrap_or("");
    *retained = retained.saturating_add(value.len());
    value.to_owned()
}

fn journal_frame_type(value: &Value) -> &'static str {
    match value["type"].as_str() {
        Some("user_message") => "user_message",
        Some("assistant_delta") => "assistant_delta",
        Some("assistant_message") => "assistant_message",
        Some("tool_call") => "tool_call",
        Some("tool_result") => "tool_result",
        Some("tool_output") => "tool_output",
        Some("reasoning_summary_delta") => "reasoning_summary_delta",
        Some("error") => "error",
        _ => "unknown",
    }
}

/// Only recognized protocol facts reach the UI/journal; provider bodies may
/// contain credentials or arbitrary content and are never echoed verbatim.
fn provider_error_summary(status: u16, body: &[u8]) -> String {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let message = value["message"].as_str().unwrap_or("");
    let tokens: Vec<_> = message.split_whitespace().take(64).collect();
    let upstream = tokens.windows(2).find_map(|pair| {
        (pair[0] == "HTTP")
            .then(|| pair[1].trim_end_matches(':').parse::<u16>().ok())
            .flatten()
            .filter(|status| (400..=599).contains(status))
    });
    crate::native_debug::event(
        "turn.provider_failure",
        serde_json::json!({"daemon_status":status,"upstream_status":upstream}),
    );
    match upstream.unwrap_or(status) {
        429 => "Provider HTTP 429: unavailable or rate-limited; prompt preserved.".into(),
        401 | 403 => "Provider authentication failed; check its API key. Prompt preserved.".into(),
        404 => "Provider model or endpoint not found; prompt preserved.".into(),
        code => {
            format!("Turn failed (HTTP {code}); prompt preserved until admission is acknowledged.")
        }
    }
}

#[cfg(test)]
mod native_error_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn actual_tool_failure_flag_and_arguments_survive_decode() {
        let call=decode(serde_json::json!({"type":"tool_call","call_id":"real","name":"edit","arguments":"{\"path\":\"source\"}"}),&mut 0).unwrap().unwrap();
        assert!(matches!(call,Event::ToolCall{arguments,..} if arguments["path"]=="source"));
        let output=decode(serde_json::json!({"type":"tool_output","call_id":"real","name":"edit","output":"permission rejected","success":false,"is_error":true}),&mut 0).unwrap().unwrap();
        assert!(
            matches!(output, Event::ToolOutput { error: Some(_), .. }),
            "explicit backend failure flag was dropped"
        );
    }

    #[test]
    fn native_journal_types_never_copy_untrusted_protocol_strings() {
        assert_eq!(
            journal_frame_type(&serde_json::json!({"type":"secret-canary-not-metadata"})),
            "unknown"
        );
        assert_eq!(
            journal_frame_type(&serde_json::json!({"type":"tool_call"})),
            "tool_call"
        );
        assert_eq!(journal_frame_type(&serde_json::json!({})), "unknown");
    }

    async fn request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&chunk[..count]);
            assert!(bytes.len() < 64 * 1024);
            if let Some(end) = bytes.windows(4).position(|row| row == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    return bytes;
                }
            }
        }
    }

    async fn mock_turn(status: u16, body: &'static str) -> Vec<Event> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let bytes = request(&mut socket).await;
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("POST /api/sessions/fixture/turns/stream HTTP/1.1"));
            let body_start = request.find("\r\n\r\n").unwrap() + 4;
            let submitted: Value = serde_json::from_str(&request[body_start..]).unwrap();
            assert_eq!(submitted["model"], "9router/oc/deepseek-v4.1-flash-free");
            assert_eq!(submitted["text"], "café 世界");
            socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        });
        let mut worker = TurnWorker::start(
            &origin,
            "fixture",
            &"a".repeat(64),
            "café 世界",
            "9router/oc/deepseek-v4.1-flash-free",
        )
        .unwrap();
        let events = tokio::time::timeout(Duration::from_secs(3), async {
            let mut events = Vec::new();
            loop {
                while let Some(event) = worker.try_next() {
                    events.push(event);
                }
                if worker.is_finished() {
                    return events;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        events
    }

    #[tokio::test]
    async fn native_worker_preserves_qualified_model_and_reports_provider_refusal_without_ack() {
        let events = mock_turn(
            502,
            r#"{"message":"provider returned HTTP 429: secret-canary"}"#,
        )
        .await;
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], Event::Error(message) if message.contains("429") && !message.contains("secret-canary"))
        );
    }

    #[tokio::test]
    async fn native_worker_rejects_truncation_and_malformed_streams() {
        for body in [
            "{\"type\":\"user_message\"}\n",
            "not-json\n",
            "{\"type\":\"assistant_message\",\"text\":\"cut off\"}",
        ] {
            let events = mock_turn(201, body).await;
            assert!(matches!(events.last(), Some(Event::Error(_))));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Event::Assistant(_)))
            );
        }
    }

    #[tokio::test]
    async fn native_worker_drop_closes_the_inflight_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            request(&mut socket).await;
            socket
                .write_all(b"HTTP/1.1 201 Created\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await
                .unwrap();
            ready_tx.send(()).unwrap();
            let mut byte = [0];
            match socket.read(&mut byte).await {
                Ok(0) => {}
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
                other => panic!("cancelled worker retained its socket: {other:?}"),
            }
        });
        let worker = TurnWorker::start(
            &origin,
            "fixture",
            &"a".repeat(64),
            "café 世界",
            "9router/oc/deepseek-v4.1-flash-free",
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(3), ready_rx)
            .await
            .unwrap()
            .unwrap();
        drop(worker);
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
    }
    #[test]
    fn native_http_failure_shows_upstream_status_without_echoing_secrets() {
        let result = provider_error_summary(
            502,
            br#"{"message":"provider returned HTTP 429: fixture-credential-never-display"}"#,
        );
        assert!(result.contains("429"));
        assert!(result.contains("prompt preserved"));
        assert!(!result.contains("fixture-credential"));
        assert!(provider_error_summary(401, b"not JSON secret").contains("authentication failed"));
    }
}

impl Drop for TurnWorker {
    fn drop(&mut self) {
        crate::native_debug::event(
            "turn.worker_drop",
            serde_json::json!({"finished":self.task.is_finished()}),
        );
        self.task.abort();
    }
}

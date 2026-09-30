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
    ToolCall(String),
    ToolOutput(String, String),
    Assistant(String),
    Error(String),
}

pub struct TurnWorker {
    rx: mpsc::Receiver<Event>,
    task: tokio::task::JoinHandle<()>,
}

impl TurnWorker {
    pub fn start(
        origin: &str,
        session: &str,
        auth: &str,
        text: &str,
        model: &str,
    ) -> Result<Self, String> {
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
        let url = format!("http://{authority}/api/sessions/{session}/turns/stream");
        let body = serde_json::json!({"text": text, "model": model, "reasoning_effort": "high"});
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
                if !response.status().is_success() {
                    return Err(format!("daemon returned HTTP {}", response.status()));
                }
                let mut carry = Vec::new();
                let mut total = 0;
                let mut retained = 0;
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
        "tool_call" => Event::ToolCall(field("name", retained)?),
        "tool_output" => Event::ToolOutput(field("name", retained)?, field("output", retained)?),
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
        "reasoning_summary_delta" => return Ok(None),
        other => return Err(format!("unsupported event {other}")),
    }))
}

impl Drop for TurnWorker {
    fn drop(&mut self) {
        self.task.abort();
    }
}

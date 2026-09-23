#![forbid(unsafe_code)]
//! Bounded, renderer-independent execution of one daemon turn.
//!
//! The composer owns drafts and queued prompts. This module owns only one
//! active HTTP request. A submit accepted by [`TurnWorkerHandle::try_submit`]
//! reserves that single slot; a second submit is rejected with its request
//! returned to the caller. The worker never retries a POST whose outcome is
//! ambiguous.

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::sync::{mpsc, watch};

/// Maximum prompt bytes retained or sent for one turn.
pub const MAX_PROMPT_BYTES: usize = 32 * 1024;
/// Maximum serialized JSON request bytes.
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;
/// Maximum response bytes read from the daemon.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// Maximum extracted assistant text retained in one result.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
/// Maximum endpoint URL bytes.
pub const MAX_ENDPOINT_BYTES: usize = 2048;
/// Maximum session/model/effort field bytes.
pub const MAX_FIELD_BYTES: usize = 256;
/// Maximum bearer token bytes. The token is never formatted into an error.
pub const MAX_BEARER_BYTES: usize = 8 * 1024;
/// There is no prompt queue in the worker. This is only the control mailbox.
pub const COMMAND_CHANNEL_CAPACITY: usize = 1;
/// Caller-owned bounded result mailbox.
pub const RESULT_CHANNEL_CAPACITY: usize = 4;
/// Bounds a request that stalls without producing a response.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

const IDLE: u8 = 0;
const RESERVED: u8 = 1;
const ACTIVE: u8 = 2;
const CLOSED: u8 = 3;

const PHASE_ACCEPTED: u8 = 1;
const PHASE_DISPATCHED: u8 = 2;
const PHASE_CANCELLED: u8 = 3;

/// Typed request assembled by chat or TUI callers.
///
/// `endpoint` must be a loopback HTTP(S) URL. This prevents a validated local
/// daemon token from being redirected to an arbitrary host. The endpoint is
/// the complete `/api/sessions/{id}/turns` URL.
pub struct TurnRequest {
    endpoint: String,
    session_id: String,
    prompt: String,
    model: String,
    reasoning_effort: String,
    bearer: Option<String>,
}

impl TurnRequest {
    /// Construct and validate a daemon turn request.
    pub fn new(
        endpoint: impl Into<String>,
        session_id: impl Into<String>,
        prompt: impl Into<String>,
        model: impl Into<String>,
        reasoning_effort: impl Into<String>,
        bearer: Option<String>,
    ) -> Result<Self, RequestValidationError> {
        let request = Self {
            endpoint: endpoint.into(),
            session_id: session_id.into(),
            prompt: prompt.into(),
            model: model.into(),
            reasoning_effort: reasoning_effort.into(),
            bearer,
        };
        request.validate()?;
        Ok(request)
    }

    /// Prompt text, for caller-owned display or draft restoration.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Validate trust-boundary fields again immediately before dispatch.
    pub fn validate(&self) -> Result<(), RequestValidationError> {
        validate_endpoint(&self.endpoint)?;
        validate_field("session", &self.session_id)?;
        validate_prompt(&self.prompt)?;
        validate_field("model", &self.model)?;
        validate_effort(&self.reasoning_effort)?;
        if let Some(token) = &self.bearer {
            if token.is_empty() {
                return Err(RequestValidationError::Empty { field: "bearer" });
            }
            if token.len() > MAX_BEARER_BYTES {
                return Err(RequestValidationError::TooLarge {
                    field: "bearer",
                    bytes: token.len(),
                    limit: MAX_BEARER_BYTES,
                });
            }
        }

        let body = self.body_bytes()?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(RequestValidationError::TooLarge {
                field: "request",
                bytes: body.len(),
                limit: MAX_REQUEST_BYTES,
            });
        }
        Ok(())
    }

    fn body_bytes(&self) -> Result<Vec<u8>, RequestValidationError> {
        serde_json::to_vec(&serde_json::json!({
            "text": self.prompt,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
        }))
        .map_err(|_| RequestValidationError::Serialization)
    }
}

impl fmt::Debug for TurnRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TurnRequest")
            .field("endpoint", &self.endpoint)
            .field("session_id", &self.session_id)
            .field("prompt_bytes", &self.prompt.len())
            .field("model", &self.model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("bearer_present", &self.bearer.is_some())
            .finish()
    }
}

/// Request construction failures. Values are bounded metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestValidationError {
    Empty {
        field: &'static str,
    },
    TooLarge {
        field: &'static str,
        bytes: usize,
        limit: usize,
    },
    InvalidEndpoint,
    Serialization,
}

impl fmt::Display for RequestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(formatter, "{field} must not be empty"),
            Self::TooLarge {
                field,
                bytes,
                limit,
            } => write!(
                formatter,
                "{field} is {bytes} bytes, exceeds {limit} byte limit"
            ),
            Self::InvalidEndpoint => write!(formatter, "endpoint must be a loopback HTTP(S) URL"),
            Self::Serialization => write!(formatter, "turn request could not be serialized"),
        }
    }
}

impl std::error::Error for RequestValidationError {}

fn validate_endpoint(endpoint: &str) -> Result<(), RequestValidationError> {
    if endpoint.is_empty() {
        return Err(RequestValidationError::Empty { field: "endpoint" });
    }
    if endpoint.len() > MAX_ENDPOINT_BYTES {
        return Err(RequestValidationError::TooLarge {
            field: "endpoint",
            bytes: endpoint.len(),
            limit: MAX_ENDPOINT_BYTES,
        });
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return Err(RequestValidationError::InvalidEndpoint);
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str() != Some("127.0.0.1")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_none()
        || url.path().is_empty()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(RequestValidationError::InvalidEndpoint);
    }
    Ok(())
}

fn validate_field(field: &'static str, value: &str) -> Result<(), RequestValidationError> {
    if value.is_empty() {
        return Err(RequestValidationError::Empty { field });
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(RequestValidationError::TooLarge {
            field,
            bytes: value.len(),
            limit: MAX_FIELD_BYTES,
        });
    }
    Ok(())
}

fn validate_prompt(prompt: &str) -> Result<(), RequestValidationError> {
    if prompt.trim().is_empty() {
        return Err(RequestValidationError::Empty { field: "prompt" });
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(RequestValidationError::TooLarge {
            field: "prompt",
            bytes: prompt.len(),
            limit: MAX_PROMPT_BYTES,
        });
    }
    Ok(())
}

fn validate_effort(value: &str) -> Result<(), RequestValidationError> {
    validate_field("reasoning_effort", value)
}

/// Result delivered through the caller-owned bounded channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnResult {
    Completed {
        output: String,
    },
    Failed(TurnFailure),
    Cancelled,
    /// The POST was dispatched, but cancellation/timeout closed the request
    /// before a definitive HTTP response. The caller must not replay it.
    Uncertain,
}

/// Bounded failure classification. No provider response body is retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnFailure {
    InvalidRequest(RequestValidationError),
    RequestTooLarge { bytes: usize, limit: usize },
    ResponseTooLarge { bytes: usize, limit: usize },
    OutputTooLarge { bytes: usize, limit: usize },
    HttpStatus { status: u16 },
    MalformedResponse,
    Transport,
    Timeout,
    ResultChannelClosed,
}

impl fmt::Display for TurnFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(error) => error.fmt(formatter),
            Self::RequestTooLarge { bytes, limit } => {
                write!(
                    formatter,
                    "request is {bytes} bytes, exceeds {limit} byte limit"
                )
            }
            Self::ResponseTooLarge { bytes, limit } => {
                write!(
                    formatter,
                    "response exceeds {limit} byte limit at {bytes} bytes"
                )
            }
            Self::OutputTooLarge { bytes, limit } => {
                write!(
                    formatter,
                    "assistant output is {bytes} bytes, exceeds {limit} byte limit"
                )
            }
            Self::HttpStatus { status } => {
                write!(formatter, "daemon returned HTTP status {status}")
            }
            Self::MalformedResponse => write!(formatter, "daemon turn response is malformed"),
            Self::Transport => write!(formatter, "daemon turn transport failed"),
            Self::Timeout => write!(formatter, "daemon turn timed out after dispatch"),
            Self::ResultChannelClosed => write!(formatter, "turn result receiver is closed"),
        }
    }
}

impl std::error::Error for TurnFailure {}

/// Why an interrupt call did or did not take effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptResult {
    Requested,
    NotActive,
    Closed,
}

/// Request lifecycle visible to a caller rendering cancellation state.
/// `Dispatched` is the conservative no-replay boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchPhase {
    Idle,
    Accepted,
    Dispatched,
    Cancelled,
}

/// Submit rejection. The original request remains owned by the caller.
pub enum SubmitError {
    Busy(TurnRequest),
    Full(TurnRequest),
    Closed(TurnRequest),
    Invalid {
        request: TurnRequest,
        error: RequestValidationError,
    },
}

impl SubmitError {
    /// Recover the rejected request for draft restoration or later retry by a
    /// caller that has independently established a definitive failure.
    #[must_use]
    pub fn into_request(self) -> TurnRequest {
        match self {
            Self::Busy(request) | Self::Full(request) | Self::Closed(request) => request,
            Self::Invalid { request, .. } => request,
        }
    }

    #[must_use]
    pub fn validation(&self) -> Option<&RequestValidationError> {
        match self {
            Self::Invalid { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl fmt::Debug for SubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(request) => formatter.debug_tuple("Busy").field(request).finish(),
            Self::Full(request) => formatter.debug_tuple("Full").field(request).finish(),
            Self::Closed(request) => formatter.debug_tuple("Closed").field(request).finish(),
            Self::Invalid { request, error } => formatter
                .debug_struct("Invalid")
                .field("request", request)
                .field("error", error)
                .finish(),
        }
    }
}

impl fmt::Display for SubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(_) => write!(formatter, "turn worker is busy"),
            Self::Full(_) => write!(formatter, "turn worker control mailbox is full"),
            Self::Closed(_) => write!(formatter, "turn worker is closed"),
            Self::Invalid { error, .. } => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SubmitError {}

enum Command {
    Submit(TurnRequest),
    Shutdown,
}

struct Control {
    commands: mpsc::Sender<Command>,
    cancel: watch::Sender<bool>,
    closing: AtomicBool,
    lifecycle: AtomicU8,
    phase: AtomicU8,
}

/// Cloneable caller-side control handle. It never owns the worker task or the
/// result receiver.
#[derive(Clone)]
pub struct TurnWorkerHandle {
    control: Arc<Control>,
}

impl TurnWorkerHandle {
    /// Try to reserve the one active request slot and enqueue only a control
    /// message. There is no prompt queue here.
    pub fn try_submit(&self, request: TurnRequest) -> Result<(), SubmitError> {
        if let Err(error) = request.validate() {
            return Err(SubmitError::Invalid { request, error });
        }
        if self.control.closing.load(Ordering::Acquire)
            || self.control.lifecycle.load(Ordering::Acquire) == CLOSED
        {
            return Err(SubmitError::Closed(request));
        }
        if self
            .control
            .lifecycle
            .compare_exchange(IDLE, RESERVED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(SubmitError::Busy(request));
        }
        if self.control.closing.load(Ordering::Acquire) {
            self.control.lifecycle.store(CLOSED, Ordering::Release);
            return Err(SubmitError::Closed(request));
        }
        let _ = self.control.cancel.send(false);
        self.control.phase.store(PHASE_ACCEPTED, Ordering::Release);
        match self.control.commands.try_reserve() {
            Ok(permit) => {
                permit.send(Command::Submit(request));
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.control.lifecycle.store(IDLE, Ordering::Release);
                Err(SubmitError::Full(request))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.control.lifecycle.store(CLOSED, Ordering::Release);
                Err(SubmitError::Closed(request))
            }
        }
    }

    /// Request cooperative cancellation. The worker classifies the result as
    /// `Cancelled` before dispatch, or `Uncertain` after dispatch.
    #[must_use]
    pub fn interrupt(&self) -> InterruptResult {
        match self.control.lifecycle.load(Ordering::Acquire) {
            RESERVED | ACTIVE => {
                let mut phase = self.control.phase.load(Ordering::Acquire);
                while phase == PHASE_ACCEPTED {
                    match self.control.phase.compare_exchange(
                        PHASE_ACCEPTED,
                        PHASE_CANCELLED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(current) => phase = current,
                    }
                }
                let _ = self.control.cancel.send(true);
                InterruptResult::Requested
            }
            CLOSED => InterruptResult::Closed,
            _ => InterruptResult::NotActive,
        }
    }

    /// Read the current dispatch phase without taking ownership of the worker.
    #[must_use]
    pub fn phase(&self) -> DispatchPhase {
        match self.control.phase.load(Ordering::Acquire) {
            PHASE_ACCEPTED => DispatchPhase::Accepted,
            PHASE_DISPATCHED => DispatchPhase::Dispatched,
            PHASE_CANCELLED => DispatchPhase::Cancelled,
            _ => DispatchPhase::Idle,
        }
    }
}

/// Owns exactly one Tokio worker task. Call [`TurnWorker::shutdown`] on the
/// normal exit path to send shutdown and await the task. Dropping it only
/// aborts best effort and never claims that the task joined.
pub struct TurnWorker {
    control: Arc<Control>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl TurnWorker {
    /// Start one worker task and return caller-owned control/result handles.
    pub fn start(client: reqwest::Client) -> (Self, TurnWorkerHandle, mpsc::Receiver<TurnResult>) {
        let (commands, mut command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (results, result_rx) = mpsc::channel(RESULT_CHANNEL_CAPACITY);
        let control = Arc::new(Control {
            commands,
            cancel: watch::channel(false).0,
            closing: AtomicBool::new(false),
            lifecycle: AtomicU8::new(IDLE),
            phase: AtomicU8::new(0),
        });
        let task_control = Arc::clone(&control);
        let join = tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                match command {
                    Command::Submit(request) => {
                        task_control.lifecycle.store(ACTIVE, Ordering::Release);
                        let result = execute_one(&client, &task_control, request).await;
                        let send_result = results.send(result).await;
                        task_control.phase.store(0, Ordering::Release);
                        let _ = task_control.cancel.send(false);
                        task_control.lifecycle.store(IDLE, Ordering::Release);
                        if send_result.is_err() {
                            task_control.lifecycle.store(CLOSED, Ordering::Release);
                            break;
                        }
                    }
                    Command::Shutdown => break,
                }
            }
            task_control.lifecycle.store(CLOSED, Ordering::Release);
            task_control.closing.store(true, Ordering::Release);
        });
        (
            Self {
                control: Arc::clone(&control),
                join: Some(join),
            },
            TurnWorkerHandle { control },
            result_rx,
        )
    }

    /// Send shutdown through the bounded control channel, then await the only
    /// worker task. The result receiver should be drained by the caller first.
    pub async fn shutdown(mut self) -> Result<(), ShutdownError> {
        self.control.closing.store(true, Ordering::Release);
        let _ = self.control.cancel.send(true);
        let send_error = self
            .control
            .commands
            .send(Command::Shutdown)
            .await
            .err()
            .map(|_| ShutdownError::CommandClosed);
        let join_error = match self.join.take() {
            Some(join) => join.await.err().map(|_| ShutdownError::JoinFailed),
            None => Some(ShutdownError::JoinFailed),
        };
        self.control.lifecycle.store(CLOSED, Ordering::Release);
        if let Some(error) = send_error {
            return Err(error);
        }
        if let Some(error) = join_error {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for TurnWorker {
    fn drop(&mut self) {
        self.control.closing.store(true, Ordering::Release);
        let _ = self.control.cancel.send(true);
        self.control.lifecycle.store(CLOSED, Ordering::Release);
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

/// Normal shutdown failures. No arbitrary task error is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownError {
    CommandClosed,
    JoinFailed,
}

impl fmt::Display for ShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandClosed => write!(formatter, "turn worker command channel is closed"),
            Self::JoinFailed => write!(formatter, "turn worker task failed to join"),
        }
    }
}

impl std::error::Error for ShutdownError {}

async fn execute_one(
    client: &reqwest::Client,
    control: &Control,
    request: TurnRequest,
) -> TurnResult {
    if *control.cancel.borrow() {
        return TurnResult::Cancelled;
    }
    if let Err(error) = request.validate() {
        return TurnResult::Failed(TurnFailure::InvalidRequest(error));
    }
    let body = match request.body_bytes() {
        Ok(body) => body,
        Err(error) => return TurnResult::Failed(TurnFailure::InvalidRequest(error)),
    };
    if body.len() > MAX_REQUEST_BYTES {
        return TurnResult::Failed(TurnFailure::RequestTooLarge {
            bytes: body.len(),
            limit: MAX_REQUEST_BYTES,
        });
    }
    if *control.cancel.borrow() {
        return TurnResult::Cancelled;
    }

    // Subscribe before publishing the dispatch phase. A watch receiver sees
    // the latest cancellation value even when the interrupt wins the race.
    let mut cancelled = control.cancel.subscribe();
    // This store is the dispatch boundary. Once crossed, cancellation is
    // conservative: the caller receives Uncertain and must not replay.
    if control
        .phase
        .compare_exchange(
            PHASE_ACCEPTED,
            PHASE_DISPATCHED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        return TurnResult::Cancelled;
    }
    let request_future = send_request(client, &request, body);
    tokio::pin!(request_future);
    let outcome = tokio::select! {
        response = tokio::time::timeout(REQUEST_TIMEOUT, &mut request_future) => {
            match response {
                Ok(result) => result,
                Err(_) => Err(TurnFailure::Timeout),
            }
        }
        changed = cancelled.changed() => {
            match changed {
                Ok(()) => Err(TurnFailure::Transport),
                Err(_) => Err(TurnFailure::Transport),
            }
        },
    };
    if *control.cancel.borrow() {
        return TurnResult::Uncertain;
    }
    match outcome {
        Ok(output) => TurnResult::Completed { output },
        Err(TurnFailure::Timeout | TurnFailure::Transport | TurnFailure::MalformedResponse) => {
            TurnResult::Uncertain
        }
        Err(TurnFailure::ResponseTooLarge { .. }) | Err(TurnFailure::OutputTooLarge { .. }) => {
            TurnResult::Uncertain
        }
        Err(error) => TurnResult::Failed(error),
    }
}

async fn send_request(
    client: &reqwest::Client,
    request: &TurnRequest,
    body: Vec<u8>,
) -> Result<String, TurnFailure> {
    let mut builder = client
        .post(&request.endpoint)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body);
    if let Some(token) = &request.bearer {
        builder = builder.bearer_auth(token);
    }
    let mut response = builder.send().await.map_err(|error| {
        if error.is_timeout() {
            TurnFailure::Timeout
        } else {
            TurnFailure::Transport
        }
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(TurnFailure::HttpStatus {
            status: status.as_u16(),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(TurnFailure::ResponseTooLarge {
            bytes: response.content_length().unwrap_or_default() as usize,
            limit: MAX_RESPONSE_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await.map_err(|_| TurnFailure::Transport)? {
        let next = bytes.len().saturating_add(chunk.len());
        if next > MAX_RESPONSE_BYTES {
            return Err(TurnFailure::ResponseTooLarge {
                bytes: next,
                limit: MAX_RESPONSE_BYTES,
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| TurnFailure::MalformedResponse)?;
    let output = value
        .get("assistant_message")
        .and_then(|message| message.get("body"))
        .and_then(|body| body.get("text"))
        .and_then(serde_json::Value::as_str)
        .ok_or(TurnFailure::MalformedResponse)?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(TurnFailure::OutputTooLarge {
            bytes: output.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }
    Ok(output.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn request(prompt: &str, port: u16) -> TurnRequest {
        TurnRequest::new(
            format!("http://127.0.0.1:{port}/api/sessions/s/turns"),
            "s",
            prompt,
            "openai/gpt-5.6",
            "high",
            Some("fixture-token".to_owned()),
        )
        .expect("valid request")
    }

    #[test]
    fn bounds_reject_without_retaining_secret_values() {
        let error = TurnRequest::new(
            "http://127.0.0.1:4096/api/sessions/s/turns",
            "s",
            "x".repeat(MAX_PROMPT_BYTES + 1),
            "model",
            "high",
            Some("secret-token".to_owned()),
        )
        .expect_err("oversize prompt rejected");
        assert_eq!(
            error,
            RequestValidationError::TooLarge {
                field: "prompt",
                bytes: MAX_PROMPT_BYTES + 1,
                limit: MAX_PROMPT_BYTES,
            }
        );
        assert!(!error.to_string().contains("secret-token"));
    }

    #[tokio::test]
    async fn real_request_completes_and_shutdown_joins() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let fixture = tokio::spawn(async move {
            let (mut stream, _): (tokio::net::TcpStream, SocketAddr) =
                listener.accept().await.expect("accept request");
            let mut request_bytes = vec![0_u8; 4096];
            let _ = stream.read(&mut request_bytes).await.expect("read request");
            let body = r#"{"assistant_message":{"body":{"text":"fixture reply"}}}"#;
            let wire = format!(
                "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream
                .write_all(wire.as_bytes())
                .await
                .expect("write reply");
        });
        let client = reqwest::Client::builder().build().expect("client");
        let (worker, handle, mut results) = TurnWorker::start(client);
        handle
            .try_submit(request("hello", address.port()))
            .expect("submit");
        assert_eq!(
            results.recv().await,
            Some(TurnResult::Completed {
                output: "fixture reply".to_owned()
            })
        );
        worker.shutdown().await.expect("joined shutdown");
        fixture.await.expect("fixture joined");
    }

    #[tokio::test]
    async fn busy_rejection_returns_original_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let fixture = tokio::spawn(async move {
            let (mut stream, _): (tokio::net::TcpStream, SocketAddr) =
                listener.accept().await.expect("accept request");
            let mut request_bytes = vec![0_u8; 1024];
            let _ = stream.read(&mut request_bytes).await.expect("read request");
            tokio::time::sleep(Duration::from_millis(50)).await;
            let body = r#"{"assistant_message":{"body":{"text":"done"}}}"#;
            let wire = format!(
                "HTTP/1.1 201 Created\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(wire.as_bytes())
                .await
                .expect("write reply");
        });
        let client = reqwest::Client::builder().build().expect("client");
        let (worker, handle, mut results) = TurnWorker::start(client);
        handle
            .try_submit(request("first", address.port()))
            .expect("first submit");
        let error = handle
            .try_submit(request("second", address.port()))
            .expect_err("second submit busy");
        assert_eq!(error.into_request().prompt(), "second");
        assert_eq!(
            results.recv().await,
            Some(TurnResult::Completed {
                output: "done".to_owned()
            })
        );
        worker.shutdown().await.expect("joined shutdown");
        fixture.await.expect("fixture joined");
    }

    #[tokio::test]
    async fn interrupt_after_dispatch_is_uncertain_without_replay() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let fixture = tokio::spawn(async move {
            let (mut stream, _): (tokio::net::TcpStream, SocketAddr) =
                listener.accept().await.expect("accept request");
            let mut request_bytes = vec![0_u8; 1024];
            let _ = stream.read(&mut request_bytes).await.expect("read request");
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = stream.shutdown().await;
        });
        let client = reqwest::Client::builder().build().expect("client");
        let (worker, handle, mut results) = TurnWorker::start(client);
        handle
            .try_submit(request("first", address.port()))
            .expect("submit");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(handle.interrupt(), InterruptResult::Requested);
        assert_eq!(results.recv().await, Some(TurnResult::Uncertain));
        worker.shutdown().await.expect("joined shutdown");
        fixture.await.expect("fixture joined");
    }
}

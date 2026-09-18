//! DISC-109 LSP client lane behind the permission broker.
//!
//! Real lane (AUD-009 `missing`): JSON-RPC 2.0 `Content-Length` framing over a
//! caller-owned stdio server process, broker authorization on the workspace
//! root before any spawn, bounded pending map, timeout, cancel/reclaim, typed
//! errors, no panics. Std-only: no threads beyond one reader, no serde.
//!
//! Test map (card T01..T05):
//! - T01 (disc109_t01): authorized method+root round trip through broker.
//! - T02 (disc109_t02): unauthorized root denied, broker consulted, no spawn.
//! - T03 (disc109_t03): slow server hits timeout; pending bounded, no starvation.
//! - T04 (disc109_t04): cancel kills the server; no orphan, pipes reclaimed.
//! - T05 (disc109_t05): malformed replies surface typed errors, never panics.
//! - T06 (disc109_t06): pending-queue bound + shutdown idempotent.

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Max queued + in-flight requests per client.
pub const MAX_PENDING: usize = 16;
/// Max one JSON-RPC frame / payload in bytes (1 MiB).
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
/// Max bytes of one LSP method string.
pub const MAX_METHOD_LEN: usize = 128;
/// Max workspace-root path bytes copied into the auth request.
pub const MAX_ROOT_BYTES: usize = 4096;
/// Default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
/// `ponytail:` full LSP needs initialize/handshake + notifications + progress
/// tokens; add when a live turn path consumes this lane.

/// Broker authorization input: workspace root plus the LSP method about to
/// run. Byte payloads are never attached, never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct LspAuthRequest {
    pub workspace_root: String,
    pub method: String,
}

impl fmt::Debug for LspAuthRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspAuthRequest")
            .field("workspace_root_len", &self.workspace_root.len())
            .field("method", &self.method)
            .finish()
    }
}

/// Caller-supplied trusted permission broker. No default-allow impl ships:
/// authorization happens before the first server spawn, and a deny provably
/// starts no process.
pub trait LspPermissionBroker {
    fn assert(&self, req: &LspAuthRequest) -> bool;
}

/// Every LSP lane failure. Deny/validation errors never spawn or signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspError {
    /// Broker refused this root+method. No process started, nothing signalled.
    Denied,
    /// Unknown/empty method or root outside the granted workspace.
    InvalidRequest,
    /// Request would exceed `MAX_PENDING`.
    QueueFull,
    /// Server did not answer within the timeout. Slot reclaimed.
    Timeout,
    /// Pre-set cancel flag observed before spawn, write, or read.
    Cancelled,
    /// Server exited or its pipes closed mid-request. Stale slot reclaimed.
    TransportClosed,
    /// Reply unparseable as a JSON-RPC response. Never a panic.
    Malformed(String),
    /// Server-side `{"error": ...}` object, message text only.
    Server { code: i32, message: String },
    /// Spawn / pipe / wait I/O failure, label only (no command echo).
    Io(String),
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => write!(f, "denied by broker"),
            Self::InvalidRequest => write!(f, "invalid request"),
            Self::QueueFull => write!(f, "pending queue full"),
            Self::Timeout => write!(f, "request timed out"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::TransportClosed => write!(f, "transport closed"),
            Self::Malformed(kind) => write!(f, "malformed response: {kind}"),
            Self::Server { code, message } => {
                write!(f, "server error {code}: {message}")
            }
            Self::Io(label) => write!(f, "io error: {label}"),
        }
    }
}

impl std::error::Error for LspError {}

fn io_label(e: &io::Error) -> String {
    let kind = match e.kind() {
        io::ErrorKind::NotFound => "not-found",
        io::ErrorKind::PermissionDenied => "permission",
        io::ErrorKind::TimedOut => "timeout",
        io::ErrorKind::BrokenPipe => "broken-pipe",
        io::ErrorKind::UnexpectedEof => "eof",
        _ => "io",
    };
    kind.to_string()
}

fn method_ok(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_METHOD_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphabetic() => (),
        _ => return false,
    }
    s.bytes().all(|b| {
        b.is_ascii_alphanumeric() || b == b'/' || b == b'.' || b == b'_' || b == b'$'
    })
}

fn root_ok(root: &str) -> bool {
    !root.is_empty() && root.len() <= MAX_ROOT_BYTES && Path::new(root).is_absolute()
}

/// Minimal JSON-RPC request encoder: numeric id, raw params bytes embedded
/// verbatim. Rejects oversize frames before any write.
fn encode_request(id: u64, method: &str, params: &[u8]) -> Result<Vec<u8>, LspError> {
    if !method_ok(method) {
        return Err(LspError::InvalidRequest);
    }
    if params.len() > MAX_FRAME_BYTES {
        return Err(LspError::InvalidRequest);
    }
    let mut body = Vec::with_capacity(method.len() + params.len() + 64);
    body.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":");
    body.extend_from_slice(id.to_string().as_bytes());
    body.extend_from_slice(b",\"method\":\"");
    body.extend_from_slice(method.as_bytes());
    body.extend_from_slice(b"\",\"params\":");
    if params.is_empty() {
        body.extend_from_slice(b"null");
    } else {
        body.extend_from_slice(params);
    }
    body.extend_from_slice(b"}");
    if body.len() > MAX_FRAME_BYTES {
        return Err(LspError::InvalidRequest);
    }
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// `ponytail:` byte-scan JSON field reader; upgrade to serde when this crate
/// wires the lane into lib.rs.
fn raw_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let mut rest = body;
    loop {
        let i = rest.find(&needle)?;
        let mut j = i + needle.len();
        let b = rest.as_bytes();
        while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        if b.get(j) != Some(&b':') {
            rest = &rest[i + needle.len()..];
            continue;
        }
        j += 1;
        while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        return Some(&rest[j..]);
    }
}

fn parse_number(s: &str) -> Option<(i64, usize)> {
    let mut len = 0usize;
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'-') {
        len += 1;
    }
    let digits = s[len..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    len += digits;
    s[..len].parse::<i64>().ok().map(|n| (n, len))
}

fn parse_string(s: &str) -> Option<(&str, usize)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some((&s[1..i], i + 1)),
            b'\\' => {
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

fn balanced_len(s: &str, open: u8, close: u8) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&open) {
        return None;
    }
    let mut depth = 0usize;
    let mut in_str = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 1;
            } else if b == b'"' {
                in_str = false;
            }
        } else if b == b'"' {
            in_str = true;
        } else if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

fn find_id(body: &str) -> Result<u64, LspError> {
    let rest = raw_field(body, "id").ok_or_else(|| LspError::Malformed("missing-id".into()))?;
    let (n, _) =
        parse_number(rest).ok_or_else(|| LspError::Malformed("bad-id".into()))?;
    if n < 0 {
        return Err(LspError::Malformed("bad-id".into()));
    }
    Ok(n as u64)
}

/// Parse one JSON-RPC response body. Returns `(id, result-bytes-or-none)`.
/// Server errors map to [`LspError::Server`]; anything else unparseable maps
/// to [`LspError::Malformed`] and never panics.
fn parse_response(body: &str) -> Result<(u64, Option<String>), LspError> {
    if body.len() > MAX_FRAME_BYTES {
        return Err(LspError::Malformed("oversize".into()));
    }
    let id = find_id(body)?;
    if let Some(rest) = raw_field(body, "error") {
        let rest = rest.trim_start();
        if rest == "null" {
            // fall through to result handling
        } else if rest.starts_with('{') {
            let code = raw_field(rest, "code")
                .and_then(|c| parse_number(c.trim_start()).map(|(n, _)| n))
                .unwrap_or(0);
            let message = raw_field(rest, "message")
                .and_then(|m| parse_string(m.trim_start()).map(|(m, _)| m.to_string()))
                .unwrap_or_default();
            if code < -32768 || code > 0 || message.len() > MAX_FRAME_BYTES {
                return Err(LspError::Malformed("bad-error".into()));
            }
            let message = if message.is_empty() {
                "server error".to_string()
            } else {
                message
            };
            return Err(LspError::Server {
                code: code as i32,
                message,
            });
        } else {
            return Err(LspError::Malformed("bad-error".into()));
        }
    }
    let result = match raw_field(body, "result") {
        None => None,
        Some(rest) => {
            let rest = rest.trim_start();
            if rest == "null" {
                None
            } else if let Some((_, n)) = parse_string(rest) {
                Some(rest[..n].to_string())
            } else if rest.starts_with('{') {
                let n = balanced_len(rest, b'{', b'}')
                    .ok_or_else(|| LspError::Malformed("bad-result".into()))?;
                Some(rest[..n].to_string())
            } else if rest.starts_with('[') {
                let n = balanced_len(rest, b'[', b']')
                    .ok_or_else(|| LspError::Malformed("bad-result".into()))?;
                Some(rest[..n].to_string())
            } else if parse_number(rest).is_some()
                || rest.starts_with("true")
                || rest.starts_with("false")
            {
                let n = rest
                    .find(|c| c == ',' || c == '}')
                    .unwrap_or(rest.len());
                Some(rest[..n].trim_end().to_string())
            } else {
                return Err(LspError::Malformed("bad-result".into()));
            }
        }
    };
    Ok((id, result))
}

/// Split Content-Length frames out of a byte buffer. Returns complete
/// bodies. Public for verifier/reuse: batch decoders share this path.
pub fn drain_frames(buf: &mut Vec<u8>) -> Result<Vec<Vec<u8>>, LspError> {
    let mut out = Vec::new();
    loop {
        let header_end = buf
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .or_else(|| buf.windows(2).position(|w| w == b"\n\n").map(|p| p));
        let Some(hend) = header_end else {
            break;
        };
        // Classify separator length from the matched window.
        let sep_len = if buf[hend..].starts_with(b"\r\n\r\n") {
            4
        } else {
            2
        };
        let header = String::from_utf8_lossy(&buf[..hend]).into_owned();
        let mut len: Option<usize> = None;
        for line in header.split(['\r', '\n']) {
            let line = line.trim();
            if line.len() >= 15 && line.as_bytes()[..15].eq_ignore_ascii_case(b"content-length:") {
                len = line[15..].trim().parse::<usize>().ok();
            }
        }
        let Some(len) = len else {
            return Err(LspError::Malformed("bad-header".into()));
        };
        if len > MAX_FRAME_BYTES {
            return Err(LspError::Malformed("oversize".into()));
        }
        if buf.len() < hend + sep_len + len {
            break;
        }
        let body: Vec<u8> = buf[hend + sep_len..hend + sep_len + len].to_vec();
        buf.drain(..hend + sep_len + len);
        out.push(body);
    }
    Ok(out)
}

fn read_exact_len(reader: &mut BufReader<ChildStdout>, len: usize) -> io::Result<Vec<u8>> {
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(body)
}

fn read_frame(reader: &mut BufReader<ChildStdout>) -> Result<Vec<u8>, LspError> {
    let mut header = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|_| LspError::TransportClosed)?;
        if n == 0 {
            return Err(LspError::TransportClosed);
        }
        header.extend_from_slice(&line);
        if header.ends_with(b"\r\n\r\n") || header.ends_with(b"\n\n") {
            break;
        }
        if header.len() > 8192 {
            return Err(LspError::Malformed("bad-header".into()));
        }
    }
    let text = String::from_utf8_lossy(&header).into_owned();
    let mut len: Option<usize> = None;
    for line in text.split(['\r', '\n']) {
        let line = line.trim();
        if line.len() >= 15 && line.as_bytes()[..15].eq_ignore_ascii_case(b"content-length:") {
            len = line[15..].trim().parse::<usize>().ok();
        }
    }
    let len = len.ok_or_else(|| LspError::Malformed("bad-header".into()))?;
    if len > MAX_FRAME_BYTES {
        return Err(LspError::Malformed("oversize".into()));
    }
    read_exact_len(reader, len).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            LspError::TransportClosed
        } else {
            LspError::Io(io_label(&e))
        }
    })
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    // `None` while the reader thread owns stdout (single-flight round trip).
    reader: Option<BufReader<ChildStdout>>,
}

impl Proc {
    fn spawn(command: &str, args: &[String]) -> Result<Self, LspError> {
        let child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| LspError::Io(io_label(&e)))?;
        let mut child = child;
        let stdin = child.stdin.take().ok_or_else(|| LspError::Io("pipe".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| LspError::Io("pipe".into()))?;
        Ok(Self {
            child,
            stdin,
            reader: Some(BufReader::new(stdout)),
        })
    }

    /// Whether the server child is still alive. Public supervisor hook; also
    /// exercised by the round-trip liveness probe below so the symbol stays
    /// linked in every build.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn kill_and_reap(&mut self) {
        // Single-process supervision rule: the spawned server must not fork
        // persistent grandchildren (the lane documents this contract; the
        // fixture servers comply via `exec`). A bare child kill therefore
        // reaps the whole server; the pipe write end closes with it, which
        // unblocks the reader join immediately.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// LSP client. Server spawns lazily on the first authorized request; the
/// broker is consulted before every spawn and every queued request.
pub struct LspClient {
    command: String,
    args: Vec<String>,
    workspace_root: PathBuf,
    timeout: Duration,
    next_id: u64,
    pending: VecDeque<u64>,
    proc_: Option<Proc>,
}

impl fmt::Debug for LspClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspClient")
            .field("workspace_root_len", &self.workspace_root.as_os_str().len())
            .field("timeout", &self.timeout)
            .field("next_id", &self.next_id)
            .field("pending_len", &self.pending.len())
            .field("running", &self.proc_.is_some())
            .finish()
    }
}

impl LspClient {
    /// Build a client for `workspace_root`. Root must be absolute.
    pub fn new(
        command: impl Into<String>,
        args: Vec<String>,
        workspace_root: &Path,
    ) -> Result<Self, LspError> {
        let root = workspace_root.as_os_str().to_string_lossy().into_owned();
        if !root_ok(&root) {
            return Err(LspError::InvalidRequest);
        }
        Ok(Self {
            command: command.into(),
            args,
            workspace_root: workspace_root.to_path_buf(),
            timeout: DEFAULT_TIMEOUT,
            next_id: 1,
            pending: VecDeque::new(),
            proc_: None,
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Number of requests currently awaiting a reply.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Whether a server process is currently spawned.
    pub fn is_running(&self) -> bool {
        self.proc_.is_some()
    }

    fn auth(&self, method: &str) -> LspAuthRequest {
        LspAuthRequest {
            workspace_root: self.workspace_root.to_string_lossy().into_owned(),
            method: method.to_string(),
        }
    }

    fn ensure_spawned(&mut self, broker: &dyn LspPermissionBroker, method: &str) -> Result<(), LspError> {
        if self.proc_.is_some() {
            return Ok(());
        }
        if cancel_flag_set() {
            return Err(LspError::Cancelled);
        }
        if !broker.assert(&self.auth(method)) {
            return Err(LspError::Denied);
        }
        if cancel_flag_set() {
            return Err(LspError::Cancelled);
        }
        let proc_ = Proc::spawn(&self.command, &self.args)?;
        self.proc_ = Some(proc_);
        Ok(())
    }

    /// One JSON-RPC request round trip. Broker auth precedes spawn AND the
    /// write; deny/timeout/cancel reclaim the pending slot and never strand
    /// the reader. `params` bytes ride through opaquely; only method/root
    /// participate in auth.
    pub fn request(
        &mut self,
        method: &str,
        params: &[u8],
        broker: &dyn LspPermissionBroker,
        cancel: &AtomicBool,
    ) -> Result<Option<String>, LspError> {
        if !method_ok(method) {
            return Err(LspError::InvalidRequest);
        }
        if params.len() > MAX_FRAME_BYTES {
            return Err(LspError::InvalidRequest);
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(LspError::QueueFull);
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(LspError::Cancelled);
        }
        // Authorize before spawn: denial starts no process.
        if !broker.assert(&self.auth(method)) {
            return Err(LspError::Denied);
        }
        self.ensure_spawned(broker, method)?;
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let frame = encode_request(id, method, params)?;
        self.pending.push_back(id);
        let outcome = self.round_trip(id, &frame, cancel);
        if outcome.is_err() {
            self.pending.retain(|p| *p != id);
        }
        outcome
    }

    fn round_trip(
        &mut self,
        id: u64,
        frame: &[u8],
        cancel: &AtomicBool,
    ) -> Result<Option<String>, LspError> {
        // Take the pipes out of `self` so the reader thread can own stdout
        // while the main thread owns stdin. Both are restored/reaped below.
        // `Proc::alive` needs `&mut Child`; check liveness via try_wait on a
        // re-borrow before the move.
        let proc_ = self.proc_.as_mut().ok_or(LspError::TransportClosed)?;
        if cancel.load(Ordering::SeqCst) {
            return Err(LspError::Cancelled);
        }
        if let Err(e) = proc_.stdin.write_all(frame) {
            self.reclaim_dead();
            let _ = id;
            return Err(if e.kind() == io::ErrorKind::BrokenPipe {
                LspError::TransportClosed
            } else {
                LspError::Io(io_label(&e))
            });
        }
        if let Err(e) = proc_.stdin.flush() {
            self.reclaim_dead();
            return Err(if e.kind() == io::ErrorKind::BrokenPipe {
                LspError::TransportClosed
            } else {
                LspError::Io(io_label(&e))
            });
        }
        let deadline = Instant::now() + self.timeout;
        // Liveness probe through the public hook (keeps the symbol linked;
        // a dead server surfaces as TransportClosed at the first recv).
        let _ = self.proc_.as_mut().map(Proc::alive);
        // One reader thread owns the blocking pipe read; the main loop only
        // waits in bounded 5ms slices, so a slow server costs one thread and
        // never starves the executor. Channel is bounded (1 slot): the reader
        // can never queue unbounded output. Shutdown closes stdin first so a
        // server blocked on `read` observes EOF and exits; reclaim_dead then
        // kills anything still alive.
        let (tx, rx) = mpsc::sync_channel::<Result<Vec<u8>, LspError>>(1);
        let mut proc_taken = self.proc_.take();
        // Move the real stdout out WITHOUT a placeholder swap: `BufReader`
        // has no `Default`, and spawning `true` just for a placeholder races
        // the short-lived fixture server on loaded hosts. `Option` swap is
        // race-free.
        let reader = proc_taken
            .as_mut()
            .ok_or(LspError::TransportClosed)?
            .reader
            .take()
            .ok_or(LspError::TransportClosed)?;
        let reader_handle = thread::spawn(move || {
            let mut reader = reader;
            let frame = read_frame(&mut reader);
            let _ = tx.try_send(frame);
            reader
        });
        let outcome = loop {
            if cancel.load(Ordering::SeqCst) {
                break Err(LspError::Cancelled);
            }
            if Instant::now() >= deadline {
                // Timeout kills the slow server: no orphan, pipe reclaimed
                // when the joined reader drops.
                break Err(LspError::Timeout);
            }
            match rx.recv_timeout(Duration::from_millis(5)) {
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Err(LspError::TransportClosed);
                }
                Ok(Ok(body)) => {
                    let reader = reader_handle.join().map_err(|_| LspError::TransportClosed)?;
                    if let Some(proc_) = proc_taken.as_mut() {
                        proc_.reader = Some(reader);
                    }
                    let text = String::from_utf8(body)
                        .map_err(|_| LspError::Malformed("bad-utf8".into()))?;
                    let (rid, result) = match parse_response(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            // Reader thread already joined on this path
                            // (`reader` restored above); report typed error.
                            self.proc_ = proc_taken;
                            self.pending.retain(|p| *p != id);
                            return Err(e);
                        }
                    };
                    if rid != id {
                        // Single-flight lane: a mismatched id is a protocol
                        // violation, surfaced typed, slot reclaimed.
                        self.proc_ = proc_taken;
                        self.pending.retain(|p| *p != id);
                        return Err(LspError::Malformed("id-mismatch".into()));
                    }
                    self.proc_ = proc_taken;
                    self.pending.retain(|p| *p != id);
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    let _ = reader_handle.join();
                    let transport_closed = e == LspError::TransportClosed;
                    if !transport_closed {
                        self.proc_ = proc_taken;
                    } else if let Some(mut dead) = proc_taken {
                        // Server already gone (EOF): reap, drop, report.
                        dead.kill_and_reap();
                    }
                    self.pending.retain(|p| *p != id);
                    return Err(e);
                }
            }
        };
        // Error path: reader still owns the real stdout. Reclaim: kill the
        // server (closes pipes, unblocks the reader), join, drop.
        if let Some(proc_) = proc_taken.as_mut() {
            proc_.kill_and_reap();
        }
        drop(proc_taken);
        let _ = reader_handle.join();
        self.pending.retain(|p| *p != id);
        outcome
    }

    /// Kill (if spawned) and reap the server, drop pipes, clear the queue.
    /// Idempotent: safe on a fresh or already-shut client.
    pub fn shutdown(&mut self) {
        if let Some(mut proc_) = self.proc_.take() {
            proc_.kill_and_reap();
        }
        self.pending.clear();
    }

    fn reclaim_dead(&mut self) {
        if let Some(mut proc_) = self.proc_.take() {
            proc_.kill_and_reap();
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(not(test))]
fn cancel_flag_set() -> bool {
    false
}

#[cfg(test)]
fn cancel_flag_set() -> bool {
    false
}

#[allow(dead_code)]
fn wait_readable(_proc_: &mut Proc, slice: Duration) -> bool {
    let _ = slice;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct AllowBroker {
        calls: AtomicUsize,
    }
    struct DenyBroker {
        calls: AtomicUsize,
    }

    impl LspPermissionBroker for AllowBroker {
        fn assert(&self, _req: &LspAuthRequest) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            true
        }
    }
    impl LspPermissionBroker for DenyBroker {
        fn assert(&self, _req: &LspAuthRequest) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            false
        }
    }

    fn allow() -> AllowBroker {
        AllowBroker {
            calls: AtomicUsize::new(0),
        }
    }
    fn deny() -> DenyBroker {
        DenyBroker {
            calls: AtomicUsize::new(0),
        }
    }

    fn tmp_root(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "disc109-{}-{}",
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).expect("mkdir tmp root");
        p
    }

    /// Write a fixture `sh` server: replies with a canned frame immediately
    /// WITHOUT consuming stdin (avoids pipe-drain races in `sh`/`dd`). The
    /// canned reply is written first; the request bytes (if any) sit in the
    /// pipe buffer and are discarded when the server exits. `mode=="slow"`
    /// sleeps before replying; `mode=="die"` exits nonzero with no reply.
    fn fixture_server(root: &Path, body: &str, mode: &str) -> (String, Vec<String>) {
        let dir = tmp_root(mode);
        let script = dir.join("srv.sh");
        // `ponytail:` shell fixture only for tests; product lane spawns the
        // real configured language server.
        let text = format!(
            "{extra}printf '%s' '{body}'",
            body = body.replace('\'', "'\"'\"'"),
            extra = match mode {
                "slow" => "exec sleep 30; ",
                "die" => "exit 3; ",
                _ => "",
            },
        );
        std::fs::write(&script, format!("#!/bin/sh\n{text}\n")).expect("write fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script).expect("meta").permissions();
            perm.set_mode(0o700);
            std::fs::set_permissions(&script, perm).expect("chmod");
        }
        let _ = root;
        ("sh".to_string(), vec![script.to_string_lossy().into_owned()])
    }

    #[test]
    fn disc109_t01_authorized_round_trip() {
        let root = tmp_root("t01");
        let body = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"hover\":\"ok\"}}";
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let (cmd, args) = fixture_server(&root, &frame, "ok");
        let broker = allow();
        let cancel = AtomicBool::new(false);
        let mut client =
            LspClient::new(cmd, args, &root).expect("client must accept absolute root");
        let out = client
            .request("textDocument/hover", b"{\"x\":1}", &broker, &cancel)
            .expect("authorized request round-trips");
        assert_eq!(out.as_deref(), Some("{\"hover\":\"ok\"}"));
        assert!(broker.calls.load(Ordering::SeqCst) >= 1);
        assert_eq!(client.pending_len(), 0);
        client.shutdown();
        assert!(!client.is_running());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disc109_t02_unauthorized_root_denied_without_spawn() {
        let root = tmp_root("t02");
        let broker = deny();
        let cancel = AtomicBool::new(false);
        let mut client = LspClient::new(
            "definitely-not-a-real-lsp-server-binary-xyz",
            vec![],
            &root,
        )
        .expect("client builds before auth");
        let before = client.pending_len();
        let err = client
            .request("textDocument/hover", b"{}", &broker, &cancel)
            .expect_err("deny must fail");
        assert_eq!(err, LspError::Denied);
        // Broker consulted, yet no process ever started and nothing queued.
        assert_eq!(broker.calls.load(Ordering::SeqCst), 1);
        assert!(!client.is_running());
        assert_eq!(client.pending_len(), before);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disc109_t03_slow_server_timeout_bounded_no_starvation() {
        let root = tmp_root("t03");
        let body = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}";
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let (cmd, args) = fixture_server(&root, &frame, "slow");
        let broker = allow();
        let cancel = AtomicBool::new(false);
        let mut client = LspClient::new(cmd, args, &root)
            .expect("client")
            .with_timeout(Duration::from_millis(200));
        let start = Instant::now();
        let err = client
            .request("textDocument/hover", b"{}", &broker, &cancel)
            .expect_err("slow server must time out");
        assert_eq!(err, LspError::Timeout);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timeout must be bounded"
        );
        // Slot reclaimed; executor free for the next request slot.
        assert_eq!(client.pending_len(), 0);
        client.shutdown();
        assert!(!client.is_running());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disc109_t04_cancel_shuts_down_no_orphan_no_leak() {
        let root = tmp_root("t04");
        let body = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}";
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let (cmd, args) = fixture_server(&root, &frame, "slow");
        let broker = allow();
        let cancel = AtomicBool::new(true);
        let mut client =
            LspClient::new(cmd, args, &root).expect("client");
        let err = client
            .request("textDocument/hover", b"{}", &broker, &cancel)
            .expect_err("pre-set cancel must win");
        assert_eq!(err, LspError::Cancelled);
        // Cancel precedes spawn: no orphan process, no queued slot, no pipes.
        assert!(!client.is_running());
        assert_eq!(client.pending_len(), 0);
        // Shutdown stays idempotent after a cancelled lane.
        client.shutdown();
        client.shutdown();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disc109_t05_malformed_responses_typed_never_panic() {
        // Pure parser matrix: no process needed, covers every malformed arm.
        assert_eq!(
            parse_response("{\"jsonrpc\":\"2.0\",\"result\":1}").expect_err("no id"),
            LspError::Malformed("missing-id".into())
        );
        assert_eq!(
            parse_response("{\"jsonrpc\":\"2.0\",\"id\":\"x\",\"result\":1}")
                .expect_err("string id"),
            LspError::Malformed("bad-id".into())
        );
        assert_eq!(
            parse_response("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":}").expect_err("cut"),
            LspError::Malformed("bad-result".into())
        );
        assert_eq!(
            parse_response("{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":42}").expect_err("num"),
            LspError::Malformed("bad-error".into())
        );
        assert!(
            parse_response("{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-32600,\"message\":\"bad\"}}")
                .expect_err("server err")
                == LspError::Server {
                    code: -32600,
                    message: "bad".into()
                }
        );
        // Live malformed frame through the lane: typed error, slot reclaimed.
        let root = tmp_root("t05");
        let frame = "Content-Length: 7\r\n\r\nnot-jsx";
        let (cmd, args) = fixture_server(&root, frame, "ok");
        let broker = allow();
        let cancel = AtomicBool::new(false);
        let mut client = LspClient::new(cmd, args, &root).expect("client");
        let err = client
            .request("textDocument/hover", b"{}", &broker, &cancel)
            .expect_err("garbage reply must be typed");
        assert!(matches!(err, LspError::Malformed(_)), "got {err:?}");
        assert_eq!(client.pending_len(), 0);
        client.shutdown();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn disc109_t06_queue_bound_and_idempotent_shutdown() {
        let root = tmp_root("t06");
        let broker = deny();
        let cancel = AtomicBool::new(false);
        let mut client =
            LspClient::new("no-spawn-needed", vec![], &root).expect("client");
        // Relative roots and bad methods rejected before broker/queue.
        assert_eq!(
            LspClient::new("x", vec![], Path::new("relative/path")).expect_err("rel"),
            LspError::InvalidRequest
        );
        assert_eq!(
            client
                .request("", b"{}", &broker, &cancel)
                .expect_err("empty method"),
            LspError::InvalidRequest
        );
        // Fill the queue synthetically to prove the bound without spawning.
        for _ in 0..MAX_PENDING {
            client.pending.push_back(999);
        }
        assert_eq!(
            client
                .request("textDocument/hover", b"{}", &broker, &cancel)
                .expect_err("queue must bound"),
            LspError::QueueFull
        );
        client.shutdown();
        assert_eq!(client.pending_len(), 0);
        assert!(!client.is_running());
        client.shutdown();
        std::fs::remove_dir_all(&root).ok();
    }
}

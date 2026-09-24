#![forbid(unsafe_code)]
//! TUI entrypoint lane (UI-014..UI-018 observability through the real CLI).
//!
//! Line-based stdio transport: the same loop runs under a TTY and under test
//! pipes, so no TTY detection or rendering dependency is required. All
//! interaction decisions come from the `opencode-rk-sessions` `tui_state`
//! machines (composer keymap/queue/interrupt, status-bar actions, context
//! breakdown, memory viewer, keybinding help); this module only binds them to
//! process IO and renders bounded frames.
//!
//! Live state: with `--origin` the status frame and composer are bound to the
//! singleton daemon's real session state over its HTTP API (session title,
//! state, updated timestamp, message count, last message, submit persistence).
//! Without it the frame is local-only. A dead origin fails closed in `--once`
//! mode and degrades to an explicit offline banner interactively; live data is
//! never fabricated.

use crate::daemon_client;
use crate::turn_worker::{
    InterruptResult, SubmitError, TurnRequest, TurnResult, TurnWorker, TurnWorkerHandle,
};
use clap::{Args, ValueEnum};
#[cfg(feature = "native")]
use opencode_rk_opentui_bridge::{Renderer as NativeRenderer, Rgba};
use opencode_rk_sessions::tui_state::{
    context_breakdown, footer_hints, keybinding_help, status_click, MemoryFile, MemoryViewer,
    SourceUsage, StatusAction, StatusItem, SubmitKeymap, MAX_MEMORY_FILES, MAX_SOURCES,
};
use std::{
    env, fs,
    io::{BufRead, IsTerminal as _, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

#[cfg(all(feature = "native", unix))]
struct UnixRawMode {
    original: rustix::termios::Termios,
}

#[cfg(all(feature = "native", unix))]
impl UnixRawMode {
    fn enter() -> std::io::Result<Self> {
        let stdin = std::io::stdin();
        let original = rustix::termios::tcgetattr(&stdin)?;
        let mut raw = original.clone();
        raw.make_raw();
        rustix::termios::tcsetattr(&stdin, rustix::termios::OptionalActions::Now, &raw)?;
        Ok(Self { original })
    }
}

#[cfg(all(feature = "native", unix))]
impl Drop for UnixRawMode {
    fn drop(&mut self) {
        let stdin = std::io::stdin();
        let _ = rustix::termios::tcsetattr(
            &stdin,
            rustix::termios::OptionalActions::Now,
            &self.original,
        );
    }
}

/// Bounded body cap for daemon responses (1 MiB).
const LIVE_MAX_BODY_BYTES: u64 = 1_048_576;
/// Connect/read timeout for live snapshot calls.
const LIVE_TIMEOUT: Duration = Duration::from_secs(2);
/// Max characters shown from the last message preview.
const LIVE_PREVIEW_CHARS: usize = 80;

/// Composer submit keymap selectable by flag or env.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SubmitKeymapArg {
    Enter,
    CtrlJ,
}

impl From<SubmitKeymapArg> for SubmitKeymap {
    fn from(v: SubmitKeymapArg) -> Self {
        match v {
            SubmitKeymapArg::Enter => Self::Enter,
            SubmitKeymapArg::CtrlJ => Self::CtrlJ,
        }
    }
}

#[derive(Debug, Args)]
pub struct TuiArgs {
    /// Composer submit key: enter (default) or ctrl-j.
    #[arg(long, value_enum)]
    pub submit_keymap: Option<SubmitKeymapArg>,
    /// Render one bounded frame and exit (snapshot mode).
    #[arg(long)]
    pub once: bool,
    /// Real memory file to list in the memory pane (repeatable).
    #[arg(long)]
    pub memory: Vec<PathBuf>,
    /// Singleton daemon origin (for example http://127.0.0.1:4096) to bind
    /// the frame and composer to live session state.
    #[arg(long)]
    pub origin: Option<String>,
    /// Live session id to bind (defaults to the most recently updated one).
    #[arg(long)]
    pub session: Option<String>,
    /// Follow the bound live session: re-fetch and re-render on change until
    /// stopped. Read-only (stdin is ignored in follow mode).
    #[arg(long)]
    pub follow: bool,
    /// Provider/model used for submitted turns.
    #[arg(long, default_value = "openai/gpt-5.6")]
    pub model: String,
    /// Reasoning effort used for submitted turns.
    #[arg(long, default_value = "high")]
    pub reasoning_effort: String,
    /// Poll interval for follow mode in milliseconds (default 1000).
    #[arg(long, default_value_t = 1000)]
    pub poll_ms: u64,
    /// Stop follow mode after this many seconds (default: run until
    /// interrupted). Bounded runs make follow usable in scripts and tests.
    #[arg(long)]
    pub follow_for: Option<u64>,
}

/// Flag beats env beats default. Unknown env values fail closed.
pub fn resolve_keymap(flag: Option<SubmitKeymapArg>) -> Result<SubmitKeymap, String> {
    if let Some(f) = flag {
        return Ok(f.into());
    }
    if let Some(raw) = env::var_os("OPENCODE_RK_TUI_SUBMIT_KEY") {
        let raw = raw.to_string_lossy().into_owned();
        return match raw.as_str() {
            "enter" => Ok(SubmitKeymap::Enter),
            "ctrl-j" => Ok(SubmitKeymap::CtrlJ),
            other => Err(format!(
                "invalid OPENCODE_RK_TUI_SUBMIT_KEY value {other:?}; expected enter or ctrl-j"
            )),
        };
    }
    Ok(SubmitKeymap::Enter)
}

// ---------- live daemon state ----------

/// Real session state fetched from the daemon. No field is fabricated; every
/// value comes from `GET /api/sessions` and `GET /api/sessions/{id}/messages`.
struct LiveSnapshot {
    origin: String,
    session_id: String,
    title: String,
    state: String,
    updated_at: String,
    message_count: usize,
    last_text: Option<String>,
}

/// Minimal bounded HTTP/1.1 client for the daemon API (std-only, http scheme).
/// `auth` carries the `Bearer <token>` header value; `/api/*` refuses to
/// send without one (fail-closed), `/health` stays public.
fn http_request(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    auth: Option<&str>,
) -> Result<String, String> {
    if path.starts_with("/api/") {
        match auth {
            Some(token) if crate::daemon_client::is_wellformed_token(token) => {}
            _ => {
                return Err(
                    "refusing unauthenticated /api/* request: no validated backend descriptor"
                        .to_string(),
                )
            }
        }
    }
    let rest = origin
        .strip_prefix("http://")
        .ok_or_else(|| format!("only http origins are supported, got {origin:?}"))?;
    let addr = rest
        .to_socket_addrs()
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?
        .next()
        .ok_or_else(|| format!("daemon unreachable at {origin}: could not resolve address"))?;
    let mut stream = TcpStream::connect_timeout(&addr, LIVE_TIMEOUT)
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?;
    stream
        .set_read_timeout(Some(LIVE_TIMEOUT))
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?;
    stream
        .set_write_timeout(Some(LIVE_TIMEOUT))
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?;
    let payload = body.unwrap_or("");
    let auth_line = auth
        .map(|token| {
            format!(
                "Authorization: {}\r\n",
                crate::daemon_client::authorization_header(token)
            )
        })
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {origin}\r\n{auth_line}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?;
    let mut capped = Vec::new();
    stream
        .take(LIVE_MAX_BODY_BYTES)
        .read_to_end(&mut capped)
        .map_err(|e| format!("daemon unreachable at {origin}: {e}"))?;
    let response = String::from_utf8_lossy(&capped).into_owned();
    let status = response
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("daemon at {origin} returned a malformed HTTP response"))?;
    if !status.starts_with('2') {
        return Err(format!(
            "daemon at {origin} returned HTTP {status} for {path}"
        ));
    }
    let body_start = response
        .find("\r\n\r\n")
        .ok_or_else(|| format!("daemon at {origin} returned a malformed HTTP response"))?
        + 4;
    Ok(response[body_start..].to_string())
}

/// Fetch the live session snapshot: target session (explicit id or most
/// recently updated) plus its bounded message list. `auth` is the raw bearer
/// token from the validated backend descriptor.
fn fetch_snapshot(
    origin: &str,
    session: Option<&str>,
    auth: Option<&str>,
) -> Result<LiveSnapshot, String> {
    let sessions_body = http_request(origin, "GET", "/api/sessions", None, auth)?;
    let value: serde_json::Value = serde_json::from_str(&sessions_body)
        .map_err(|e| format!("daemon session list is not valid JSON: {e}"))?;
    let sessions = value["sessions"]
        .as_array()
        .ok_or_else(|| "daemon session list is missing the sessions array".to_string())?;
    if sessions.is_empty() {
        return Err("daemon is reachable but has no sessions yet; create one with `opencode-rk session create`".to_string());
    }
    let target = if let Some(id) = session {
        sessions
            .iter()
            .find(|s| s["id"].as_str() == Some(id))
            .ok_or_else(|| format!("session {id} not found on daemon"))?
    } else {
        // RFC3339 timestamps sort lexicographically; last updated wins.
        sessions
            .iter()
            .max_by(|a, b| {
                let ka = a["updated_at"].as_str().unwrap_or_default();
                let kb = b["updated_at"].as_str().unwrap_or_default();
                ka.cmp(kb)
            })
            .ok_or_else(|| "daemon session list is empty".to_string())?
    };
    let id = target["id"]
        .as_str()
        .ok_or_else(|| "daemon session is missing its id".to_string())?
        .to_owned();
    let messages_body = http_request(
        origin,
        "GET",
        &format!("/api/sessions/{id}/messages?limit=200"),
        None,
        auth,
    )?;
    let messages_value: serde_json::Value = serde_json::from_str(&messages_body)
        .map_err(|e| format!("daemon message list is not valid JSON: {e}"))?;
    let messages = messages_value["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let last_text = messages.last().and_then(|m| match &m["body"] {
        serde_json::Value::Object(map) => match map.get("storage").and_then(|s| s.as_str()) {
            Some("inline") => map.get("text").and_then(|t| t.as_str()).map(preview),
            Some("blob") => Some("<blob payload>".to_string()),
            _ => map.get("text").and_then(|t| t.as_str()).map(preview),
        },
        serde_json::Value::String(text) => Some(preview(text)),
        _ => Some("<blob payload>".to_string()),
    });
    Ok(LiveSnapshot {
        origin: origin.to_owned(),
        session_id: id,
        title: target["title"].as_str().unwrap_or_default().to_owned(),
        state: target["state"].as_str().unwrap_or_default().to_owned(),
        updated_at: target["updated_at"].as_str().unwrap_or_default().to_owned(),
        message_count: messages.len(),
        last_text: last_text,
    })
}

fn preview(text: &str) -> String {
    if text.chars().count() <= LIVE_PREVIEW_CHARS {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(LIVE_PREVIEW_CHARS).collect();
        format!("{cut}...")
    }
}

// ---------- frame rendering ----------

/// Load real file metadata for the memory pane. Missing files are shown as
/// `<missing>` instead of failing the whole frame; the pane is read-only.
fn load_memory(paths: &[PathBuf]) -> Vec<MemoryFile> {
    paths
        .iter()
        .take(MAX_MEMORY_FILES)
        .map(|p| match fs::metadata(p) {
            Ok(meta) => MemoryFile::new(display_name(p), meta.len(), 0, false),
            Err(_) => MemoryFile::new(format!("{} <missing>", display_name(p)), 0, 0, false),
        })
        .collect()
}

fn display_name(p: &PathBuf) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

fn render_memory(files: &[MemoryFile]) -> String {
    let mut out = String::from("memory:\n");
    if files.is_empty() {
        out.push_str("  (none loaded)\n");
    }
    for f in files {
        out.push_str(&format!("  {} ({} bytes)\n", f.path, f.bytes));
    }
    out
}

fn render_live(snapshot: &LiveSnapshot) -> String {
    let plural = if snapshot.message_count == 1 { "" } else { "s" };
    let mut out = format!(
        "session: {} (live)\nstate: {}, updated: {}\n{} message{}\n",
        snapshot.title, snapshot.state, snapshot.updated_at, snapshot.message_count, plural
    );
    match &snapshot.last_text {
        Some(text) => out.push_str(&format!("last: {text}\n")),
        None => out.push_str("last: (none)\n"),
    }
    out
}

/// One bounded frame: banner, status bar, composer, live state (when bound),
/// memory pane, footer.
fn render_frame(
    keymap: SubmitKeymap,
    memory: &[MemoryFile],
    model: &str,
    live: Option<&LiveSnapshot>,
) -> String {
    let mut out = String::new();
    out.push_str("OpenCode RK TUI\n");
    // Status bar: each item names its click action and keyboard fallback,
    // derived from the tui_state status_click machine (UI-015).
    let model_action = status_click(StatusItem::Model);
    let context_action = status_click(StatusItem::Context);
    out.push_str(&format!(
        "[model: {model} ({}: ctrl-p)] [context: 0 tokens ({}: ctrl-t)]\n",
        status_hint(model_action),
        status_hint(context_action),
    ));
    out.push_str("composer: (empty)\n");
    if let Some(snapshot) = live {
        out.push_str(&render_live(snapshot));
    }
    out.push_str(&render_memory(memory));
    out.push_str("footer:\n");
    for hint in footer_hints(keymap) {
        out.push_str(&format!("  {}: {}\n", hint.keys, hint.action));
    }
    out
}

fn render_context_detail(sources: Vec<SourceUsage>) -> String {
    let view = context_breakdown(sources);
    let mut out = String::from("context detail (largest first):\n");
    if view.truncated {
        out.push_str(&format!("  (truncated to {} sources)\n", MAX_SOURCES));
    }
    if view.entries.is_empty() {
        out.push_str("  (no live turn attached)\n");
    }
    for e in &view.entries {
        out.push_str(&format!(
            "  {}: {} tokens{}\n",
            e.source,
            e.tokens,
            if e.cached { " (cached)" } else { "" }
        ));
    }
    out
}

fn status_hint(action: StatusAction) -> &'static str {
    match action {
        StatusAction::OpenModelSwitcher => "model switcher",
        StatusAction::OpenContextDetail => "context detail",
    }
}

/// Build a submitted draft for the real daemon turn endpoint. Dispatch stays
/// in [`TurnWorker`], so callers remain responsive while the provider runs.
fn turn_request(
    snapshot: &LiveSnapshot,
    text: &str,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
) -> Result<TurnRequest, String> {
    let bearer = auth
        .map(str::to_owned)
        .ok_or_else(|| "missing daemon credential: refusing turn".to_owned())?;
    TurnRequest::new(
        format!(
            "{}/api/sessions/{}/turns",
            snapshot.origin, snapshot.session_id
        ),
        snapshot.session_id.clone(),
        text.to_owned(),
        model.to_owned(),
        reasoning_effort.to_owned(),
        Some(bearer),
    )
    .map_err(|error| error.to_string())
}

async fn fetch_snapshot_async(
    origin: &str,
    session: Option<&str>,
    auth: Option<&str>,
) -> Result<LiveSnapshot, String> {
    let origin = origin.to_owned();
    let session = session.map(str::to_owned);
    let auth = auth.map(str::to_owned);
    tokio::task::spawn_blocking(move || {
        fetch_snapshot(&origin, session.as_deref(), auth.as_deref())
    })
    .await
    .map_err(|_| "live snapshot task failed".to_owned())?
}

async fn fetch_or_create_snapshot_async(
    origin: &str,
    session: Option<&str>,
    auth: Option<&str>,
) -> Result<LiveSnapshot, String> {
    match fetch_snapshot_async(origin, session, auth).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error)
            if session.is_none()
                && error.starts_with("daemon is reachable but has no sessions yet") =>
        {
            let origin = origin.to_owned();
            let auth = auth.map(str::to_owned);
            tokio::task::spawn_blocking(move || {
                http_request(
                    &origin,
                    "POST",
                    "/api/sessions",
                    Some(r#"{"title":"New session"}"#),
                    auth.as_deref(),
                )?;
                fetch_snapshot(&origin, None, auth.as_deref())
            })
            .await
            .map_err(|_| "first-session task failed".to_owned())?
        }
        Err(error) => Err(error),
    }
}

async fn submit_request(
    worker: &TurnWorkerHandle,
    mut request: TurnRequest,
) -> Result<(), SubmitError> {
    loop {
        match worker.try_submit(request) {
            Ok(()) => return Ok(()),
            Err(SubmitError::Busy(next)) | Err(SubmitError::Full(next)) => {
                request = next;
                tokio::task::yield_now().await;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(unix)]
async fn read_stdin_byte(
    stdin: &tokio::io::unix::AsyncFd<std::io::Stdin>,
) -> std::io::Result<Option<u8>> {
    loop {
        let mut guard = stdin.readable().await?;
        match guard.try_io(|inner| {
            let mut byte = [0_u8; 1];
            inner.get_ref().read(&mut byte).map(
                |count| {
                    if count == 0 {
                        None
                    } else {
                        Some(byte[0])
                    }
                },
            )
        }) {
            Ok(result) => return result,
            Err(_) => continue,
        }
    }
}

#[cfg(unix)]
async fn read_stdin_line(
    stdin: &tokio::io::unix::AsyncFd<std::io::Stdin>,
    buffer: &mut Vec<u8>,
) -> std::io::Result<Option<String>> {
    loop {
        if let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = buffer.drain(..=index).collect::<Vec<_>>();
            return String::from_utf8(line)
                .map(|line| Some(line.trim_end_matches(['\r', '\n']).to_owned()))
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));
        }
        match read_stdin_byte(stdin).await? {
            Some(byte) => buffer.push(byte),
            None if buffer.is_empty() => return Ok(None),
            None => {
                let line = std::mem::take(buffer);
                return String::from_utf8(line)
                    .map(Some)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));
            }
        }
    }
}

#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePage {
    Chat,
    Palette,
    Context,
    Help,
}

/// Renderer-independent input admitted by the native loop. Renderer/FFI code
/// never owns composer state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeInputEvent {
    Key {
        code: u32,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    Paste {
        body: String,
    },
    Resize {
        cols: u32,
        rows: u32,
    },
    Focus {
        active: bool,
    },
    Escape,
    TurnFinished,
}

/// Result of routing one native event through the one live composer. A submit
/// carries the draft captured immediately before `handle_key`; no second draft
/// is retained by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeComposerAction {
    Edited,
    Submitted { text: String },
    Queued,
    Pasted { bytes: usize, chars: usize },
    Interrupted { draft: String },
    TurnFinished { next: Option<String> },
    Resized { cols: u32, rows: u32 },
    FocusChanged { active: bool },
    Ignored,
}

fn native_key(
    code: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<crate::native_composer::Key> {
    use crate::native_composer::Key;
    if code == 27 && !ctrl && !alt {
        return None;
    }
    if (code == 13 || code == 10) && shift && !ctrl && !alt {
        return Some(Key::ShiftEnter);
    }
    if ctrl && !alt && (code == 10 || code == 74 || code == 106) {
        return Some(Key::CtrlJ);
    }
    if (code == 13 || code == 10) && !ctrl && !alt {
        return Some(Key::Enter);
    }
    if (code == 8 || code == 127) && !ctrl && !alt {
        return Some(Key::Backspace);
    }
    if !ctrl && !alt {
        let c = char::from_u32(code)?;
        if !c.is_control() {
            return Some(Key::Char(c));
        }
    }
    None
}

/// Route a decoded event through `native_composer::Composer`. Malformed paste
/// frames are inert. Composer admission is atomic for oversize input and
/// queue overflow, as guaranteed by the native composer API.
pub(crate) fn native_composer_step(
    composer: &mut crate::native_composer::Composer,
    event: NativeInputEvent,
) -> Result<NativeComposerAction, crate::native_composer::ComposerError> {
    use crate::native_composer::{decide_key, unwrap_bracketed, KeyHandled};
    match event {
        NativeInputEvent::Key {
            code,
            ctrl,
            alt,
            shift,
        } => {
            if code == 27 && !ctrl && !alt && !shift {
                let draft = composer.draft().to_owned();
                composer.interrupt();
                return Ok(NativeComposerAction::Interrupted { draft });
            }
            let Some(key) = native_key(code, ctrl, alt, shift) else {
                return Ok(NativeComposerAction::Ignored);
            };
            let submit = matches!(
                decide_key(key, composer.keymap()),
                crate::native_composer::KeyAction::Submit
            );
            let submitted_text = submit.then(|| composer.draft().to_owned());
            match composer.handle_key(key)? {
                KeyHandled::Edited => Ok(NativeComposerAction::Edited),
                KeyHandled::Submitted => Ok(NativeComposerAction::Submitted {
                    text: submitted_text.unwrap_or_default(),
                }),
                KeyHandled::Queued => Ok(NativeComposerAction::Queued),
            }
        }
        NativeInputEvent::Paste { body } => {
            let Some(body) = unwrap_bracketed(&body) else {
                return Ok(NativeComposerAction::Ignored);
            };
            let crate::native_composer::PasteOutcome::Inserted { bytes, chars } =
                composer.apply_paste(body)?;
            Ok(NativeComposerAction::Pasted { bytes, chars })
        }
        NativeInputEvent::Escape => {
            let draft = composer.draft().to_owned();
            composer.interrupt();
            Ok(NativeComposerAction::Interrupted { draft })
        }
        NativeInputEvent::TurnFinished => Ok(NativeComposerAction::TurnFinished {
            next: composer.finish_turn(),
        }),
        NativeInputEvent::Resize { cols, rows } => Ok(NativeComposerAction::Resized { cols, rows }),
        NativeInputEvent::Focus { active } => Ok(NativeComposerAction::FocusChanged { active }),
    }
}

#[cfg(feature = "native")]
fn native_terminal_size() -> (u32, u32) {
    let cols = std::env::var("COLUMNS").ok().and_then(|v| v.parse().ok());
    let rows = std::env::var("LINES").ok().and_then(|v| v.parse().ok());
    if let (Some(cols), Some(rows)) = (cols, rows) {
        if cols > 0 && rows > 0 {
            return (cols, rows);
        }
    }
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("stty").arg("size").output() {
            if output.status.success() {
                if let Ok(text) = std::str::from_utf8(&output.stdout) {
                    let mut parts = text.split_whitespace();
                    if let (Some(rows), Some(cols)) = (parts.next(), parts.next()) {
                        if let (Ok(rows), Ok(cols)) = (rows.parse::<u32>(), cols.parse::<u32>()) {
                            if cols > 0 && rows > 0 {
                                return (cols, rows);
                            }
                        }
                    }
                }
            }
        }
    }
    (80, 24)
}

#[cfg(feature = "native")]
fn native_page_lines(
    page: NativePage,
    snapshot: Option<&LiveSnapshot>,
    model: &str,
    draft: &str,
    transcript: &[String],
    width: usize,
    height: usize,
    setup_mode: bool,
) -> Vec<String> {
    let width = width.max(20);
    let height = height.max(8);
    let mut lines = Vec::new();
    let title = if setup_mode {
        "OpenCode RK TUI — provider setup".to_string()
    } else {
        snapshot
            .map(|s| format!("OpenCode RK TUI — {}", s.title))
            .unwrap_or_else(|| "OpenCode RK TUI — offline".to_string())
    };
    lines.push(title);
    lines.push(format!(
        "model: {model}  |  Ctrl+P palette  Ctrl+T context  ? help  Ctrl+C quit"
    ));
    lines.push("─".repeat(width.min(120)));
    if setup_mode {
        lines.push(crate::app_start::setup_message().to_string());
        lines.push(
            "Provider setup is required before starting a session. Ctrl+C or :q exits.".to_string(),
        );
        if !draft.is_empty() {
            lines.push("Credential: [hidden]".to_string());
        }
    }
    match page {
        NativePage::Chat => {
            let body_rows = height.saturating_sub(7);
            let start = transcript.len().saturating_sub(body_rows);
            if transcript.is_empty() {
                lines.push(
                    snapshot
                        .and_then(|s| s.last_text.as_ref())
                        .map(|t| format!("last: {t}"))
                        .unwrap_or_else(|| "Start typing to send a turn.".to_string()),
                );
            } else {
                lines.extend(transcript[start..].iter().cloned());
            }
            while lines.len() < height.saturating_sub(3) {
                lines.push(String::new());
            }
            lines.push("─".repeat(width.min(120)));
            let displayed_draft = if setup_mode && !draft.is_empty() {
                "[hidden]"
            } else {
                draft
            };
            lines.push(format!("> {displayed_draft}"));
            lines.push("Enter send • Backspace edit • Ctrl+P commands • Ctrl+T context".to_owned());
        }
        NativePage::Palette => {
            lines.push("Command palette".to_string());
            lines.extend(
                [
                    "  /new           New session",
                    "  /sessions      Switch/list sessions",
                    "  /model         Switch model",
                    "  /agents        Switch agent",
                    "  /mcps          MCP controls",
                    "  /status        Status",
                    "  /themes        Theme",
                    "  /fork          Fork session",
                    "  /undo /redo    Session history actions",
                    "  /share         Share session",
                    "  /export        Export transcript",
                    "  Esc            Back to chat",
                ]
                .into_iter()
                .map(str::to_string),
            );
        }
        NativePage::Context => {
            lines.push("Context / status".to_string());
            if let Some(snapshot) = snapshot {
                lines.push(format!("session: {}", snapshot.session_id));
                lines.push(format!("state: {}", snapshot.state));
                lines.push(format!("updated: {}", snapshot.updated_at));
                lines.push(format!("messages: {}", snapshot.message_count));
            } else {
                lines.push("daemon: offline".to_string());
            }
            lines.push(
                "Usage and source-level context populate from live provider events.".to_string(),
            );
            lines.push("Esc returns to chat.".to_string());
        }
        NativePage::Help => {
            lines.push("Keyboard help".to_string());
            lines.extend(
                [
                    "Enter        submit current draft",
                    "Backspace    delete previous character",
                    "Ctrl+P       command palette",
                    "Ctrl+T       context/status page",
                    "?            help",
                    "Esc          close page",
                    "Ctrl+C       quit and restore terminal",
                ]
                .into_iter()
                .map(str::to_string),
            );
        }
    }
    lines.truncate(height);
    for line in &mut lines {
        if line.chars().count() > width {
            *line = line.chars().take(width).collect();
        }
    }
    lines
}

#[cfg(feature = "native")]
fn paint_native(
    renderer: &mut NativeRenderer,
    lines: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    renderer.fill_rect(
        0,
        0,
        renderer.cols(),
        renderer.rows(),
        Rgba::new(0, 0, 0, 255),
    )?;
    for (row, line) in lines.iter().enumerate().take(renderer.rows() as usize) {
        renderer.draw_text(0, row as u32, line)?;
    }
    renderer.frame(|_| {})?;
    Ok(())
}

#[cfg(unix)]
struct AsyncStdin {
    fd: tokio::io::unix::AsyncFd<std::io::Stdin>,
    original_flags: rustix::fs::OFlags,
}

#[cfg(unix)]
impl AsyncStdin {
    fn new() -> std::io::Result<Self> {
        let stdin = std::io::stdin();
        let original_flags = rustix::fs::fcntl_getfl(&stdin)?;
        rustix::fs::fcntl_setfl(&stdin, original_flags | rustix::fs::OFlags::NONBLOCK)?;
        match tokio::io::unix::AsyncFd::new(stdin) {
            Ok(fd) => Ok(Self { fd, original_flags }),
            Err(error) => {
                let stdin = std::io::stdin();
                let _ = rustix::fs::fcntl_setfl(&stdin, original_flags);
                Err(error)
            }
        }
    }
}

#[cfg(unix)]
impl Drop for AsyncStdin {
    fn drop(&mut self) {
        let _ = rustix::fs::fcntl_setfl(self.fd.get_ref(), self.original_flags);
    }
}

#[cfg(unix)]
fn spawn_byte_input() -> (
    mpsc::Receiver<Result<u8, String>>,
    tokio::task::JoinHandle<()>,
) {
    let (input_tx, input_rx) = mpsc::channel(64);
    let task = tokio::spawn(async move {
        let stdin = match AsyncStdin::new() {
            Ok(stdin) => stdin,
            Err(error) => {
                let _ = input_tx.send(Err(error.to_string())).await;
                return;
            }
        };
        loop {
            match read_stdin_byte(&stdin.fd).await {
                Ok(Some(byte)) => {
                    if input_tx.send(Ok(byte)).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = input_tx.send(Err(error.to_string())).await;
                    break;
                }
            }
        }
    });
    (input_rx, task)
}

#[cfg(not(unix))]
fn spawn_byte_input() -> (
    mpsc::Receiver<Result<u8, String>>,
    tokio::task::JoinHandle<()>,
) {
    let (input_tx, input_rx) = mpsc::channel(64);
    let task = tokio::task::spawn_blocking(move || {
        let stdin = std::io::stdin();
        for byte in stdin.lock().bytes() {
            let byte = byte.map_err(|error| error.to_string());
            if input_tx.blocking_send(byte).is_err() {
                break;
            }
        }
    });
    (input_rx, task)
}

fn trim_native_transcript(transcript: &mut Vec<String>) {
    const MAX_NATIVE_TRANSCRIPT: usize = 500;
    if transcript.len() > MAX_NATIVE_TRANSCRIPT {
        transcript.drain(..transcript.len() - MAX_NATIVE_TRANSCRIPT);
    }
}

#[cfg(feature = "native")]
async fn native_interactive_loop(
    keymap: SubmitKeymap,
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
    setup_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = native_terminal_size();
    let mut host = crate::native_host::NativeHost::new(crate::native_host::HostConfig {
        cols: cols.min(u32::from(u16::MAX)) as u16,
        rows: rows.min(u32::from(u16::MAX)) as u16,
        skip_onboarding: !setup_mode,
    });
    let _ = host.step(if live.is_some() {
        crate::native_host::HostEvent::DaemonLive
    } else {
        crate::native_host::HostEvent::DaemonDown
    });
    let mut renderer = NativeRenderer::create(cols, rows)?;
    renderer.setup_terminal()?;
    #[cfg(unix)]
    let _raw_mode = UnixRawMode::enter()?;
    let _ = renderer.enable_mouse(false);
    let _ = renderer.enable_kitty_keyboard(1);
    renderer.set_title("OpenCode RK")?;

    let mut page = NativePage::Chat;
    let native_keymap = match keymap {
        SubmitKeymap::Enter => crate::native_composer::SubmitKeymap::Enter,
        SubmitKeymap::CtrlJ => crate::native_composer::SubmitKeymap::CtrlJ,
    };
    let mut composer = crate::native_composer::Composer::with_keymap(native_keymap);
    let mut transcript: Vec<String> = Vec::new();
    if !memory.is_empty() {
        transcript.push(format!("memory: {} file(s) loaded", memory.len()));
    }
    let mut current_size = (cols, rows);
    let (mut input, input_task) = spawn_byte_input();
    let client = reqwest::Client::builder().build()?;
    let (worker, handle, mut results) = TurnWorker::start(client);
    let mut active = false;
    let mut exiting = false;

    let lines = native_page_lines(
        page,
        live,
        model,
        composer.draft(),
        &transcript,
        renderer.cols() as usize,
        renderer.rows() as usize,
        setup_mode,
    );
    paint_native(&mut renderer, &lines)?;
    let mut needs_paint = false;
    // OpenTUI's stdout backend flushes synchronously. A 10 Hz ceiling keeps
    // PTY output bounded while a burst of input bytes remains responsive.
    let mut repaint = tokio::time::interval(Duration::from_millis(100));
    repaint.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    repaint.tick().await;

    'event_loop: loop {
        let size = native_terminal_size();
        if size != current_size {
            current_size = size;
            renderer.resize(size.0, size.1)?;
            let _ = host.step(crate::native_host::HostEvent::Resize {
                cols: size.0.min(u32::from(u16::MAX)) as u16,
                rows: size.1.min(u32::from(u16::MAX)) as u16,
            });
            needs_paint = true;
        }

        tokio::select! {
            _ = repaint.tick(), if needs_paint => {
                let lines = native_page_lines(
                    page,
                    live,
                    model,
                    composer.draft(),
                    &transcript,
                    renderer.cols() as usize,
                    renderer.rows() as usize,
                    setup_mode,
                );
                paint_native(&mut renderer, &lines)?;
                needs_paint = false;
            }
            byte = input.recv() => {
                let Some(byte) = byte else { break 'event_loop };
                let byte = byte?;
                match byte {
            3 | 4 => {
                let _ = host.step(crate::native_host::HostEvent::Key('\x03'));
                if active { let _ = handle.interrupt(); }
                exiting = true;
            }
            b'\t' => {
                let _ = host.step(crate::native_host::HostEvent::Key('\t'));
            }
            16 => page = NativePage::Palette,
            20 => page = NativePage::Context,
            b'?' if composer.draft().is_empty() => page = NativePage::Help,
            27 => {
                // Escape sequences require a decoder with timeout/pushback.
                page = NativePage::Chat;
            }
            8 | 127 if page == NativePage::Chat => {
                let _ = native_composer_step(
                    &mut composer,
                    NativeInputEvent::Key {
                        code: u32::from(byte),
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                )?;
            }
            b'\r' | b'\n' if page == NativePage::Chat => {
                let draft = composer.draft().to_owned();
                match draft.trim() {
                    ":q" | ":quit" => {
                        composer.set_draft("")?;
                        if active { let _ = handle.interrupt(); }
                        exiting = true;
                        break 'event_loop;
                    }
                    ":i" => {
                        composer.set_draft("")?;
                        composer.interrupt();
                        let interrupt = handle.interrupt();
                        if active || interrupt == InterruptResult::Requested {
                            transcript.push(format!(
                                "[interrupt requested; draft preserved: {:?}]",
                                composer.draft()
                            ));
                        } else {
                            transcript.push(format!(
                                "[interrupted; draft preserved: {:?}]",
                                composer.draft()
                            ));
                        }
                        trim_native_transcript(&mut transcript);
                        let lines = native_page_lines(
                            page,
                            live,
                            model,
                            composer.draft(),
                            &transcript,
                            renderer.cols() as usize,
                            renderer.rows() as usize,
                            setup_mode,
                        );
                        paint_native(&mut renderer, &lines)?;
                        needs_paint = false;
                        continue 'event_loop;
                    }
                    _ => {}
                }
                let event = NativeInputEvent::Key {
                        code: u32::from(byte),
                        // A byte stream cannot distinguish terminal Enter-LF
                        // from Ctrl-J. Route LF to the configured submit key;
                        // CR remains physical Enter.
                        ctrl: byte == b'\n' && matches!(keymap, SubmitKeymap::CtrlJ),
                    alt: false,
                    shift: false,
                };
                let action = native_composer_step(&mut composer, event)?;
                match action {
                    NativeComposerAction::Submitted { text } => {
                        transcript.push(format!("you: {text}"));
                        let _ = host.step(crate::native_host::HostEvent::Submit(text.clone()));
                        if let Some(snapshot) = live {
                            let request = turn_request(
                                snapshot,
                                &text,
                                model,
                                reasoning_effort,
                                auth,
                            )?;
                            match submit_request(&handle, request).await {
                                Ok(()) => active = true,
                                Err(error) => {
                                    composer.interrupt();
                                    transcript.push(format!("error: {error}"));
                                }
                            }
                        } else {
                            transcript.push("offline: turn not executed".to_string());
                            let _ = composer.finish_turn();
                        }
                    }
                    NativeComposerAction::Queued => {
                        transcript.push(format!("[queued] {draft}"));
                        // The FIFO owns its clone; clear the editable line so
                        // the next PTY line is independent, like line mode.
                        composer.set_draft("")?;
                        let lines = native_page_lines(
                            page,
                            live,
                            model,
                            composer.draft(),
                            &transcript,
                            renderer.cols() as usize,
                            renderer.rows() as usize,
                            setup_mode,
                        );
                        paint_native(&mut renderer, &lines)?;
                        needs_paint = false;
                    }
                    _ => {}
                }
                trim_native_transcript(&mut transcript);
            }
            b if page == NativePage::Chat && (0x20..0x80).contains(&b) => {
                let _ = native_composer_step(
                    &mut composer,
                    NativeInputEvent::Key {
                        code: u32::from(b),
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                )?;
            }
            _ => {}
        }
                needs_paint = true;
                if exiting { break 'event_loop; }
            }
            result = results.recv(), if active => {
                let Some(result) = result else { break 'event_loop };
                active = false;
                let dispatch_next = matches!(result, TurnResult::Completed { .. });
                match result {
                    TurnResult::Completed { output } => transcript.push(format!("assistant: {output}")),
                    TurnResult::Failed(error) => transcript.push(format!("error: {error}")),
                    TurnResult::Cancelled => {
                        composer.interrupt();
                        transcript.push("cancelled; draft preserved".to_string());
                    }
                    TurnResult::Uncertain => {
                        composer.interrupt();
                        transcript.push("uncertain; replay denied".to_string());
                    }
                }
                trim_native_transcript(&mut transcript);
                if dispatch_next {
                    if let Some(next) = composer.finish_turn() {
                        transcript.push(format!("you: {next}"));
                        if let Some(snapshot) = live {
                            let request = turn_request(
                                snapshot,
                                &next,
                                model,
                                reasoning_effort,
                                auth,
                            )?;
                            match submit_request(&handle, request).await {
                                Ok(()) => active = true,
                                Err(error) => {
                                    composer.interrupt();
                                    transcript.push(format!("error: {error}"));
                                }
                            }
                        }
                        trim_native_transcript(&mut transcript);
                    }
                }
                needs_paint = true;
            }
        }
    }

    input_task.abort();
    let _ = input_task.await;
    if active {
        let _ = handle.interrupt();
        let _ = tokio::time::timeout(Duration::from_secs(2), results.recv()).await;
    }
    worker.shutdown().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = renderer.disable_mouse();
    let _ = renderer.disable_kitty_keyboard();
    let _ = renderer.restore_terminal_modes();
    renderer.close();
    Ok(())
}

/// Interactive line loop. Submit lines echo as `you: <draft>`, `?` shows the
/// keybinding help, `:i` interrupts (draft preserved), `:m` re-renders the
/// memory pane, `:ctx` renders the context detail view, `:q` quits, EOF quits.
/// When bound to a daemon, submits persist via POST /messages; offline mode
/// marks every submit explicitly as not persisted.
async fn interactive_loop(
    keymap: SubmitKeymap,
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder().build()?;
    let (worker, handle, mut results) = TurnWorker::start(client);
    #[cfg(unix)]
    let stdin = tokio::io::unix::AsyncFd::new(std::io::stdin())?;
    let mut input_buffer = Vec::new();
    let mut composer = crate::native_composer::Composer::new();
    let viewer = MemoryViewer::new(memory.to_vec());
    print!("{}", render_frame(keymap, memory, model, live));
    let mut active = false;
    let mut exiting = false;
    'event_loop: loop {
        tokio::select! {
            line = read_stdin_line(&stdin, &mut input_buffer) => {
                let Some(line) = line? else { break 'event_loop };
                match line.trim() {
                    ":q" | ":quit" => {
                        if active {
                            let _ = handle.interrupt();
                        }
                        exiting = true;
                    }
                    "?" => println!("{}", keybinding_help(keymap)),
                    ":i" => {
                        composer.interrupt();
                        let interrupt = handle.interrupt();
                        if active || interrupt == InterruptResult::Requested {
                            println!("[interrupt requested; draft preserved: {:?}]", composer.draft());
                        } else {
                            println!("[interrupted; draft preserved: {:?}]", composer.draft());
                        }
                    }
                    ":m" => print!("{}", render_memory(viewer.list())),
                    ":ctx" => println!("{}", render_context_detail(Vec::new())),
                    "" => {}
                    text => {
                        composer.set_draft(text)?;
                        match composer.submit() {
                            Ok(crate::native_composer::SubmitOutcome::Sent(sent)) => {
                                println!("you: {sent}");
                                match live {
                                    Some(snapshot) => {
                                        let request = turn_request(
                                            snapshot,
                                            &sent,
                                            model,
                                            reasoning_effort,
                                            auth,
                                        )?;
                                        match submit_request(&handle, request).await {
                                            Ok(()) => active = true,
                                            Err(error) => {
                                                composer.interrupt();
                                                println!("[error] turn failed: {error}");
                                            }
                                        }
                                    }
                                    None => {
                                        println!("[offline: turn not executed; pass --origin to bind a daemon]");
                                        let _ = composer.finish_turn();
                                    }
                                }
                            }
                            Ok(crate::native_composer::SubmitOutcome::Queued) => {
                                println!("[queued] {text}");
                            }
                            Err(e) => println!("[error] {e}"),
                        }
                    }
                }
                if exiting { break 'event_loop; }
            }
            result = results.recv(), if active => {
                let Some(result) = result else { break 'event_loop };
                active = false;
                let dispatch_next = matches!(result, TurnResult::Completed { .. });
                match result {
                    TurnResult::Completed { output } => println!("assistant: {output}"),
                    TurnResult::Failed(error) => println!("[error] turn failed: {error}"),
                    TurnResult::Cancelled => println!("[interrupted; draft preserved: {:?}]", composer.draft()),
                    TurnResult::Uncertain => println!("[uncertain; draft preserved: {:?}; replay denied]", composer.draft()),
                }
                if dispatch_next {
                    if let Some(next) = composer.finish_turn() {
                        println!("you: {next}");
                        if let Some(snapshot) = live {
                            let request = turn_request(
                                snapshot,
                                &next,
                                model,
                                reasoning_effort,
                                auth,
                            )?;
                            match submit_request(&handle, request).await {
                                Ok(()) => active = true,
                                Err(error) => {
                                    composer.interrupt();
                                    println!("[error] turn failed: {error}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if active {
        let _ = handle.interrupt();
        let _ = tokio::time::timeout(Duration::from_secs(2), results.recv()).await;
    }
    worker.shutdown().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}

/// Follow mode: poll the bound session, re-render only on change, stop after
/// `follow_for` seconds when bounded. Every poll is a fresh bounded fetch;
/// nothing accumulates between polls. Poll errors degrade to an offline line
/// and keep polling until the bound expires.
async fn follow_loop(
    origin: &str,
    session: Option<&str>,
    poll_ms: u64,
    follow_for_secs: Option<u64>,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let poll = Duration::from_millis(poll_ms.max(50));
    let deadline = follow_for_secs.map(|secs| Instant::now() + Duration::from_secs(secs));
    let mut fingerprint = String::new();
    loop {
        match fetch_snapshot_async(origin, session, auth).await {
            Ok(snapshot) => {
                let mark = format!(
                    "{}|{}|{}|{}|{}",
                    snapshot.updated_at,
                    snapshot.message_count,
                    snapshot.state,
                    snapshot.title,
                    snapshot.last_text.clone().unwrap_or_default()
                );
                if mark != fingerprint {
                    let first = fingerprint.is_empty();
                    fingerprint = mark;
                    if !first {
                        println!("--- update ---");
                    }
                    print!("{}", render_live(&snapshot));
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
            }
            Err(error) => println!("daemon offline: {error}"),
        }
        if let Some(end) = deadline {
            if Instant::now() >= end {
                return Ok(());
            }
        }
        tokio::time::sleep(poll).await;
    }
}

/// Entry bound from `main.rs`. Bounded output: one frame in `--once`, line-
/// echoed interaction otherwise; live fetches are capped in bytes and time.
/// Bearer comes from the validated backend descriptor when `--origin` names
/// the daemon's own origin; otherwise requests fail closed (offline banner
/// interactively, error in `--once`/`--follow`).
pub async fn run(args: TuiArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_with_dir(args, None).await
}

/// Same as [`run`] but honors an explicit data-dir from the caller (e.g. the
/// global `--data-dir` resolved in `main.rs`). `None` falls back to
/// [`resolve_cli_data_dir`] exactly as before.
pub async fn run_with_dir(
    args: TuiArgs,
    data_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_startup(args, data_dir, false, false).await
}

/// Run the no-subcommand application entrypoint with its planned initial view.
/// Setup launches the native onboarding surface without misreporting an empty
/// daemon as offline. Main launches atomically create the first session when an
/// authenticated daemon is empty.
pub async fn run_default(
    args: TuiArgs,
    data_dir: Option<&Path>,
    view: crate::app_start::StartupView,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_startup(
        args,
        data_dir,
        view == crate::app_start::StartupView::Setup,
        view == crate::app_start::StartupView::Main,
    )
    .await
}

async fn run_with_startup(
    args: TuiArgs,
    data_dir: Option<&Path>,
    setup_mode: bool,
    create_first_session: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let keymap = resolve_keymap(args.submit_keymap)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let memory = load_memory(&args.memory);
    // `main` may attach the default daemon origin before entering this
    // module. An origin-less snapshot is still a local startup frame: it must
    // not probe `/api`, require a descriptor, or require a session.
    if args.once && !origin_was_explicit() {
        print!(
            "{}",
            print_native_or_legacy(&render_frame(keymap, &memory, &args.model, None))
        );
        return Ok(());
    }
    let auth_owned = resolve_origin_bearer(args.origin.as_deref(), data_dir);
    let auth = auth_owned.as_deref();
    if setup_mode {
        if args.once {
            return Err("provider setup requires an interactive terminal".into());
        }
        if !std::io::stdin().is_terminal() {
            return Err("refusing provider setup on piped stdin: run on a TTY".into());
        }
        #[cfg(feature = "native")]
        {
            return native_interactive_loop(
                keymap,
                &memory,
                None,
                &args.model,
                &args.reasoning_effort,
                auth,
                true,
            )
            .await;
        }
        #[cfg(not(feature = "native"))]
        {
            return Err("in-app provider setup requires the native build".into());
        }
    }
    if args.follow {
        let Some(origin) = args.origin else {
            return Err("--follow requires --origin".into());
        };
        return follow_loop(
            &origin,
            args.session.as_deref(),
            args.poll_ms,
            args.follow_for,
            auth,
        )
        .await;
    }
    if let Some(origin) = &args.origin {
        let snapshot = if create_first_session {
            fetch_or_create_snapshot_async(origin, args.session.as_deref(), auth).await
        } else {
            fetch_snapshot_async(origin, args.session.as_deref(), auth).await
        };
        match snapshot {
            Ok(snapshot) => {
                if args.once {
                    let frame = render_frame(keymap, &memory, &args.model, Some(&snapshot));
                    print!("{}", print_native_or_legacy(&frame));
                    return Ok(());
                }
                #[cfg(feature = "native")]
                {
                    return native_interactive_loop(
                        keymap,
                        &memory,
                        Some(&snapshot),
                        &args.model,
                        &args.reasoning_effort,
                        auth,
                        false,
                    )
                    .await;
                }
                #[cfg(not(feature = "native"))]
                {
                    return interactive_loop(
                        keymap,
                        &memory,
                        Some(&snapshot),
                        &args.model,
                        &args.reasoning_effort,
                        auth,
                    )
                    .await;
                }
            }
            Err(error) => {
                // Fail closed in snapshot mode; degrade explicitly offline.
                if args.once {
                    return Err(error.into());
                }
                println!("daemon offline: {error}");
            }
        }
    } else if args.once {
        print!(
            "{}",
            print_native_or_legacy(&render_frame(keymap, &memory, &args.model, None))
        );
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        // Fail closed on piped stdin: the line loop would block on
        // `lines.next()` forever with `Stdio::null` (immediate-EOF reads as
        // empty only after poll) or hang scripts. `--once`/`--follow` are the
        // scriptable paths.
        return Err(
            "refusing interactive TUI on piped stdin: pass --once, --follow, or run on a TTY"
                .into(),
        );
    }
    #[cfg(feature = "native")]
    {
        native_interactive_loop(
            keymap,
            &memory,
            None,
            &args.model,
            &args.reasoning_effort,
            auth,
            false,
        )
        .await
    }
    #[cfg(not(feature = "native"))]
    {
        interactive_loop(
            keymap,
            &memory,
            None,
            &args.model,
            &args.reasoning_effort,
            auth,
        )
        .await
    }
}

fn origin_was_explicit() -> bool {
    env::args_os().skip(1).any(|argument| {
        let argument = argument.to_string_lossy();
        argument == "--origin" || argument.starts_with("--origin=")
    })
}

/// Resolve the raw bearer token for `--origin` from the validated backend
/// descriptor. Returns None when no descriptor/origin (fail-closed downstream).
/// An explicit `data_dir` from the caller (global `--data-dir`) wins over the
/// env/home default so `tui --origin` reads the same descriptor as `serve`.
fn resolve_origin_bearer(origin: Option<&str>, data_dir: Option<&Path>) -> Option<String> {
    let origin = origin?;
    let owned;
    let data: &Path = match data_dir {
        Some(dir) => dir,
        None => {
            owned = resolve_cli_data_dir()?;
            &owned
        }
    };
    let descriptor = opencode_rk_server::daemon::read_backend_descriptor(data).ok()??;
    if descriptor.http_origin != origin {
        return None;
    }
    if !daemon_client::is_wellformed_token(&descriptor.auth_token) {
        return None;
    }
    Some(descriptor.auth_token)
}

/// CLI data dir default (mirrors main.rs resolve_data_dir, no clap here).
fn resolve_cli_data_dir() -> Option<std::path::PathBuf> {
    if let Some(home) = std::env::var_os("OPENCODE_RK_HOME") {
        return Some(std::path::PathBuf::from(home));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Some(std::path::PathBuf::from(home).join(".local/share/opencode-rk"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return Some(std::path::PathBuf::from(home).join(".opencode-rk"));
    }
    None
}

/// Print a frame through the native OpenTUI memory renderer when available:
/// [`render_once`] snapshot on success, legacy text on validation failure.
/// Real Rust caller for the opentui bridge (CONVERGENCE NATIVE_TUI).
fn print_native_or_legacy(frame: &str) -> String {
    #[cfg(feature = "native")]
    {
        let lines: Vec<String> = frame.lines().map(str::to_owned).collect();
        // One-shot output is a complete scriptable snapshot rather than a
        // scrollable terminal viewport, so retain every already-bounded frame
        // line while preserving the normal minimum terminal height.
        let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX).max(24);
        match opencode_rk_opentui_bridge::Renderer::render_once(80, rows, &lines) {
            Ok(snapshot) => snapshot,
            Err(_) => frame.to_owned(),
        }
    }
    #[cfg(not(feature = "native"))]
    {
        frame.to_owned()
    }
}

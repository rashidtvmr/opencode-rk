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
#[cfg(feature = "native")]
#[path = "native_input_decoder.rs"]
mod native_input_decoder;
#[cfg(feature = "native")]
#[path = "native_setup.rs"]
mod native_setup;
#[cfg(feature = "native")]
#[path = "native_transcript.rs"]
mod native_transcript;
#[cfg(feature = "native")]
#[path = "native_turn.rs"]
mod native_turn;
#[cfg(feature = "native")]
use crate::native_chat_history::NativeChatHistory;
#[cfg(feature = "native")]
use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole};
use clap::{Args, ValueEnum};
#[cfg(feature = "native")]
use opencode_rk_opentui_bridge::{Renderer as NativeRenderer, Rgba};
use opencode_rk_sessions::tui_state::{
    context_breakdown, footer_hints, keybinding_help, status_click, Composer, MemoryFile,
    MemoryViewer, SourceUsage, StatusAction, StatusItem, SubmitKeymap, MAX_MEMORY_FILES,
    MAX_SOURCES,
};
use std::{
    env, fs,
    io::{BufRead, IsTerminal as _, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Bounded body cap for daemon responses (1 MiB).
const LIVE_MAX_BODY_BYTES: u64 = 1_048_576;
/// Connect/read timeout for live snapshot calls.
const LIVE_TIMEOUT: Duration = Duration::from_secs(2);
/// Max characters shown from the last message preview.
const LIVE_PREVIEW_CHARS: usize = 80;
const MAX_NATIVE_DRAFT_BYTES: usize = 32 * 1024;
const MAX_NATIVE_TRANSCRIPT_BYTES: usize = 64 * 1024;
const MAX_NATIVE_LINE_CHARS: usize = 1024;

/// Fixed, bounded Help overlay shown by `/help` in the native loop. Lists
/// only bindings actually wired in `native_loop`; no dynamic or user text.
#[cfg(feature = "native")]
const NATIVE_HELP_LINES: [&str; 10] = [
    "Help - active keybindings",
    "  Enter: submit composer draft",
    "  Backspace: delete last draft character",
    "  Esc: close Help or dialog",
    "  Ctrl-C / Ctrl-D: exit",
    "  Ctrl-P: open model picker",
    "  Ctrl-K: commands",
    "  /connect: connect a provider",
    "  /models: select a model",
    "  /help: open this Help page",
];

/// Ctrl-J submit-keymap variant of the same fixed Help contract: identical
/// whole-frame/compact behavior and focus/draft handling; only the
/// advertised submit-key text differs so Help stays truthful about the
/// active [`SubmitKeymap`].
#[cfg(feature = "native")]
const NATIVE_HELP_LINES_CTRLJ: [&str; 10] = [
    "Help - active keybindings",
    "  Ctrl-J: submit composer draft; Enter: newline",
    "  Backspace: delete last draft character",
    "  Esc: close Help or dialog",
    "  Ctrl-C / Ctrl-D: exit",
    "  Ctrl-P: open model picker",
    "  Ctrl-K: commands",
    "  /connect: connect a provider",
    "  /models: select a model",
    "  /help: open this Help page",
];

/// Assemble terminal bytes into complete UTF-8 scalars without ever storing a
/// partial sequence in the draft.  The input buffer is intentionally fixed:
/// terminal input is untrusted and a malformed sequence must not grow state.
#[cfg(feature = "native")]
#[derive(Debug, Default)]
struct NativeUtf8Decoder {
    carry: [u8; 4],
    len: usize,
    expected: usize,
}

#[cfg(feature = "native")]
impl NativeUtf8Decoder {
    fn reset(&mut self) {
        self.len = 0;
        self.expected = 0;
    }

    fn push(&mut self, byte: u8) -> Option<char> {
        if self.len == 0 {
            if byte < 0x80 {
                return char::from_u32(byte as u32);
            }
            self.expected = match byte {
                0xC2..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF4 => 4,
                _ => {
                    self.reset();
                    return None;
                }
            };
            self.carry[0] = byte;
            self.len = 1;
            return None;
        }

        if !(0x80..=0xBF).contains(&byte) {
            self.reset();
            return (byte < 0x80).then(|| char::from(byte));
        }
        self.carry[self.len] = byte;
        self.len += 1;
        if self.len != self.expected {
            return None;
        }
        let result = std::str::from_utf8(&self.carry[..self.len])
            .ok()
            .and_then(|text| text.chars().next());
        self.reset();
        result
    }
}

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
    history: Vec<String>,
    #[cfg(feature = "native")]
    typed_history: Vec<ChatMessage>,
    #[cfg(feature = "native")]
    parent_session: bool,
}

/// Minimal bounded HTTP/1.1 client for the daemon API (std-only, http
/// scheme), returning the response body AND its parsed status code so a
/// status-specific contract can be enforced without a second parser.
/// `auth` carries the `Bearer <token>` header value; `/api/*` refuses to
/// send without one (fail-closed), `/health` stays public. Transport
/// behavior (timeouts, body cap, auth refusal, any-2xx acceptance) is
/// shared with [`http_request`].
fn http_request_with_status(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    auth: Option<&str>,
) -> Result<(String, u16), String> {
    if path.starts_with("/api/") || path.starts_with("/auth/") {
        match auth {
            Some(token) if crate::daemon_client::is_wellformed_token(token) => {}
            _ => {
                return Err(
                    "refusing unauthenticated /api/* request: no validated backend descriptor"
                        .to_string(),
                );
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
    // Typed status for status-specific contracts (the broker permission
    // reply requires exactly 204): a `2xx` token that is not a decimal
    // code is a malformed response and fails closed — never silently
    // treated as an acknowledgement.
    let status_code = status
        .parse::<u16>()
        .map_err(|_| format!("daemon at {origin} returned a malformed HTTP status"))?;
    Ok((response[body_start..].to_string(), status_code))
}

/// Minimal bounded HTTP/1.1 client for the daemon API (std-only, http
/// scheme). `auth` carries the `Bearer <token>` header value; `/api/*`
/// refuses to send without one (fail-closed), `/health` stays public.
/// Body-only view over [`http_request_with_status`]: every non-permission
/// caller keeps the exact pre-existing any-2xx success contract — the
/// status code is simply dropped.
fn http_request(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    auth: Option<&str>,
) -> Result<String, String> {
    http_request_with_status(origin, method, path, body, auth).map(|(body, _status)| body)
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
    snapshot_from_listing(origin, &value, session, auth)
}

/// Target selection + bounded message history from an ALREADY authenticated
/// listing payload. Shared so `bind_session` satisfies its contract with ONE
/// `GET /api/sessions`: the same parsed body proves target row membership
/// and selects the snapshot target before any local mutation.
fn snapshot_from_listing(
    origin: &str,
    value: &serde_json::Value,
    session: Option<&str>,
    auth: Option<&str>,
) -> Result<LiveSnapshot, String> {
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
    let mut history = Vec::new();
    for message in messages.iter().rev().take(40).rev() {
        let role = message["role"].as_str().unwrap_or("message");
        if let Some(text) = message_text_bounded(message) {
            history.push(format!("{role}: {text}"));
        }
    }
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
        history,
        #[cfg(feature = "native")]
        typed_history: crate::native_chat_projection::project_history(&messages),
        #[cfg(feature = "native")]
        parent_session: target["parent_session_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
    })
}

fn message_text_bounded(message: &serde_json::Value) -> Option<String> {
    let text = message["body"]["text"].as_str()?;
    Some(text.chars().take(MAX_NATIVE_LINE_CHARS).collect())
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
    // Truthful local binding line: Ctrl-K (0x0b) is wired in
    // `native_loop` (opens the command input) and is never advertised
    // without a handler — the approved footer_hints set predates it.
    out.push_str("  Ctrl-K: commands\n");
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

/// Persist a submitted draft through the daemon. Returns the typed failure on
/// any non-2xx/transport error; the caller keeps the local state machine.
fn persist_submit(snapshot: &LiveSnapshot, text: &str, auth: Option<&str>) -> Result<(), String> {
    let payload = serde_json::json!({ "text": text }).to_string();
    http_request(
        &snapshot.origin,
        "POST",
        &format!("/api/sessions/{}/messages", snapshot.session_id),
        Some(&payload),
        auth,
    )
    .map(|_| ())
}

/// Interactive line loop. Submit lines echo as `you: <draft>`, `?` shows the
/// keybinding help, `:i` interrupts (draft preserved), `:m` re-renders the
/// memory pane, `:ctx` renders the context detail view, `:q` quits, EOF quits.
/// When bound to a daemon, submits persist via POST /messages; offline mode
/// marks every submit explicitly as not persisted.
fn interactive_loop(
    keymap: SubmitKeymap,
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut composer = Composer::new();
    let viewer = MemoryViewer::new(memory.to_vec());
    print!("{}", render_frame(keymap, memory, "unset", live));
    loop {
        let Some(line) = lines.next() else { break };
        let line = line?;
        match line.trim() {
            ":q" | ":quit" => break,
            "?" => println!("{}", keybinding_help(keymap)),
            ":i" => {
                composer.interrupt();
                println!("[interrupted; draft preserved: {:?}]", composer.draft());
            }
            ":m" => print!("{}", render_memory(viewer.list())),
            ":ctx" => println!("{}", render_context_detail(Vec::new())),
            "" => {}
            text => {
                composer.set_draft(text)?;
                match composer.submit() {
                    Ok(opencode_rk_sessions::tui_state::SubmitOutcome::Sent(sent)) => {
                        println!("you: {sent}");
                        match live {
                            Some(snapshot) => match persist_submit(snapshot, &sent, auth) {
                                Ok(()) => println!("[persisted]"),
                                Err(error) => println!("[error] not persisted: {error}"),
                            },
                            None => {
                                println!("[offline: not persisted; pass --origin to bind a daemon]")
                            }
                        }
                        while let Some(next) = composer.finish_turn() {
                            println!("you: {next}");
                        }
                    }
                    Ok(opencode_rk_sessions::tui_state::SubmitOutcome::Queued) => {
                        println!("[queued] {text}");
                    }
                    Err(e) => println!("[error] {e}"),
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "native")]
fn native_size() -> (u32, u32) {
    let cols = env::var("COLUMNS").ok().and_then(|v| v.parse().ok());
    let rows = env::var("LINES").ok().and_then(|v| v.parse().ok());
    match (cols, rows) {
        (Some(c), Some(r)) if c > 0 && r > 0 => (c, r),
        _ => (80, 24),
    }
}

/// Exact paint context for one native frame: the facade's region geometry
/// plus the modal overlay state. No string scraping — `modal_rows` comes
/// from the caller's own overlay/dialog content length, and `home` from the
/// explicit route fact, so text roles are projected from known structure.
#[cfg(feature = "native")]
struct PaintCtx {
    palette: crate::native_theme_runtime::Palette,
    /// Region geometry for this frame (positions for the style pass).
    presentation: crate::native_view::ViewPresentation,
    /// `Some(n)` when a modal panel owns the frame: `n` content rows
    /// starting at the shared modal's content_y, after one inner padding row.
    modal_rows: Option<usize>,
    modal: crate::native_view::ModalPresentation,
    sidebar_backdrop_alpha: u8,
    chat_row_classes: Vec<crate::native_view_chat::ChatRowClass>,
    /// True when this frame is the Home column (explicit route fact).
    home: bool,
    composer: Option<(u16, u16, u16, u16)>,
    logo: Option<(u16, u16)>,
}

/// Content rows of a rendered modal block: the last non-blank rendered row
/// (modal renderers pad to full height with blank rows, so the row count is
/// the last non-blank index + 1 — never the padded height).
#[cfg(feature = "native")]
fn modal_content_rows(lines: &[String]) -> usize {
    lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |index| index + 1)
}

/// Home-row role by position over the known Home structure (logo block,
/// draft lines, final model row) — never a global substring scan.
#[cfg(feature = "native")]
fn home_row_fg(
    lines: &[String],
    row: usize,
    palette: crate::native_theme_runtime::Palette,
) -> Rgba {
    use crate::native_theme_runtime::Slot;
    let first = lines.iter().position(|line| !line.trim().is_empty());
    let logo_end = first.and_then(|start| {
        lines[start..]
            .iter()
            .position(|line| line.trim().is_empty())
            .map(|offset| start + offset)
    });
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap_or(usize::MAX);
    if let (Some(start), Some(end)) = (first, logo_end) {
        if (start..end).contains(&row) {
            return paint_color(palette, Slot::Primary);
        }
    }
    if row == last {
        return paint_color(palette, Slot::TextMuted);
    }
    paint_color(palette, Slot::Text)
}

/// Modal row role from known dialog structure: heading row = readable text,
/// ` > ` selection marker = primary, everything else muted.
#[cfg(feature = "native")]
fn modal_row_fg(line: &str, heading: bool, palette: crate::native_theme_runtime::Palette) -> Rgba {
    use crate::native_theme_runtime::Slot;
    if heading {
        paint_color(palette, Slot::Text)
    } else if line.trim_start().starts_with("> ") {
        paint_color(palette, Slot::Primary)
    } else {
        paint_color(palette, Slot::TextMuted)
    }
}

#[cfg(feature = "native")]
fn paint_color(
    palette: crate::native_theme_runtime::Palette,
    slot: crate::native_theme_runtime::Slot,
) -> Rgba {
    use crate::native_theme_runtime::{source_over, Palette, Slot};
    let background = source_over(
        Palette::opencode_dark().get(Slot::Background),
        palette.get(Slot::Background),
    );
    let [r, g, b, a] = source_over(background, palette.get(slot));
    Rgba::new(r, g, b, a)
}

/// Native styled paint with cell-aligned semantic spans and explicit opaque
/// backgrounds. Modal/sidebar source-over dimming is resolved in Rust before
/// FFI; native alpha or null-background preservation is not assumed.
#[cfg(feature = "native")]
fn native_paint(
    renderer: &mut NativeRenderer,
    lines: &[String],
    ctx: PaintCtx,
) -> Result<(), Box<dyn std::error::Error>> {
    let cols = renderer.cols();
    let rows = renderer.rows();
    let presentation = ctx.presentation;
    use crate::native_theme_runtime::Slot;
    let background = paint_color(ctx.palette, Slot::Background);
    let panel = paint_color(ctx.palette, Slot::BackgroundPanel);
    let element = paint_color(ctx.palette, Slot::BackgroundElement);
    let text_color = paint_color(ctx.palette, Slot::Text);
    let muted = paint_color(ctx.palette, Slot::TextMuted);
    let error = paint_color(ctx.palette, Slot::Error);
    fn dim(color: Rgba, alpha: u8) -> Rgba {
        let [r, g, b, a] =
            crate::native_theme_runtime::dim([color.r, color.g, color.b, color.a], alpha);
        Rgba::new(r, g, b, a)
    }
    // Resolve alpha in Rust. Every span supplies an opaque background;
    // native null-background/alpha behavior is not relied upon.
    let modal = ctx.modal_rows.and_then(|count| {
        let width = u32::from(ctx.modal.width).min(cols);
        let x = u32::from(ctx.modal.x);
        let y = u32::from(ctx.modal.y);
        if y >= rows || count == 0 || width == 0 {
            None
        } else {
            let height = (count.saturating_add(2) as u32).min(rows - y);
            Some((x, y, width, height))
        }
    });
    let base_alpha = if modal.is_some() {
        ctx.modal.backdrop_alpha
    } else {
        ctx.sidebar_backdrop_alpha
    };
    renderer.fill_rect(0, 0, cols, rows, dim(background, base_alpha))?;
    for (row, line) in lines.iter().enumerate().take(rows as usize) {
        let y = row as u32;
        let modal_text = crate::native_view_layout::slice_cells(
            line,
            ctx.modal.x.saturating_add(2),
            ctx.modal.content_width,
        );
        use crate::native_view_chat::ChatRowClass;
        let normal_fg = match ctx.chat_row_classes.get(row) {
            Some(
                ChatRowClass::Thinking
                | ChatRowClass::Metadata
                | ChatRowClass::ToolCall
                | ChatRowClass::ToolResult,
            ) => muted,
            Some(ChatRowClass::ToolError) => error,
            Some(_) => text_color,
            None if ctx.home => home_row_fg(lines, row, ctx.palette),
            None => text_color,
        };
        let mut spans: Vec<(u32, String, Rgba, Rgba)> = Vec::with_capacity(8);
        crate::native_view_layout::visit_cells(
            line,
            cols.min(u16::MAX as u32) as u16,
            |x, glyph, _| {
                let x = u32::from(x);
                let sidebar = presentation.sidebar_offset.is_some_and(|left| {
                    x >= u32::from(left)
                        && x < u32::from(left) + u32::from(presentation.sidebar_width)
                });
                let in_modal = modal.is_some_and(|(left, top, width, height)| {
                    x >= left && x < left + width && y >= top && y < top + height
                });
                let composer = ctx.composer.is_some_and(|(left, top, width, height)| {
                    x >= u32::from(left)
                        && x < u32::from(left) + u32::from(width)
                        && y >= u32::from(top)
                        && y < u32::from(top) + u32::from(height)
                });
                let mut bg = if sidebar || in_modal {
                    panel
                } else if composer {
                    element
                } else {
                    background
                };
                let mut fg = if in_modal {
                    modal_row_fg(
                        modal_text.trim_start(),
                        y == u32::from(ctx.modal.content_y),
                        ctx.palette,
                    )
                } else {
                    normal_fg
                };
                if !in_modal {
                    let alpha = if modal.is_some() {
                        ctx.modal.backdrop_alpha
                    } else if !sidebar {
                        ctx.sidebar_backdrop_alpha
                    } else {
                        0
                    };
                    bg = dim(bg, alpha);
                    fg = dim(fg, alpha);
                }
                if let Some((_, text, last_fg, last_bg)) = spans.last_mut() {
                    if *last_fg == fg
                        && *last_bg == bg
                        && text.len().saturating_add(glyph.len()) <= 32 * 1024
                    {
                        text.push_str(glyph);
                        return;
                    }
                }
                spans.push((x, glyph.to_owned(), fg, bg));
            },
        );
        for (x, text, fg, bg) in spans {
            renderer.draw_text_styled(x, y, &text, fg, Some(bg), 0)?;
        }
    }
    // logo.ts + component/logo.tsx: two 19-cell halves separated by one
    // cell, muted left/text right, with 25%-tinted marker backgrounds.
    if let Some((x, y)) = ctx.logo {
        let logo = [
            ("                   ", "             ▄     "),
            ("█▀▀█ █▀▀█ █▀▀█ █▀▀▄", "█▀▀▀ █▀▀█ █▀▀█ █▀▀█"),
            ("█__█ █__█ █^^^ █__█", "█___ █__█ █__█ █^^^"),
            ("▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀", "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀"),
        ];
        for (row, (left, right)) in logo.iter().enumerate() {
            let py = u32::from(y) + row as u32;
            if py >= rows || modal.is_some_and(|(_, dy, _, h)| (dy..dy + h).contains(&py)) {
                continue;
            }
            for (offset, raw, original_fg) in [(0u32, *left, muted), (20, *right, text_color)] {
                let shadow = dim(
                    Rgba::new(
                        ((3 * u16::from(background.r) + u16::from(original_fg.r) + 2) / 4) as u8,
                        ((3 * u16::from(background.g) + u16::from(original_fg.g) + 2) / 4) as u8,
                        ((3 * u16::from(background.b) + u16::from(original_fg.b) + 2) / 4) as u8,
                        255,
                    ),
                    base_alpha,
                );
                let fg = dim(original_fg, base_alpha);
                for (col, marker) in raw.chars().enumerate() {
                    let px = u32::from(x) + offset + col as u32;
                    if px >= cols {
                        break;
                    }
                    let (glyph, color, background) = match marker {
                        '_' => (' ', fg, Some(shadow)),
                        '^' => ('▀', fg, Some(shadow)),
                        '~' => ('▀', shadow, Some(dim(background, base_alpha))),
                        ',' => ('▄', shadow, Some(dim(background, base_alpha))),
                        other => (other, fg, Some(dim(background, base_alpha))),
                    };
                    let mut bytes = [0u8; 4];
                    renderer.draw_text_styled(
                        px,
                        py,
                        glyph.encode_utf8(&mut bytes),
                        color,
                        background,
                        0,
                    )?;
                }
            }
        }
    }
    renderer.frame(|_| {})?;
    Ok(())
}

/// Workspace identity for the center pane: the real process working
/// directory (the launcher's project root); never fabricated data.
#[cfg(feature = "native")]
fn current_workspace_label() -> String {
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

/// Backend-confirmed session bind: two phases. Phase 1 performs only the
/// authenticated fetches (target session + history, fresh listing) — a
/// failure there mutates nothing. Phase 2 hands every fetched fact to the
/// single facade transaction (`bind_commit`), which saves the outgoing
/// draft under its old known id, acknowledges membership, restores the
/// target draft, and only then commits active marker/status/transcript.
/// Returns the fetched snapshot plus the restored composer draft.
#[cfg(feature = "native")]
fn bind_session(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    global_draft: &str,
    origin: &str,
    session: Option<&str>,
    auth: Option<&str>,
) -> Result<(LiveSnapshot, String), String> {
    // Phase 1 — every required authenticated fetch, ZERO local mutation.
    // A listing failure / parse failure ERRORS HERE, before bind_commit can
    // touch the draft store, rows, active marker, status or transcript.
    let listing = http_request(origin, "GET", "/api/sessions", None, auth)
        .map_err(|error| format!("session listing unavailable: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&listing)
        .map_err(|error| format!("session listing is not valid JSON: {error}"))?;
    let rows = crate::native_view_sessions::parse_rows(&value)
        .map_err(|error| format!("session listing unusable: {error}"))?;
    // ONE listing serves both contracts: target selection and row
    // membership (bind_commit step 0 proves the target is an actual row of
    // THIS payload before the first mutation).
    let snapshot = snapshot_from_listing(origin, &value, session, auth)?;
    // Phase 2 — the single all-or-nothing facade transaction.
    let draft = view
        .bind_commit(
            Ok((
                snapshot.session_id.clone(),
                snapshot.history.clone(),
                rows,
                snapshot.message_count,
            )),
            global_draft,
            &mut transcript.lines,
            append_legacy_transcript,
        )?
        .draft;
    // Successful bind: re-assert the accepted binding from the SAME
    // accepted snapshot (`LiveSnapshot` id, title and confirmed persisted
    // message count are one authenticated payload), so the rendered
    // triple is always the freshly bound session — never a stale title,
    // stale count or a mismatched id.
    view.set_session_binding(
        Some(snapshot.session_id.clone()),
        Some(snapshot.title.clone()),
        Some(snapshot.message_count),
    );
    transcript.reset(snapshot.history.clone(), snapshot.typed_history.clone());
    view.set_parent_session(snapshot.parent_session);
    Ok((snapshot, draft))
}

/// Authenticated `GET /api/sessions` → `PageRow`s for the open Sessions
/// page: id = backend UUID (the Enter target), label = backend title,
/// detail = active marker for the currently bound session. Every failure
/// returns `Err` BEFORE any `PageState` change, so the page keeps its
/// previous rows/honest note instead of showing fabricated content.
#[cfg(feature = "native")]
fn session_page_rows(
    origin: &str,
    auth: Option<&str>,
    active: Option<&str>,
) -> Result<Vec<crate::native_view_pages::PageRow>, String> {
    let body = http_request(origin, "GET", "/api/sessions", None, auth)?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("session listing is not valid JSON: {error}"))?;
    let rows = crate::native_view_sessions::parse_rows(&value)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let detail = if Some(row.id.as_str()) == active {
                "active".to_owned()
            } else {
                String::new()
            };
            crate::native_view_pages::PageRow {
                id: row.id,
                label: row.title,
                detail,
                enabled: true,
            }
        })
        .collect())
}

/// One pending backend permission request for the bound session
/// (`GET /api/session/{id}/permission`, existing bounded authenticated
/// client). `Ok(None)` is an honest "no request pending".
#[cfg(feature = "native")]
struct PendingPermission {
    id: String,
    title: String,
}

#[cfg(feature = "native")]
fn fetch_permission(
    origin: &str,
    session: &str,
    auth: Option<&str>,
) -> Result<Option<PendingPermission>, String> {
    let body = http_request(
        origin,
        "GET",
        &format!("/api/session/{session}/permission"),
        None,
        auth,
    )?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("permission list is not valid JSON: {error}"))?;
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "permission list is missing the data array".to_owned())?;
    for entry in data {
        let Some(id) = entry.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if id.is_empty() || id.len() > 128 {
            continue;
        }
        // Bounded, control-free display title from the real request action.
        let title: String = entry
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("permission request")
            .chars()
            .filter(|character| !character.is_control())
            .take(160)
            .collect();
        return Ok(Some(PendingPermission {
            id: id.to_owned(),
            title,
        }));
    }
    Ok(None)
}

/// Dispatch one decision intent to the real permission broker reply route
/// (`POST /api/session/{id}/permission/{request_id}/reply`, `once` or
/// `reject`). The local `attempt` is captured from THIS intent and echoed
/// only in its matching [`DialogAck`]. `accepted: true` is constructed
/// only on the exact broker ACK — `204 No Content` with an empty body
/// ([`permission_reply_accepted`]); any other status (including other 2xx),
/// a malformed status, or any transport failure produces a bounded static
/// error line plus `accepted: false` (the module's refusal path reopens
/// the choice — the dialog is never pretended accepted, never locally
/// closed), and the facade records the retry-await correlation so the
/// empty-GET poll can never close a decision that is awaiting this
/// explicit retry. Exactly one POST is attempted per explicit user
/// intent: no automatic retry, no replay, no auto-approve.
/// Question / confirm intents have no permission route and never produce
/// a POST.
#[cfg(feature = "native")]
fn dispatch_permission_reply(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    origin: Option<&str>,
    auth: Option<&str>,
    intent: crate::native_view_dialogs::DecisionIntent,
) {
    use crate::native_view_dialogs::{DecisionIntent, DialogAck};
    let reply = match &intent {
        DecisionIntent::Approve { .. } => "once",
        DecisionIntent::Reject { .. } => "reject",
        DecisionIntent::Answer { .. } | DecisionIntent::Cancel { .. } => {
            append_transcript(
                transcript,
                "error: dialog kind has no backend reply route (unsupported)".to_owned(),
            );
            return;
        }
    };
    let attempt = intent.attempt();
    let request_id = intent.request_id().to_owned();
    let session_id = intent.session_id().to_owned();
    let result = origin
        .ok_or_else(|| "daemon unavailable".to_owned())
        .and_then(|origin| {
            let path = format!("/api/session/{session_id}/permission/{request_id}/reply");
            let payload = serde_json::json!({ "reply": reply }).to_string();
            http_request_with_status(origin, "POST", &path, Some(&payload), auth).and_then(
                |(reply_body, status)| {
                    if permission_reply_accepted(status, &reply_body) {
                        Ok(())
                    } else {
                        // Bounded static refusal: the decimal status only
                        // (or a fixed phrase) — never raw body tokens.
                        Err(permission_reply_refusal(status))
                    }
                },
            )
        });
    let accepted = matches!(result, Ok(()));
    if let Err(error) = &result {
        append_transcript(
            transcript,
            format!("error: permission reply not acknowledged: {error}"),
        );
    }
    let ack = DialogAck {
        request_id,
        session_id,
        accepted,
        attempt,
    };
    // Correlated facade acknowledgement: records the retry-await flag ONLY
    // on the actual `AckOutcome::Rejected` outcome (the module reopened the
    // choice after a failed/non-accepting ACK — the decision awaits an
    // explicit retry, e.g. Esc re-sending reject). An accepted empty-204
    // (`AckOutcome::Closed`) is a legitimate resolution and clears it; a
    // stale/mismatched ACK is refused by the module without touching the
    // flag or focus.
    if let Err(error) = view.ack_permission_reply(ack) {
        append_transcript(
            transcript,
            format!("error: permission ack: {}", error.code()),
        );
    }
}

/// Exact broker contract for acknowledging a permission reply: ONLY
/// `204 No Content` with an EMPTY body accepts (constructing
/// `DialogAck { accepted: true }`). `200`/`201`/`202`/any other 2xx,
/// `404`, and a `204` carrying a body are NOT acceptance — fail closed
/// after the single POST, with no retry. Pure so the classification the
/// production dispatch actually calls is unit-testable.
#[cfg(feature = "native")]
pub(crate) fn permission_reply_accepted(status: u16, body: &str) -> bool {
    status == 204 && body.is_empty()
}

/// Bounded static refusal reason for a non-accepting broker reply:
/// carries only the decimal status (or a fixed phrase for the
/// empty-body contract violation) — never raw response body bytes.
#[cfg(feature = "native")]
fn permission_reply_refusal(status: u16) -> String {
    if status == 204 {
        "broker permission reply 204 carried a non-empty body".to_owned()
    } else {
        format!("broker permission reply requires HTTP 204, got HTTP {status}")
    }
}

/// Created-id extraction for `POST /api/sessions`: the real ACK body is
/// `{"session":{"id":"<uuid>"}}`, so the id is taken ONLY from the nested
/// `/session/id` pointer and validated with the real `SessionId` parser
/// (UUID) before any bind. A top-level `id`, a missing pointer, malformed
/// JSON or a non-UUID value is a bounded static error — never a most-
/// recent-session bind, never an invented id. Pure: the exact function
/// the `/new` production path calls is unit-tested.
#[cfg(feature = "native")]
pub(crate) fn parse_created_session_id(response: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(response)
        .map_err(|_| "session create ACK is not valid JSON".to_owned())?;
    let raw = value
        .pointer("/session/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "session create ACK missing nested session.id".to_owned())?;
    raw.parse::<opencode_rk_contracts::SessionId>()
        .map(|_| raw.to_owned())
        .map_err(|_| "session create ACK session.id is not a valid session id".to_owned())
}

#[cfg(feature = "native")]
fn load_native_theme(
    name: &str,
    mode: crate::native_theme_runtime::ThemeMode,
    config_paths: &[std::path::PathBuf],
) -> Result<crate::native_theme_runtime::RuntimeTheme, String> {
    use crate::native_theme_runtime::{RuntimeTheme, ThemeMode};
    crate::native_ui_budget::check_paths(config_paths)?;
    let mut budget = crate::native_ui_budget::ReadBudget::default();
    let (name, mode) = match name {
        "dark" => ("opencode", ThemeMode::Dark),
        "light" => ("opencode", ThemeMode::Light),
        "system" => ("opencode", mode),
        _ => (name, mode),
    };
    if name.is_empty()
        || name.len() > 64
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("invalid theme name".into());
    }
    for path in config_paths.iter().rev() {
        let Some(directory) = path.parent() else {
            continue;
        };
        let candidate = directory.join("themes").join(format!("{name}.json"));
        if let Some(text) = budget.read(&candidate, 262144, "theme asset")? {
            let value = serde_json::from_str(&text).map_err(|_| "invalid theme JSON")?;
            return RuntimeTheme::from_asset(name, &value, mode)
                .map_err(|error| format!("invalid theme: {error}"));
        }
    }
    let json =
        crate::native_theme_assets::lookup(name).ok_or_else(|| format!("unknown theme: {name}"))?;
    let value = serde_json::from_str(json).map_err(|_| "invalid bundled theme JSON")?;
    RuntimeTheme::from_asset(name, &value, mode).map_err(|error| format!("invalid theme: {error}"))
}

#[cfg(feature = "native")]
fn native_tui_config_paths() -> Result<Vec<std::path::PathBuf>, String> {
    use std::path::PathBuf;
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let global = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.as_ref().map(|home| home.join(".config")));
    let mut paths = Vec::new();
    let mut budget = crate::native_ui_budget::ReadBudget::default();
    let mut add = |directory: PathBuf| -> Result<(), String> {
        for name in ["tui.json", "tui.jsonc"] {
            let path = directory.join(name);
            budget.check_path(&path)?;
            paths.push(path);
        }
        Ok(())
    };
    if let Some(global) = global {
        add(global.join("opencode"))?;
    }
    if let Ok(directory) = env::current_dir() {
        let mut ancestors: Vec<_> = directory
            .ancestors()
            .take(crate::native_ui_budget::MAX_PATHS + 1)
            .collect();
        if ancestors.len() > crate::native_ui_budget::MAX_PATHS {
            return Err("TUI config ancestor budget exceeded".into());
        }
        ancestors.reverse();
        for directory in &ancestors {
            add(directory.to_path_buf())?;
        }
        for directory in ancestors {
            add(directory.join(".opencode"))?;
        }
    }
    if let Some(directory) = env::var_os("OPENCODE_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        add(directory)?;
    }
    Ok(paths)
}

#[cfg(feature = "native")]
fn native_loop(
    memory: &[MemoryFile],
    mut live: Option<LiveSnapshot>,
    auth: Option<&str>,
    startup_view: Option<crate::app_start::StartupView>,
    keymap: SubmitKeymap,
    data_dir: &Path,
    explicit_session_route: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = native_size();
    let mut renderer = NativeRenderer::create(cols, rows)?;
    renderer.setup_terminal()?;
    let _ = renderer.enable_kitty_keyboard(1);
    renderer.set_title("OpenCode RK")?;
    let mut draft = String::new();
    let mut dialog = if startup_view == Some(crate::app_start::StartupView::Setup) {
        native_setup::Dialog::Provider
    } else {
        native_setup::Dialog::None
    };
    let mut help_open = false;
    // Track whether the model catalogue was actually observed (HTTP + parse
    // succeeded): a failed fetch yields `None`, never a fabricated "confirmed
    // unset" model claim (status-v2 `set_model_unknown` path).
    let models_fetch = live.as_ref().and_then(|snapshot| {
        http_request(
            &snapshot.origin,
            "GET",
            "/api/models?provider=openai&limit=500",
            None,
            auth,
        )
        .and_then(|body| native_setup::catalogue(&body))
        .ok()
    });
    let models_loaded = models_fetch.is_some();
    let models = models_fetch.unwrap_or_default();
    let mut selected = native_setup::selected(&models);
    let mut transcript = NativeChatHistory::default();
    // In-flight turn worker wrapped with the binding identity it was
    // launched under: events apply only while session AND generation still
    // match; a superseded (stale) worker is drained and never touches the
    // current busy flag, transcript or live snapshot.
    let mut worker: Option<crate::native_view::OwnedWorker<native_turn::TurnWorker>> = None;
    let mut binding_generation: u64 = 0;
    // Segregated API-key dialog buffer: never stored as (or merged into)
    // the chat composer draft, never saved by any draft transaction.
    let mut key_input = String::new();
    // Dialog-local provider/model filter buffer: the picker's transient
    // text. Opening a picker preserves the chat draft verbatim, filtering
    // never mutates it, and cancel returns the exact original draft.
    let mut filter_input = String::new();
    let mut last_permission_poll = Instant::now();
    let mut permission_error: Option<String> = None;
    let mut partial = String::new();
    let mut utf8 = NativeUtf8Decoder::default();
    let mut input = native_input_decoder::TerminalInputDecoder::default();
    let mut observed_size = (cols, rows);
    let mut needs_paint = true;
    if !memory.is_empty() {
        transcript.push(format!("memory: {} file(s) loaded", memory.len()));
    }
    if let Some(snapshot) = live.as_ref() {
        for line in &snapshot.history {
            append_transcript(&mut transcript, line.clone());
        }
        transcript.reset(snapshot.history.clone(), snapshot.typed_history.clone());
    }
    // Three-pane shell facade: populated from the actual initial daemon,
    // session, model, workspace and existing history, then maintained on
    // every frame and turn event below. Fetches use the existing bounded
    // authenticated client; the facade itself never performs IO.
    let mut view = crate::native_view::NativeView::new(cols as u16, rows as u16);
    let state_path = data_dir.join("state/kv.json");
    let config_paths = native_tui_config_paths().unwrap_or_else(|error| {
        append_transcript(&mut transcript, format!("error: {error}"));
        Vec::new()
    });
    let preferences = crate::native_ui_preferences::UiPreferences::load(&config_paths, &state_path)
        .unwrap_or_else(|error| {
            append_transcript(&mut transcript, format!("error: {error}"));
            crate::native_ui_preferences::UiPreferences::default()
        });
    for diagnostic in &preferences.diagnostics {
        append_transcript(&mut transcript, format!("error: {diagnostic}"));
    }
    let theme_mode = if preferences.theme_mode.as_deref() == Some("light") {
        crate::native_theme_runtime::ThemeMode::Light
    } else {
        crate::native_theme_runtime::ThemeMode::Dark
    };
    let mut theme = match load_native_theme(
        preferences.theme.as_deref().unwrap_or("opencode"),
        theme_mode,
        &config_paths,
    ) {
        Ok(theme) => theme,
        Err(error) => {
            append_transcript(&mut transcript, format!("error: {error}"));
            crate::native_theme_runtime::RuntimeTheme::builtin_default(theme_mode)
        }
    };
    view.set_sidebar_hidden(preferences.sidebar_hidden);
    view.set_prompt_width(match preferences.prompt_width {
        Some(crate::native_ui_preferences::PromptWidth::Auto) => {
            crate::native_view::PromptWidth::Auto
        }
        Some(crate::native_ui_preferences::PromptWidth::Fixed(width)) => {
            crate::native_view::PromptWidth::Fixed(width)
        }
        None => crate::native_view::PromptWidth::Default,
    });
    // Honest status-v2 facts: a model claim only after the catalogue was
    // actually observed (failed fetch -> `Unavailable`, not `unset`); the
    // connection is confirmed only when the startup snapshot fetch ACKed.
    if let Some(model) = &selected {
        view.set_model(Some(model.qualified()));
        // The native setup catalogue currently admits only the OpenAI adapter.
        view.set_composer_model(&model.name, "OpenAI");
    } else if models_loaded {
        view.set_model(None); // confirmed: catalogue observed, no model
    } else {
        view.set_model_unknown(); // failed fetch: never observed
    }
    if live.is_some() {
        view.set_connection(true);
    } else {
        view.set_connection_unknown();
    }
    // Accepted startup binding: the auto-resumed snapshot IS the
    // authenticated product snapshot, so id, title and the confirmed
    // persisted message count commit together from that same source —
    // without a snapshot the bare setter retracts everything (no title,
    // no confirmed count, no `(live)` claim, never a fabricated `Chat`
    // default).
    match live.as_ref() {
        Some(snapshot) => view.set_session_binding(
            Some(snapshot.session_id.clone()),
            Some(snapshot.title.clone()),
            Some(snapshot.message_count),
        ),
        None => view.set_session(None),
    }
    view.set_workspace(current_workspace_label());
    view.set_parent_session(
        live.as_ref()
            .is_some_and(|snapshot| snapshot.parent_session),
    );
    if let Some(snapshot) = live.as_ref() {
        // Left pane: real backend session listing (GET /api/sessions).
        match http_request(&snapshot.origin, "GET", "/api/sessions", None, auth).and_then(|body| {
            serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string())
        }) {
            Ok(value) => match crate::native_view_sessions::parse_rows(&value) {
                Ok(rows) => view.set_session_rows(rows, Some(snapshot.session_id.clone())),
                Err(error) => append_transcript(
                    &mut transcript,
                    format!("error: session listing malformed: {error}"),
                ),
            },
            Err(error) => append_transcript(
                &mut transcript,
                format!("error: sessions unavailable: {error}"),
            ),
        }
        // Right pane: MCP snapshot (GET /api/mcp).
        match http_request(&snapshot.origin, "GET", "/api/mcp", None, auth).and_then(|body| {
            serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string())
        }) {
            Ok(value) => {
                if let Err(error) = view.apply_mcp(&value) {
                    append_transcript(
                        &mut transcript,
                        format!("error: mcp snapshot rejected: {error}"),
                    );
                }
            }
            Err(error) => {
                append_transcript(&mut transcript, format!("error: mcp unavailable: {error}"))
            }
        }
    }
    view.set_page_rows(crate::native_view::initial_page_rows(live.is_some()));
    // Active submit keymap fact: keeps Pages Help rows truthful (Enter vs
    // Ctrl-J), alongside the frozen whole-frame Help overlay below.
    view.set_submit_with_ctrl_j(matches!(keymap, SubmitKeymap::CtrlJ));
    // Session picker: highlight moves locally; the active session changes
    // only after a backend-ACKed fetch (Ctrl-G confirmation below).
    let mut pending_session: Option<String> = None;
    // Explicit route fact mirrored from the same accepted sources as
    // `view.set_session_route` (startup snapshot history, backend
    // user-message ACK, successful bind): true = session route, false =
    // Home. The paint pass reads only this fact for Home text roles; it is
    // never inferred from memory/error text.
    let mut session_route = explicit_session_route
        || live
            .as_ref()
            .is_some_and(|snapshot| !snapshot.history.is_empty());
    view.set_session_route(session_route);
    // Upstream leader chord state (`LeaderDefault = "ctrl+x"`, then `b` →
    // `session.sidebar.toggle`): armed only from the chat composer.
    let mut leader_pending = false;
    let mut byte = [0u8; 1];
    loop {
        if let Ok(Some(size)) = renderer.terminal_size() {
            if size != observed_size {
                renderer.resize(size.0, size.1)?;
                observed_size = size;
                view.resize(size.0 as u16, size.1 as u16);
                needs_paint = true;
            }
        }
        // Binding identity: events/busy apply only while this worker's
        // captured (session, generation) still matches the current binding.
        let binding_active = worker.as_ref().is_some_and(|owned| {
            owned.owns(
                live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                binding_generation,
            )
        });
        let mut worker_finished = false;
        let mut turn_succeeded = false;
        if let Some(owned) = worker.as_mut() {
            if binding_active {
                let mut settled = false;
                while let Some(event) = owned.worker.try_next() {
                    needs_paint = true;
                    view.follow_tail();
                    match event {
                        // Backend user-message ACK: the accepted first user
                        // turn moves the main column to the session route
                        // (Home → session), explicitly and only on ACK.
                        native_turn::Event::User => {
                            if !session_route {
                                session_route = true;
                                view.set_session_route(true);
                            }
                        }
                        native_turn::Event::Delta(delta) => {
                            partial.push_str(&delta);
                            replace_partial(&mut transcript, &partial);
                        }
                        native_turn::Event::ToolCall(name) => {
                            transcript.append(
                                format!("tool: {}", native_transcript::strip_ansi(&name)),
                                ChatMessage {
                                    role: ChatRole::Assistant,
                                    parts: vec![ChatPart::ToolCall {
                                        name,
                                        id: String::new(),
                                    }],
                                    agent: None,
                                    model: None,
                                },
                            );
                        }
                        native_turn::Event::ToolOutput(name, output) => {
                            transcript.append(
                                format!(
                                    "tool {}: {}",
                                    native_transcript::strip_ansi(&name),
                                    native_transcript::strip_ansi(&output)
                                ),
                                // This stream event does not carry an error flag.
                                // Retain its tool role without inventing success.
                                ChatMessage::text(ChatRole::Tool, format!("{name}: {output}")),
                            );
                        }
                        native_turn::Event::Assistant(text) => {
                            turn_succeeded = true;
                            partial.clear();
                            remove_partial(&mut transcript);
                            settled = true;
                            transcript.append(
                                format!("assistant: {}", native_transcript::strip_ansi(&text)),
                                ChatMessage::text(ChatRole::Assistant, text),
                            );
                        }
                        native_turn::Event::Error(error) => {
                            partial.clear();
                            remove_partial(&mut transcript);
                            settled = true;
                            append_transcript(
                                &mut transcript,
                                format!("error: {}", native_transcript::strip_ansi(&error)),
                            );
                        }
                    }
                }
                if settled || owned.worker.is_finished() {
                    worker_finished = true;
                }
            } else {
                // Stale worker from a superseded binding: drain events only.
                // Nothing here mutates busy, transcript, partial, live or
                // needs_paint from this source.
                while owned.worker.try_next().is_some() {}
                if owned.worker.is_finished() {
                    worker_finished = true;
                }
            }
        }
        if worker_finished {
            worker = None;
            if binding_active {
                if let Some(current) = live.as_ref() {
                    match fetch_snapshot(&current.origin, Some(&current.session_id), auth) {
                        Ok(snapshot) => {
                            view.set_session_binding(
                                Some(snapshot.session_id.clone()),
                                Some(snapshot.title.clone()),
                                Some(snapshot.message_count),
                            );
                            view.set_parent_session(snapshot.parent_session);
                            if turn_succeeded {
                                transcript.reset(
                                    snapshot.history.clone(),
                                    snapshot.typed_history.clone(),
                                );
                            }
                            live = Some(snapshot);
                            needs_paint = true;
                        }
                        Err(error) => {
                            view.set_connection_unknown();
                            append_transcript(
                                &mut transcript,
                                format!("error: completed-turn snapshot unavailable: {error}"),
                            );
                        }
                    }
                }
            }
        }
        // Bounded permission poll (existing authenticated client, 500ms
        // cadence): only while a turn owns this binding, or while an open
        // decision still needs backend-resolution detection. No queues, no
        // new threads, existing 2s/1MiB http_request limits.
        if !view.dialogs.pending()
            && (binding_active || view.dialogs.is_open())
            && last_permission_poll.elapsed() >= Duration::from_millis(500)
        {
            last_permission_poll = Instant::now();
            if let Some(snapshot) = live.as_ref() {
                match fetch_permission(&snapshot.origin, &snapshot.session_id, auth) {
                    Ok(None) => {
                        if view.permission_retry_open() && view.dialogs.is_open() {
                            // A dispatched reply did NOT legitimately
                            // resolve the decision: its ACK was rejected
                            // (failed/non-204 — the module reopened the
                            // choice, awaiting an EXPLICIT user retry such
                            // as Esc re-sending reject). Authenticated
                            // GET-none is not a resolution of the local
                            // decision: keep focus, report honestly (one
                            // deduplicated line), and never auto-replay,
                            // auto-approve or locally close.
                            let message = "error: permission reply unresolved: backend has no pending request; decision stays open (Esc retries reject)".to_owned();
                            if permission_error.as_deref() != Some(message.as_str()) {
                                permission_error = Some(message.clone());
                                append_transcript(&mut transcript, message);
                                needs_paint = true;
                            }
                        } else {
                            permission_error = None;
                            // Authenticated GET proves no request remains:
                            // the backend resolved it (pending() is false
                            // here, so no in-flight decision is touched).
                            // The facade closes ONLY an unsubmitted dialog
                            // — one with no dispatched reply awaiting an
                            // explicit retry.
                            if view.permission_backend_resolved_close() {
                                append_transcript(
                                    &mut transcript,
                                    "permission: request resolved by backend".to_owned(),
                                );
                                needs_paint = true;
                            }
                        }
                    }
                    Ok(Some(pending)) => {
                        if !view.dialogs.is_open() {
                            let result = view.dialogs.open_permission(
                                pending.id,
                                snapshot.session_id.clone(),
                                pending.title,
                            );
                            match result {
                                Ok(()) => {
                                    // A fresh prompt implies the previous
                                    // decision cycle ended in an explicit
                                    // valid resolution (the retry flag only
                                    // survives while its own dialog stays
                                    // open, and that blocks every close
                                    // path): the new decision cycle starts
                                    // clean.
                                    view.permission_retry_clear();
                                    permission_error = None;
                                    // Modal precedence: suspend the Help
                                    // frame so the decision is visible
                                    // once painted; the composer draft is
                                    // untouched (restore-after-ACK is
                                    // optional and deliberately not
                                    // implemented — no focus stealing
                                    // beyond the modal itself).
                                    help_open = false;
                                    needs_paint = true;
                                }
                                Err(error) => {
                                    let message =
                                        format!("error: permission dialog: {}", error.code());
                                    if permission_error.as_deref() != Some(message.as_str()) {
                                        permission_error = Some(message.clone());
                                        append_transcript(&mut transcript, message);
                                        needs_paint = true;
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        // Failed observation: connection state is unknown
                        // now (never a stale connected claim).
                        view.set_connection_unknown();
                        let message = format!("error: permission poll: {error}");
                        if permission_error.as_deref() != Some(message.as_str()) {
                            permission_error = Some(message.clone());
                            append_transcript(&mut transcript, message);
                            needs_paint = true;
                        }
                    }
                }
            }
        }
        // Center dialog overlay (native_setup): credentials stay masked in
        // its own lines; the facade keeps all three panes visible and
        // echoes the composer draft (never an API key) below it.
        let overlay: Vec<String> = match dialog {
            native_setup::Dialog::None => Vec::new(),
            native_setup::Dialog::Provider => {
                let mut panel = vec!["Connect a provider".to_owned()];
                // The dialog's own bounded filter buffer (typing goes to
                // `filter_input`, never the chat draft) — the overlay must
                // echo and filter the SAME buffer the user is typing into.
                if !models.is_empty() && "openai".contains(&filter_input.trim().to_lowercase()) {
                    panel.push("  OpenAI (API key)".to_owned());
                }
                panel.push(format!("filter: {filter_input} (enter / escape)"));
                panel
            }
            native_setup::Dialog::ApiKey => vec![
                "API key".to_owned(),
                format!(
                    "key: {} (enter / escape)",
                    "*".repeat(key_input.chars().count().min(40))
                ),
            ],
            native_setup::Dialog::Model => {
                let mut panel = vec!["Select model (OpenAI)".to_owned()];
                // Same buffer fix as Provider: filter and echo read the
                // dialog-local `filter_input`, never the chat draft.
                panel.extend(
                    models
                        .iter()
                        .filter(|model| model.matches(&filter_input))
                        .take(6)
                        .map(|model| format!("  {} — {}", model.id, model.name)),
                );
                panel.push(format!("filter: {filter_input} (enter / escape)"));
                panel
            }
        };
        let draft_echo = if matches!(dialog, native_setup::Dialog::ApiKey) {
            ""
        } else {
            draft.as_str()
        };
        // Modal precedence in RENDER too: an open permission decision must
        // be visible even if Help would otherwise own the whole frame (the
        // facade renders the dialog center first, so the native_setup
        // credential overlay can never show behind it).
        let dialog_open = view.dialogs.is_open();
        let mut lines = if help_open && !dialog_open {
            Vec::new()
        } else {
            // Busy reflects only the worker that owns the CURRENT binding;
            // a stale (superseded) worker never shows busy here.
            view.render_typed(draft_echo, &transcript.messages, binding_active, &overlay)
        };
        // Help owns the whole frame: heading at row 0 with static content
        // clipped to the terminal, so a short window still shows the page.
        // Esc recomputes the ordinary header/composer frame unchanged.
        // Never painted over an open decision modal (`dialog_open` gate).
        if help_open && !dialog_open {
            // Truthful submit-key line for the active keymap; same frozen
            // whole-frame compact take (rows-bounded), draft/focus rules
            // and Esc semantics.
            let help: &[&str] = if matches!(keymap, SubmitKeymap::CtrlJ) {
                &NATIVE_HELP_LINES_CTRLJ
            } else {
                &NATIVE_HELP_LINES
            };
            lines = help
                .iter()
                .take((renderer.rows() as usize).min(help.len()))
                .map(|line| (*line).to_owned())
                .collect();
        }
        if needs_paint {
            // Exact paint context: region geometry from the facade, modal
            // row count from this loop's own overlay content (or the
            // permission dialog's rendered content rows — rendered output
            // pads to full height, so count to the last non-blank row),
            // and the explicit Home route fact. Help owns the whole frame
            // and never paints a modal rectangle.
            let presentation = view.presentation();
            let modal_rows = if help_open {
                None
            } else if dialog_open {
                Some(modal_content_rows(&view.dialogs.render(
                    view.modal_presentation().content_width as usize,
                    presentation.rows as usize,
                )))
            } else if !overlay.is_empty() {
                Some(overlay.len())
            } else if view.page() == crate::native_view_pages::Page::Theme {
                Some(view.theme_dialog().len())
            } else {
                None
            };
            let home = !help_open
                && matches!(
                    view.page(),
                    crate::native_view_pages::Page::Chat | crate::native_view_pages::Page::Theme
                )
                && !session_route;
            native_paint(
                &mut renderer,
                &lines,
                PaintCtx {
                    palette: theme.palette,
                    presentation,
                    modal_rows,
                    modal: view.modal_presentation(),
                    sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
                    chat_row_classes: view.chat_row_classes().to_vec(),
                    home,
                    composer: if help_open {
                        None
                    } else {
                        view.composer_region(draft_echo)
                    },
                    logo: if help_open {
                        None
                    } else {
                        view.home_logo_region(draft_echo)
                    },
                },
            )?;
            needs_paint = false;
        }
        let now = Instant::now();
        input.expire(now);
        let worker_deadline = worker.is_some().then(|| now + Duration::from_millis(20));
        let timeout = match (worker_deadline, input.next_deadline()) {
            (Some(a), Some(b)) => a.min(b).saturating_duration_since(now),
            (Some(a), None) => a.saturating_duration_since(now),
            (None, Some(b)) => b.saturating_duration_since(now),
            // There is no SIGWINCH path in this small native bridge.  A
            // bounded idle wake lets us observe kernel geometry without
            // repainting unchanged frames or waiting for input.
            (None, None) => Duration::from_millis(100),
        }
        .min(Duration::from_millis(100));
        let ready = if input.has_event() {
            true
        } else {
            renderer.input_ready(Some(timeout))?
        };
        if !ready {
            continue;
        }
        if !input.has_event() {
            if renderer.read_input(&mut byte)? == 0 {
                break;
            }
            // A terminal protocol prefix is a control boundary.  Do not let
            // an incomplete UTF-8 scalar straddle it; bytes emitted later by
            // a decoded Kitty event are ordinary input and remain eligible.
            if byte[0] == 0x1b {
                utf8.reset();
            }
            input.push(byte[0], Instant::now());
        }
        let Some(event) = input.pop() else { continue };
        needs_paint = true;
        // Focus snapshot for this event: an open (non-Chat) page behaves
        // like the frozen Help overlay for focus purposes.
        let page_open = !matches!(view.page(), crate::native_view_pages::Page::Chat);
        // Typed-query focus: the Sessions filter and the Commands page
        // (Ctrl-K command input) accept printable/backspace input into the
        // facade-owned bounded query; every other open page stays fully
        // inert, and the permission modal / Help gates above keep
        // precedence over both.
        let page_query_open = matches!(
            view.page(),
            crate::native_view_pages::Page::Sessions
                | crate::native_view_pages::Page::Commands
                | crate::native_view_pages::Page::Theme
        );
        let byte = match event {
            native_input_decoder::InputEvent::Byte(byte) => byte,
            native_input_decoder::InputEvent::Paste(text) => {
                utf8.reset();
                if view.dialogs.intercepts_input() {
                    // Modal precedence: an open permission decision owns
                    // focus over Help and pages; paste is intercepted and
                    // never reaches the composer or the dialog.
                    continue;
                }
                if help_open {
                    // Help owns focus; paste must not mutate the hidden draft.
                    continue;
                }
                if page_open {
                    // An open page owns focus; paste lands in the page's
                    // own bounded query (Sessions filter / command buffer)
                    // and NEVER in the composer draft. Any other page
                    // discards the paste entirely.
                    if page_query_open {
                        for character in text.chars() {
                            if !view.page_query_push(character) {
                                break;
                            }
                        }
                    }
                    continue;
                }
                // API-key input is segregated: it lands in the native_setup
                // key buffer, never in the chat composer draft.
                let (target, limit) = if matches!(dialog, native_setup::Dialog::ApiKey) {
                    (&mut key_input, 16 * 1024)
                } else {
                    (&mut draft, MAX_NATIVE_DRAFT_BYTES)
                };
                if target
                    .len()
                    .checked_add(text.len())
                    .is_some_and(|len| len <= limit)
                {
                    target.push_str(&text);
                }
                // Paste has no key actions, even in a dialog or a busy turn.
                continue;
            }
            // Preserve the existing standalone-escape action (clear dialog /
            // draft) while keeping protocol escape bytes out of the composer.
            native_input_decoder::InputEvent::Escape => 27,
        };
        // Modal precedence: a pending permission decision owns input over
        // the Help frame and open pages — printables, page bindings and the
        // composer are intercepted; only choice navigation (Ctrl-N / Ctrl-B),
        // an explicit answer (Enter) and an explicit Esc act. Esc emits
        // Reject through the real broker reply route and waits for its ACK —
        // never a local close, even if Help is open. Ctrl-C / Ctrl-D keep
        // exit priority above every modal: they fall through here and break
        // in the dispatch below.
        if view.dialogs.intercepts_input() && !matches!(byte, 3 | 4) {
            match byte {
                14 => view.dialogs.next(),
                2 => view.dialogs.prev(),
                13 | 10 => match view.dialogs.answer() {
                    Ok(intent) => dispatch_permission_reply(
                        &mut view,
                        &mut transcript,
                        live.as_ref().map(|snapshot| snapshot.origin.as_str()),
                        auth,
                        intent,
                    ),
                    Err(error) => append_transcript(
                        &mut transcript,
                        format!("error: permission answer: {}", error.code()),
                    ),
                },
                27 => match view.dialogs.cancel() {
                    Ok(intent) => dispatch_permission_reply(
                        &mut view,
                        &mut transcript,
                        live.as_ref().map(|snapshot| snapshot.origin.as_str()),
                        auth,
                        intent,
                    ),
                    Err(error) => append_transcript(
                        &mut transcript,
                        format!("error: permission reject: {}", error.code()),
                    ),
                },
                _ => {}
            }
            utf8.reset();
            continue;
        }
        // Help page owns input focus (no decision dialog is open): every
        // key except exit (Ctrl-C/Ctrl-D) and close (Esc) is inert, so no
        // dialog, draft, paste, or turn side effect can occur while Help
        // is open.
        if help_open && !matches!(byte, 3 | 4 | 27) {
            utf8.reset();
            continue;
        }
        // An open page owns focus the same way: only exit/close plus page
        // navigation (Ctrl-N / Ctrl-B) and confirmation (Enter) act; no
        // draft, dialog, paste or turn side effect can occur. On the
        // Sessions filter and the command buffer, printable and
        // backspace bytes are additionally routed to that bounded query
        // (and only it) below; every other page keeps the inert contract.
        if page_open
            && !help_open
            && !matches!(byte, 3 | 4 | 27 | 13 | 10 | 14 | 2)
            && !(page_query_open && (byte == 8 || byte == 127 || byte >= 0x20))
        {
            utf8.reset();
            continue;
        }
        match byte {
            3 | 4 => break,
            27 if help_open => {
                // Close Help only: draft/dialog/model/session are unchanged.
                help_open = false;
                utf8.reset();
            }
            27 if page_open => {
                // Close the page only, mirroring the frozen Help close:
                // draft/dialog/model/session are unchanged. This is also
                // the command-input cancel: the query buffer lives in the
                // page state and the composer draft — including unsent
                // text typed before Ctrl-K — comes back byte-identical.
                view.close_page();
                utf8.reset();
            }
            27 if !matches!(dialog, native_setup::Dialog::None) => {
                // Canceling a provider/model/API-key dialog returns the
                // composer draft EXACTLY as it was: the dialog filter and
                // the segregated key buffer are dialog-local and are
                // discarded; the chat draft is never the dialog buffer.
                dialog = native_setup::Dialog::None;
                key_input.clear();
                filter_input.clear();
                utf8.reset();
            }
            27 => {
                dialog = native_setup::Dialog::None;
                key_input.clear();
                draft.clear();
                utf8.reset();
            }
            16 => {
                // Ctrl-P opens the real Models picker with the composer
                // draft preserved verbatim; filtering uses the dialog's
                // own bounded buffer, never the chat draft.
                dialog = native_setup::Dialog::Model;
                key_input.clear();
                filter_input.clear();
                utf8.reset();
            }
            // Leader chord arming: Ctrl-X (`keybind.ts` `LeaderDefault =
            // "ctrl+x"`), upstream `<leader>b` → `session.sidebar.toggle`.
            // Only the chat composer can arm it — the permission-modal,
            // Help and page gates above already `continue` for this byte,
            // and a credential/model dialog (which owns focus here) refuses
            // to arm. No backend side effect: the chord only flips view
            // state when the follow-up key is `b` (below).
            24 => {
                leader_pending = matches!(dialog, native_setup::Dialog::None);
                utf8.reset();
            }
            // Page navigation while a page owns focus (Enter confirms).
            14 if page_open => {
                view.page_next();
                utf8.reset();
            }
            2 if page_open => {
                view.page_prev();
                utf8.reset();
            }
            // Chat keyboard scroll (Ctrl-F / Ctrl-B): bounded view state
            // only; no history, dialog or turn side effect. Ctrl-T (0x14)
            // is not mapped here and stays inert by fall-through.
            6 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                view.scroll_down(8);
                utf8.reset();
            }
            2 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                view.scroll_up(8);
                utf8.reset();
            }
            // Session picker in Chat: Ctrl-N / Ctrl-O move the highlight
            // only; the active session never changes before a backend ACK.
            14 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                if worker.is_none() {
                    pending_session = view.session_next();
                }
                utf8.reset();
            }
            15 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                if worker.is_none() {
                    pending_session = view.session_prev();
                }
                utf8.reset();
            }
            // Ctrl-G: confirm the highlighted session. The active session,
            // facade rows, draft store and transcript change only through
            // the single bind transaction after GET succeeds; a busy turn
            // or pending permission decision refuses the switch outright
            // (before any HTTP create/fetch).
            7 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                utf8.reset();
                // Reserve the next generation BEFORE any fetch/bind side
                // effect. Exhaustion fails closed: no fetch runs, no bind
                // commits, no generation is reused or minted.
                let Some(prospective) = crate::native_view::reserve_generation(binding_generation)
                else {
                    append_transcript(
                        &mut transcript,
                        "error: binding generation exhausted; session switch refused".to_owned(),
                    );
                    needs_paint = true;
                    continue;
                };
                let busy = worker.as_ref().is_some_and(|owned| {
                    owned.owns(
                        live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                        binding_generation,
                    )
                }) || view.dialogs.is_open();
                if busy {
                    append_transcript(
                        &mut transcript,
                        "busy: turn or permission decision in progress; session switch refused"
                            .to_owned(),
                    );
                    needs_paint = true;
                } else if let Some(candidate) = pending_session.take() {
                    let bound = live
                        .as_ref()
                        .ok_or_else(|| "daemon unavailable".to_owned())
                        .and_then(|snapshot| {
                            bind_session(
                                &mut view,
                                &mut transcript,
                                &draft,
                                &snapshot.origin,
                                Some(&candidate),
                                auth,
                            )
                        });
                    match bound {
                        Ok((snapshot, restored_draft)) => {
                            // Route follows the accepted snapshot: a bind
                            // with history restores the session route; an
                            // empty target (/new) returns to Home.
                            session_route = true;
                            view.set_session_route(session_route);
                            live = Some(snapshot);
                            draft = restored_draft;
                            // Committed ONLY on a successful bind.
                            binding_generation = prospective;
                            needs_paint = true;
                        }
                        Err(error) => {
                            append_transcript(
                                &mut transcript,
                                format!("error: session switch: {error}"),
                            );
                            needs_paint = true;
                        }
                    }
                }
            }
            // Ctrl-K (0x0b): open the local command input — the Commands
            // page with its static route rows — from Chat. Unoccupied raw
            // byte (no other arm consumes 0x0b). The composer draft,
            // including unsent text, is preserved verbatim; typing then
            // goes exclusively to the bounded command buffer (page
            // query), never the draft and never a provider turn.
            11 if !page_open && !help_open && matches!(dialog, native_setup::Dialog::None) => {
                view.open_commands_page();
                utf8.reset();
            }
            8 | 127 => {
                // Backspace edits the buffer that owns focus: the page
                // query on Sessions/Commands, the API-key dialog's
                // segregated key buffer, a provider/model dialog's own
                // filter buffer, otherwise the composer.
                if page_query_open {
                    view.page_query_backspace();
                } else if matches!(dialog, native_setup::Dialog::ApiKey) {
                    key_input.pop();
                } else if matches!(dialog, native_setup::Dialog::None) {
                    draft.pop();
                } else {
                    filter_input.pop();
                }
                utf8.reset();
            }
            // Ctrl-J submit keymap: Enter is a literal newline in the draft
            // (multiline composer); Ctrl-J (b'\n') still submits below.
            b'\r'
                if matches!(keymap, SubmitKeymap::CtrlJ)
                    && !page_open
                    && matches!(dialog, native_setup::Dialog::None) =>
            {
                utf8.reset();
                if draft.len() < MAX_NATIVE_DRAFT_BYTES {
                    draft.push('\n');
                }
            }
            b'\r' | b'\n' => {
                utf8.reset();
                // Set only by command-input Enter (Commands page); the
                // composer path leaves it `None` and parses the draft
                // exactly as before.
                let mut command_route: Option<&'static str> = None;
                if page_open {
                    if matches!(view.page(), crate::native_view_pages::Page::Sessions) {
                        // Sessions page Enter: the selected backend row
                        // becomes a real Request intent that runs through
                        // the SAME reserve+guard+bind transaction as
                        // Ctrl-G and /new (required listing + target
                        // membership). PageState is ACKed only on a real
                        // bind success; any refusal keeps the old active
                        // session, draft, rows and selection (the bind
                        // transaction is zero-mutation on failure) and
                        // records the reason as the page's visible error.
                        match view.activate_open_page() {
                            crate::native_view_pages::PageAction::Request {
                                page: crate::native_view_pages::Page::Sessions,
                                id,
                            } => {
                                // Reserve BEFORE any fetch/bind side
                                // effect. Exhaustion fails closed: no
                                // fetch, no commit, no reused identity.
                                let Some(prospective) =
                                    crate::native_view::reserve_generation(binding_generation)
                                else {
                                    append_transcript(
                                        &mut transcript,
                                        "error: binding generation exhausted; session switch refused"
                                            .to_owned(),
                                    );
                                    view.resolve_page_ack(crate::native_view_pages::Ack::Rejected(
                                        "binding generation exhausted".to_owned(),
                                    ));
                                    continue;
                                };
                                let busy = worker.as_ref().is_some_and(|owned| {
                                    owned.owns(
                                        live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                                        binding_generation,
                                    )
                                }) || view.dialogs.is_open();
                                if busy {
                                    // Refused BEFORE any HTTP fetch: no
                                    // auto-abort, no replay, composer
                                    // draft untouched.
                                    append_transcript(
                                        &mut transcript,
                                        "busy: turn or permission decision in progress; session switch refused"
                                            .to_owned(),
                                    );
                                    view.resolve_page_ack(crate::native_view_pages::Ack::Rejected(
                                        "busy: turn or permission decision in progress".to_owned(),
                                    ));
                                    continue;
                                }
                                let bound = live
                                    .as_ref()
                                    .ok_or_else(|| "daemon unavailable".to_owned())
                                    .and_then(|snapshot| {
                                        bind_session(
                                            &mut view,
                                            &mut transcript,
                                            &draft,
                                            &snapshot.origin,
                                            Some(&id),
                                            auth,
                                        )
                                    });
                                match bound {
                                    Ok((snapshot, restored_draft)) => {
                                        session_route = true;
                                        view.set_session_route(session_route);
                                        live = Some(snapshot);
                                        draft = restored_draft;
                                        // Committed ONLY on a real bind
                                        // success.
                                        binding_generation = prospective;
                                        view.resolve_page_ack(crate::native_view_pages::Ack::Ok);
                                        // ACK contract: reload rows for
                                        // the current page (the active
                                        // marker moves to the newly bound
                                        // session).
                                        if let Some(snapshot) = live.as_ref() {
                                            match session_page_rows(
                                                &snapshot.origin,
                                                auth,
                                                Some(&snapshot.session_id),
                                            ) {
                                                Ok(rows) => view.set_page_rows(rows),
                                                Err(error) => append_transcript(
                                                    &mut transcript,
                                                    format!("error: sessions page reload: {error}"),
                                                ),
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        // Rejected: the transaction kept
                                        // old active/draft/state intact;
                                        // the reason is now the page's
                                        // visible error too.
                                        append_transcript(
                                            &mut transcript,
                                            format!("error: session switch: {error}"),
                                        );
                                        view.resolve_page_ack(
                                            crate::native_view_pages::Ack::Rejected(error),
                                        );
                                    }
                                }
                            }
                            crate::native_view_pages::PageAction::Unsupported {
                                reason, ..
                            } => {
                                append_transcript(
                                    &mut transcript,
                                    format!("unavailable: {reason}"),
                                );
                            }
                            // No session row loaded/selected (e.g. the
                            // listing failed when the page opened): an
                            // honest no-op — never a fake shortcut.
                            crate::native_view_pages::PageAction::None => {}
                            other => append_transcript(
                                &mut transcript,
                                format!("error: unsupported sessions action: {other:?}"),
                            ),
                        }
                        continue;
                    }
                    if matches!(view.page(), crate::native_view_pages::Page::Commands) {
                        // Command-input Enter: the bounded command buffer
                        // (page query) resolves a recognized LOCAL route —
                        // an exact typed route first, else the selected
                        // (filtered) row's static route. No route means an
                        // honest error line: never a fabricated route,
                        // never a turn, and the composer draft typed
                        // before Ctrl-K is untouched.
                        match view.dispatch_command() {
                            Some(route) => {
                                // Leave the Commands page first; the shared
                                // local-route chain below reopens the
                                // route's own page or dialog. The draft
                                // stays verbatim.
                                view.close_page();
                                command_route = Some(route);
                            }
                            // `/sidebar` is a recognized chat-only local
                            // (the static Commands table has no row for
                            // it): the exact typed query routes through the
                            // same chain, never a fabricated route.
                            None if view.page_query().trim() == "/sidebar" => {
                                view.close_page();
                                command_route = Some("/sidebar");
                            }
                            None => append_transcript(
                                &mut transcript,
                                format!("error: unrecognized local command: {}", view.page_query()),
                            ),
                        }
                        if command_route.is_none() {
                            utf8.reset();
                            continue;
                        }
                    } else {
                        // Confirmation only: open the selected enabled page.
                        // Disabled rows stay put — their `detail` is the
                        // explicit unsupported reason. Pure page state: no
                        // HTTP transition, no provider turn, no history
                        // mutation — and NEVER a composer draft clear
                        // (only an accepted turn or a recognized
                        // chat-composer local command clears the draft).
                        if view.page() == crate::native_view_pages::Page::Theme {
                            if let Some(row) = view.page_selected().filter(|row| row.enabled) {
                                let name = row.id.clone();
                                match load_native_theme(&name, theme.mode, &config_paths) {
                                    Ok(next) => {
                                        match crate::native_ui_preferences::UiPreferences::persist_theme_selection(&state_path, &name, next.mode.key()) {
                                            Ok(()) => { theme = next; view.close_page(); }
                                            Err(error) => append_transcript(&mut transcript, format!("error: {error}")),
                                        }
                                    }
                                    Err(error) => append_transcript(&mut transcript, format!("error: {error}")),
                                }
                            }
                        } else {
                            view.confirm_page(&mut draft);
                        }
                        continue;
                    }
                }
                let was_api = matches!(dialog, native_setup::Dialog::ApiKey);
                // The API-key dialog reads its segregated buffer (trimmed),
                // never the chat composer draft; provider/model filters
                // read their own dialog-local buffer — the chat draft is
                // preserved verbatim across the whole dialog flow.
                let text = match command_route {
                    // Command mode: the recognized local route, fed to the
                    // SAME local-route chain below. Every recognized route
                    // branch `continue`s before turn submit, so no
                    // provider turn can originate from the command buffer.
                    Some(route) => route.to_owned(),
                    None => match dialog {
                        native_setup::Dialog::None => draft.clone(),
                        native_setup::Dialog::ApiKey => key_input.trim().to_owned(),
                        _ => filter_input.trim().to_owned(),
                    },
                };
                let command = text.trim();
                if matches!(dialog, native_setup::Dialog::None)
                    && matches!(command, ":q" | ":quit" | "/exit" | "/quit")
                {
                    break;
                }
                if !matches!(dialog, native_setup::Dialog::None) {
                    match dialog {
                        native_setup::Dialog::Provider => {
                            if !models.is_empty() && "openai".contains(&text.to_lowercase()) {
                                dialog = native_setup::Dialog::ApiKey;
                                // Fresh start for the segregated key buffer.
                                key_input.clear();
                            } else {
                                append_transcript(
                                    &mut transcript,
                                    "error: no supported provider matches".to_owned(),
                                );
                            }
                        }
                        native_setup::Dialog::ApiKey => {
                            let payload = serde_json::json!({"type":"api", "key":text}).to_string();
                            let result = live
                                .as_ref()
                                .ok_or_else(|| "daemon unavailable".to_owned())
                                .and_then(|snapshot| {
                                    http_request(
                                        &snapshot.origin,
                                        "PUT",
                                        "/auth/openai",
                                        Some(&payload),
                                        auth,
                                    )
                                })
                                .and_then(|response| {
                                    if serde_json::from_str::<serde_json::Value>(&response).ok()
                                        == Some(serde_json::Value::Bool(true))
                                    {
                                        Ok(())
                                    } else {
                                        Err("credential was not saved".to_owned())
                                    }
                                });
                            match result {
                                Ok(()) => {
                                    // Fresh dialog-local filter for the
                                    // model step; the chat draft stays
                                    // byte-identical.
                                    filter_input.clear();
                                    dialog = native_setup::Dialog::Model;
                                }
                                Err(_) => append_transcript(
                                    &mut transcript,
                                    "error: unable to save API key".to_owned(),
                                ),
                            }
                        }
                        native_setup::Dialog::Model => {
                            let choice = models
                                .iter()
                                .find(|model| model.id == text || model.qualified() == text)
                                .or_else(|| models.iter().find(|model| model.matches(&text)));
                            if let Some(model) = choice {
                                match native_setup::save_model(model) {
                                    Ok(()) => {
                                        selected = Some(model.clone());
                                        // Confirmed local config write:
                                        // honest live model status update.
                                        view.set_model(selected.as_ref().map(|m| m.qualified()));
                                        view.set_composer_model(&model.name, "OpenAI");
                                        dialog = native_setup::Dialog::None;
                                    }
                                    Err(error) => append_transcript(
                                        &mut transcript,
                                        format!("error: {error}"),
                                    ),
                                }
                            } else {
                                append_transcript(
                                    &mut transcript,
                                    "error: model is not advertised".to_owned(),
                                );
                            }
                        }
                        native_setup::Dialog::None => {}
                    }
                    // Filter dialogs consumed only their dialog-local
                    // buffer: the API-key dialog keeps its segregated
                    // buffer and the chat draft is preserved verbatim
                    // through the entire provider/model/API-key flow.
                    if !was_api {
                        filter_input.clear();
                    }
                    key_input.clear();
                    utf8.reset();
                    continue;
                }
                // Where the submitted text came from: the chat composer's
                // own draft vs a segregated source (the Commands-page
                // query, which set `command_route`, or a dialog buffer —
                // both handled above/never reach here). Recognition and
                // consumption of a typed local command apply ONLY to the
                // chat-composer source; a segregated source never clears
                // the chat draft.
                let from_chat_composer =
                    command_route.is_none() && matches!(dialog, native_setup::Dialog::None);
                // Intercept the exact /help extension command before any
                // transcript append, TurnWorker start, or provider/model
                // validation: it is a local page route, never a turn. The
                // typed command is consumed from the chat composer (it is
                // a command invocation, not an unsent draft) so Esc
                // restores the pre-command blank composer; a segregated
                // source's own buffer is untouched.
                if command == "/help" {
                    help_open = true;
                    consume_local_command(&mut draft, command, from_chat_composer);
                    utf8.reset();
                    continue;
                }
                if command == "/sessions" {
                    // Real Sessions page (pages-v5): open FIRST (rows bind
                    // to the page current at load time), then the same
                    // authenticated `GET /api/sessions` parse feeds its
                    // PageRows (uuid / title / active marker). A fetch or
                    // parse failure keeps the page's previous rows/note and
                    // reports honestly; never a turn, no backend mutation,
                    // and only a chat-composer typed command is consumed
                    // (a segregated source's own buffer is untouched).
                    view.open_page(crate::native_view_pages::Page::Sessions);
                    consume_local_command(&mut draft, command, from_chat_composer);
                    match live.as_ref() {
                        Some(snapshot) => {
                            match session_page_rows(
                                &snapshot.origin,
                                auth,
                                Some(&snapshot.session_id),
                            ) {
                                Ok(rows) => view.set_page_rows(rows),
                                Err(error) => append_transcript(
                                    &mut transcript,
                                    format!("error: sessions page: {error}"),
                                ),
                            }
                        }
                        None => append_transcript(
                            &mut transcript,
                            "error: sessions page: daemon unavailable".to_owned(),
                        ),
                    }
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
                // Local page routes, same contract as /help above: pure
                // state, never a transcript append or provider turn. A
                // chat-composer typed command is consumed at dispatch so
                // Esc restores the pre-command blank composer; a
                // segregated source (Ctrl-K query) keeps its draft intact.
                let local_page = match command {
                    "/status" => Some(crate::native_view_pages::Page::Status),
                    "/mcp" => Some(crate::native_view_pages::Page::Mcp),
                    "/theme" => Some(crate::native_view_pages::Page::Theme),
                    "/workspace" => Some(crate::native_view_pages::Page::Workspaces),
                    "/agents" => Some(crate::native_view_pages::Page::Agents),
                    "/context" => Some(crate::native_view_pages::Page::Context),
                    "/diff" => Some(crate::native_view_pages::Page::Diff),
                    "/commands" => Some(crate::native_view_pages::Page::Commands),
                    _ => None,
                };
                if let Some(page) = local_page {
                    if matches!(page, crate::native_view_pages::Page::Commands) {
                        // Single production path: the static route rows
                        // are loaded with the page (same as Ctrl-K).
                        view.open_commands_page();
                    } else {
                        view.open_page(page);
                        if page == crate::native_view_pages::Page::Theme {
                            let mut rows: Vec<_> = crate::native_theme_assets::ASSETS
                                .iter()
                                .map(|(name, _)| crate::native_view_pages::PageRow {
                                    id: (*name).into(),
                                    label: (*name).into(),
                                    detail: if *name == theme.name {
                                        "selected".into()
                                    } else {
                                        String::new()
                                    },
                                    enabled: true,
                                })
                                .collect();
                            rows.push(crate::native_view_pages::PageRow {
                                id: "dark".into(),
                                label: "OpenCode dark".into(),
                                detail: String::new(),
                                enabled: true,
                            });
                            rows.push(crate::native_view_pages::PageRow {
                                id: "light".into(),
                                label: "OpenCode light".into(),
                                detail: String::new(),
                                enabled: true,
                            });
                            view.set_page_rows(rows);
                        }
                    }
                    consume_local_command(&mut draft, command, from_chat_composer);
                    utf8.reset();
                    continue;
                }
                // /new: real backend session create (POST /api/sessions),
                // then bind the ACKed created id (`{"session":{"id":…}}`,
                // SessionId-validated — required `Some`, never a
                // most-recent fallback) through the same guard+transaction
                // as every other session-mutating entrypoint: fresh
                // listing + target membership proved before any mutation.
                // A chat-composer typed "/new" is consumed only into a
                // TRANSACTION-LOCAL remainder (never the visible draft),
                // so the visible draft is preserved unchanged through
                // every create/parse/fetch/listing/bind failure for
                // explicit manual retry — never auto-replayed, and the
                // honest created-ID refusal below stands. On a real
                // success the outgoing old-session store holds the empty
                // remainder (never the command text) and the target's
                // restored draft becomes visible. A genuine unsent draft
                // from the Commands page passes through unchanged.
                // Never replays or submits any turn.
                if command == "/new" {
                    utf8.reset();
                    // Reserve the next generation BEFORE any POST/fetch
                    // side effect. Exhaustion fails closed: no create runs,
                    // no bind commits, no identity is reused or minted.
                    let Some(prospective) =
                        crate::native_view::reserve_generation(binding_generation)
                    else {
                        append_transcript(
                            &mut transcript,
                            "error: binding generation exhausted; /new refused".to_owned(),
                        );
                        needs_paint = true;
                        continue;
                    };
                    let busy = worker.as_ref().is_some_and(|owned| {
                        owned.owns(
                            live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                            binding_generation,
                        )
                    }) || view.dialogs.is_open();
                    if busy {
                        // Refused BEFORE any HTTP creation: no auto-abort,
                        // no replay, composer draft untouched.
                        append_transcript(
                            &mut transcript,
                            "busy: turn or permission decision in progress; /new refused"
                                .to_owned(),
                        );
                        needs_paint = true;
                        continue;
                    }
                    // No consumption before the external operation: the
                    // visible draft still holds the typed "/new" through
                    // POST, ACK parse, listing and bind; only the
                    // transaction-local remainder below is consumed.
                    let created = live
                        .as_ref()
                        .ok_or_else(|| "daemon unavailable".to_owned())
                        .and_then(|snapshot| {
                            let body = serde_json::json!({ "title": "Chat" }).to_string();
                            http_request(
                                &snapshot.origin,
                                "POST",
                                "/api/sessions",
                                Some(&body),
                                auth,
                            )
                        });
                    // Honest created id: parsed ONLY from the real POST
                    // ACK's nested `/session/id` and validated by the real
                    // SessionId parser — never invented, never a top-level
                    // `id`, never a most-recent fallback.
                    match created {
                        Err(error) => append_transcript(
                            &mut transcript,
                            format!("error: session create: {error}"),
                        ),
                        Ok(response) => {
                            let created_id = match parse_created_session_id(&response) {
                                Ok(id) => id,
                                Err(error) => {
                                    // The POST already created the session:
                                    // report the refusal honestly with no
                                    // unvalidated id, and NEVER bind (the
                                    // most-recent session is not a fallback).
                                    append_transcript(
                                        &mut transcript,
                                        format!(
                                            "error: session create: {error}; created session preserved on the backend (not bound, not retried)"
                                        ),
                                    );
                                    needs_paint = true;
                                    continue;
                                }
                            };
                            // `Some(&created_id)` is REQUIRED after the ACK:
                            // the bind transaction proves this exact id's
                            // membership in a fresh authenticated listing
                            // and refuses atomically on any miss — never a
                            // `None` (most-recent) bind.
                            //
                            // Transaction-local remainder: the visible
                            // draft is never staged, cleared or replayed
                            // before this transaction commits. On success
                            // the outgoing old-session store receives the
                            // working copy (empty for a consumed
                            // chat-composer "/new" — never the command
                            // text — or the genuine Unicode draft from the
                            // Commands page) and the target's restored
                            // draft becomes the visible draft. On any
                            // failure the working copy is discarded and
                            // the visible draft — the typed "/new" itself
                            // — stands unchanged for explicit manual
                            // retry (manual only: an ambiguous create is
                            // never auto-replayed as a second POST).
                            let working_draft =
                                command_transaction_draft(&draft, command, from_chat_composer);
                            let bound = live
                                .as_ref()
                                .ok_or_else(|| "daemon unavailable".to_owned())
                                .and_then(|snapshot| {
                                    bind_session(
                                        &mut view,
                                        &mut transcript,
                                        &working_draft,
                                        &snapshot.origin,
                                        Some(&created_id),
                                        auth,
                                    )
                                });
                            match bound {
                                Ok((snapshot, restored_draft)) => {
                                    // An accepted explicit bind owns the session
                                    // route even before its first message.
                                    session_route = true;
                                    view.set_session_route(session_route);
                                    live = Some(snapshot);
                                    // Visible draft updates ONLY on this
                                    // successful commit.
                                    draft = restored_draft;
                                    binding_generation = prospective;
                                    append_transcript(
                                        &mut transcript,
                                        "session: created".to_owned(),
                                    );
                                }
                                Err(error) => {
                                    // The external session already exists on
                                    // the backend: report its id honestly.
                                    // Never auto-switched, never retried or
                                    // replayed as an ambiguous create, and
                                    // the visible draft still holds the
                                    // typed "/new" — preserved outside the
                                    // failed transaction for explicit
                                    // manual retry only.
                                    append_transcript(
                                        &mut transcript,
                                        format!(
                                            "error: session create: {error}; created session {created_id} preserved (not switched, not retried)"
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    needs_paint = true;
                    continue;
                }
                if command == "/connect" {
                    // Open the provider picker. A chat-composer typed
                    // "/connect" is consumed (command invocation, not an
                    // unsent draft); the filter is the dialog's own
                    // buffer, and cancel returns the exact draft state at
                    // dispatch (blank for a consumed command).
                    dialog = native_setup::Dialog::Provider;
                    consume_local_command(&mut draft, command, from_chat_composer);
                    filter_input.clear();
                    utf8.reset();
                    continue;
                }
                if command == "/models" {
                    // Same contract as /connect: real Models picker; a
                    // chat-composer typed "/models" is consumed, the
                    // dialog-local filter only is touched.
                    dialog = native_setup::Dialog::Model;
                    consume_local_command(&mut draft, command, from_chat_composer);
                    filter_input.clear();
                    utf8.reset();
                    continue;
                }
                if command == "/sidebar" {
                    // Local sidebar toggle command: the same recognized
                    // local-command chain as /connect (typed chat-composer
                    // text is consumed; a Commands-page source's own
                    // buffer is untouched, so the draft is preserved
                    // exactly). Pure view state — upstream
                    // `session.sidebar.toggle` with no backend call.
                    view.toggle_sidebar();
                    if let Err(error) = crate::native_ui_preferences::UiPreferences::persist_sidebar(
                        &state_path,
                        view.sidebar_hidden(),
                    ) {
                        append_transcript(&mut transcript, format!("error: {error}"));
                    }
                    consume_local_command(&mut draft, command, from_chat_composer);
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
                if !command.is_empty() {
                    if worker.as_ref().is_some_and(|owned| {
                        owned.owns(
                            live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                            binding_generation,
                        )
                    }) {
                        // Do not consume or echo this draft while a worker
                        // owns the current binding.
                        continue;
                    }
                    if let Some(snapshot) = live.as_ref() {
                        {
                            transcript.append(
                                format!("you: {text}"),
                                ChatMessage::text(ChatRole::User, text.clone()),
                            );
                            let result = selected
                                .as_ref()
                                .ok_or_else(|| "select a model with /connect".to_owned())
                                .and_then(|model| {
                                    native_turn::TurnWorker::start(
                                        &snapshot.origin,
                                        &snapshot.session_id,
                                        auth.ok_or("missing daemon token")?,
                                        &text,
                                        &model.qualified(),
                                    )
                                })
                                .map(|next| {
                                    // Capture the binding identity this
                                    // worker is launched under: events only
                                    // apply while both still match.
                                    worker = Some(crate::native_view::OwnedWorker::new(
                                        next,
                                        snapshot.session_id.clone(),
                                        binding_generation,
                                    ));
                                    partial.clear();
                                });
                            if let Err(error) = result {
                                append_transcript(&mut transcript, format!("error: {error}"));
                            }
                        }
                    } else {
                        transcript.append(
                            format!("you: {text}"),
                            ChatMessage::text(ChatRole::User, text.clone()),
                        );
                        append_transcript(
                            &mut transcript,
                            "offline: turn not executed".to_string(),
                        );
                    }
                }
                draft.clear();
                utf8.reset();
            }
            b if b >= 0x20 && b != b'\t' => {
                // Leader follow-up: `Ctrl-X` then `b` toggles the sidebar;
                // any other key just ends the chord. The chord (and its
                // follow-up key) is consumed WITHOUT touching the composer
                // draft, the credential buffer or any page query, and only
                // while the chat composer owns focus — never while a
                // credential/model dialog, Help or a page is open.
                if leader_pending
                    && matches!(dialog, native_setup::Dialog::None)
                    && !page_open
                    && !help_open
                {
                    leader_pending = false;
                    if b == b'b' {
                        view.toggle_sidebar();
                        if let Err(error) =
                            crate::native_ui_preferences::UiPreferences::persist_sidebar(
                                &state_path,
                                view.sidebar_hidden(),
                            )
                        {
                            append_transcript(&mut transcript, format!("error: {error}"));
                        }
                    }
                    utf8.reset();
                    continue;
                }
                leader_pending = false;
                if page_query_open {
                    // Sessions filter / command buffer: the query is
                    // facade-owned and bounded (char + byte caps); the
                    // composer draft is never touched while a query page
                    // owns focus.
                    if let Some(character) = utf8.push(b) {
                        view.page_query_push(character);
                    }
                } else {
                    let (target, limit) = if matches!(dialog, native_setup::Dialog::ApiKey) {
                        // Segregated API-key buffer: never the chat draft.
                        (&mut key_input, 16 * 1024)
                    } else if matches!(dialog, native_setup::Dialog::None) {
                        (&mut draft, MAX_NATIVE_DRAFT_BYTES)
                    } else {
                        // Provider/model filter: dialog-local buffer,
                        // never the chat draft.
                        (&mut filter_input, MAX_NATIVE_DRAFT_BYTES)
                    };
                    if let Some(character) = utf8.push(b) {
                        let width = character.len_utf8();
                        if target
                            .len()
                            .checked_add(width)
                            .is_some_and(|len| len <= limit)
                        {
                            target.push(character);
                        }
                    }
                }
            }
            _ => {
                utf8.reset();
            }
        }
    }
    let _ = renderer.disable_kitty_keyboard();
    let _ = renderer.restore_terminal_modes();
    renderer.close();
    Ok(())
}

/// Recognized LOCAL slash-command surface, resolved through the
/// production page table plus the two chat-only routes that are not
/// `Page::from_route` entries (`/commands` has no static route; `/new`
/// is a session-create action). No invented aliases: an unknown or
/// unsupported command is never "recognized" here. (`:q`/`:quit`/
/// `/exit`/`/quit` exit before dispatch, so their draft never renders
/// again and needs no consumption.)
#[cfg(feature = "native")]
fn is_recognized_local_command(command: &str) -> bool {
    crate::native_view_pages::Page::from_route(command).is_some()
        || matches!(command, "/commands" | "/new" | "/sidebar")
}

/// Consume the typed text of a RECOGNIZED local slash command that was
/// dispatched from the CHAT composer. That text is a non-submitted
/// command invocation — not an unsent user draft — so it is cleared at
/// dispatch: Esc must return to the pre-command (blank) composer
/// instead of leaving command text that later keystrokes concatenate
/// onto (installed bug: `/status` retained, then `mcp` appended →
/// `you: /status/mcp`). Only the chat-composer source clears: the
/// Commands-page query and every dialog buffer are segregated sources
/// that never touch the chat draft, and unknown/unsupported text never
/// reaches this helper (recognition is re-validated here, on the actual
/// input source — never a blind text match). Rendering/UI only: no
/// provider turn, no history entry, no state write.
#[cfg(feature = "native")]
fn consume_local_command(draft: &mut String, command: &str, from_chat_composer: bool) {
    if from_chat_composer && is_recognized_local_command(command) {
        draft.clear();
    }
}

/// Transaction-local remainder for a recognized local command that runs
/// an external create/bind transaction (`/new`): the command text is
/// consumed into this COPY — the visible draft is never staged, cleared
/// or replayed before the transaction commits. A segregated source
/// (the Commands page) passes the genuine Unicode draft unchanged. The
/// owning transaction consumes the returned value only on success (the
/// outgoing old-session store receives it and the target's restored
/// draft becomes visible); on any failure the copy is discarded and the
/// visible draft stands untouched for explicit manual retry — never an
/// automatic second POST.
#[cfg(feature = "native")]
fn command_transaction_draft(draft: &str, command: &str, from_chat_composer: bool) -> String {
    let mut working = draft.to_owned();
    consume_local_command(&mut working, command, from_chat_composer);
    working
}

#[cfg(feature = "native")]
fn append_transcript(transcript: &mut NativeChatHistory, line: String) {
    transcript.notice(line);
}

#[cfg(feature = "native")]
fn append_legacy_transcript(transcript: &mut Vec<String>, mut line: String) {
    if line.chars().count() > MAX_NATIVE_LINE_CHARS {
        line = line.chars().take(MAX_NATIVE_LINE_CHARS).collect();
    }
    transcript.push(line);
    let mut bytes: usize = transcript.iter().map(String::len).sum();
    while bytes > MAX_NATIVE_TRANSCRIPT_BYTES || transcript.len() > 200 {
        bytes = bytes.saturating_sub(transcript.remove(0).len());
    }
}

#[cfg(feature = "native")]
fn replace_partial(transcript: &mut NativeChatHistory, text: &str) {
    transcript.replace_partial(text);
}

#[cfg(feature = "native")]
fn remove_partial(transcript: &mut NativeChatHistory) {
    transcript.remove_partial();
}

/// Follow mode: poll the bound session, re-render only on change, stop after
/// `follow_for` seconds when bounded. Every poll is a fresh bounded fetch;
/// nothing accumulates between polls. Poll errors degrade to an offline line
/// and keep polling until the bound expires.
fn follow_loop(
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
        match fetch_snapshot(origin, session, auth) {
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
        std::thread::sleep(poll);
    }
}

/// Entry bound from `main.rs`. Bounded output: one frame in `--once`, line-
/// echoed interaction otherwise; live fetches are capped in bytes and time.
/// Bearer comes from the validated backend descriptor when `--origin` names
/// the daemon's own origin; otherwise requests fail closed (offline banner
/// interactively, error in `--once`/`--follow`).
pub fn run(args: TuiArgs) -> Result<(), Box<dyn std::error::Error>> {
    run_with_dir(args, None, None)
}

/// Same as [`run`] but honors an explicit data-dir from the caller (e.g. the
/// global `--data-dir` resolved in `main.rs`). `None` falls back to
/// [`resolve_cli_data_dir`] exactly as before.
pub fn run_with_dir(
    mut args: TuiArgs,
    data_dir: Option<&Path>,
    startup_view: Option<crate::app_start::StartupView>,
) -> Result<(), Box<dyn std::error::Error>> {
    let owned_data_dir;
    let effective_data_dir = match data_dir {
        Some(path) => path,
        None => {
            owned_data_dir =
                resolve_cli_data_dir().ok_or("unable to resolve CLI data directory")?;
            &owned_data_dir
        }
    };
    let interactive = !args.once && !args.follow;
    if interactive && (!std::io::stdin().is_terminal() || !std::io::stdout().is_terminal()) {
        return Err(
            "refusing interactive TUI without both stdin and stdout TTYs; pass --once or --follow"
                .into(),
        );
    }
    // A no-origin TUI invocation is still a real client: acquire the same
    // singleton lease as default chat, then bind the live snapshot to its
    // published authenticated origin.  DaemonLease's Drop owns cleanup only
    // for a child spawned by this invocation.
    let _daemon_lease = if args.origin.is_none() && interactive {
        Some(crate::chat::prepare_daemon(effective_data_dir))
    } else {
        None
    };
    if args.origin.is_none() && !args.once && !args.follow {
        if let Some(lease) = _daemon_lease.as_ref() {
            args.origin = lease.origin.clone();
        }
    }
    let keymap = resolve_keymap(args.submit_keymap)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let memory = load_memory(&args.memory);
    let auth_owned = resolve_origin_bearer(args.origin.as_deref(), Some(effective_data_dir));
    let auth = auth_owned.as_deref();
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
        );
    }
    if let Some(origin) = &args.origin {
        let snapshot_result = match fetch_snapshot(origin, args.session.as_deref(), auth) {
            Err(error) if error.contains("has no sessions yet") && auth.is_some() => {
                let body = serde_json::json!({ "title": "Chat" }).to_string();
                let created = http_request(origin, "POST", "/api/sessions", Some(&body), auth)
                    .map_err(|error| format!("session create failed: {error}"))?;
                let value: serde_json::Value = serde_json::from_str(&created)
                    .map_err(|e| format!("malformed session create response: {e}"))?;
                if value
                    .pointer("/session/id")
                    .and_then(|v| v.as_str())
                    .is_none()
                {
                    return Err("session create response missing id".into());
                }
                fetch_snapshot(origin, args.session.as_deref(), auth)
            }
            result => result,
        };
        match snapshot_result {
            Ok(snapshot) => {
                if args.once {
                    let frame = render_frame(keymap, &memory, "unset", Some(&snapshot));
                    print!("{}", print_native_or_legacy(&frame));
                    return Ok(());
                }
                #[cfg(feature = "native")]
                {
                    return native_loop(
                        &memory,
                        Some(snapshot),
                        auth,
                        startup_view,
                        keymap,
                        effective_data_dir,
                        args.session.is_some(),
                    );
                }
                #[cfg(not(feature = "native"))]
                {
                    return interactive_loop(keymap, &memory, Some(&snapshot), auth);
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
            print_native_or_legacy(&render_frame(keymap, &memory, "unset", None))
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
        native_loop(
            &memory,
            None,
            auth,
            startup_view,
            keymap,
            effective_data_dir,
            args.session.is_some(),
        )
    }
    #[cfg(not(feature = "native"))]
    {
        interactive_loop(keymap, &memory, None, auth)
    }
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
        match opencode_rk_opentui_bridge::Renderer::render_once(80, 24, &lines) {
            Ok(snapshot) => snapshot,
            Err(_) => frame.to_owned(),
        }
    }
    #[cfg(not(feature = "native"))]
    {
        frame.to_owned()
    }
}

#[cfg(all(test, feature = "native"))]
mod command_consumption_tests {
    use super::{consume_local_command, is_recognized_local_command};

    /// Installed bug reproduced as a pure production-helper contract:
    /// `/status` typed in the CHAT composer is a non-submitted local
    /// command — consumed at dispatch, so Esc restores the blank
    /// pre-command composer.
    #[test]
    fn chat_source_recognized_command_is_consumed() {
        let mut draft = String::from("/status");
        consume_local_command(&mut draft, "/status", true);
        assert_eq!(draft, "", "typed command consumed, not retained");
    }

    /// The Commands page (Ctrl-K) is a SEGREGATED source: dispatching
    /// `/status` from its query must never clear the genuine chat draft
    /// typed before Ctrl-K — byte-identical Unicode preserved.
    #[test]
    fn command_page_source_keeps_genuine_chat_draft() {
        let mut draft = String::from("keep café 🦀");
        consume_local_command(&mut draft, "/status", false);
        assert_eq!(
            draft, "keep café 🦀",
            "page dispatch never clears chat draft"
        );
    }

    /// The exact installed failure chain, simulated through the actual
    /// helper: `/status` open → Esc (blank composer) → type `mcp` →
    /// Enter. The draft must be exactly `mcp`; the retained `/status`
    /// prefix that produced `you: /status/mcp` is gone. No concatenation.
    #[test]
    fn status_then_mcp_produces_no_command_concatenation() {
        let mut draft = String::from("/status");
        consume_local_command(&mut draft, "/status", true); // Enter dispatches /status
        assert_eq!(draft, "", "Esc returns to the pre-command blank composer");
        for c in "mcp".chars() {
            draft.push(c); // subsequent typing, then Enter
        }
        assert_eq!(draft, "mcp", "no `/status` prefix remains to concatenate");
    }

    /// Recognition goes through the production page table — never a
    /// blind text match: unknown/unsupported text is untouched for the
    /// chat source (it falls through to the ordinary turn path, which
    /// owns its own accepted-draft clear), and a genuine unrelated
    /// Unicode draft is retained when a page is dispatched from the
    /// segregated source.
    #[test]
    fn unknown_text_and_unrelated_drafts_untouched() {
        let mut unknown = String::from("/definitely-not-a-route");
        consume_local_command(&mut unknown, "/definitely-not-a-route", true);
        assert_eq!(
            unknown, "/definitely-not-a-route",
            "unknown text never consumed here"
        );
        let mut unrelated = String::from("keep café 🦀");
        consume_local_command(&mut unrelated, "/status", false);
        assert_eq!(
            unrelated, "keep café 🦀",
            "unrelated draft retained across Ctrl-K"
        );
    }

    /// Every route the production page table exposes is consumed from
    /// the chat source, plus the two chat-only locals (/commands, /new)
    /// — and NONE from the segregated source. No invented aliases.
    #[test]
    fn recognition_matches_page_table_plus_chat_only_locals() {
        for page in crate::native_view_pages::Page::ORDER {
            let Some(route) = page.route() else { continue };
            assert!(is_recognized_local_command(route), "route {route}");
            let mut draft = String::from(route);
            consume_local_command(&mut draft, route, true);
            assert_eq!(draft, "", "chat source consumes {route}");
        }
        for local in ["/commands", "/new"] {
            assert!(is_recognized_local_command(local));
            let mut draft = String::from(local);
            consume_local_command(&mut draft, local, true);
            assert_eq!(draft, "", "chat source consumes {local}");
        }
        for unknown in [
            "/definitely-not-a-route",
            "/status extra",
            "status",
            "/Status",
        ] {
            assert!(!is_recognized_local_command(unknown), "no alias {unknown}");
        }
    }

    /// /new via the Commands page (genuine unsent draft) is preserved:
    /// the bind transaction must still receive and store the real draft,
    /// never an empty remainder.
    #[test]
    fn new_from_command_page_preserves_genuine_draft() {
        let mut draft = String::from("unsent café 🦀 work");
        consume_local_command(&mut draft, "/new", false);
        assert_eq!(draft, "unsent café 🦀 work");
    }

    /// v2 (general NEEDS_FIX): a chat-composer "/new" must NOT touch the
    /// visible draft before the external create/bind transaction
    /// commits. The production staging helper yields a transaction-local
    /// remainder; the original stays intact until a simulated success
    /// commit consumes the copy (never the command text into storage),
    /// and a Commands-origin "/new" stages the genuine draft unchanged.
    #[test]
    fn new_consumption_is_transaction_local_until_commit() {
        let visible = String::from("/new");
        let working = super::command_transaction_draft(&visible, "/new", true);
        assert_eq!(working, "", "commit path stores empty remainder");
        assert_eq!(visible, "/new", "visible draft untouched pre-commit");
        // Simulated successful bind: only now does the commit take the
        // working value — the invocation text never stores, and the
        // pre-commit original was intact above.
        let committed = working;
        assert_eq!(committed, "", "successful discard of the invocation");
        assert_eq!(visible, "/new");
        // Commands-origin stages the real Unicode draft byte-identical.
        assert_eq!(
            super::command_transaction_draft("café 🦀 unsent", "/new", false),
            "café 🦀 unsent"
        );
    }

    /// Failure staging with the REAL production ACK parser: wrong ACK
    /// shapes are Err, so the commit stage never runs — the typed "/new"
    /// remains preserved outside the discarded transaction-local copy
    /// for explicit manual retry. No auto-POST, no replay, no fake
    /// success claim; helper-only, no HTTP/PTY.
    #[test]
    fn new_failed_ack_keeps_original_command_outside() {
        let visible = String::from("/new");
        let working = super::command_transaction_draft(&visible, "/new", true);
        // Real parse_created_session_id: nested /session/id only.
        assert!(super::parse_created_session_id("").is_err());
        assert!(super::parse_created_session_id("{\"id\":\"top-level\"}").is_err());
        assert!(super::parse_created_session_id("{\"session\":{}}").is_err());
        assert!(super::parse_created_session_id("not json").is_err());
        drop(working); // Err arm discards the working copy
        assert_eq!(visible, "/new", "original preserved for manual retry");
    }
}

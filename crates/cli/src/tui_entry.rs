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

use clap::{Args, ValueEnum};
use opencode_rk_sessions::tui_state::{
    context_breakdown, footer_hints, keybinding_help, status_click, Composer, MemoryFile,
    MemoryViewer, SourceUsage, StatusAction, StatusItem, SubmitKeymap, MAX_MEMORY_FILES,
    MAX_SOURCES,
};
use crate::daemon_client;
#[cfg(feature = "native")]
use opencode_rk_opentui_bridge::{Renderer as NativeRenderer, Rgba};
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
        .map(|token| format!("Authorization: {}\r\n", crate::daemon_client::authorization_header(token)))
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

/// Execute a submitted draft through the real daemon turn endpoint. This is
/// deliberately not the append-message route: a TUI submit must exercise the
/// same provider/tool/session pipeline as headless and web clients.
fn execute_submit(
    snapshot: &LiveSnapshot,
    text: &str,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "text": text,
        "model": model,
        "reasoning_effort": reasoning_effort,
    })
    .to_string();
    let body = http_request(
        &snapshot.origin,
        "POST",
        &format!("/api/sessions/{}/turns", snapshot.session_id),
        Some(&payload),
        auth,
    )?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("daemon turn response is not valid JSON: {e}"))?;
    value
        .get("assistant_message")
        .and_then(|message| message.get("body"))
        .and_then(|body| body.get("text"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "daemon turn response is missing assistant_message.body.text".to_string())
}

#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePage {
    Chat,
    Palette,
    Context,
    Help,
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
) -> Vec<String> {
    let width = width.max(20);
    let height = height.max(8);
    let mut lines = Vec::new();
    let title = snapshot
        .map(|s| format!("OpenCode RK — {}", s.title))
        .unwrap_or_else(|| "OpenCode RK — offline".to_string());
    lines.push(title);
    lines.push(format!(
        "model: {model}  |  Ctrl+P palette  Ctrl+T context  ? help  Ctrl+C quit"
    ));
    lines.push("─".repeat(width.min(120)));
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
            lines.push(format!("> {draft}"));
            lines.push("Enter send • Backspace edit • Ctrl+P commands • Ctrl+T context");
        }
        NativePage::Palette => {
            lines.push("Command palette".to_string());
            lines.extend([
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
            ].into_iter().map(str::to_string));
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
            lines.push("Usage and source-level context populate from live provider events.".to_string());
            lines.push("Esc returns to chat.".to_string());
        }
        NativePage::Help => {
            lines.push("Keyboard help".to_string());
            lines.extend([
                "Enter        submit current draft",
                "Backspace    delete previous character",
                "Ctrl+P       command palette",
                "Ctrl+T       context/status page",
                "?            help",
                "Esc          close page",
                "Ctrl+C       quit and restore terminal",
            ].into_iter().map(str::to_string));
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

#[cfg(feature = "native")]
fn native_interactive_loop(
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read as _;

    let (cols, rows) = native_terminal_size();
    let mut renderer = NativeRenderer::create(cols, rows)?;
    renderer.setup_terminal()?;
    let _ = renderer.enable_mouse(false);
    let _ = renderer.enable_kitty_keyboard(1);
    renderer.set_title("OpenCode RK")?;

    let mut page = NativePage::Chat;
    let mut draft = String::new();
    let mut transcript: Vec<String> = Vec::new();
    if !memory.is_empty() {
        transcript.push(format!("memory: {} file(s) loaded", memory.len()));
    }
    let mut current_size = (cols, rows);
    let mut stdin = std::io::stdin();
    let mut byte = [0u8; 1];

    loop {
        let size = native_terminal_size();
        if size != current_size {
            current_size = size;
            renderer.resize(size.0, size.1)?;
        }
        let lines = native_page_lines(
            page,
            live,
            model,
            &draft,
            &transcript,
            renderer.cols() as usize,
            renderer.rows() as usize,
        );
        paint_native(&mut renderer, &lines)?;

        let read = stdin.read(&mut byte)?;
        if read == 0 {
            break;
        }
        match byte[0] {
            3 | 4 => break,
            16 => page = NativePage::Palette,
            20 => page = NativePage::Context,
            b'?' if draft.is_empty() => page = NativePage::Help,
            27 => page = NativePage::Chat,
            8 | 127 if page == NativePage::Chat => {
                draft.pop();
            }
            b'\r' | b'\n' if page == NativePage::Chat => {
                let text = draft.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                draft.clear();
                transcript.push(format!("you: {text}"));
                match live {
                    Some(snapshot) => match execute_submit(
                        snapshot,
                        &text,
                        model,
                        reasoning_effort,
                        auth,
                    ) {
                        Ok(reply) => transcript.push(format!("assistant: {reply}")),
                        Err(error) => transcript.push(format!("error: {error}")),
                    },
                    None => transcript.push("offline: turn not executed".to_string()),
                }
                const MAX_NATIVE_TRANSCRIPT: usize = 500;
                if transcript.len() > MAX_NATIVE_TRANSCRIPT {
                    let drop_count = transcript.len() - MAX_NATIVE_TRANSCRIPT;
                    transcript.drain(..drop_count);
                }
            }
            b if page == NativePage::Chat && (b == b'\t' || b >= 0x20) => {
                if b != b'\t' {
                    draft.push(char::from(b));
                }
            }
            _ => {}
        }
    }

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
fn interactive_loop(
    keymap: SubmitKeymap,
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    model: &str,
    reasoning_effort: &str,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut composer = Composer::new();
    let viewer = MemoryViewer::new(memory.to_vec());
    print!("{}", render_frame(keymap, memory, model, live));
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
                            Some(snapshot) => match execute_submit(
                                snapshot,
                                &sent,
                                model,
                                reasoning_effort,
                                auth,
                            ) {
                                Ok(reply) => println!("assistant: {reply}"),
                                Err(error) => println!("[error] turn failed: {error}"),
                            },
                            None => {
                                println!("[offline: turn not executed; pass --origin to bind a daemon]")
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
    run_with_dir(args, None)
}

/// Same as [`run`] but honors an explicit data-dir from the caller (e.g. the
/// global `--data-dir` resolved in `main.rs`). `None` falls back to
/// [`resolve_cli_data_dir`] exactly as before.
pub fn run_with_dir(
    args: TuiArgs,
    data_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let keymap = resolve_keymap(args.submit_keymap)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let memory = load_memory(&args.memory);
    let auth_owned = resolve_origin_bearer(args.origin.as_deref(), data_dir);
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
        match fetch_snapshot(origin, args.session.as_deref(), auth) {
            Ok(snapshot) => {
                if args.once {
                    let frame = render_frame(keymap, &memory, &args.model, Some(&snapshot));
                    print!("{}", print_native_or_legacy(&frame));
                    return Ok(());
                }
                #[cfg(feature = "native")]
                {
                    return native_interactive_loop(
                        &memory,
                        Some(&snapshot),
                        &args.model,
                        &args.reasoning_effort,
                        auth,
                    );
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
                    );
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
        print!("{}", print_native_or_legacy(&render_frame(keymap, &memory, &args.model, None)));
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        // Fail closed on piped stdin: the line loop would block on
        // `lines.next()` forever with `Stdio::null` (immediate-EOF reads as
        // empty only after poll) or hang scripts. `--once`/`--follow` are the
        // scriptable paths.
        return Err(
            "refusing interactive TUI on piped stdin: pass --once, --follow, or run on a TTY".into(),
        );
    }
    #[cfg(feature = "native")]
    {
        native_interactive_loop(
            &memory,
            None,
            &args.model,
            &args.reasoning_effort,
            auth,
        )
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
    let descriptor =
        opencode_rk_server::daemon::read_backend_descriptor(data).ok()??;
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

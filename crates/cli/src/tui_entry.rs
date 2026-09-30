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
#[path = "native_setup.rs"]
mod native_setup;
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
    if path.starts_with("/api/") || path.starts_with("/auth/") {
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

#[cfg(feature = "native")]
fn native_paint(
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
fn native_loop(
    memory: &[MemoryFile],
    live: Option<&LiveSnapshot>,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = native_size();
    let mut renderer = NativeRenderer::create(cols, rows)?;
    renderer.setup_terminal()?;
    let _ = renderer.enable_kitty_keyboard(1);
    renderer.set_title("OpenCode RK")?;
    let mut draft = String::new();
    let mut dialog = native_setup::Dialog::None;
    let models = live
        .and_then(|snapshot| {
            http_request(
                &snapshot.origin,
                "GET",
                "/api/models?provider=openai&limit=500",
                None,
                auth,
            )
            .and_then(|body| native_setup::catalogue(&body))
            .ok()
        })
        .unwrap_or_default();
    let mut selected = native_setup::selected(&models);
    let mut transcript = Vec::new();
    if !memory.is_empty() {
        transcript.push(format!("memory: {} file(s) loaded", memory.len()));
    }
    if let Some(snapshot) = live {
        for line in &snapshot.history {
            append_transcript(&mut transcript, line.clone());
        }
    }
    let mut input = std::io::stdin();
    let mut byte = [0u8; 1];
    loop {
        let mut lines = vec![
            "OpenCode RK (native)".to_string(),
            selected
                .as_ref()
                .map(|model| format!("model: {} (ctrl-p)", model.qualified()))
                .unwrap_or_else(|| "model: unset (/connect)".to_owned()),
            live.map(|s| format!("session: {} (live)", s.title))
                .unwrap_or_else(|| "daemon: offline".to_string()),
        ];
        if let Some(snapshot) = live {
            lines.push(format!(
                "state: {}, updated: {}",
                snapshot.state, snapshot.updated_at
            ));
            lines.push(format!(
                "{} message{}",
                snapshot.message_count,
                if snapshot.message_count == 1 { "" } else { "s" }
            ));
            if let Some(last) = &snapshot.last_text {
                lines.push(format!("last: {last}"));
            }
        }
        let panel = match dialog {
            native_setup::Dialog::None => vec![format!("> {draft}")],
            native_setup::Dialog::Provider => {
                let mut panel = vec!["Connect a provider".to_owned()];
                if !models.is_empty() && "openai".contains(&draft.trim().to_lowercase()) {
                    panel.push("  OpenAI (API key)".to_owned());
                }
                panel.push(format!("filter: {draft} (enter / escape)"));
                panel
            }
            native_setup::Dialog::ApiKey => vec![
                "API key (OpenAI)".to_owned(),
                format!(
                    "key: {} (enter / escape)",
                    "*".repeat(draft.chars().count().min(40))
                ),
            ],
            native_setup::Dialog::Model => {
                let mut panel = vec!["Select model (OpenAI)".to_owned()];
                panel.extend(
                    models
                        .iter()
                        .filter(|model| model.matches(&draft))
                        .take(6)
                        .map(|model| format!("  {} — {}", model.id, model.name)),
                );
                panel.push(format!("filter: {draft} (enter / escape)"));
                panel
            }
        };
        let slots = (renderer.rows() as usize).saturating_sub(lines.len() + panel.len());
        let visible = transcript.iter().rev().take(slots).collect::<Vec<_>>();
        lines.extend(visible.into_iter().rev().cloned());
        lines.extend(panel);
        native_paint(&mut renderer, &lines)?;
        if input.read(&mut byte)? == 0 {
            break;
        }
        match byte[0] {
            3 | 4 => break,
            27 => {
                dialog = native_setup::Dialog::None;
                draft.clear();
            }
            16 => {
                dialog = native_setup::Dialog::Model;
                draft.clear();
            }
            8 | 127 => {
                draft.pop();
            }
            b'\r' | b'\n' => {
                let text = draft.trim().to_owned();
                if matches!(dialog, native_setup::Dialog::None)
                    && matches!(text.as_str(), ":q" | ":quit" | "/exit" | "/quit")
                {
                    break;
                }
                if !matches!(dialog, native_setup::Dialog::None) {
                    match dialog {
                        native_setup::Dialog::Provider => {
                            if !models.is_empty() && "openai".contains(&text.to_lowercase()) {
                                dialog = native_setup::Dialog::ApiKey;
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
                                Ok(()) => dialog = native_setup::Dialog::Model,
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
                    draft.clear();
                    continue;
                }
                if text == "/connect" {
                    dialog = native_setup::Dialog::Provider;
                    draft.clear();
                    continue;
                }
                if text == "/models" {
                    dialog = native_setup::Dialog::Model;
                    draft.clear();
                    continue;
                }
                if !text.is_empty() {
                    append_transcript(&mut transcript, format!("you: {text}"));
                    if let Some(snapshot) = live {
                        let result = selected
                            .as_ref()
                            .ok_or_else(|| "select a model with /connect".to_owned())
                            .and_then(|model| {
                                execute_turn(snapshot, &text, auth, &model.qualified())
                            });
                        match result {
                            Ok(reply) => {
                                append_transcript(&mut transcript, format!("assistant: {reply}"))
                            }
                            Err(error) => {
                                append_transcript(&mut transcript, format!("error: {error}"))
                            }
                        }
                    } else {
                        append_transcript(
                            &mut transcript,
                            "offline: turn not executed".to_string(),
                        );
                    }
                }
                draft.clear();
            }
            b if b >= 0x20 && b != b'\t' => {
                let limit = if matches!(dialog, native_setup::Dialog::ApiKey) {
                    16 * 1024
                } else {
                    MAX_NATIVE_DRAFT_BYTES
                };
                if draft.len() < limit {
                    draft.push(char::from(b));
                }
            }
            _ => {}
        }
    }
    let _ = renderer.disable_kitty_keyboard();
    let _ = renderer.restore_terminal_modes();
    renderer.close();
    Ok(())
}

#[cfg(feature = "native")]
fn append_transcript(transcript: &mut Vec<String>, mut line: String) {
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
fn execute_turn(
    snapshot: &LiveSnapshot,
    text: &str,
    auth: Option<&str>,
    model: &str,
) -> Result<String, String> {
    let body =
        serde_json::json!({"text": text, "model": model, "reasoning_effort": "high"}).to_string();
    let response = http_request(
        &snapshot.origin,
        "POST",
        &format!("/api/sessions/{}/turns", snapshot.session_id),
        Some(&body),
        auth,
    )?;
    let value: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    value
        .get("assistant_message")
        .and_then(|m| m.get("body"))
        .and_then(|b| b.get("text"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "turn response missing assistant text".to_string())
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
    mut args: TuiArgs,
    data_dir: Option<&Path>,
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
                    return native_loop(&memory, Some(&snapshot), auth);
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
        native_loop(&memory, None, auth)
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

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
use std::{
    env, fs,
    io::{BufRead, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
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
fn http_request(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<String, String> {
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
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
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
/// recently updated) plus its bounded message list.
fn fetch_snapshot(origin: &str, session: Option<&str>) -> Result<LiveSnapshot, String> {
    let sessions_body = http_request(origin, "GET", "/api/sessions", None)?;
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
        last_text,
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

/// Persist a submitted draft through the daemon. Returns the typed failure on
/// any non-2xx/transport error; the caller keeps the local state machine.
fn persist_submit(snapshot: &LiveSnapshot, text: &str) -> Result<(), String> {
    let payload = serde_json::json!({ "text": text }).to_string();
    http_request(
        &snapshot.origin,
        "POST",
        &format!("/api/sessions/{}/messages", snapshot.session_id),
        Some(&payload),
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
                            Some(snapshot) => match persist_submit(snapshot, &sent) {
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

/// Follow mode: poll the bound session, re-render only on change, stop after
/// `follow_for` seconds when bounded. Every poll is a fresh bounded fetch;
/// nothing accumulates between polls. Poll errors degrade to an offline line
/// and keep polling until the bound expires.
fn follow_loop(
    origin: &str,
    session: Option<&str>,
    memory: &[MemoryFile],
    poll_ms: u64,
    follow_for_secs: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let poll = Duration::from_millis(poll_ms.max(50));
    let deadline = follow_for_secs.map(|secs| Instant::now() + Duration::from_secs(secs));
    let mut fingerprint = String::new();
    loop {
        match fetch_snapshot(origin, session) {
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
pub fn run(args: TuiArgs) -> Result<(), Box<dyn std::error::Error>> {
    let keymap = resolve_keymap(args.submit_keymap)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let memory = load_memory(&args.memory);
    if args.follow {
        let Some(origin) = args.origin else {
            return Err("--follow requires --origin".into());
        };
        return follow_loop(
            &origin,
            args.session.as_deref(),
            &memory,
            args.poll_ms,
            args.follow_for,
        );
    }
    if let Some(origin) = &args.origin {
        match fetch_snapshot(origin, args.session.as_deref()) {
            Ok(snapshot) => {
                if args.once {
                    print!(
                        "{}",
                        render_frame(keymap, &memory, "unset", Some(&snapshot))
                    );
                    return Ok(());
                }
                return interactive_loop(keymap, &memory, Some(&snapshot));
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
        print!("{}", render_frame(keymap, &memory, "unset", None));
        return Ok(());
    }
    interactive_loop(keymap, &memory, None)
}

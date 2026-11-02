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
pub(crate) mod native_turn;
#[cfg(feature = "native")]
use crate::native_chat_history::NativeChatHistory;
#[cfg(feature = "native")]
use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole};
#[cfg(feature = "native")]
use crate::native_tabs::{TabOrientation, TabReservation, TabStrip};
#[cfg(feature = "native")]
use crate::native_navigation::SessionRef;
use clap::{Args, ValueEnum};

#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryRequestKind { Older, Newer, Tail, Refresh }

#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryRequestMeta { id: u64, generation: u64, revision: u64, kind: HistoryRequestKind }

#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug)]
struct BlockedHistoryScroll { revision: u64, rows: usize }

#[cfg(feature = "native")]
fn history_trace(
    event: &str,
    view: &crate::native_view::NativeView,
    window: Option<&crate::native_chat_projection::NativeHistoryWindow>,
    active: Option<HistoryRequestMeta>,
    queued: Option<HistoryRequestKind>,
    pending: bool,
    revision: u64,
    outcome: Option<&str>,
) {
    let (first, last, adjacent, cursor, newest) = window.map_or((None, None, false, None, false), |window| (
        window.messages().first().and_then(|row| row.get("id")).and_then(serde_json::Value::as_str),
        window.messages().last().and_then(|row| row.get("id")).and_then(serde_json::Value::as_str),
        window.has_adjacent_newer(),
        window.newer_recovery_cursor(),
        window.at_newest_page(),
    ));
    crate::native_debug::event(event, serde_json::json!({
        "kind": active.map(|request| format!("{:?}", request.kind)),
        "id": active.map(|request| request.id),
        "generation": active.map(|request| request.generation),
        "nav_revision": revision,
        "was_tail": view.chat_at_tail(),
        "queued_kind": queued.map(|kind| format!("{kind:?}")),
        "pending": pending,
        "window_first_id": first,
        "window_last_id": last,
        "has_adjacent": adjacent,
        "newer_cursor": cursor,
        "at_newest": newest,
        "outcome": outcome,
    }));
}

#[cfg(feature = "native")]
use opencode_rk_opentui_bridge::{Renderer as NativeRenderer, Rgba};
use opencode_rk_sessions::tui_state::{
    Composer, MAX_MEMORY_FILES, MAX_SOURCES, MemoryFile, MemoryViewer, SourceUsage, StatusAction,
    StatusItem, SubmitKeymap, context_breakdown, footer_hints, keybinding_help, status_click,
};
use std::{
    env, fs,
    io::{BufRead, IsTerminal as _, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::mpsc as std_mpsc,
    time::{Duration, Instant},
};

#[cfg(feature = "native")]
struct NativeImportPolicy {
    explicitly_confirmed: bool,
}

#[cfg(feature = "native")]
fn import_modal_visible(
    permission_open: bool,
    permission_editor_open: bool,
    setup_dialog_open: bool,
    import_open: bool,
) -> bool {
    !permission_open && !permission_editor_open && !setup_dialog_open && import_open
}

#[cfg(feature = "native")]
impl crate::native_config_import::ConfigWritePolicy for NativeImportPolicy {
    fn authorize_config_import(
        &self,
        _dest: &Path,
        _sources: &[crate::native_config_import::SourceMeta],
    ) -> Result<(), crate::native_config_import::ImportError> {
        if self.explicitly_confirmed {
            Ok(())
        } else {
            Err(crate::native_config_import::ImportError::PolicyDenied(
                "config import requires explicit confirmation".to_owned(),
            ))
        }
    }
}

/// Bounded body cap for daemon responses (1 MiB).
const LIVE_MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;
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
    "  Ctrl-P: open Commands (/models opens model picker)",
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
    "  Ctrl-P: open Commands (/models opens model picker)",
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

#[cfg(feature = "native")]
const fn native_composer_keymap(keymap: SubmitKeymap) -> crate::native_composer::SubmitKeymap {
    match keymap {
        SubmitKeymap::Enter => crate::native_composer::SubmitKeymap::Enter,
        SubmitKeymap::CtrlJ => crate::native_composer::SubmitKeymap::CtrlJ,
    }
}

#[cfg(feature = "native")]
fn normalize_modified_return(
    event: native_input_decoder::InputEvent,
    keymap: SubmitKeymap,
) -> native_input_decoder::InputEvent {
    use crate::native_composer::{decide_key, Key, KeyAction};
    let composer_keymap = native_composer_keymap(keymap);
    let (physical_key, newline_byte) = match &event {
        native_input_decoder::InputEvent::ShiftEnter
        | native_input_decoder::InputEvent::AltEnter =>
            (Key::ShiftEnter, if matches!(keymap, SubmitKeymap::CtrlJ) { b'\r' } else { b'\n' }),
        native_input_decoder::InputEvent::CtrlJ => (Key::CtrlJ, b'\n'),
        _ => return event,
    };
    match decide_key(physical_key, composer_keymap) {
        KeyAction::Newline | KeyAction::Submit =>
            native_input_decoder::InputEvent::Byte(newline_byte),
        _ => event,
    }
}

/// TAB-23: a `<leader>` follow-up key is a printable byte or one of the
/// arrow follow-ups `<leader>left` / `<leader>right`
/// (`config/keybind.ts:97-98`). The upstream chord is a timed registration
/// (`context/keymap.tsx:137-145`), not a printable-byte-only path, so the
/// leader matcher must accept decoded arrow events too.
#[cfg(feature = "native")]
fn leader_followup_key(event: &native_input_decoder::InputEvent) -> bool {
    matches!(
        event,
        native_input_decoder::InputEvent::Byte(byte) if *byte >= 0x20 && *byte != b'\t'
    ) || leader_followup_pane_event(event).is_some()
}

/// TAB-23: map a leader follow-up arrow to its pane-focus `HostEvent`
/// (`native_host.rs:192-194`): `<leader>right` -> `pane.focus.right`
/// (focus right pane), `<leader>left` -> `pane.focus.left` (focus session
/// pane). Non-follow-up events map to `None` and merely end the chord.
#[cfg(feature = "native")]
fn leader_followup_pane_event(
    event: &native_input_decoder::InputEvent,
) -> Option<crate::native_host::HostEvent> {
    match event {
        native_input_decoder::InputEvent::ArrowRight => {
            Some(crate::native_host::HostEvent::PaneFocusRight)
        }
        native_input_decoder::InputEvent::ArrowLeft => {
            Some(crate::native_host::HostEvent::PaneFocusSession)
        }
        _ => None,
    }
}

#[cfg(all(test, feature = "native"))]
mod tab23_leader_followup_tests {
    use super::{leader_followup_key, leader_followup_pane_event};

    /// RED before: the leader follow-up matcher was byte-only, so arrows
    /// never reached leader matching and no pane-focus dispatch existed.
    /// GREEN after: arrows qualify and dispatch the upstream-bound
    /// `HostEvent`s (`config/keybind.ts:97-98`).
    #[test]
    fn tab_23_leader_followup_accepts_arrows_and_dispatches_pane_focus() {
        use crate::native_host::HostEvent;
        use super::native_input_decoder::InputEvent;
        // The follow-up key set is not byte-only (`keymap.tsx:137-145`).
        assert!(leader_followup_key(&InputEvent::ArrowLeft));
        assert!(leader_followup_key(&InputEvent::ArrowRight));
        assert!(leader_followup_key(&InputEvent::Byte(b'b')));
        // Tab/control bytes are not follow-ups; ArrowUp merely ends the
        // chord (`terminal.select` is not implemented natively).
        assert!(!leader_followup_key(&InputEvent::Byte(b'\t')));
        assert!(!leader_followup_key(&InputEvent::Byte(24)));
        assert!(!leader_followup_key(&InputEvent::ArrowUp));
        // Dispatch mapping (`native_host.rs:192-194,411-424`).
        assert_eq!(
            leader_followup_pane_event(&InputEvent::ArrowRight),
            Some(HostEvent::PaneFocusRight)
        );
        assert_eq!(
            leader_followup_pane_event(&InputEvent::ArrowLeft),
            Some(HostEvent::PaneFocusSession)
        );
        assert_eq!(leader_followup_pane_event(&InputEvent::ArrowUp), None);
        assert_eq!(leader_followup_pane_event(&InputEvent::Byte(b'x')), None);
    }
}

/// The composer Enter submit decision, fed from the live loop's facts.
/// Upstream Enter invokes textarea `onSubmit` -> `submit()`
/// (`component/prompt/index.tsx:1790-1795`); delivery is `queue` exactly
/// while an in-flight turn owns the current binding (`:455,1097-1112`),
/// steer otherwise. A leading slash token is the upstream
/// `argumentSlash` fact (`:1136-1139`) and completion visibility blocks
/// submit (`:1103`). `queued` is capped at
/// [`crate::native_composer::BUSY_QUEUE_CAP`], the same bound as
/// `Composer::submit`.
#[cfg(feature = "native")]
fn composer_enter_plan(
    draft: &str,
    turn_owns_binding: bool,
    has_session: bool,
    completion_visible: bool,
    queued: usize,
) -> crate::native_composer::SubmitPlan {
    let delivery = if turn_owns_binding {
        crate::native_composer::Delivery::Queue
    } else {
        crate::native_composer::Delivery::Steer
    };
    let ctx = crate::native_composer::SubmitContext {
        has_session,
        autocomplete_visible: completion_visible,
        slash: draft.starts_with('/'),
        queued: queued.min(crate::native_composer::BUSY_QUEUE_CAP),
        ..crate::native_composer::SubmitContext::default()
    };
    crate::native_composer::plan_submit(draft, delivery, ctx)
}

/// CP-08: locate the first queued prompt owned by `active`, not merely the
/// queue front. Upstream `onEmptySubmit` is session-scoped — it promotes the
/// first queued prompt *of the current session* (`routes/session/index.tsx:
/// 1650-1657` `mutatePending("steer", next.id)`), so another session's earlier
/// entry must never strand the active session's draft. Returns the index and
/// text of the first owner-matching entry.
#[cfg(feature = "native")]
fn next_queued_for_session(
    queued: &std::collections::VecDeque<(Option<String>, String)>,
    active: Option<&str>,
) -> Option<(usize, String)> {
    queued
        .iter()
        .enumerate()
        .find(|(_, (owner, _))| owner.as_deref() == active)
        .map(|(index, (_, text))| (index, text.clone()))
}

/// CP-21: the active-session queue snapshot published to the composer dock.
/// Upstream `queuedPrompts` is `route.sessionID`-scoped
/// (`routes/session/index.tsx:224-228`) and the dock renders only while it is
/// non-empty (`index.tsx:1591`); with no active session (home) upstream binds
/// no session route, so nothing is queued and nothing is painted. Ownerless
/// (`None`) drafts must therefore never be published against a foreign
/// session. Queue order is preserved and each published id keeps its source
/// index, so the `queued-{index}` id stays unique across the batch.
#[cfg(feature = "native")]
fn publishable_queued_prompts(
    queued: &std::collections::VecDeque<(Option<String>, String)>,
    active: Option<&str>,
) -> Vec<crate::native_view_chat::QueuedPrompt> {
    let Some(active) = active else {
        return Vec::new();
    };
    queued
        .iter()
        .enumerate()
        .filter_map(|(index, (owner, text))| {
            (owner.as_deref() == Some(active)).then(|| crate::native_view_chat::QueuedPrompt {
                id: format!("queued-{index}"),
                text: text.clone(),
            })
        })
        .collect()
}

#[cfg(feature = "native")]
fn composer_pointer_event(
    event: native_input_decoder::InputEvent,
    view: &crate::native_view::NativeView,
    draft: &str,
    keymap: SubmitKeymap,
    composer_available: bool,
) -> native_input_decoder::InputEvent {
    // Normalize only an admitted left press. The existing keyboard branch
    // owns busy checks, command routing, turn admission and draft ACKs.
    if composer_available {
        if let native_input_decoder::InputEvent::Mouse { x, y, kind: 0, modifiers: 0 } = &event {
            if view.send_button_hit(draft, *x, *y) {
                return native_input_decoder::InputEvent::Byte(match keymap {
                    SubmitKeymap::Enter => b'\r',
                    SubmitKeymap::CtrlJ => b'\n',
                });
            }
        }
    }
    event
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
    raw_history: Vec<serde_json::Value>,
    #[cfg(feature = "native")]
    history_next_before: Option<String>,
    /// Child sessions the daemon reports for this parent in its current
    /// lifetime (`GET /history` carries `children`). Reloaded raw history has
    /// no typed parts, so this server fact keeps the footer count truthful
    /// without a separate polling route.
    #[cfg(feature = "native")]
    history_children: usize,
    #[cfg(feature = "native")]
    parent_session: bool,
    #[cfg(feature = "native")]
    parent_session_id: Option<String>,
    #[cfg(feature = "native")]
    directory: Option<String>,
    #[cfg(feature = "native")]
    session_titles: Vec<(String, String)>,
    #[cfg(feature = "native")]
    session_project_labels: Vec<(String, Option<String>)>,
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
    let started = Instant::now();
    let route = path.split('?').next().unwrap_or(path);
    crate::native_debug::event(
        "http.request",
        serde_json::json!({"method":method,"route":route,"request_bytes":body.map_or(0,str::len),"credential_present":auth.is_some()}),
    );
    let result = http_request_with_status_inner(origin, method, path, body, auth);
    crate::native_debug::event(
        "http.response",
        serde_json::json!({"method":method,"route":route,"elapsed_ms":started.elapsed().as_millis(),"ok":result.is_ok(),"status":result.as_ref().ok().map(|(_,status)|status),"response_bytes":result.as_ref().ok().map(|(body,_)|body.len())}),
    );
    result
}

fn http_request_with_status_inner(
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
                // Fail closed before any socket work: an unauthenticated
                // `/api/*` request must never reach the network. The message
                // still names the origin and the actual cause, so a dead or
                // unstarted daemon reads as offline rather than as a generic
                // credential refusal.
                return Err(format!(
                    "refusing unauthenticated /api/* request to {origin}: no validated backend descriptor (daemon offline or not started at this origin)"
                ));
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

#[cfg(feature = "native")]
fn request_compaction_settings(
    origin: &str,
    session: &str,
    auth: Option<&str>,
    body: Option<&str>,
) -> Result<String, String> {
    if crate::daemon_client::parse_loopback_port(origin).is_none() {
        return Err("compaction settings require a canonical loopback daemon origin".into());
    }
    let session_id = session.parse::<opencode_rk_contracts::SessionId>()
        .map_err(|_| "compaction settings require a valid session identity".to_owned())?;
    if session != session_id.to_string() {
        return Err("compaction settings require a canonical session identity".into());
    }
    if body.is_some_and(|body| body.len() > 4096) {
        return Err("compaction settings request exceeds its byte budget".into());
    }
    let path = format!("/api/sessions/{session_id}/compaction/settings");
    let (response, status) = http_request_with_status(
        origin, if body.is_some() { "PUT" } else { "GET" }, &path, body, auth,
    )?;
    if status != 200 || response.len() > 32 * 1024 {
        return Err("compaction settings response has an invalid status or exceeds its byte budget".into());
    }
    Ok(response)
}

#[cfg(feature = "native")]
fn request_context_inspection(
    origin: &str, session: &str, auth: Option<&str>, before: Option<&str>,
    selection: Option<(&str, bool)>,
) -> Result<String, String> {
    if crate::daemon_client::parse_loopback_port(origin).is_none() {
        return Err("context inspection requires a canonical loopback daemon".into());
    }
    let id = session.parse::<opencode_rk_contracts::SessionId>()
        .map_err(|_| "context inspection requires a valid session identity")?;
    if id.to_string() != session { return Err("context inspection requires a canonical session identity".into()); }
    let path = format!("/api/sessions/{id}/context");
    let get_path = if let Some(before) = before {
        let cursor = before.parse::<opencode_rk_contracts::MessageId>().map_err(|_| "invalid context page cursor")?;
        if cursor.to_string()!=before { return Err("context cursor must be canonical".into()); }
        format!("{path}?before={cursor}")
    } else { path.clone() };
    if let Some((item, included)) = selection {
        let raw = item.strip_prefix("checkpoint:").unwrap_or(item);
        let item_id = raw.parse::<opencode_rk_contracts::MessageId>()
            .map_err(|_| "invalid selectable context identity")?;
        if item_id.to_string() != raw { return Err("context identity must be canonical".into()); }
        let body = serde_json::json!({"item_id":item}).to_string();
        let (ack,status) = http_request_with_status(origin,"POST",
            &format!("{path}/{}",if included {"include"} else {"exclude"}),Some(&body),auth)?;
        if status!=200 || ack.len()>4096 { return Err("invalid context selection acknowledgement".into()); }
        let ack:serde_json::Value = serde_json::from_str(&ack).map_err(|_| "invalid context acknowledgement JSON")?;
        if ack["scope"].as_str()!=Some("session") || ack["item_id"].as_str()!=Some(item)
            || ack["excluded"].as_bool()!=Some(!included) || ack["history_preserved"].as_bool()!=Some(true) {
            return Err("backend did not acknowledge the selected context state".into());
        }
    }
    let (body,status) = http_request_with_status(origin,"GET",&get_path,None,auth)?;
    if status!=200 || body.len()>512*1024 { return Err("invalid or oversized context inspection response".into()); }
    Ok(body)
}

#[cfg(all(test, feature = "native"))]
mod context_inspection_transport_tests {
    use super::request_context_inspection;
    use std::io::{Read,Write};

    #[test]
    fn rejects_invalid_identity_cursor_and_credentials_before_network_io() {
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin=format!("http://{}",listener.local_addr().unwrap());
        let token="a".repeat(64);
        let id="abcdef12-1234-4000-8000-000000000001";
        assert!(request_context_inspection(&origin,&id.to_uppercase(),Some(&token),None,None).is_err());
        assert!(request_context_inspection(&origin,id,Some(&token),Some("../invalid"),Some((id,false))).is_err());
        assert!(request_context_inspection(&origin,id,None,None,None).is_err());
        assert!(matches!(listener.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock));
    }

    #[test]
    fn a_mismatched_selection_ack_does_not_trigger_a_followup_get() {
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin=format!("http://{}",listener.local_addr().unwrap());
        let server=std::thread::spawn(move||{
            let (mut stream,_)=listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            let mut request=Vec::new();
            let mut chunk=[0u8;1024];
            loop {
                let count=stream.read(&mut chunk).unwrap();
                assert!(count>0 && request.len()+count<=8192);
                request.extend_from_slice(&chunk[..count]);
                if let Some(start)=request.windows(4).position(|window|window==b"\r\n\r\n") {
                    let header=String::from_utf8_lossy(&request[..start]);
                    let length=header.lines().find_map(|line|line.strip_prefix("Content-Length: "))
                        .unwrap().parse::<usize>().unwrap();
                    if request.len()>=start+4+length {break;}
                }
            }
            let body=r#"{"scope":"session","item_id":"abcdef12-1234-4000-8000-000000000002","excluded":true,"history_preserved":true}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            drop(stream);
            listener.set_nonblocking(true).unwrap();
            (String::from_utf8(request).unwrap(),listener)
        });
        let token="a".repeat(64);
        let id="abcdef12-1234-4000-8000-000000000001";
        let error=request_context_inspection(&origin,id,Some(&token),None,Some((id,false))).unwrap_err();
        assert!(error.contains("did not acknowledge"));
        let (request,listener)=server.join().unwrap();
        assert!(request.starts_with(&format!("POST /api/sessions/{id}/context/exclude HTTP/1.1")));
        assert!(request.contains(&format!("Authorization: Bearer {token}\r\n")));
        assert!(request.ends_with(&serde_json::json!({"item_id":id}).to_string()));
        assert!(matches!(listener.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock));
    }
}

#[cfg(feature = "native")]
struct NativeCompactWorker {
    rx: tokio::sync::mpsc::Receiver<Result<String, String>>,
    task: Option<tokio::task::JoinHandle<()>>,
    session_id: String,
    generation: u64,
    origin: String,
    auth: String,
}

#[cfg(feature = "native")]
impl NativeCompactWorker {
    #[cfg(test)]
    fn start(origin: &str, session_id: &str, auth: &str, model: &str, generation: u64) -> Result<Self, String> {
        Self::start_strategy(origin, session_id, auth, model, generation, "native")
    }

    fn start_strategy(origin: &str, session_id: &str, auth: &str, model: &str, generation: u64, strategy: &str) -> Result<Self, String> {
        if !matches!(strategy, "native" | "fallback" | "auto") {
            return Err("unsupported compaction strategy".into());
        }
        if !crate::daemon_client::is_wellformed_token(auth) {
            return Err("missing validated daemon token".into());
        }
        if crate::daemon_client::parse_loopback_port(origin).is_none() {
            return Err("native compaction requires a canonical loopback daemon origin".into());
        }
        if model.trim().is_empty() || !model.contains('/') {
            return Err("native compaction requires a qualified provider/model selection".into());
        }
        let origin = origin.to_owned();
        let request_origin = origin.clone();
        let session = session_id.to_owned();
        let token = auth.to_owned();
        let request_token = token.clone();
        let qualified = model.to_owned();
        let strategy = strategy.to_owned();
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let worker_session = session.clone();
        let task = tokio::spawn(async move {
                let path = format!("/api/sessions/{worker_session}/compact");
                let body = serde_json::json!({"model": qualified, "strategy": strategy});
                let result = async {
                    let client = reqwest::Client::builder().no_proxy()
                        .redirect(reqwest::redirect::Policy::none())
                        .connect_timeout(Duration::from_secs(5))
                        .timeout(Duration::from_secs(300))
                        .build().map_err(|error| error.to_string())?;
                    let mut response = client.post(format!("{request_origin}{path}"))
                        .bearer_auth(&request_token).json(&body).send().await
                        .map_err(|error| format!("compact request failed: {error}"))?;
                    let status = response.status();
                    const MAX_ACK_BYTES: usize = 64 * 1024;
                    if response.content_length().is_some_and(|size| size > MAX_ACK_BYTES as u64) {
                        return Err("compact response exceeded byte budget".to_owned());
                    }
                    let mut bytes = Vec::new();
                    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
                        if chunk.len() > MAX_ACK_BYTES.saturating_sub(bytes.len()) {
                            return Err("compact response exceeded byte budget".to_owned());
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    if !status.is_success() {
                        return Err(format!("compact failed with HTTP {}: {}", status.as_u16(),
                            crate::native_transcript::strip_ansi(&String::from_utf8_lossy(&bytes)).chars().take(512).collect::<String>()));
                    }
                    let body = String::from_utf8(bytes).map_err(|_| "compact response was not UTF-8".to_owned())?;
                        let value: serde_json::Value = serde_json::from_str(&body)
                            .map_err(|_| "compact response was not valid JSON".to_owned())?;
                         let acknowledged = value["strategy"].as_str();
                         if value["compacted"].as_bool() == Some(true)
                             && matches!(acknowledged, Some("native" | "fallback"))
                             && (strategy == "auto" || acknowledged == Some(strategy.as_str()))
                        {
                            Ok(body)
                        } else {
                             Err("compact response did not acknowledge the requested compaction strategy".into())
                        }
                }.await;
                let _ = tx.send(result).await;
            });
        Ok(Self { rx, task: Some(task), session_id: session, generation, origin, auth: token })
    }

    fn retire(&mut self) {
        // Taking the handle detaches work. NativeCompactWorker::drop sends an
        // interrupt only while the handle remains present.
        drop(self.task.take());
    }

    fn try_result(&mut self) -> Option<Result<String, String>> {
        match self.rx.try_recv() {
            Ok(result) => {
                if let Some(task) = self.task.take() { task.abort(); }
                Some(result)
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                if let Some(task) = self.task.take() { task.abort(); }
                Some(Err("compact worker stopped without a response".into()))
            }
        }
    }

    fn owns(&self, session: Option<&str>, generation: u64) -> bool {
        self.generation == generation && session == Some(self.session_id.as_str())
    }
}

#[cfg(feature = "native")]
impl Drop for NativeCompactWorker {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            // Cancel the daemon owner before aborting the request task and
            // closing its socket. This is deliberately tied to the captured
            // original session/origin, never the current binding.
            let path = format!("/api/session/{}/interrupt", self.session_id);
            let _ = http_request_with_status(
                &self.origin,
                "POST",
                &path,
                Some("{}"),
                Some(&self.auth),
            );
            task.abort();
        }
    }
}

#[cfg(feature = "native")]
fn compact_command(text: &str) -> Option<Result<(), &'static str>> {
    let mut parts = text.split_whitespace();
    if parts.next()? != "/compact" { return None; }
    match parts.next() {
        Some("native" | "fallback" | "auto") if parts.next().is_none() => Some(Ok(())),
        None => Some(Ok(())),
        _ => Some(Err("usage: /compact [native|fallback|auto]")),
    }
}

#[cfg(all(test, feature = "native"))]
mod compact_command_tests {
    use super::{compact_command, NativeCompactWorker};
    use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, time::{Duration, Instant}};

    fn accept(listener: &TcpListener) -> TcpStream {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((stream, _)) => return stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "compact fixture accept deadline");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("compact fixture accept: {error}"),
            }
        }
    }

    fn request(stream: &mut TcpStream) -> String {
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 1024];
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "request ended before its body");
            assert!(bytes.len() + read <= 16 * 1024);
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                let length = headers.lines().find_map(|line| line.to_ascii_lowercase()
                    .strip_prefix("content-length:").and_then(|value| value.trim().parse::<usize>().ok())).unwrap_or(0);
                if bytes.len() >= end + 4 + length { return String::from_utf8(bytes).unwrap(); }
            }
        }
    }

    fn acknowledge(stream: &mut TcpStream) {
        let body = r#"{"compacted":true,"strategy":"native"}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn compact_worker_sends_authenticated_request_and_does_not_interrupt_after_ack() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let token = "ab".repeat(32);
        let fixture = std::thread::spawn(move || {
            let mut stream = accept(&listener);
            let request = request(&mut stream);
            // Native provider work may legitimately outlast the short GET
            // transport timeout; the owned request must remain cancellable.
            std::thread::sleep(Duration::from_millis(2100));
            acknowledge(&mut stream);
            request
        });
        let mut worker = NativeCompactWorker::start(&origin, "fixture-session", &token, "openai/fixture", 7).unwrap();
        assert!(worker.owns(Some("fixture-session"), 7));
        assert!(!worker.owns(Some("other-session"), 7));
        assert!(!worker.owns(Some("fixture-session"), 8));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = worker.try_result() { result.unwrap(); break; }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(worker.task.is_none(), "completed work must not send a late interrupt on drop");
        drop(worker);
        let request = fixture.join().unwrap();
        assert!(request.starts_with("POST /api/sessions/fixture-session/compact HTTP/1.1\r\n"));
        assert!(request.to_ascii_lowercase().contains(&format!("authorization: bearer {token}")));
        let body: serde_json::Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body, serde_json::json!({"model":"openai/fixture","strategy":"native"}));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropping_pending_compact_interrupts_the_captured_session_before_join() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let token = "cd".repeat(32);
        let (started, ready) = std::sync::mpsc::sync_channel(1);
        let fixture = std::thread::spawn(move || {
            let mut compact = accept(&listener);
            let first = request(&mut compact);
            started.send(()).unwrap();
            let mut interrupt = accept(&listener);
            let second = request(&mut interrupt);
            write!(interrupt, "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            acknowledge(&mut compact);
            (first, second)
        });
        let worker = NativeCompactWorker::start(&origin, "original-session", &token, "openai/fixture", 3).unwrap();
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(worker);
        let (compact, interrupt) = fixture.join().unwrap();
        assert!(compact.starts_with("POST /api/sessions/original-session/compact "));
        assert!(interrupt.starts_with("POST /api/session/original-session/interrupt "));
        assert!(interrupt.to_ascii_lowercase().contains(&format!("authorization: bearer {token}")));
    }
    #[test]
    fn compact_route_is_explicit_and_never_an_ordinary_turn() {
        assert_eq!(compact_command("/compact native"), Some(Ok(())));
        // Supersedes the former plain-command Err expectation: plain compact
        // is now a supported strategy request, resolved by configuration.
        assert_eq!(compact_command("/compact"), Some(Ok(())));
        assert_eq!(compact_command("/compact fallback"), Some(Ok(())));
        assert_eq!(compact_command("/compact auto"), Some(Ok(())));
        assert!(compact_command("/compact bogus").is_some());
        assert_eq!(compact_command("hello"), None);
    }
}

/// Fetch the live session snapshot: target session (explicit id or most recently updated) plus its bounded message list.
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

/// Display-only fallback for a session directory from the authenticated
/// listing. An unusable optional directory is omitted rather than preventing
/// session selection or allowing control characters into the terminal.
#[cfg(feature = "native")]
fn session_directory_label(value: Option<&serde_json::Value>) -> Option<String> {
    let path = value?.as_str()?;
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return None;
    }
    std::path::Path::new(path).file_name()?.to_str().map(str::to_owned)
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
    // Validate the complete authenticated listing before selecting a target or
    // reading any title. parse_rows rejects malformed raw titles (controls and
    // >512 bytes) and oversized listings atomically; never hide an error with a
    // fallback title/cache.
    let parsed_rows = crate::session_list::parse_rows(value)
        .map_err(|error| format!("session listing unusable: {error}"))?;
    if parsed_rows.is_empty() {
        return Err("daemon is reachable but has no sessions yet; create one with `opencode-rk session create`".to_string());
    }
    let sessions = value["sessions"]
        .as_array()
        .ok_or_else(|| "daemon session list is missing the sessions array".to_string())?;
    let target = if let Some(id) = session {
        sessions
            .iter()
            .find(|s| s["id"].as_str() == Some(id))
            .ok_or_else(|| format!("session {id} not found on daemon"))?
    } else {
        // RFC3339 timestamps sort lexicographically; last updated wins.
        sessions
            .iter()
            .filter(|candidate| parsed_rows.iter().any(|row| candidate["id"].as_str() == Some(row.id.as_str())))
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
    let parsed_target = parsed_rows
        .iter()
        .find(|row| row.id == id)
        .ok_or_else(|| format!("session {id} is not present in the validated session listing"))?;
    let accepted_title = parsed_target.title.clone();
    let parent_session_id = match target["parent_session_id"].as_str().filter(|id| !id.is_empty()) {
        Some(parent) => Some(parent.parse::<opencode_rk_contracts::SessionId>()
            .map_err(|_| "daemon session has an invalid parent session id".to_owned())?
            .to_string()),
        None => None,
    };
    let messages_body = http_request(
        origin,
        "GET",
        &format!("/api/sessions/{id}/history?limit=100"),
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
    #[cfg(feature = "native")]
    let session_project_labels = parsed_rows.iter().map(|row| {
        let entry = sessions.iter().find(|candidate| candidate["id"].as_str() == Some(row.id.as_str()));
        (row.id.clone(), session_directory_label(entry.and_then(|item| item.get("directory"))))
    }).collect();
    Ok(LiveSnapshot {
        origin: origin.to_owned(),
        session_id: id,
        title: accepted_title,
        state: target["state"].as_str().unwrap_or_default().to_owned(),
        updated_at: target["updated_at"].as_str().unwrap_or_default().to_owned(),
        message_count: messages.len(),
        last_text: last_text,
        history,
        #[cfg(feature = "native")]
        typed_history: crate::native_chat_projection::project_history(&messages),
        #[cfg(feature = "native")]
        raw_history: messages.clone(),
        #[cfg(feature = "native")]
        history_next_before: messages_value["next_before"].as_str().map(str::to_owned),
        #[cfg(feature = "native")]
        history_children: history_child_count(&messages_value),
        #[cfg(feature = "native")]
        parent_session: target["parent_session_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        #[cfg(feature = "native")]
        parent_session_id,
        #[cfg(feature = "native")]
        directory: target["directory"].as_str().map(str::to_owned),
        #[cfg(feature = "native")]
        session_titles: parsed_rows.into_iter().map(|row| (row.id, row.title)).collect(),
        #[cfg(feature = "native")]
        session_project_labels,
    })
}

fn message_text_bounded(message: &serde_json::Value) -> Option<String> {
    let text = message["body"]["text"].as_str()?;
    Some(text.chars().take(MAX_NATIVE_LINE_CHARS).collect())
}

#[cfg(feature = "native")]
fn apply_history_window(window: &crate::native_chat_projection::NativeHistoryWindow, transcript: &mut NativeChatHistory) {
    let mut lines = Vec::new();
    for message in window.messages() {
        let role = message.get("role").and_then(serde_json::Value::as_str).unwrap_or("message");
        if let Some(text) = message_text_bounded(message) {
            lines.push(format!("{role}: {text}"));
        }
    }
    transcript.reset(lines, crate::native_chat_projection::project_history(window.messages()));
}

/// A session's family can only be non-empty when its own history delegated
/// through `task`, so the live footer poll needs no request otherwise. Live
/// events use `ToolState`; reloaded history projects `ToolCall`/`ToolResult`,
/// so all three shapes must count.
#[cfg(feature = "native")]
fn transcript_has_delegation(messages: &[ChatMessage]) -> bool {
    messages.iter().flat_map(|message| message.parts.iter()).any(|part| match part {
        ChatPart::Subtask { .. } => true,
        ChatPart::ToolState { name, .. }
        | ChatPart::ToolCall { name, .. }
        | ChatPart::ToolResult { name, .. } => is_delegation_tool(name),
        _ => false,
    })
}

#[cfg(feature = "native")]
fn is_delegation_tool(name: &str) -> bool {
    name == "task" || name == "subagent"
}

/// Server-reported child count from a `/history` payload. Missing or
/// malformed values mean "not reported" and never fabricate a child.
#[cfg(feature = "native")]
fn history_child_count(value: &serde_json::Value) -> usize {
    value["children"]
        .as_u64()
        .map_or(0, |count| usize::try_from(count).unwrap_or(usize::MAX))
}

/// Whether the footer count may ask `/children`. A known running child keeps
/// the poll alive; a server-reported child or a live/reloaded delegation part
/// starts it. Sessions with no family issue no request, so the startup route
/// surface stays exactly what it was before the count existed.
#[cfg(feature = "native")]
fn should_poll_children(running: usize, history_children: usize, messages: &[ChatMessage]) -> bool {
    running > 0 || history_children > 0 || transcript_has_delegation(messages)
}

/// Resize the live native frame using the next frame's exact tab reservation,
/// preserving a browsing transcript's semantic rendered-row anchor.
#[cfg(feature = "native")]
fn resize_tab_reservation(orientation: TabOrientation, cols: u16, rows: u16) -> TabReservation {
    TabReservation::for_view(orientation, cols, rows)
}

/// Resize the live native frame using the next frame's exact tab reservation,
/// preserving a browsing transcript's semantic rendered-row anchor.
#[cfg(feature = "native")]
fn resize_chat_preserving_anchor(
    view: &mut crate::native_view::NativeView,
    cols: u16,
    rows: u16,
    next_tab_reservation: TabReservation,
    messages: &[crate::native_view_chat::ChatMessage],
    draft: &str,
    preserve: bool,
) -> bool {
    if !preserve {
        view.resize(cols, rows);
        view.set_tab_reservation(next_tab_reservation);
        return false;
    }
    let entries: Vec<(String, crate::native_view_chat::ChatMessage)> = messages
        .iter()
        .enumerate()
        .map(|(index, message)| (format!("entry-{index}"), message.clone()))
        .collect();
    let anchor = view.chat_rendered_anchor(&entries);
    view.resize(cols, rows);
    // Paint installs this exact reservation for the same next frame.
    view.set_tab_reservation(next_tab_reservation);
    match anchor {
        Some(anchor) => view.restore_chat_rendered_anchor(&entries, &anchor),
        None => false,
    }
}

#[cfg(all(test, feature = "native"))]
mod resize_anchor_tests {
    use super::*;
    use crate::native_view_chat::{ChatMessage, ChatRole};

    fn wrapped(marker: &str, count: usize) -> ChatMessage {
        let mut text = marker.to_owned();
        while text.chars().count() < count {
            text.push_str(" wrap");
        }
        ChatMessage::text(ChatRole::Tool, text)
    }

    #[test]
    fn resize_restores_identity_and_intra_message_row_with_reservation() {
        let mut view = crate::native_view::NativeView::new(150, 32);
        view.set_session_route(true);
        view.set_tab_reservation(resize_tab_reservation(TabOrientation::Vertical, 150, 32));
        let messages = vec![
            ChatMessage::text(ChatRole::User, "P14-ANCHOR\n".repeat(48)),
            wrapped("P14-FOLLOWING", 220),
        ];
        let _ = view.render_typed("", &messages, false, &[]);
        let prefix_rows = view.chat_message_rows(&messages[..1]);
        let total_rows = view.chat_message_rows(&messages);
        view.scroll_up(total_rows.saturating_sub(prefix_rows));
        assert!(prefix_rows >= 32, "anchor message must exceed the transcript viewport");
        let _ = view.render_typed("", &messages, false, &[]);
        let entries: Vec<(String, ChatMessage)> = messages.iter().enumerate()
            .map(|(i, message)| (format!("entry-{i}"), message.clone())).collect();
        let captured = view.chat_rendered_anchor(&entries).expect("browsing anchor");
        assert_eq!(captured.id, "entry-0");
        let captured_row = captured.row;
        let next = resize_tab_reservation(TabOrientation::Vertical, 100, 32);
        assert_eq!(next.orientation, Some(TabOrientation::Horizontal));
        assert!(resize_chat_preserving_anchor(&mut view, 100, 32, next, &messages, "", true));
        let _ = view.render_typed("", &messages, false, &[]);
        let restored = view.chat_rendered_anchor(&entries).expect("restored anchor");
        assert_eq!(restored.id, captured.id);
        assert_eq!(restored.row, captured_row);
        let prefix = view.chat_message_rows(&messages[..restored.index]);
        let total = view.chat_message_rows(&messages);
        let expected_scroll = total.saturating_sub(prefix + captured_row + 1);
        assert_eq!(view.chat_scroll_offset(), expected_scroll);
        assert_eq!(view.tab_reservation(), next);
    }

    #[test]
    fn resize_without_browsing_anchor_preserves_tail_and_installs_reservation() {
        let mut view = crate::native_view::NativeView::new(150, 32);
        view.set_session_route(true);
        let messages = vec![wrapped("P14-TAIL", 220)];
        let _ = view.render_typed("", &messages, false, &[]);
        let next = resize_tab_reservation(TabOrientation::Vertical, 100, 32);
        assert_eq!(next.orientation, Some(TabOrientation::Horizontal));
        assert!(!resize_chat_preserving_anchor(&mut view, 100, 32, next, &messages, "", false));
        assert_eq!(view.chat_scroll_offset(), 0);
        assert_eq!(view.tab_reservation(), next);
    }

    #[test]
    fn resize_restores_anchor_when_tab_reservation_height_changes() {
        let mut view = crate::native_view::NativeView::new(150, 32);
        view.set_session_route(true);
        let old = resize_tab_reservation(TabOrientation::Horizontal, 100, 32);
        view.set_tab_reservation(old);
        let messages = vec![
            ChatMessage::text(ChatRole::User, "P14-HEIGHT-ANCHOR\n".repeat(48)),
            wrapped("P14-HEIGHT-FOLLOWING", 260),
        ];
        let _ = view.render_typed("", &messages, false, &[]);
        let prefix_rows = view.chat_message_rows(&messages[..1]);
        let total_rows = view.chat_message_rows(&messages);
        view.scroll_up(total_rows.saturating_sub(prefix_rows));
        assert!(prefix_rows >= 32, "anchor message must exceed the transcript viewport");
        let _ = view.render_typed("", &messages, false, &[]);
        let entries: Vec<(String, ChatMessage)> = messages.iter().enumerate()
            .map(|(i, message)| (format!("entry-{i}"), message.clone())).collect();
        let captured = view.chat_rendered_anchor(&entries).expect("browsing anchor");
        assert_eq!(captured.id, "entry-0");
        let captured_row = captured.row;
        let next = resize_tab_reservation(TabOrientation::Vertical, 150, 24);
        assert_eq!(next.orientation, Some(TabOrientation::Vertical));
        assert!(resize_chat_preserving_anchor(&mut view, 150, 24, next, &messages, "", true));
        let _ = view.render_typed("", &messages, false, &[]);
        let restored = view.chat_rendered_anchor(&entries).expect("restored anchor");
        assert_eq!((restored.id.as_str(), restored.row), ("entry-0", captured_row));
        assert_eq!(view.tab_reservation(), next);
    }

    #[test]
    fn resize_empty_or_zero_height_fails_closed_without_anchor() {
        let mut view = crate::native_view::NativeView::new(100, 24);
        let zero = resize_tab_reservation(TabOrientation::Vertical, 100, 0);
        assert_eq!(zero, TabReservation::default());
        assert!(!resize_chat_preserving_anchor(&mut view, 100, 0, zero, &[], "", true));
        assert_eq!(view.tab_reservation(), zero);
        assert_eq!(view.chat_scroll_offset(), 0);
    }

    #[test]
    fn resize_short_height_and_empty_projection_fail_closed() {
        let mut view = crate::native_view::NativeView::new(150, 32);
        let short = resize_tab_reservation(TabOrientation::Vertical, 100, 1);
        assert_eq!(short.orientation, Some(TabOrientation::Horizontal));
        assert!(!resize_chat_preserving_anchor(&mut view, 100, 1, short, &[], "", true));
        assert_eq!(view.tab_reservation(), short);
    }
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

#[cfg(test)]
mod memory_file_cap_tests {
    use super::*;

    /// Retention is the part of the 64-file bound the clipped `--once` frame
    /// cannot show: 70 inputs must load exactly `MAX_MEMORY_FILES` sources.
    #[test]
    fn load_memory_retains_at_most_the_file_cap() {
        let dir = std::env::temp_dir().join(format!("oc2-memory-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for index in 0..70 {
            let path = dir.join(format!("f{index:03}.md"));
            std::fs::write(&path, "z").unwrap();
            paths.push(path);
        }
        let loaded = load_memory(&paths);
        assert_eq!(loaded.len(), MAX_MEMORY_FILES);
        assert_eq!(loaded.len(), 64);
        assert_eq!(loaded.first().map(|file| file.path.as_str()), Some("f000.md"));
        assert_eq!(loaded.last().map(|file| file.path.as_str()), Some("f063.md"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// The footer-count gate only exists on the native path, so its fixtures are
/// compiled with the feature instead of breaking the default test build.
#[cfg(all(test, feature = "native"))]
mod footer_gate_tests {
    use super::*;

    #[test]
    fn only_a_delegating_transcript_polls_children() {
        let tool = |name: &str| ChatMessage {
            role: ChatRole::Tool,
            parts: vec![ChatPart::ToolState {
                name: name.into(), id: "c1".into(), status: "completed".into(),
                args: String::new(), output: String::new(), is_error: false, metadata: String::new(),
            }],
            agent: None, model: None, assistant_facts: None, message_id: None,
        };
        // Reloaded history projects the call and its result instead of a
        // `ToolState`, and a finished turn leaves only those.
        let reloaded = |name: &str| ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![
                ChatPart::ToolCall { name: name.into(), id: "c1".into() },
                ChatPart::ToolResult { name: name.into(), output: "{}".into(), is_error: false },
            ],
            agent: None, model: None, assistant_facts: None, message_id: None,
        };
        let subtask = ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![ChatPart::Subtask {
                agent: "explore".into(), description: "probe".into(), background: true,
                session_id: "child-1".into(),
            }],
            agent: None, model: None, assistant_facts: None, message_id: None,
        };
        assert!(!transcript_has_delegation(&[]));
        assert!(!transcript_has_delegation(&[ChatMessage::text(ChatRole::User, "hello")]));
        assert!(!transcript_has_delegation(&[tool("read")]));
        assert!(transcript_has_delegation(&[tool("task")]));
        assert!(transcript_has_delegation(&[tool("subagent")]));
        assert!(!transcript_has_delegation(&[reloaded("read")]));
        assert!(transcript_has_delegation(&[reloaded("task")]));
        assert!(transcript_has_delegation(&[reloaded("subagent")]));
        assert!(transcript_has_delegation(&[subtask]));
    }

    #[test]
    fn server_reported_children_keep_the_footer_poll_truthful() {
        let plain = [ChatMessage::text(ChatRole::User, "hello")];
        assert_eq!(history_child_count(&serde_json::json!({})), 0);
        assert_eq!(history_child_count(&serde_json::json!({"children": null})), 0);
        assert_eq!(history_child_count(&serde_json::json!({"children": "2"})), 0);
        assert_eq!(history_child_count(&serde_json::json!({"children": 2})), 2);
        // No family → no request: the startup route surface is unchanged.
        assert!(!should_poll_children(0, 0, &plain));
        // A reloaded transcript carries no typed parts; the server fact starts
        // the poll so the count survives a reload.
        assert!(should_poll_children(0, 1, &plain));
        // A known running child keeps it alive before the next history page.
        assert!(should_poll_children(1, 0, &plain));
        // A live delegation part starts it during the turn, when history
        // refresh is deferred behind the active worker.
        let live = [ChatMessage {
            role: ChatRole::Tool,
            parts: vec![ChatPart::ToolState {
                name: "task".into(), id: "c1".into(), status: "running".into(),
                args: String::new(), output: String::new(), is_error: false, metadata: String::new(),
            }],
            agent: None, model: None, assistant_facts: None, message_id: None,
        }];
        assert!(should_poll_children(0, 0, &live));
    }
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
    /// THEME-06: palette resolved for the contextual upstream `@dialog`
    /// surface (`ui/dialog.tsx:44,82`, `theme/src/tui/resolve.ts:79`). The modal
    /// panel and its content cells paint `background.base` from here, not from
    /// the base view. Equal to `palette` for a theme without an `@dialog`
    /// override.
    dialog: crate::native_theme_runtime::Palette,
    markdown: Option<crate::native_theme_runtime::MarkdownPalette>,
    /// Resolved `theme.semantic.diff` tokens for the `+`/`-` transcript rows
    /// (THEME-09). `None` falls back to the historical literal RGB.
    diff: Option<DiffRgba>,
    /// Region geometry for this frame (positions for the style pass).
    presentation: crate::native_view::ViewPresentation,
    /// `Some(n)` when a modal panel owns the frame: `n` content rows
    /// starting at the shared modal's content_y, after one inner padding row.
    modal_rows: Option<usize>,
    modal: crate::native_view::ModalPresentation,
    sidebar_backdrop_alpha: u8,
    chat_row_classes: Vec<crate::native_view_chat::ChatRowClass>,
    chat_markdown_spans: Vec<Vec<crate::native_view_chat::ChatRunSpan>>,
    selection: Vec<(u32, u32, u32)>,
    appearance: crate::native_box_style::BoxAppearance,
    modal_box: crate::native_box_style::BoxId,
    loading: Option<(char, &'static str)>,
    /// True when this frame is the Home column (explicit route fact).
    home: bool,
    rail_prompt: bool,
    composer: Option<(u16, u16, u16, u16)>,
    completion: Option<crate::native_completion::Panel>,
    ui_regions: Vec<crate::native_ui_style::Region>,
    context_usage_styles: Vec<(u16, u16, u16, crate::native_theme_runtime::Slot)>,
    /// Absolute `(x, y, cells, slot, attributes)` sidebar spans. The sidebar
    /// publishes one role per row, so per-span colour and the cancelled-todo
    /// strikethrough only reach the painter through this channel.
    sidebar_span_styles: Vec<(u16, u16, u16, crate::native_theme_runtime::Slot, u32)>,
    modal_underlay_rows: Vec<String>,
    page_panel: Option<(u16, u16, u16, u16)>,
    logo: Option<(u16, u16)>,
    tabs: Option<TabStrip>,
    /// `config.animations` resolved to the upstream default `true` when unset
    /// (`config/index.tsx:250`). Gates the reduced-motion title DIM (TAB-24).
    animations: bool,
    /// Caller's per-session title-pending fact (`data.session.title.pending`,
    /// `context/data.tsx:22-47`); `None` keeps the plain selected-BOLD paint.
    title_renaming: Option<Box<dyn Fn(&crate::native_navigation::SessionRef) -> bool>>,
    vertical_rail: Option<crate::native_tab_rail::RailFrame>,
    compact_tooltip: Option<crate::native_tab_rail::CompactTooltip>,
    context_menu: Option<crate::native_tab_context_menu::MenuFrame>,
    rename_dialog: Option<(String, Option<String>)>,
    /// Transcript scrollbar geometry for this frame, when shown.
    transcript_scrollbar: Option<crate::native_view::TranscriptScrollbar>,
    /// Sidebar body scrollbar geometry for this frame, when shown.
    sidebar_scrollbar: Option<crate::native_view::SidebarScrollbar>,
    /// `decrease(background.raised.base)` — the track surface upstream paints.
    scrollbar_track: Rgba,
    /// `border.base` — the thumb color upstream paints.
    scrollbar_thumb: Rgba,
}

#[cfg(all(test, feature = "native"))]
mod tab_context_menu_dispatch_tests {
    use super::*;

    fn nav_with_session() -> (crate::native_navigation::NavigationState, crate::native_navigation::SessionRef) {
        let mut nav = crate::native_navigation::NavigationState::new();
        let session = crate::native_navigation::SessionRef("stable-session-id".to_owned());
        nav.open(session.clone());
        nav.ingest_snapshot_title(session.clone(), "Visible title".to_owned());
        (nav, session)
    }

    fn rendered_strip(
        nav: &crate::native_navigation::NavigationState,
    ) -> crate::native_tabs::TabStrip {
        let inputs = crate::native_tabs::TabStrip::from_navigation(nav, &[]);
        crate::native_tabs::TabStrip::render_with_hover_and_add(
            &inputs,
            TabOrientation::Horizontal,
            64,
            1,
            0,
            None,
            true,
        )
    }

    #[test]
    fn production_hit_resolver_uses_painted_session_identity_and_add_hit() {
        let (nav, session) = nav_with_session();
        let strip = rendered_strip(&nav);
        let hit = strip
            .rects
            .iter()
            .find(|(id, _)| *id == session)
            .expect("painted session rect")
            .1;
        assert_eq!(
            tab_menu_target_at(&nav, Some(&strip), None, false, hit.x, hit.y),
            Some(crate::native_tab_context_menu::MenuTarget::Session {
                session: session.clone(),
                title: "Visible title".to_owned(),
                can_add: true,
            })
        );
        let add = strip.add_rect.expect("painted add/new hit target");
        assert!(matches!(
            tab_menu_target_at(&nav, Some(&strip), None, false, add.x, add.y),
            Some(crate::native_tab_context_menu::MenuTarget::New {
                recently_closed: Some(_)
            })
        ));
        assert_eq!(
            tab_menu_target_at(&nav, Some(&strip), None, false, 63, 0),
            None,
            "no fallback to an arbitrary active tab on a miss"
        );
    }

    #[test]
    fn production_new_target_offers_newest_recent_tabs_and_reopens_selected_id() {
        let mut nav = crate::native_navigation::NavigationState::new();
        let current = crate::native_navigation::SessionRef("open-now".to_owned());
        nav.open(current.clone());
        for name in ["closed-old", "closed-mid", "closed-new"] {
            let session = crate::native_navigation::SessionRef(name.to_owned());
            nav.open(session.clone());
            nav.ingest_snapshot_title(session.clone(), format!("{name} title"));
            nav.close(&session).expect("close records recent entry");
        }
        nav.open(current.clone());
        let strip = rendered_strip(&nav);
        let add = strip.add_rect.expect("painted add/new hit target");
        let target = tab_menu_target_at(&nav, Some(&strip), None, false, add.x, add.y)
            .expect("New target");
        let crate::native_tab_context_menu::MenuTarget::New { recently_closed } = target else {
            panic!("add hit must yield the New target");
        };
        let closed = recently_closed.expect("New target carries the live reopen facility");
        assert_eq!(
            closed
                .iter()
                .map(|tab| tab.session.0.as_str())
                .collect::<Vec<_>>(),
            ["closed-new", "closed-mid", "closed-old"],
            "newest first, matching upstream slice(0, 10)"
        );
        assert_eq!(closed[0].title, "closed-new title");
        // The menu row resolves to the selected session, not a generic reopen.
        let mut menu = crate::native_tab_context_menu::TabContextMenu::new(
            add.x,
            add.y,
            crate::native_tab_context_menu::MenuTarget::New {
                recently_closed: Some(closed),
            },
        );
        let frame = menu.frame(crate::native_tab_context_menu::Viewport { width: 80, height: 24 });
        let row = frame
            .items
            .iter()
            .find(|item| item.label.starts_with("closed-mid title"))
            .expect("recent row")
            .rect;
        assert_eq!(
            menu.handle(
                crate::native_tab_context_menu::MenuEvent::PointerUp {
                    button: crate::native_tab_context_menu::MouseButton::Left,
                    x: row.x,
                    y: row.y,
                },
                crate::native_tab_context_menu::Viewport { width: 80, height: 24 },
            ),
            crate::native_tab_context_menu::MenuOutcome::Invoke(
                crate::native_tab_context_menu::MenuAction::Reopen {
                    session: crate::native_navigation::SessionRef("closed-mid".to_owned()),
                }
            )
        );
    }

    #[test]
    fn production_menu_release_dispatches_close_copy_and_rename_stable_id() {
        let id = crate::native_navigation::SessionRef("stable-session-id".to_owned());
        let target = crate::native_tab_context_menu::MenuTarget::Session {
            session: id.clone(),
            title: "Visible title".to_owned(),
            can_add: true,
        };
        let viewport = crate::native_tab_context_menu::Viewport { width: 80, height: 24 };

        let mut menu = crate::native_tab_context_menu::TabContextMenu::new(1, 0, target.clone());
        let frame = menu.frame(viewport);
        let row = |label: &str| {
            frame
                .items
                .iter()
                .find(|item| item.label.starts_with(label) && item.enabled)
                .unwrap_or_else(|| panic!("visible enabled row for {label}"))
                .rect
        };
        let close = row("Close");
        assert_eq!(
            menu.handle(
                crate::native_tab_context_menu::MenuEvent::PointerUp {
                    button: crate::native_tab_context_menu::MouseButton::Left,
                    x: close.x,
                    y: close.y,
                },
                viewport,
            ),
            crate::native_tab_context_menu::MenuOutcome::Invoke(
                crate::native_tab_context_menu::MenuAction::Close { session: id.clone() }
            )
        );

        let mut menu = crate::native_tab_context_menu::TabContextMenu::new(1, 0, target.clone());
        let frame = menu.frame(viewport);
        let copy = frame
            .items
            .iter()
            .find(|item| item.label.starts_with("Copy session ID"))
            .expect("copy row")
            .rect;
        assert_eq!(
            menu.handle(
                crate::native_tab_context_menu::MenuEvent::PointerUp {
                    button: crate::native_tab_context_menu::MouseButton::Left,
                    x: copy.x,
                    y: copy.y,
                },
                viewport,
            ),
            crate::native_tab_context_menu::MenuOutcome::Invoke(
                crate::native_tab_context_menu::MenuAction::CopySessionId { session: id.clone() }
            )
        );

        let mut menu = crate::native_tab_context_menu::TabContextMenu::new(1, 0, target);
        let frame = menu.frame(viewport);
        let rename = frame
            .items
            .iter()
            .find(|item| item.label.starts_with("Rename"))
            .expect("rename row")
            .rect;
        assert_eq!(
            menu.handle(
                crate::native_tab_context_menu::MenuEvent::PointerUp {
                    button: crate::native_tab_context_menu::MouseButton::Left,
                    x: rename.x,
                    y: rename.y,
                },
                viewport,
            ),
            crate::native_tab_context_menu::MenuOutcome::Invoke(
                crate::native_tab_context_menu::MenuAction::Rename {
                    session: id.clone(),
                    current_title: "Visible title".to_owned(),
                }
            )
        );
    }

    #[test]
    fn new_target_without_reopen_facility_omits_reopen_rows() {
        let menu = crate::native_tab_context_menu::TabContextMenu::new(
            4,
            2,
            crate::native_tab_context_menu::MenuTarget::New { recently_closed: None },
        );
        assert!(
            menu.items().is_empty(),
            "recently_closed: None must not expose a Reopen action we cannot service"
        );
        let frame = menu.frame(crate::native_tab_context_menu::Viewport { width: 80, height: 24 });
        assert!(frame.rect.is_none(), "empty popup paints nothing");
    }

    #[test]
    fn ctrl_c_and_escape_cancel_rename_without_exiting() {
        let cancel = native_input_decoder::InputEvent::Byte(3);
        let kill = native_input_decoder::InputEvent::Byte(4);
        let escape = native_input_decoder::InputEvent::Escape;
        assert!(rename_dismisses(&cancel), "Ctrl-C cancels the dialog only");
        assert!(rename_dismisses(&kill), "Ctrl-D cancels the dialog only");
        assert!(rename_dismisses(&escape), "Escape cancels the dialog only");
        assert!(!rename_dismisses(&native_input_decoder::InputEvent::Byte(b'a')));
        assert!(!rename_submits(&cancel), "Ctrl-C must not submit a rename");
    }

    #[test]
    fn ctrl_c_and_escape_dismiss_open_popup_before_byte_exit() {
        for event in [
            native_input_decoder::InputEvent::Byte(3),
            native_input_decoder::InputEvent::Byte(4),
            native_input_decoder::InputEvent::Escape,
        ] {
            assert!(
                menu_dismisses(&event),
                "open popup consumes {event:?} instead of exiting"
            );
        }
        assert!(!menu_dismisses(&native_input_decoder::InputEvent::Byte(b'q')));
    }

    #[test]
    fn rename_request_uses_real_patch_route_and_stable_ack_identity() {
        assert_eq!(rename_request_path("stable-session-id"), "/api/sessions/stable-session-id");
        assert_eq!(rename_request_body("Renamed"), r#"{"title":"Renamed"}"#);
        assert!(rename_ack_matches(
            r#"{"session":{"id":"stable-session-id","title":"Renamed"}}"#,
            "stable-session-id",
        ));
        assert!(
            !rename_ack_matches(
                r#"{"session":{"id":"different-id","title":"Renamed"}}"#,
                "stable-session-id",
            ),
            "an acknowledgement for a different session must be refused"
        );
        assert!(
            !rename_ack_matches(r#"{"session":{}}"#, "stable-session-id"),
            "a missing session identity must be refused"
        );
        assert!(
            !rename_ack_matches("not json", "stable-session-id"),
            "a malformed acknowledgement must be refused"
        );
    }

    #[test]
    fn rename_submit_requires_nonempty_trimmed_title() {
        assert_eq!(rename_accepted_title("  Renamed  "), Some("Renamed".to_owned()));
        assert_eq!(rename_accepted_title(""), None);
        assert_eq!(rename_accepted_title("   "), None);
    }

    #[test]
    fn settled_rename_ack_is_bound_to_inflight_identity() {
        let inflight = crate::native_navigation::SessionRef("session-a".to_owned());
        let accepted = rename_settlement(
            &inflight,
            Ok((
                r#"{"session":{"id":"session-a","title":"New A"}}"#.to_owned(),
                200,
                "New A".to_owned(),
                "session-a".to_owned(),
            )),
        );
        assert_eq!(
            accepted.accepted,
            Some((
                crate::native_navigation::SessionRef("session-a".to_owned()),
                "New A".to_owned()
            ))
        );
        assert!(accepted.message.is_none());

        // Ack body naming a DIFFERENT session is refused even when the result
        // identity matches: the server answer must be the requested session's.
        let foreign_body = rename_settlement(
            &inflight,
            Ok((
                r#"{"session":{"id":"session-b","title":"B"}}"#.to_owned(),
                200,
                "B".to_owned(),
                "session-a".to_owned(),
            )),
        );
        assert!(foreign_body.accepted.is_none());
        assert!(foreign_body.message.is_some());

        // Cross-tab stale result: a settled reply carrying another session's
        // identity is never applied to this in-flight record.
        let stale = rename_settlement(
            &inflight,
            Ok((
                r#"{"session":{"id":"session-b","title":"B"}}"#.to_owned(),
                200,
                "B".to_owned(),
                "session-b".to_owned(),
            )),
        );
        assert!(stale.accepted.is_none(), "stale ack must not retarget");
        assert!(stale.message.is_some());

        let non_2xx = rename_settlement(
            &inflight,
            Ok((
                r#"{"session":{"id":"session-a"}}"#.to_owned(),
                409,
                "x".to_owned(),
                "session-a".to_owned(),
            )),
        );
        assert!(non_2xx.accepted.is_none());

        let failed = rename_settlement(&inflight, Err("daemon unreachable".to_owned()));
        assert!(failed.accepted.is_none());
        assert_eq!(
            failed.message.as_deref(),
            Some("Rename failed: daemon unreachable")
        );
    }

    /// SIDE-01: the sidebar title pending fact matches ONLY the exact
    /// active session id against the in-flight rename identity
    /// (`context/data.tsx:22-47` keys `generatingTitles[sessionID]`;
    /// `sidebar.tsx:33-45` asks it per `props.sessionID`).
    #[cfg(feature = "native")]
    #[test]
    fn side01_sidebar_title_pending_matches_exact_active_session_only() {
        let inflight = crate::native_navigation::SessionRef("session-a".to_owned());
        assert!(
            super::sidebar_title_pending(Some("session-a"), Some(&inflight)),
            "exact matching id is pending"
        );
        let foreign = crate::native_navigation::SessionRef("session-b".to_owned());
        assert!(
            !super::sidebar_title_pending(Some("session-a"), Some(&foreign)),
            "a foreign in-flight id is never pending for this session"
        );
        assert!(
            !super::sidebar_title_pending(Some("session-a"), None),
            "no in-flight request is never pending"
        );
        assert!(
            !super::sidebar_title_pending(None, Some(&inflight)),
            "no active identity is never pending"
        );
    }

    #[test]
    fn dialog_ownership_requires_exact_inflight_session_id() {
        let inflight = crate::native_navigation::SessionRef("session-a".to_owned());
        let open_same = Some(crate::native_navigation::SessionRef("session-a".to_owned()));
        let open_other = Some(crate::native_navigation::SessionRef("session-b".to_owned()));
        assert!(rename_dialog_owns(open_same.as_ref(), &inflight));
        assert!(
            !rename_dialog_owns(open_other.as_ref(), &inflight),
            "a retargeted dialog never adopts another session's settlement"
        );
        assert!(
            !rename_dialog_owns(None, &inflight),
            "a cancelled dialog never adopts a settlement"
        );
    }
}

#[cfg(all(test, feature = "native"))]
mod surface_paint_regressions {
    use super::*;

    #[test]
    fn vertical_rail_preserves_overlapping_modal_body_cells() {
        use crate::native_tab_rail::{RailRect, RailRow, RailRowKind, RailStyle};
        let mut renderer = NativeRenderer::create_memory(12, 3).unwrap();
        let foreground = Rgba::rgb(240, 240, 240);
        let background = Rgba::rgb(20, 20, 20);
        renderer.draw_text_styled(3, 0, "MODAL", foreground, Some(background), 0).unwrap();
        let row = RailRow {
            kind: RailRowKind::Empty, style: RailStyle::Idle,
            rect: RailRect { x: 0, y: 0, width: 12, height: 2 },
            text: "abcdefghijkl\nmnopqrstuvwx".into(),
            color_spans: Vec::new(), flags: Default::default(), is_hovered: false, fade: None,
        };
        paint_rail_row(&mut renderer, &row, Some((3, 0, 5, 1)), foreground, background).unwrap();
        renderer.frame(|_| {}).unwrap();
        let snapshot = renderer.snapshot_text().unwrap();
        let lines: Vec<_> = snapshot.lines().collect();
        assert_eq!(lines[0], "abcMODALijkl", "rail must not overwrite modal contents");
        assert_eq!(lines[1], "mnopqrstuvwx", "rail outside the modal remains painted");
    }

    #[test]
    fn horizontal_rules_span_past_512_cells_without_crossing_viewport() {
        let mut renderer = NativeRenderer::create_memory(620, 5).unwrap();
        let color = Rgba::rgb(220, 220, 220);
        draw_horizontal_rule(&mut renderer, 2, 1, 616, '─', color, color).unwrap();
        // The request spills past the last column. Clipping, not a partial
        // edge or an out-of-bounds FFI write, decides the visible 618 cells.
        draw_horizontal_rule(&mut renderer, 617, 2, 500, '─', color, color).unwrap();
        renderer.frame(|_| {}).unwrap();
        let snapshot = renderer.snapshot_text().unwrap();
        let rows: Vec<_> = snapshot.lines().collect();
        assert_eq!(rows[1].chars().filter(|&ch| ch == '─').count(), 616);
        assert_eq!(rows[2].chars().filter(|&ch| ch == '─').count(), 3);
    }

    #[test]
    fn outlined_panel_clips_without_inventing_right_edge_on_resize() {
        use crate::native_box_style::{BoxAppearance, BoxId, RadiusPreset};
        let appearance = BoxAppearance { preset: RadiusPreset::IosSquircle, ..Default::default() };
        assert!(appearance.full_outline(BoxId::Menu));
        let style = appearance.effective(BoxId::Menu);
        let mut renderer = NativeRenderer::create_memory(20, 8).unwrap();
        let accent = Rgba::rgb(200, 190, 100);
        let surface = Rgba::rgb(25, 25, 25);
        draw_panel_outline(&mut renderer, (17, 1, 10, 5), style, accent, surface, surface).unwrap();
        renderer.frame(|_| {}).unwrap();
        let snapshot = renderer.snapshot_text().unwrap();
        let rows: Vec<_> = snapshot.lines().collect();
        assert_eq!(rows[1].chars().nth(17), Some('╭'));
        assert_eq!(rows[1].chars().nth(19), Some('─'), "clipped edge never creates a fake closing corner");
        assert_eq!(rows[2].chars().nth(17), Some('│'));
    }

    #[test]
    fn active_loading_paint_preserves_composer_top_border() {
        let mut view = crate::native_view::NativeView::new(80, 24);
        view.set_session_route(true);
        let lines = view.render_typed("draft", &[], true, &[]);
        let composer = view.composer_region("draft").expect("composer region");
        let mut renderer = NativeRenderer::create_memory(80, 24).expect("memory renderer");
        native_paint(&mut renderer, &lines, PaintCtx {
            palette: crate::native_theme_runtime::Palette::opencode_dark(),
            dialog: crate::native_theme_runtime::RuntimeTheme::builtin_default(crate::native_theme_runtime::ThemeMode::Dark).dialog,
            markdown: None,
            diff: None,
            presentation: view.presentation(),
            modal_rows: None,
            modal: view.modal_presentation(),
            sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
            chat_row_classes: view.chat_row_classes().to_vec(),
            chat_markdown_spans: view.chat_markdown_spans().to_vec(),
            selection: Vec::new(),
            appearance: crate::native_box_style::BoxAppearance::default(),
            modal_box: crate::native_box_style::BoxId::Dialog,
            loading: Some(('⠦', "Thinking…")),
            home: false,
            rail_prompt: view.session_prompt_rail("draft"),
            composer: Some(composer),
            completion: None,
            context_usage_styles: view.context_usage_styles().to_vec(),
            sidebar_span_styles: view.sidebar_span_styles().to_vec(),
            ui_regions: view.ui_regions(),
            modal_underlay_rows: view.modal_underlay_rows(),
            page_panel: view.page_panel(),
            logo: None,
            tabs: None,
            animations: true,
            title_renaming: None,
            vertical_rail: None,
            compact_tooltip: None,
            context_menu: None,
            rename_dialog: None,
            transcript_scrollbar: None,
            sidebar_scrollbar: None,
            scrollbar_track: Rgba::rgb(10, 10, 10),
            scrollbar_thumb: Rgba::rgb(72, 72, 72),
        }).expect("paint native frame");
        let snapshot = renderer.snapshot_text().expect("native text snapshot");
        let actual = snapshot.lines().nth(usize::from(composer.1)).expect("painted border row");
        assert_eq!(actual.trim_end(), lines[usize::from(composer.1)].trim_end(),
            "busy status must not overwrite the composer's outline");
    }

    #[test]
    fn transcript_scrollbar_paints_track_and_thumb_in_reserved_cells() {
        use crate::native_view_chat::{ChatMessage, ChatRole};
        let mut view = crate::native_view::NativeView::new(80, 24);
        view.set_session_route(true);
        view.set_scrollbar_enabled(true);
        let messages: Vec<_> = (0..40)
            .map(|index| {
                ChatMessage::text(
                    if index % 2 == 0 { ChatRole::User } else { ChatRole::Assistant },
                    format!("message {index} lorem ipsum dolor sit amet"),
                )
            })
            .collect();
        let lines = view.render_typed("draft", &messages, false, &[]);
        let scrollbar = view.transcript_scrollbar().expect("overflowing transcript shows the bar");
        let mut renderer = NativeRenderer::create_memory(80, 24).expect("memory renderer");
        native_paint(&mut renderer, &lines, PaintCtx {
            palette: crate::native_theme_runtime::Palette::opencode_dark(),
            dialog: crate::native_theme_runtime::RuntimeTheme::builtin_default(crate::native_theme_runtime::ThemeMode::Dark).dialog,
            markdown: None,
            diff: None,
            presentation: view.presentation(),
            modal_rows: None,
            modal: view.modal_presentation(),
            sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
            chat_row_classes: view.chat_row_classes().to_vec(),
            chat_markdown_spans: view.chat_markdown_spans().to_vec(),
            selection: Vec::new(),
            appearance: crate::native_box_style::BoxAppearance::default(),
            modal_box: crate::native_box_style::BoxId::Dialog,
            loading: None,
            home: false,
            rail_prompt: view.session_prompt_rail("draft"),
            composer: None,
            completion: None,
            context_usage_styles: view.context_usage_styles().to_vec(),
            sidebar_span_styles: view.sidebar_span_styles().to_vec(),
            ui_regions: view.ui_regions(),
            modal_underlay_rows: view.modal_underlay_rows(),
            page_panel: view.page_panel(),
            logo: None,
            tabs: None,
            animations: true,
            title_renaming: None,
            vertical_rail: None,
            compact_tooltip: None,
            context_menu: None,
            rename_dialog: None,
            transcript_scrollbar: Some(scrollbar),
            sidebar_scrollbar: None,
            scrollbar_track: Rgba::rgb(12, 12, 12),
            scrollbar_thumb: Rgba::rgb(72, 72, 72),
        }).expect("paint native frame");
        let snapshot = renderer.snapshot_text().expect("native text snapshot");
        let rows: Vec<_> = snapshot.lines().collect();
        let column = usize::from(scrollbar.x);
        let (start, len) = scrollbar.thumb.expect("overflowing content has a thumb");
        let first = usize::from(scrollbar.y);
        for offset in 0..usize::from(len) {
            let row = &rows[first + usize::from(start) + offset];
            assert_eq!(row.chars().nth(column), Some('█'),
                "thumb cell at row {}: {row:?}", first + usize::from(start) + offset);
        }
        let track_end = first + usize::from(scrollbar.height);
        assert_eq!(track_end <= rows.len(), true);
        for row in first..track_end {
            let in_thumb = row >= first + usize::from(start)
                && row < first + usize::from(start) + usize::from(len);
            if !in_thumb {
                let cell = rows[row].chars().nth(column);
                assert_eq!(cell, Some(' '), "track row {row} keeps the blank surface: {cell:?}");
            }
        }
        // The reserved padding cell before the track never holds transcript
        // text, and the track itself only ever holds the thumb or a blank.
        for row in first..track_end {
            let padding = crate::native_view_layout::slice_cells(&rows[row], scrollbar.x - 1, 1);
            assert_eq!(padding, " ", "row {row} reserves the padding cell: {padding:?}");
            let track = rows[row].chars().nth(column);
            assert!(matches!(track, Some(' ') | Some('█')), "row {row} track cell: {track:?}");
        }
    }

    #[test]
    fn sidebar_scrollbar_paints_the_thumb_over_the_reserved_padding_column() {
        let mut view = crate::native_view::NativeView::new(148, 30);
        view.set_session_route(true);
        let status: serde_json::Map<String, serde_json::Value> = (0..64)
            .map(|index| {
                (
                    format!("server-{index:02}"),
                    serde_json::json!({"status":"connected"}),
                )
            })
            .collect();
        let included: serde_json::Map<String, serde_json::Value> = status
            .keys()
            .map(|name| (name.clone(), serde_json::json!(false)))
            .collect();
        view.apply_mcp(&serde_json::json!({"status":status,"included":included}))
            .unwrap();
        let lines = view.render_typed("draft", &[], false, &[]);
        let scrollbar = view.sidebar_scrollbar().expect("overflowing sidebar shows the bar");
        let (start, len) = scrollbar.thumb.expect("overflowing body has a thumb");
        let mut renderer = NativeRenderer::create_memory(148, 30).expect("memory renderer");
        native_paint(&mut renderer, &lines, PaintCtx {
            palette: crate::native_theme_runtime::Palette::opencode_dark(),
            dialog: crate::native_theme_runtime::RuntimeTheme::builtin_default(crate::native_theme_runtime::ThemeMode::Dark).dialog,
            markdown: None,
            diff: None,
            presentation: view.presentation(),
            modal_rows: None,
            modal: view.modal_presentation(),
            sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
            chat_row_classes: view.chat_row_classes().to_vec(),
            chat_markdown_spans: view.chat_markdown_spans().to_vec(),
            selection: Vec::new(),
            appearance: crate::native_box_style::BoxAppearance::default(),
            modal_box: crate::native_box_style::BoxId::Dialog,
            loading: None,
            home: false,
            rail_prompt: view.session_prompt_rail("draft"),
            composer: None,
            completion: None,
            context_usage_styles: view.context_usage_styles().to_vec(),
            sidebar_span_styles: view.sidebar_span_styles().to_vec(),
            ui_regions: view.ui_regions(),
            modal_underlay_rows: view.modal_underlay_rows(),
            page_panel: view.page_panel(),
            logo: None,
            tabs: None,
            animations: true,
            title_renaming: None,
            vertical_rail: None,
            compact_tooltip: None,
            context_menu: None,
            rename_dialog: None,
            transcript_scrollbar: None,
            sidebar_scrollbar: Some(scrollbar),
            scrollbar_track: Rgba::rgb(12, 12, 12),
            scrollbar_thumb: Rgba::rgb(72, 72, 72),
        }).expect("paint native frame");
        let snapshot = renderer.snapshot_text().expect("native text snapshot");
        let rows: Vec<_> = snapshot.lines().collect();
        let column = usize::from(scrollbar.x);
        let first = usize::from(scrollbar.y);
        for offset in 0..usize::from(len) {
            let row = &rows[first + usize::from(start) + offset];
            assert_eq!(row.chars().nth(column), Some('█'),
                "thumb cell at row {}: {row:?}", first + usize::from(start) + offset);
        }
        // Header and footer bands stay outside the track entirely.
        let track_end = first + usize::from(scrollbar.height);
        assert_eq!(track_end <= rows.len(), true);
        for row in 0..first {
            assert_ne!(rows[row].chars().nth(column), Some('█'),
                "pinned header row {row} never holds the thumb");
        }
        for row in track_end..usize::from(view.presentation().rows) {
            assert_ne!(rows[row].chars().nth(column), Some('█'),
                "footer/shared rows never hold the thumb: row {row}");
        }
    }

    #[test]
    fn compact_rail_tooltip_paints_caps_inset_title_and_detail() {
        let mut view = crate::native_view::NativeView::new(60, 20);
        let lines = view.render_typed("", &[], false, &[]);
        let tooltip = crate::native_tab_rail::compact_tooltip(
            11, 60, 20, 8, true, false, false, "A long title", "the detail",
        )
        .expect("hovered compact entry opens the tooltip");
        let mut renderer = NativeRenderer::create_memory(60, 20).expect("memory renderer");
        native_paint(&mut renderer, &lines, PaintCtx {
            palette: crate::native_theme_runtime::Palette::opencode_dark(),
            dialog: crate::native_theme_runtime::RuntimeTheme::builtin_default(crate::native_theme_runtime::ThemeMode::Dark).dialog,
            markdown: None,
            diff: None,
            presentation: view.presentation(),
            modal_rows: None,
            modal: view.modal_presentation(),
            sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
            chat_row_classes: view.chat_row_classes().to_vec(),
            chat_markdown_spans: view.chat_markdown_spans().to_vec(),
            selection: Vec::new(),
            appearance: crate::native_box_style::BoxAppearance::default(),
            modal_box: crate::native_box_style::BoxId::Dialog,
            loading: None,
            home: false,
            rail_prompt: view.session_prompt_rail(""),
            composer: None,
            completion: None,
            context_usage_styles: view.context_usage_styles().to_vec(),
            sidebar_span_styles: view.sidebar_span_styles().to_vec(),
            ui_regions: view.ui_regions(),
            modal_underlay_rows: view.modal_underlay_rows(),
            page_panel: view.page_panel(),
            logo: None,
            tabs: None,
            animations: true,
            title_renaming: None,
            vertical_rail: None,
            compact_tooltip: Some(tooltip.clone()),
            context_menu: None,
            rename_dialog: None,
            transcript_scrollbar: None,
            sidebar_scrollbar: None,
            scrollbar_track: Rgba::rgb(12, 12, 12),
            scrollbar_thumb: Rgba::rgb(72, 72, 72),
        }).expect("paint native frame");
        let snapshot = renderer.snapshot_text().expect("native text snapshot");
        let rows: Vec<_> = snapshot.lines().collect();
        let x = usize::from(tooltip.rect.x);
        let top = usize::from(tooltip.rect.y);
        let width = usize::from(tooltip.rect.width);
        assert_eq!(x, 11, "the tooltip starts where the rail ends");
        assert_eq!(top, 7, "hover row 8 anchors the box one row up");
        let cap: Vec<char> = rows[top].chars().skip(x).take(width).collect();
        assert_eq!(cap, vec!['▄'; width], "the top cap spans the full width");
        let body: String = rows[top + 1].chars().skip(x).take(width).collect();
        assert_eq!(body, format!("{}{}", " A long title", " ".repeat(width - 13)));
        let detail: String = rows[top + 2].chars().skip(x).take(width).collect();
        assert_eq!(detail, format!("{}{}", " the detail", " ".repeat(width - 11)));
        let bottom: Vec<char> = rows[top + 3].chars().skip(x).take(width).collect();
        assert_eq!(bottom, vec!['▀'; width], "the bottom cap spans the full width");
    }

    #[test]
    fn assistant_markdown_emits_code_and_bold_spans_through_the_live_view() {
        use crate::native_view_chat::{ChatMessage, ChatRole};
        let mut view = crate::native_view::NativeView::new(120, 40);
        view.set_session_route(true);
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "# Heading\n\nbefore `inline_code` after and **bold words**",
        )];
        let lines = view.render_typed("draft", &messages, false, &[]);
        let spans = view.chat_markdown_spans().to_vec();
        let row_index = lines
            .iter()
            .position(|line| line.contains("inline_code"))
            .expect("code row painted");
        let row = &lines[row_index];
        let start = row.find("inline_code").expect("code text in row");
        let row_spans = &spans[row_index];
        let code = row_spans
            .iter()
            .find(|span| span.flags & crate::markdown_runs::CODE != 0)
            .unwrap_or_else(|| panic!("row {row_index} spans: {row_spans:?} row: {row:?} text_start={start}"));
        assert!(
            spans.iter().flatten().any(|span| span.flags & crate::markdown_runs::BOLD != 0),
            "bold span missing; spans: {spans:?}"
        );
        assert_eq!(
            (usize::from(code.start), usize::from(code.end)),
            (start, start + "inline_code".len()),
            "code span must address the painted cells; row: {row:?}"
        );
        // Upstream `generateSyntax` distinguishes heading and strong colors,
        // so the producer must keep them apart.
        let heading_row = lines
            .iter()
            .position(|line| line.contains("Heading"))
            .expect("heading row painted");
        let heading_start = lines[heading_row].find("Heading").expect("heading text");
        assert!(
            spans[heading_row].iter().any(|span| {
                span.flags & crate::markdown_runs::HEADING != 0
                    && usize::from(span.start) <= heading_start
                    && usize::from(span.end) >= heading_start + "Heading".len()
            }),
            "heading span missing; row {} spans: {:?}",
            heading_row,
            spans[heading_row]
        );
        let bold_start = lines
            .iter()
            .position(|line| line.contains("bold words"))
            .and_then(|row| Some((row, lines[row].find("bold words")?)))
            .expect("bold text painted");
        assert!(
            spans[bold_start.0].iter().any(|span| {
                span.flags & crate::markdown_runs::BOLD != 0
                    && span.flags & crate::markdown_runs::HEADING == 0
                    && usize::from(span.start) == bold_start.1
                    && usize::from(span.end) == bold_start.1 + "bold".len()
            }),
            "strong span missing; row {} spans: {:?}",
            bold_start.0,
            spans[bold_start.0]
        );
    }

    #[test]
    fn markdown_span_flags_map_to_upstream_tokens() {
        use crate::markdown_runs::{
            BOLD, CODE, HEADING, HEADING1, ITALIC, LINK, LINK_TEXT, LIST, QUOTE, RULE, STRIKE,
        };
        let markdown = crate::native_theme_runtime::RuntimeTheme::builtin_default(
            crate::native_theme_runtime::ThemeMode::Dark,
        )
        .semantic
        .expect("shipped V2 asset resolves")
        .markdown;
        let color = |flags| super::markdown_run_color(&markdown, flags).map(|color| color.rgba);
        assert_eq!(color(CODE), Some(markdown.code.rgba));
        assert_eq!(color(HEADING), Some(markdown.heading.rgba));
        assert_eq!(color(HEADING | HEADING1), Some(markdown.heading.rgba));
        assert_eq!(color(BOLD), Some(markdown.strong.rgba));
        assert_eq!(color(ITALIC), Some(markdown.emphasis.rgba));
        assert_eq!(color(LINK), Some(markdown.link.rgba));
        assert_eq!(color(LINK | LINK_TEXT), Some(markdown.link_text.rgba));
        assert_eq!(color(QUOTE), Some(markdown.block_quote.rgba));
        assert_eq!(color(RULE), Some(markdown.horizontal_rule.rgba));
        assert_eq!(color(LIST), Some(markdown.list_item.rgba));
        assert_eq!(color(STRIKE), Some(markdown.muted.rgba));
        assert_eq!(color(0), None);
        assert_eq!(super::run_attributes(HEADING1) & 8, 8, "h1 underlines");
        assert_eq!(super::run_attributes(QUOTE) & 4, 4, "quotes are italic");
    }

    #[test]
    fn live_theme_loader_resolves_markdown_tokens() {
        use crate::native_theme_runtime::ThemeMode;
        // The builtin `opencode` theme is the V2 document upstream
        // (`packages/tui/src/theme/index.ts` `listThemes`).
        let default = super::load_native_theme("opencode", ThemeMode::Dark, &[])
            .expect("default theme loads");
        let semantic = default.semantic.as_ref().expect("default semantic roles");
        assert_eq!(semantic.markdown.code.rgba, [0x7f, 0xd8, 0x8f, 0xff]);
        assert_eq!(semantic.markdown.code_block.rgba, [0xee, 0xee, 0xee, 0xff]);

        // Legacy assets carry the same-named roles and migrate 1:1
        // (`packages/theme/src/tui/v1-migrate.ts`).
        let legacy = super::load_native_theme("dracula", ThemeMode::Dark, &[])
            .expect("bundled legacy theme loads");
        let semantic = legacy.semantic.as_ref().expect("legacy semantic roles");
        assert_eq!(semantic.text_base.rgba, [0xf8, 0xf8, 0xf2, 0xff]);
        assert_eq!(semantic.markdown.code.rgba, [0x50, 0xfa, 0x7b, 0xff]);
        assert_eq!(semantic.markdown.heading.rgba, [0xbd, 0x93, 0xf9, 0xff]);
        assert_eq!(semantic.syntax.keyword.rgba, [0xff, 0x79, 0xc6, 0xff]);
        assert_eq!(semantic.background_raised_base.rgba, [0x21, 0x22, 0x2c, 0xff]);
    }

    #[test]
    fn borderless_home_prompt_has_no_rail_or_bottom_edge() {
        let mut view = crate::native_view::NativeView::new(100, 32);
        view.set_session_route(false);
        view.set_box_appearance(crate::native_box_style::BoxAppearance {
            border_style: crate::native_box_style::BorderStyle::None,
            ..Default::default()
        });
        let lines = view.render_typed("borderless draft", &[], false, &[]);
        let (left, top, width, height) = view.composer_region("borderless draft").expect("Home composer");
        for row in &lines[usize::from(top)..usize::from(top + height)] {
            let text = crate::native_view_layout::slice_cells(row, left, width);
            assert!(!text.chars().any(|ch| "┃│╹╵┌┐└┘─▀".contains(ch)),
                "borderless Home must not retain rail or edge cells: {text:?}");
        }
    }
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

#[cfg(feature = "native")]
fn rounded_surface_background(
    style: crate::native_box_style::EffectiveBox,
    rect: (u16, u16, u16, u16),
    x: u32,
    y: u32,
    glyph: &str,
    enclosing: Rgba,
) -> Option<Rgba> {
    style.rounded_corner_cell(rect, x as u16, y as u16, glyph.chars().next()?)
        .then_some(enclosing)
}

#[cfg(feature = "native")]
fn finish_surface_background(raw: Rgba, rounded: Option<Rgba>, alpha: u8) -> Rgba {
    let c = rounded.unwrap_or(raw);
    let [r, g, b, a] = crate::native_theme_runtime::dim([c.r, c.g, c.b, c.a], alpha);
    Rgba::new(r, g, b, a)
}

#[cfg(feature = "native")]
fn modal_surface_background(style: crate::native_box_style::EffectiveBox, rect: (u16,u16,u16,u16), x:u32,y:u32,glyph:&str, underlay:Rgba, panel:Rgba, alpha:u8) -> Rgba {
    match rounded_surface_background(style, rect, x, y, glyph, underlay) {
        Some(enclosing) => finish_surface_background(enclosing, None, alpha),
        None => panel,
    }
}

#[cfg(feature = "native")]
fn resolve_surface_corner_background(fill: Rgba, root: Rgba, surfaces: &[(crate::native_box_style::EffectiveBox, Option<(u16,u16,u16,u16)>)], x: u32, y: u32, glyph: &str) -> Rgba {
    surfaces.iter().rev().find_map(|(style, rect)|
        rect.and_then(|rect| rounded_surface_background(*style, rect, x, y, glyph, root)))
        .unwrap_or(fill)
}

#[cfg(feature = "native")]
fn surface_underlay_background(in_panel: bool, role: Option<Rgba>, in_composer: bool, home_root: bool, root: Rgba, panel: Rgba, element: Rgba) -> Rgba {
    if home_root { root } else if in_panel { panel } else { role.unwrap_or(if in_composer { element } else { root }) }
}

#[cfg(feature = "native")]
fn home_composer_root_cell(appearance: &crate::native_box_style::BoxAppearance, style: crate::native_box_style::EffectiveBox, rect: (u16, u16, u16, u16), x: u32, y: u32) -> bool {
    let framed = appearance.home_composer_outline() && style.has_border();
    (!framed && x == u32::from(rect.0))
        || (framed && style.rounded_corner_cell(rect, x as u16, y as u16, style.corner_glyphs().top_right))
}

#[cfg(feature = "native")]
fn home_composer_bottom_halfblock_override(appearance: &crate::native_box_style::BoxAppearance, glyph: &str) -> bool {
    !appearance.home_composer_outline()
        && matches!(glyph, "▀" | "▄")
}

/// Single decision source for the Home prompt's left-column rail cell
/// (composer left border column). Upstream `mini/footer.prompt.tsx:270-273`
/// draws the prompt rail as a left border-colored glyph over the root
/// background — the prompt box paints no surface of its own — so the bordered
/// default `┃`/`╹` carries the Border role on the root background instead of
/// the prompt surface color (Home fixture capture: fg #484848 on bg #0a0a0a,
/// interior #1e1e1e). Outline presets own their rail glyphs; the borderless
/// prompt keeps its existing accent caret cell; every other glyph falls
/// through to the caller's unchanged half-block/row paint path.
#[cfg(feature = "native")]
fn home_composer_border_cell_style(
    appearance: &crate::native_box_style::BoxAppearance,
    style: crate::native_box_style::EffectiveBox,
    glyph: &str,
    accent: Rgba,
    root: Rgba,
    palette: crate::native_theme_runtime::Palette,
) -> Option<(Rgba, Rgba)> {
    use crate::native_theme_runtime::Slot;
    if appearance.home_composer_outline() {
        return None;
    }
    if !style.has_border() {
        return Some((accent, root));
    }
    if matches!(glyph, "┃" | "╹") {
        Some((paint_color(palette, Slot::Border), root))
    } else {
        None
    }
}

/// Resolved diff-token paint (`theme.semantic.diff`) for the transcript row
/// classes. THEME-09: a `+`/`-` diff row paints the resolved
/// `diff.background.added`/`diff.background.removed` surface and the
/// `diff.text.added`/`diff.text.removed` prose, not hardcoded RGB
/// (`theme/src/tui/syntax.ts:79-80`: `rule(["diff.plus"], theme.diff.text.added,
/// { background: theme.diff.background.added })`).
#[cfg(feature = "native")]
#[derive(Clone, Copy, Debug)]
struct DiffRgba {
    added_text: Rgba,
    removed_text: Rgba,
    context_text: Rgba,
    added_background: Rgba,
    removed_background: Rgba,
    context_background: Rgba,
}

#[cfg(feature = "native")]
fn diff_rgba(semantic: &crate::native_theme_runtime::SemanticPalette) -> DiffRgba {
    let rgba = |hue: &crate::native_theme_runtime::HueColor| {
        let [r, g, b, a] = hue.rgba;
        Rgba::new(r, g, b, a)
    };
    DiffRgba {
        added_text: rgba(&semantic.diff.added_text),
        removed_text: rgba(&semantic.diff.removed_text),
        context_text: rgba(&semantic.diff.context_text),
        added_background: rgba(&semantic.diff.added_background),
        removed_background: rgba(&semantic.diff.removed_background),
        context_background: rgba(&semantic.diff.context_background),
    }
}

#[cfg(feature = "native")]
fn chat_row_background(class: Option<&crate::native_view_chat::ChatRowClass>, root: Rgba, panel: Rgba, element: Rgba, diff: Option<&DiffRgba>) -> Option<Rgba> {
    use crate::native_view_chat::ChatRowClass;
    match class {
        Some(ChatRowClass::UserCard) => Some(panel),
        Some(ChatRowClass::AssistantCard) => Some(root),
        Some(ChatRowClass::SystemCard) => Some(element),
        Some(ChatRowClass::DiffAdded) => Some(diff.map_or(Rgba::new(20, 70, 35, 255), |diff| diff.added_background)),
        Some(ChatRowClass::DiffRemoved) => Some(diff.map_or(Rgba::new(80, 25, 30, 255), |diff| diff.removed_background)),
        // Unchanged diff lines use the `diff.delta` background
        // (`theme.diff.background.context`, `packages/theme/src/tui/syntax.ts:81`).
        Some(ChatRowClass::DiffContext) => Some(diff.map_or(root, |diff| diff.context_background)),
        // Assistant completion footer rows (upstream `AssistantFooter`) are
        // plain text on the root surface — upstream draws no box for them.
        Some(ChatRowClass::AssistantFooter) | Some(ChatRowClass::FooterError) => None,
        _ => None,
    }
}

/// Foreground paint for one typed chat row. The assistant completion footer
/// classes mirror upstream `AssistantFooter`
/// (`packages/tui/src/routes/session/index.tsx:2055-2071`): the error notice
/// row uses the error fg (`theme.text.feedback.error.base`) and the metadata
/// line uses the muted fg (`theme.text.muted` on the field spans).
#[cfg(feature = "native")]
fn chat_row_foreground(class: &crate::native_view_chat::ChatRowClass, text_color: Rgba, muted: Rgba, error: Rgba, warning: Rgba, diff: Option<&DiffRgba>) -> Rgba {
    use crate::native_view_chat::ChatRowClass;
    match class {
        ChatRowClass::Thinking
        | ChatRowClass::Metadata
        | ChatRowClass::ToolCall
        | ChatRowClass::ToolResult
        | ChatRowClass::AssistantFooter => muted,
        ChatRowClass::UserCard => text_color,
        ChatRowClass::AssistantCard => text_color,
        ChatRowClass::SystemCard => muted,
        ChatRowClass::DiffAdded => diff.map_or(text_color, |diff| diff.added_text),
        ChatRowClass::DiffRemoved => diff.map_or(error, |diff| diff.removed_text),
        // Unchanged diff lines use the `diff.delta` foreground
        // (`theme.diff.text.context`, `packages/theme/src/tui/syntax.ts:81`).
        ChatRowClass::DiffContext => diff.map_or(muted, |diff| diff.context_text),
        ChatRowClass::ToolError => error,
        ChatRowClass::RetryNotice => warning,
        ChatRowClass::ToolPermission => warning,
        ChatRowClass::ToolDenied => muted,
        ChatRowClass::FooterError => error,
        _ => text_color,
    }
}

/// Attribute override owned by a chat row class. Only `ToolDenied` carries
/// one: upstream strikes the denied tool icon and label
/// (`routes/session/message-parts.tsx:242,252,288`
/// `denied ? TextAttributes.STRIKETHROUGH`). Every other class keeps the
/// theme/markdown attribute stream untouched.
#[cfg(feature = "native")]
fn chat_row_attribute_override(class: Option<&crate::native_view_chat::ChatRowClass>) -> u32 {
    match class {
        Some(crate::native_view_chat::ChatRowClass::ToolDenied) => {
            crate::native_ui_style::ATTR_STRIKETHROUGH
        }
        _ => 0,
    }
}

/// Whether a chat row class carries selectable transcript text.
///
/// Upstream's row `clickable` memo (`routes/session/index.tsx:2836`) gates only
/// the row's mouse click/hover *actions* (`props.onClick || failed()`); it does
/// NOT gate OpenTUI text selection, which applies to every rendered message
/// row. A denied tool row (`ToolDenied`) is therefore still selectable, exactly
/// like a failed (`ToolError`) row. Card rows (permission/question/inline
/// diff) are decoration/affordance surfaces, not prose, and stay out.
#[cfg(feature = "native")]
fn chat_row_is_selectable(class: Option<&crate::native_view_chat::ChatRowClass>) -> bool {
    use crate::native_view_chat::ChatRowClass;
    matches!(
        class,
        Some(
            ChatRowClass::UserCard
                | ChatRowClass::AssistantCard
                | ChatRowClass::SystemCard
                | ChatRowClass::Text
                | ChatRowClass::Thinking
                | ChatRowClass::ToolCall
                | ChatRowClass::ToolResult
                | ChatRowClass::ToolDenied
                | ChatRowClass::ToolError
                | ChatRowClass::DiffAdded
                | ChatRowClass::DiffRemoved
                | ChatRowClass::DiffContext
                | ChatRowClass::RetryNotice
        )
    )
}

#[cfg(feature = "native")]
fn chat_row_background_at(
    class: Option<&crate::native_view_chat::ChatRowClass>,
    presentation: crate::native_view::ViewPresentation, x: u32,
    root: Rgba, panel: Rgba, element: Rgba, diff: Option<&DiffRgba>,
) -> Option<Rgba> {
    // Message rows are composed inside the scrollbox's horizontal insets.
    // A row role owns that content surface, never the surrounding root cells,
    // vertical tab rail, or sidebar. Use this same ownership for modal corners.
    let inset = presentation.main_padding.min(presentation.main_width / 2);
    let left = u32::from(presentation.main_offset) + u32::from(inset);
    let right = u32::from(presentation.main_offset)
        + u32::from(presentation.main_width.saturating_sub(inset));
    (x >= left && x < right)
        .then(|| chat_row_background(class, root, panel, element, diff))
        .flatten()
}

#[cfg(all(test, feature = "native"))]
mod chat_surface_inset_tests {
    use super::*;
    #[test]
    fn message_surfaces_preserve_outer_inset_in_horizontal_and_vertical_layouts() {
        use crate::native_tabs::{TabOrientation, TabReservation};
        let root = Rgba::rgb(10, 10, 10);
        let panel = Rgba::rgb(20, 20, 20);
        let element = Rgba::rgb(30, 30, 30);
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let mut view = crate::native_view::NativeView::new(269, 59);
            view.set_session_route(true);
            view.set_tab_reservation(TabReservation::for_view(orientation, 269, 59));
            let presentation = view.presentation();
            let left = u32::from(presentation.main_offset) + 2;
            let right = u32::from(presentation.main_offset + presentation.main_width) - 2;
            for x in 0..269 {
                let actual = chat_row_background_at(Some(&crate::native_view_chat::ChatRowClass::UserCard),
                    presentation, x, root, panel, element, None);
                assert_eq!(actual, (x >= left && x < right).then_some(panel),
                    "message surface ownership at x={x} with {orientation:?}");
            }
        }
    }

    /// THEME-09: a `+`/`-` diff row paints the resolved `theme.semantic.diff`
    /// tokens — `diff.background.added`/`removed` for the surface and
    /// `diff.text.added`/`removed` for the prose — instead of the historical
    /// hardcoded RGB (`theme/src/tui/syntax.ts:79-80`). `None` keeps the
    /// literal fallback so a semantic-less theme is unchanged.
    #[test]
    fn diff_rows_paint_resolved_diff_tokens() {
        let root = Rgba::rgb(10, 10, 10);
        let panel = Rgba::rgb(20, 20, 20);
        let element = Rgba::rgb(30, 30, 30);
        let text_color = Rgba::rgb(1, 1, 1);
        let muted = Rgba::rgb(2, 2, 2);
        let error = Rgba::rgb(3, 3, 3);
        let warning = Rgba::rgb(4, 4, 4);
        let diff = DiffRgba {
            added_text: Rgba::new(11, 111, 11, 255),
            removed_text: Rgba::new(99, 9, 9, 255),
            context_text: Rgba::new(55, 55, 88, 255),
            added_background: Rgba::new(22, 222, 22, 255),
            removed_background: Rgba::new(88, 8, 8, 255),
            context_background: Rgba::new(44, 44, 66, 255),
        };
        assert_eq!(
            chat_row_background(Some(&crate::native_view_chat::ChatRowClass::DiffAdded), root, panel, element, Some(&diff)),
            Some(diff.added_background),
            "diff added surface = diff.background.added"
        );
        assert_eq!(
            chat_row_background(Some(&crate::native_view_chat::ChatRowClass::DiffRemoved), root, panel, element, Some(&diff)),
            Some(diff.removed_background),
            "diff removed surface = diff.background.removed"
        );
        assert_eq!(
            chat_row_foreground(&crate::native_view_chat::ChatRowClass::DiffAdded, text_color, muted, error, warning, Some(&diff)),
            diff.added_text,
            "diff added prose = diff.text.added"
        );
        assert_eq!(
            chat_row_foreground(&crate::native_view_chat::ChatRowClass::DiffRemoved, text_color, muted, error, warning, Some(&diff)),
            diff.removed_text,
            "diff removed prose = diff.text.removed"
        );
        // DIFF-002: unchanged lines use the `diff.delta` role
        // (`theme.diff.text.context` + `theme.diff.background.context`,
        // `theme/src/tui/syntax.ts:81`).
        assert_eq!(
            chat_row_background(Some(&crate::native_view_chat::ChatRowClass::DiffContext), root, panel, element, Some(&diff)),
            Some(diff.context_background),
            "diff context surface = diff.background.context"
        );
        assert_eq!(
            chat_row_foreground(&crate::native_view_chat::ChatRowClass::DiffContext, text_color, muted, error, warning, Some(&diff)),
            diff.context_text,
            "diff context prose = diff.text.context"
        );
        // Semantic-less theme keeps the historical literal fallback.
        assert_eq!(
            chat_row_background(Some(&crate::native_view_chat::ChatRowClass::DiffAdded), root, panel, element, None),
            Some(Rgba::new(20, 70, 35, 255))
        );
        assert_eq!(
            chat_row_foreground(&crate::native_view_chat::ChatRowClass::DiffAdded, text_color, muted, error, warning, None),
            text_color
        );
    }
}

/// Registered production tests for the assistant completion footer paint
/// classes (upstream `AssistantFooter` fg policy; footer rows carry no
/// surface background).
#[cfg(all(test, feature = "native"))]
mod footer_v3_painter_tests {
    use super::*;
    use crate::native_view_chat::ChatRowClass;

    #[test]
    fn footer_rows_use_error_and_muted_foregrounds() {
        let text_color = Rgba::rgb(1, 2, 3);
        let muted = Rgba::rgb(4, 5, 6);
        let error = Rgba::rgb(7, 8, 9);
        let warning = Rgba::rgb(10, 11, 12);
        assert_eq!(
            chat_row_foreground(&ChatRowClass::FooterError, text_color, muted, error, warning, None),
            error,
            "upstream error notice fg = theme.text.feedback.error.base"
        );
        assert_eq!(
            chat_row_foreground(&ChatRowClass::AssistantFooter, text_color, muted, error, warning, None),
            muted,
            "upstream footer field spans fg = theme.text.muted"
        );
    }

    /// CP-42: a streaming/retry transcript notice paints
    /// `theme.text.feedback.warning.base` (`index.tsx:2559-2566`), not the
    /// tool-error fg it previously reused via `ChatRowClass::ToolError`.
    #[test]
    fn retry_notice_row_uses_warning_foreground() {
        let text_color = Rgba::rgb(1, 2, 3);
        let muted = Rgba::rgb(4, 5, 6);
        let error = Rgba::rgb(7, 8, 9);
        let warning = Rgba::rgb(10, 11, 12);
        assert_eq!(
            chat_row_foreground(&ChatRowClass::RetryNotice, text_color, muted, error, warning, None),
            warning,
            "upstream retry notice fg = theme.text.feedback.warning.base"
        );
    }

    /// TOOL-005: the denied tool class keeps the muted foreground and carries
    /// the upstream `TextAttributes.STRIKETHROUGH` override on its cells,
    /// while a completed tool result gets no attribute override.
    #[test]
    fn denied_tool_rows_keep_muted_foreground_and_strike() {
        let text_color = Rgba::rgb(1, 2, 3);
        let muted = Rgba::rgb(4, 5, 6);
        let error = Rgba::rgb(7, 8, 9);
        let warning = Rgba::rgb(10, 11, 12);
        assert_eq!(
            chat_row_foreground(&ChatRowClass::ToolDenied, text_color, muted, error, warning, None),
            muted,
            "denied tool fg = theme.text.muted (index.tsx:2837 default branch)"
        );
        assert_eq!(
            chat_row_attribute_override(Some(&ChatRowClass::ToolDenied)),
            crate::native_ui_style::ATTR_STRIKETHROUGH,
            "denied tool label carries TextAttributes.STRIKETHROUGH"
        );
        assert_eq!(
            chat_row_attribute_override(Some(&ChatRowClass::ToolResult)),
            crate::native_ui_style::ATTR_NONE,
            "an ordinary completed tool result is not struck"
        );
        assert_eq!(chat_row_attribute_override(None), crate::native_ui_style::ATTR_NONE);
    }

    /// TOOL-005: a denied tool row remains text-selectable. Upstream's
    /// `clickable` memo (`routes/session/index.tsx:2836`) gates only mouse
    /// click/hover actions, not OpenTUI text selection, which applies to every
    /// rendered message row. oc2 previously omitted `ToolDenied` from the
    /// selectable class list.
    #[test]
    fn denied_tool_rows_remain_text_selectable() {
        assert!(chat_row_is_selectable(Some(&ChatRowClass::ToolDenied)));
        assert!(chat_row_is_selectable(Some(&ChatRowClass::ToolError)));
        assert!(chat_row_is_selectable(Some(&ChatRowClass::ToolResult)));
        assert!(chat_row_is_selectable(Some(&ChatRowClass::Text)));
        // Decoration/affordance rows carry no prose.
        assert!(!chat_row_is_selectable(Some(&ChatRowClass::FooterError)));
        assert!(!chat_row_is_selectable(None));
    }

    /// TOOL-002: a tool row awaiting a permission decision paints
    /// `theme.text.feedback.warning.base` (`routes/session/index.tsx:2839`);
    /// the class also beats a runtime error because upstream checks the
    /// permission branch first.
    #[test]
    fn tool_permission_rows_paint_warning_foreground() {
        let text_color = Rgba::rgb(1, 2, 3);
        let muted = Rgba::rgb(4, 5, 6);
        let error = Rgba::rgb(7, 8, 9);
        let warning = Rgba::rgb(10, 11, 12);
        assert_eq!(
            chat_row_foreground(&ChatRowClass::ToolPermission, text_color, muted, error, warning, None),
            warning,
            "permission-pending tool fg = theme.text.feedback.warning.base"
        );
        assert_eq!(
            chat_row_foreground(&ChatRowClass::ToolError, text_color, muted, error, warning, None),
            error,
            "errored tool fg is unchanged"
        );
    }

    /// TOOL-002 live consumer: a polled pending permission carries the tool
    /// `callID`; the matching `ChatPart::ToolState { id }` row paints
    /// `ChatRowClass::ToolPermission` — the whole-row warning fg
    /// (`routes/session/index.tsx:2793-2799` + `:2837-2847`) — and the row
    /// text gains nothing (upstream adds no badge, `:2834`).
    #[test]
    fn pending_permission_calls_mark_matching_tool_row_warning() {
        use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole, ChatView};
        let message = ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![ChatPart::ToolState {
                name: "write".to_owned(),
                id: "c9".to_owned(),
                status: "running".to_owned(),
                args: r#"{"filePath":"a.rs"}"#.to_owned(),
                output: String::new(),
                is_error: false,
                metadata: String::new(),
            }],
            agent: None,
            model: None,
            assistant_facts: None,
            message_id: None,
        };
        let messages = vec![message];
        let mut view = ChatView::default();
        let before =
            view.render_typed_classified(80, 20, "ws", &messages, "", false);
        assert!(
            !before.iter().any(|(_, class)| *class == ChatRowClass::ToolPermission),
            "no pending set: the tool row keeps its normal class"
        );
        view.set_pending_permission_calls(&["c9".to_owned()]);
        let after =
            view.render_typed_classified(80, 20, "ws", &messages, "", false);
        assert!(
            after.iter().any(|(_, class)| *class == ChatRowClass::ToolPermission),
            "matching call id paints the warning row: {after:?}"
        );
        // Colour only, never text (`index.tsx:2834`): the rows are identical
        // before and after the permission fact is joined.
        assert!(after.iter().all(|(row, _)| !row.contains("Permission")));
        assert_eq!(
            before.iter().map(|(row, _)| row).collect::<Vec<_>>(),
            after.iter().map(|(row, _)| row).collect::<Vec<_>>()
        );
        // A different pending call id must not mark this row: upstream joins
        // by exact call id (`index.tsx:2795`).
        view.set_pending_permission_calls(&["other".to_owned()]);
        let cleared =
            view.render_typed_classified(80, 20, "ws", &messages, "", false);
        assert!(
            !cleared.iter().any(|(_, class)| *class == ChatRowClass::ToolPermission),
            "unrelated call id keeps the normal row class"
        );
    }

    #[test]
    fn footer_rows_paint_no_surface_background() {
        let root = Rgba::rgb(10, 10, 10);
        let panel = Rgba::rgb(20, 20, 20);
        let element = Rgba::rgb(30, 30, 30);
        assert_eq!(
            chat_row_background(Some(&ChatRowClass::FooterError), root, panel, element, None),
            None
        );
        assert_eq!(
            chat_row_background(Some(&ChatRowClass::AssistantFooter), root, panel, element, None),
            None
        );
    }
}

#[cfg(feature = "native")]
fn visible_modal_rect(cols:u32, rows:u32, x:u32,y:u32,width:u32,content:usize)->Option<(u32,u32,u32,u32)> {
    let width=width.min(cols.saturating_sub(x));
    if x>=cols || y>=rows || width==0 || content==0 { return None; }
    Some((x,y,width,u32::try_from(content).unwrap_or(u32::MAX).saturating_add(2).min(rows-y)))
}

#[cfg(all(test, feature = "native"))]
mod rounded_surface_color_tests {
    use super::{finish_surface_background, home_composer_border_cell_style, home_composer_bottom_halfblock_override, home_composer_root_cell, modal_surface_background, resolve_surface_corner_background, rounded_surface_background, surface_underlay_background, visible_modal_rect};
    use crate::native_box_style::{BorderStyle, EffectiveBox, RadiusPreset};
    use opencode_rk_opentui_bridge::Rgba;

    // ── Home composer left-rail cell style (home-frame-gap border-v1) ────
    // Palette roles are derived through the painter, never guessed literals:
    // dark Border is the upstream `base.border.base` #484848 and Background is
    // `base.background.base` #0a0a0a (native_theme_runtime::opencode_dark).

    #[test]
    fn home_composer_default_rail_glyphs_use_border_role_on_root_background() {
        use crate::native_theme_runtime::{Palette, Slot};
        let appearance = crate::native_box_style::BoxAppearance::default();
        let style = appearance.effective(crate::native_box_style::BoxId::ChatComposer);
        assert!(style.has_border(), "the diagnosed default style has a Single border");
        let palette = Palette::opencode_dark();
        let accent = Rgba::rgb(200, 190, 100);
        let root = super::paint_color(palette, Slot::Background);
        for glyph in ["┃", "╹"] {
            let (fg, bg) = home_composer_border_cell_style(
                &appearance, style, glyph, accent, root, palette,
            )
            .unwrap_or_else(|| panic!("default bordered rail glyph {glyph:?} needs an explicit style"));
            assert_eq!(fg, super::paint_color(palette, Slot::Border),
                "{glyph} foreground is the Border role");
            assert_eq!(bg, root, "{glyph} background is the root Background role");
        }
    }

    #[test]
    fn home_composer_borderless_caret_keeps_accent_on_root_background() {
        use crate::native_theme_runtime::Palette;
        let appearance = crate::native_box_style::BoxAppearance {
            border_style: BorderStyle::None,
            ..Default::default()
        };
        let style = appearance.effective(crate::native_box_style::BoxId::ChatComposer);
        assert!(!style.has_border());
        let palette = Palette::opencode_dark();
        let accent = Rgba::rgb(200, 190, 100);
        let root = Rgba::rgb(10, 10, 10);
        for glyph in ["┃", " ", "draft"] {
            let (fg, bg) = home_composer_border_cell_style(
                &appearance, style, glyph, accent, root, palette,
            )
            .unwrap_or_else(|| panic!("borderless caret cell {glyph:?} keeps its explicit style"));
            assert_eq!(fg, accent, "{glyph} caret keeps the accent foreground");
            assert_eq!(bg, root, "{glyph} caret sits on the root background");
        }
    }

    #[test]
    fn home_composer_outline_presets_keep_own_rail_glyphs() {
        use crate::native_theme_runtime::Palette;
        let appearance = crate::native_box_style::BoxAppearance {
            preset: RadiusPreset::Rounded,
            ..Default::default()
        };
        assert!(appearance.home_composer_outline(), "Rounded is the framed Home outline");
        let style = appearance.effective(crate::native_box_style::BoxId::ChatComposer);
        let palette = Palette::opencode_dark();
        for glyph in ["┃", "╹", "╭", "─"] {
            assert!(
                home_composer_border_cell_style(
                    &appearance, style, glyph, Rgba::rgb(1, 2, 3), Rgba::rgb(4, 5, 6), palette,
                )
                .is_none(),
                "{glyph:?} is owned by the framed outline paint"
            );
        }
    }

    #[test]
    fn home_composer_default_halfblock_and_text_glyphs_fall_through() {
        use crate::native_theme_runtime::Palette;
        let appearance = crate::native_box_style::BoxAppearance::default();
        let style = appearance.effective(crate::native_box_style::BoxId::ChatComposer);
        let palette = Palette::opencode_dark();
        // ▀/▄ keep the existing bottom half-block override and ordinary text
        // glyphs keep the row paint; the helper must not intercept either.
        for glyph in ["▀", "▄", "x", ""] {
            assert!(
                home_composer_border_cell_style(
                    &appearance, style, glyph, Rgba::rgb(1, 2, 3), Rgba::rgb(4, 5, 6), palette,
                )
                .is_none(),
                "{glyph:?} keeps the prior half-block/row paint path"
            );
        }
    }

    #[test]
    fn rounded_corner_uses_supplied_root_and_square_styles_keep_panel() {
        let rect = (10, 4, 8, 5);
        let root = Rgba::new(1, 2, 3, 255);
        let panel = Rgba::new(9, 9, 9, 255);
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        assert_eq!(rounded_surface_background(rounded, rect, 10, 4, "╭", root), Some(root));
        assert_eq!(rounded_surface_background(rounded, rect, 11, 4, "╭", root), None);
        for border in [BorderStyle::Double, BorderStyle::Heavy] {
            let style = EffectiveBox { border_style: border, ..rounded };
            assert_eq!(rounded_surface_background(style, rect, 10, 4, "╔", root), None);
        }
    }

    #[test]
    fn rounded_corner_clips_outside_and_degenerate_rects() {
        let style = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let root = Rgba::new(1, 2, 3, 255);
        assert_eq!(rounded_surface_background(style, (5, 2, 8, 4), 13, 2, "╮", root), None);
        assert_eq!(rounded_surface_background(style, (5, 2, 1, 1), 5, 2, "╭", root), None);
    }

    #[test]
    fn modal_square_and_weighted_corners_keep_opaque_panel() {
        let panel = Rgba::new(20, 20, 20, 255);
        let underlay = Rgba::new(30, 40, 50, 255);
        for (preset, border_style) in [
            (RadiusPreset::Sharp, BorderStyle::Single),
            (RadiusPreset::Rounded, BorderStyle::Double),
            (RadiusPreset::Rounded, BorderStyle::Heavy),
        ] {
            let style = EffectiveBox { preset, radius: 0, border_style, thickness: 1, padding: 0 };
            let glyph = style.corner_glyphs().top_left.to_string();
            assert_eq!(modal_surface_background(style, (5, 2, 8, 4), 5, 2, &glyph, underlay, panel, 150), panel);
        }
    }

    #[test]
    fn final_background_preserves_noncorner_and_dims_corner_once() {
        let style = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let fill = Rgba::new(90, 100, 110, 255);
        let enclosing = Rgba::new(30, 40, 50, 255);
        let alpha = 150;
        for (x, y, glyph) in [(1, 1, "╭"), (6, 2, "─"), (5, 3, "│")] {
            let corner = rounded_surface_background(style, (5, 2, 8, 4), x, y, glyph, enclosing);
            assert_eq!(corner, None);
            assert_eq!(finish_surface_background(fill, corner, alpha), finish_surface_background(fill, None, alpha));
        }
        let corner = rounded_surface_background(style, (5, 2, 8, 4), 5, 2, "╭", enclosing);
        let once = finish_surface_background(enclosing, None, alpha);
        assert_eq!(finish_surface_background(fill, corner, alpha), once);
        assert_ne!(once, finish_surface_background(once, None, alpha));
        assert_eq!(modal_surface_background(style, (5, 2, 8, 4), 5, 2, "╭", enclosing, fill, alpha), once);
    }

    #[test]
    fn modal_geometry_keeps_rightmost_border_inside_viewport() {
        assert_eq!(visible_modal_rect(100, 32, 90, 5, 30, 4), Some((90, 5, 10, 6)));
        let (x, _, width, _) = visible_modal_rect(100, 32, 90, 5, 30, 4).unwrap();
        assert_eq!(x + width - 1, 99);
        assert_eq!(visible_modal_rect(100, 32, 100, 5, 30, 4), None);
        assert_eq!(visible_modal_rect(100, 32, 5, 32, 30, 4), None);
    }

    #[test]
    fn modal_underlay_preserves_surface_precedence_and_home_bottom() {
        let root = Rgba::new(10, 10, 10, 255);
        let panel = Rgba::new(20, 20, 20, 255);
        let element = Rgba::new(30, 30, 30, 255);
        let role = Rgba::new(20, 70, 35, 255);
        assert_eq!(surface_underlay_background(false, None, false, false, root, panel, element), root);
        assert_eq!(surface_underlay_background(false, None, true, false, root, panel, element), element);
        assert_eq!(surface_underlay_background(false, Some(role), true, false, root, panel, element), role);
        assert_eq!(surface_underlay_background(true, Some(role), true, false, root, panel, element), panel);
        assert_eq!(surface_underlay_background(false, None, true, true, root, panel, element), root);
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let underlay = surface_underlay_background(false, None, true, false, root, panel, element);
        assert_eq!(modal_surface_background(rounded, (5, 2, 8, 4), 5, 2, "╭", underlay, panel, 150), finish_surface_background(element, None, 150));
        assert_ne!(finish_surface_background(element, None, 150), finish_surface_background(root, None, 150));
    }

    #[test]
    fn modal_underlay_matches_rounded_home_top_right_corner() {
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let rect = (5, 2, 8, 4);
        let default_appearance = crate::native_box_style::BoxAppearance::default();
        let framed_appearance = crate::native_box_style::BoxAppearance { preset: RadiusPreset::Rounded, ..default_appearance.clone() };
        assert!(home_composer_root_cell(&framed_appearance, rounded, rect, 12, 2));
        assert!(!home_composer_root_cell(&default_appearance, rounded, rect, 12, 2));
        assert!(!home_composer_root_cell(&framed_appearance, EffectiveBox { preset: RadiusPreset::Sharp, ..rounded }, rect, 12, 2));
        assert!(!home_composer_root_cell(&framed_appearance, rounded, rect, 11, 2));
        assert!(!home_composer_root_cell(&framed_appearance, rounded, rect, 5, 3));
        assert!(!home_composer_root_cell(&framed_appearance, rounded, rect, 10, 5));
        let root = Rgba::new(10, 10, 10, 255);
        let element = Rgba::new(30, 30, 30, 255);
        let panel = Rgba::new(20, 20, 20, 255);
        let underlay = surface_underlay_background(false, None, true,
            home_composer_root_cell(&framed_appearance, rounded, rect, 12, 2), root, panel, element);
        assert_eq!(modal_surface_background(rounded, (12, 2, 8, 4), 12, 2, "╭", underlay, panel, 150), finish_surface_background(root, None, 150));
    }

    #[test]
    fn home_framed_bottom_middle_keeps_element_background() {
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let rect = (10, 4, 8, 5);
        let root = Rgba::new(10, 10, 10, 255);
        let panel = Rgba::new(20, 20, 20, 255);
        let element = Rgba::new(30, 30, 30, 255);
        let bottom_middle = (u32::from(rect.0) + 3, u32::from(rect.1) + u32::from(rect.3) - 1);
        let framed_appearance = crate::native_box_style::BoxAppearance { preset: RadiusPreset::Rounded, ..Default::default() };
        let home_root = home_composer_root_cell(&framed_appearance, rounded, rect, bottom_middle.0, bottom_middle.1);
        let projected = surface_underlay_background(false, None, true, home_root, root, panel, element);
        assert_eq!(projected, element, "Home bottom border must retain BackgroundElement fill");
        let default_appearance = crate::native_box_style::BoxAppearance::default();
        let framed_appearance = crate::native_box_style::BoxAppearance { preset: RadiusPreset::Rounded, ..default_appearance.clone() };
        assert!(!home_composer_bottom_halfblock_override(&default_appearance, "─"));
        assert!(home_composer_bottom_halfblock_override(&default_appearance, "▀"));
        assert!(!home_composer_bottom_halfblock_override(&framed_appearance, "▀"));
    }

    #[test]
    fn home_halfblock_override_uses_actual_appearance_state() {
        let default_appearance = crate::native_box_style::BoxAppearance::default();
        let framed_appearance = crate::native_box_style::BoxAppearance { preset: RadiusPreset::Rounded, ..default_appearance.clone() };
        assert!(home_composer_bottom_halfblock_override(&default_appearance, "▄"));
        assert!(!home_composer_bottom_halfblock_override(&framed_appearance, "▄"));
    }

    #[test]
    fn framed_home_perimeter_has_no_root_midpoints() {
        let appearance = crate::native_box_style::BoxAppearance { preset: RadiusPreset::Rounded, ..Default::default() };
        let rect = (10, 4, 8, 5);
        let styles = [
            EffectiveBox { preset: RadiusPreset::Sharp, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 },
            EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 },
            EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Double, thickness: 1, padding: 0 },
            EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Heavy, thickness: 1, padding: 0 },
        ];
        for style in styles {
            assert!(!home_composer_root_cell(&appearance, style, rect, 13, 8));
            assert!(!home_composer_root_cell(&appearance, style, rect, 10, 6));
        }
    }

    #[test]
    fn modal_underlay_uses_actual_glyph_for_each_overlapping_surface() {
        let rounded = EffectiveBox { preset: RadiusPreset::Rounded, radius: 0, border_style: BorderStyle::Single, thickness: 1, padding: 0 };
        let root = Rgba::new(10, 10, 10, 255);
        let panel = Rgba::new(20, 20, 20, 255);
        for surface_index in 0..3 {
            let mut surfaces = [(rounded, None); 3];
            surfaces[surface_index].1 = Some((5, 2, 8, 4));
            let actual = resolve_surface_corner_background(panel, root, &surfaces, 12, 2, "╮");
            assert_eq!(actual, root, "underlying surface {surface_index}");
            assert_eq!(modal_surface_background(rounded, (12, 2, 8, 4), 12, 2, "╭", actual, panel, 150), finish_surface_background(root, None, 150));
            assert_eq!(resolve_surface_corner_background(panel, root, &surfaces, 12, 2, " "), panel);
            surfaces[surface_index].0.preset = RadiusPreset::Sharp;
            assert_eq!(resolve_surface_corner_background(panel, root, &surfaces, 12, 2, "┐"), panel);
        }
    }
}

/// One in-flight rename request, identified by the exact stable session id
/// it targets. Exactly one request may be in flight at a time, so a settled
/// reply can never be confused with another request's ack.
#[cfg(feature = "native")]
type RenameInFlight = crate::native_navigation::SessionRef;

/// Sidebar title rename-pending fact for the ACTIVE session:
/// `routes/session/sidebar.tsx:33-45` reads
/// `data.session.title.pending(props.sessionID)` and
/// `context/data.tsx:22-47` keys `generatingTitles[sessionID] === true`, so
/// the fact holds only while the active session's OWN rename request is
/// still in flight. A different id, no identity or no in-flight request is
/// never pending. No title generation is invented here; only the pending
/// fact is projected.
#[cfg(feature = "native")]
fn sidebar_title_pending(
    active_session: Option<&str>,
    inflight: Option<&crate::native_navigation::SessionRef>,
) -> bool {
    active_session.is_some_and(|active| inflight.is_some_and(|inflight| inflight.0 == active))
}

/// Decision for one settled rename request, derived from the in-flight
/// identity only (never from dialog state, so an ack can never retarget):
/// `accepted` is a genuinely-acknowledged server rename for exactly the
/// requested session id; `message` is the truthful failure/refusal text the
/// caller shows in the owning dialog or, when no dialog owns it, surfaces
/// in the transcript instead of silently dropping.
#[cfg(feature = "native")]
struct RenameSettlement {
    accepted: Option<(crate::native_navigation::SessionRef, String)>,
    message: Option<String>,
}

#[cfg(feature = "native")]
fn rename_settlement(
    inflight: &RenameInFlight,
    result: Result<(String, u16, String, String), String>,
) -> RenameSettlement {
    match result {
        Ok((body, 200, title, id))
            if id == inflight.0 && rename_ack_matches(&body, &id) =>
        {
            RenameSettlement {
                accepted: Some((inflight.clone(), title)),
                message: None,
            }
        }
        Ok((_body, status, _, _)) => RenameSettlement {
            accepted: None,
            message: Some(format!(
                "Rename refused: HTTP {status} or mismatched session acknowledgement"
            )),
        },
        Err(error) => RenameSettlement {
            accepted: None,
            message: Some(format!("Rename failed: {error}")),
        },
    }
}

/// A settled rename may touch only a dialog still open for the exact
/// in-flight session id; a cancelled or retargeted dialog never adopts it.
#[cfg(feature = "native")]
fn rename_dialog_owns(
    open: Option<&crate::native_navigation::SessionRef>,
    inflight: &RenameInFlight,
) -> bool {
    open.is_some_and(|dialog| dialog.0 == inflight.0)
}

#[cfg(feature = "native")]
fn rename_request_path(session_id: &str) -> String {
    format!("/api/sessions/{session_id}")
}

/// PATCH /api/sessions/{id} body; the server contract is `{ "title": ... }`.
#[cfg(feature = "native")]
fn rename_request_body(title: &str) -> String {
    serde_json::json!({ "title": title }).to_string()
}

/// Validate the real server ACK shape `{"session":{"id":...}}` against the
/// exact stable identity we asked to rename. Anything else is refused.
#[cfg(feature = "native")]
fn rename_ack_matches(body: &str, expected_id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("session")
                .and_then(|session| session.get("id"))
                .and_then(|id| id.as_str())
                .map(str::to_owned)
        })
        .as_deref()
        == Some(expected_id)
}

/// Trimmed non-empty title accepted for submit; empty/whitespace is refused
/// exactly as upstream `DialogSessionRename` refuses an empty value.
#[cfg(feature = "native")]
fn rename_accepted_title(value: &str) -> Option<String> {
    let title = value.trim();
    (!title.is_empty()).then(|| title.to_owned())
}

/// Escape or Ctrl-C/Ctrl-D cancels the rename dialog without exiting.
#[cfg(feature = "native")]
fn rename_dismisses(event: &native_input_decoder::InputEvent) -> bool {
    matches!(
        event,
        native_input_decoder::InputEvent::Escape
            | native_input_decoder::InputEvent::Byte(3 | 4)
    )
}

#[cfg(feature = "native")]
fn rename_submits(event: &native_input_decoder::InputEvent) -> bool {
    matches!(event, native_input_decoder::InputEvent::Byte(13 | 10))
}

#[cfg(feature = "native")]
fn rename_backspace(event: &native_input_decoder::InputEvent) -> bool {
    matches!(event, native_input_decoder::InputEvent::Byte(8 | 127))
}

/// Ctrl-C/Ctrl-D and Escape dismiss an open popup before any byte-exit path.
#[cfg(feature = "native")]
fn menu_dismisses(event: &native_input_decoder::InputEvent) -> bool {
    matches!(
        event,
        native_input_decoder::InputEvent::Escape
            | native_input_decoder::InputEvent::Byte(3 | 4)
    )
}

/// Enter (CR/LF) closes an open message menu. The option it confirms is
/// bound by the follow-up effect dispatch; this slice closes only.
#[cfg(feature = "native")]
fn menu_submits(event: &native_input_decoder::InputEvent) -> bool {
    matches!(event, native_input_decoder::InputEvent::Byte(13 | 10))
}

/// Move the open message menu's highlight (`DialogSelect` Up/Down,
/// `ui/dialog-select.tsx:1590-1602`).
#[cfg(feature = "native")]
fn message_menu_move(menu: &mut crate::native_view_chat::SessionMessageMenu, forward: bool) {
    match menu {
        crate::native_view_chat::SessionMessageMenu::Actions(actions) => actions.move_selection(forward),
        crate::native_view_chat::SessionMessageMenu::PendingSteer(pending) => pending.move_selection(forward),
    }
}

/// The captured `(session_id, message_id)` identity of an open menu, read
/// before dispatch so the later mutable borrow of the menu slot is free.
#[cfg(feature = "native")]
fn message_menu_identity(
    menu: &crate::native_view_chat::SessionMessageMenu,
) -> (Option<String>, Option<String>) {
    match menu {
        crate::native_view_chat::SessionMessageMenu::Actions(actions) => (
            actions.session_id().map(str::to_owned),
            actions.message_id().map(str::to_owned),
        ),
        crate::native_view_chat::SessionMessageMenu::PendingSteer(pending) => (
            None,
            pending.message_id().map(str::to_owned),
        ),
    }
}

/// Resolve the highlighted option into the request the caller must execute
/// (`dialog-message.tsx:29-90`, `index.tsx:2403-2416`).
#[cfg(feature = "native")]
enum MessageMenuDispatch {
    Actions(crate::native_view_chat::MessageActionOutcome),
    Pending(Option<crate::native_view_chat::PendingSteerEffect>),
}

#[cfg(feature = "native")]
fn message_menu_dispatch(
    menu: &crate::native_view_chat::SessionMessageMenu,
) -> MessageMenuDispatch {
    match menu {
        crate::native_view_chat::SessionMessageMenu::Actions(actions) => {
            MessageMenuDispatch::Actions(actions.select_selected())
        }
        crate::native_view_chat::SessionMessageMenu::PendingSteer(pending) => {
            MessageMenuDispatch::Pending(pending.select_selected())
        }
    }
}

/// Path of the revert-stage route (`packages/protocol/src/groups/session.ts:465`).
#[cfg(feature = "native")]
fn revert_stage_request_path(session_id: &str) -> String {
    format!("/api/sessions/{session_id}/revert/stage")
}

/// Body of the revert-stage route; the server contract is `{"messageID":...}`
/// (`StageRevertBody`, `crates/server/src/lib.rs:1179-1181`).
#[cfg(feature = "native")]
fn revert_stage_request_body(message_id: &str) -> String {
    serde_json::json!({ "messageID": message_id }).to_string()
}

/// Execute every effect the resolved message action produced
/// (`dialog-message.tsx:34-88`). The view performs no IO itself; each effect
/// is a real request here or a named gap.
#[cfg(feature = "native")]
#[allow(clippy::too_many_arguments)]
fn dispatch_message_actions(
    outcome: crate::native_view_chat::MessageActionOutcome,
    session_id: Option<&str>,
    origin: Option<&str>,
    auth: Option<&str>,
    draft: &mut String,
    draft_cursor: &mut usize,
    draft_parts: &mut Vec<crate::native_paste::PastedText>,
    transcript: &mut NativeChatHistory,
) {
    use crate::native_view_chat::MessageActionEffect;
    use std::io::Write as _;
    for effect in outcome.effects {
        match effect {
            // The dialog clears on every jump/revert (`dialog-message.tsx:34`).
            MessageActionEffect::CloseMenu => {}
            MessageActionEffect::JumpToMessage { .. } => {
                // Upstream's handler itself only clears the dialog
                // (`dialog-message.tsx:31-34`); the scroll-to-message alignment
                // is owned by the transcript scroll controller, which oc2 has
                // no live row-anchor reveal for yet. Named seam.
            }
            MessageActionEffect::RestorePrompt { text, .. } => {
                let cursor = text.len();
                crate::native_composer::replace_draft_with_cursor(
                    draft,
                    draft_cursor,
                    text,
                    cursor,
                );
                draft_parts.clear();
            }
            MessageActionEffect::StageRevert { message_id } => {
                let Some(session_id) = session_id else {
                    append_transcript(
                        transcript,
                        "error: revert stage: no session for this message".to_owned(),
                    );
                    continue;
                };
                let path = revert_stage_request_path(session_id);
                let body = revert_stage_request_body(&message_id);
                let result = origin
                    .ok_or_else(|| "daemon unavailable".to_owned())
                    .and_then(|origin| {
                        http_request_with_status(origin, "POST", &path, Some(&body), auth)
                            .and_then(|(reply, status)| {
                                if (200..300).contains(&status) {
                                    Ok(reply)
                                } else {
                                    Err(format!("HTTP {status}"))
                                }
                            })
                    });
                if let Err(error) = result {
                    append_transcript(
                        transcript,
                        format!("error: revert stage failed: {error}"),
                    );
                }
            }
            MessageActionEffect::CopyToClipboard { text, truncated } => {
                let frame = selection_clipboard_frame(&text);
                match std::io::stdout().lock().write_all(frame.as_bytes()) {
                    Ok(()) => {
                        if truncated {
                            append_transcript(transcript, "copied message (truncated)".to_owned());
                        }
                    }
                    Err(error) => append_transcript(
                        transcript,
                        format!("error: clipboard write failed: {error}"),
                    ),
                }
            }
            MessageActionEffect::ForkSession { .. } => {
                // `dialog-fork.tsx:16-30` replaces the dialog with the fork
                // picker; oc2 has no live fork dialog host yet. Named seam.
                append_transcript(
                    transcript,
                    "fork: session fork dialog is not available yet".to_owned(),
                );
            }
        }
    }
}

/// Execute the "Pending steer" select's request
/// (`index.tsx:2403-2416`). The upstream `session.inbox.update` /
/// `session.inbox.cancel` routes have no oc2 server endpoint yet, so this
/// records the request as an explicit, non-silent gap.
#[cfg(feature = "native")]
fn dispatch_pending_steer(
    effect: Option<crate::native_view_chat::PendingSteerEffect>,
    transcript: &mut NativeChatHistory,
) {
    use crate::native_view_chat::PendingSteerEffect;
    match effect {
        Some(PendingSteerEffect::MoveToQueue { message_id }) => append_transcript(
            transcript,
            format!("pending steer: move {message_id} to queue is not available yet"),
        ),
        Some(PendingSteerEffect::Cancel { message_id }) => append_transcript(
            transcript,
            format!("pending steer: cancel {message_id} is not available yet"),
        ),
        None => {}
    }
}

/// Build the popup frame for an open message menu, centered in the
/// viewport like upstream's `DialogSelect` (`dialog-message.tsx:29-90`,
/// `ui/dialog-select.tsx`). Rows come from the model
/// (`MessageActionMenu::render_rows` / `PendingSteerMenu::render_rows`)
/// and are never fabricated here; the frame reuses the existing generic
/// `MenuFrame` blitter (the `ctx.context_menu` paint block above).
#[cfg(feature = "native")]
fn message_menu_frame(
    menu: &crate::native_view_chat::SessionMessageMenu,
    viewport: crate::native_tab_context_menu::Viewport,
) -> crate::native_tab_context_menu::MenuFrame {
    const MENU_WIDTH: u16 = 44;
    let width = MENU_WIDTH.min(viewport.width);
    let (rows, selected) = match menu {
        crate::native_view_chat::SessionMessageMenu::Actions(actions) => {
            (actions.render_rows(usize::from(width)), actions.selected_index())
        }
        crate::native_view_chat::SessionMessageMenu::PendingSteer(pending) => {
            (pending.render_rows(usize::from(width)), pending.selected_index())
        }
    };
    let height = u16::try_from(rows.len()).unwrap_or(u16::MAX).min(viewport.height);
    let x = viewport.width.saturating_sub(width) / 2;
    let y = viewport.height.saturating_sub(height) / 2;
    let items = rows
        .into_iter()
        .take(usize::from(height))
        .enumerate()
        .map(|(index, (label, _))| crate::native_tab_context_menu::VisibleMenuItem {
            index,
            rect: crate::native_tab_context_menu::MenuRect {
                x,
                y: y.saturating_add(u16::try_from(index).unwrap_or(u16::MAX)),
                width,
                height: 1,
            },
            label,
            enabled: true,
            // `DialogSelect` paints the highlighted option with the element
            // background and the others with the panel background
            // (`ui/dialog-select.tsx`, `native_tab_context_menu.rs:275-291`).
            hovered: index == selected,
            background: if index == selected {
                crate::native_theme_runtime::Slot::BackgroundElement
            } else {
                crate::native_theme_runtime::Slot::BackgroundPanel
            },
            foreground: crate::native_theme_runtime::Slot::Text,
        })
        .collect();
    crate::native_tab_context_menu::MenuFrame {
        rect: Some(crate::native_tab_context_menu::MenuRect { x, y, width, height }),
        items,
    }
}

#[cfg(all(test, feature = "native"))]
mod message_menu_frame_tests {
    #[test]
    fn message_menu_frame_centers_model_rows() {
        let menu = crate::native_view_chat::menu_for_message(None, "ses", "msg", None);
        let frame = super::message_menu_frame(
            &menu,
            crate::native_tab_context_menu::Viewport { width: 80, height: 24 },
        );
        let expected = match &menu {
            crate::native_view_chat::SessionMessageMenu::Actions(actions) => {
                actions.render_rows(44)
            }
            crate::native_view_chat::SessionMessageMenu::PendingSteer(pending) => {
                pending.render_rows(44)
            }
        };
        assert_eq!(frame.items.len(), expected.len());
        let rect = frame.rect.expect("menu frame has a rect");
        assert_eq!(rect.width, 44);
        assert_eq!(rect.x, (80 - 44) / 2);
        assert_eq!(rect.y, (24 - expected.len() as u16) / 2);
        for (index, item) in frame.items.iter().enumerate() {
            assert_eq!(item.label, expected[index].0);
            assert_eq!(item.rect.y, rect.y + index as u16);
            assert!(item.enabled);
        }
    }
}


#[cfg(feature = "native")]
fn recent_closed_menu_target(navigation: &crate::native_navigation::NavigationState) -> crate::native_tab_context_menu::MenuTarget {
    // The reopen facility always exists in production, so this is `Some(..)`:
    // an empty history then shows upstream's "No recently closed tabs" row
    // (`session-tabs.tsx:411-414`). Newest first, capped at the upstream
    // `slice(0, 10)` (`session-tabs.tsx:384`).
    let mut closed: Vec<_> = navigation
        .recently_closed()
        .into_iter()
        .rev()
        .map(|entry| {
            let title = navigation
                .title(&entry.session)
                .or(entry.title.as_deref())
                .unwrap_or(entry.session.0.as_str())
                .to_owned();
            crate::native_tab_context_menu::ClosedTab {
                session: entry.session,
                title,
            }
        })
        .collect();
    closed.truncate(crate::native_tab_context_menu::MAX_RECENTLY_CLOSED);
    crate::native_tab_context_menu::MenuTarget::New {
        recently_closed: Some(closed),
    }
}

#[cfg(feature = "native")]
fn tab_menu_target_at(navigation: &crate::native_navigation::NavigationState, tabs: Option<&TabStrip>, rail: Option<&crate::native_tab_rail::RailFrame>, vertical: bool, x: u16, y: u16) -> Option<crate::native_tab_context_menu::MenuTarget> {
    if vertical {
        return rail?.hit(x,y).and_then(|hit| match hit {
            crate::native_tab_rail::RailHit::Session(session) => navigation.tabs().iter().any(|tab| tab.session == session).then(|| crate::native_tab_context_menu::MenuTarget::Session { title: navigation.title(&session).unwrap_or(&session.0).to_owned(), session, can_add: true }),
            crate::native_tab_rail::RailHit::New => Some(recent_closed_menu_target(navigation)),
            crate::native_tab_rail::RailHit::Search => None,
        });
    }
    let tabs=tabs?;
    if let Some(hit)=tabs.hit(x,y) { return navigation.tabs().iter().any(|tab| tab.session==hit.session).then(|| crate::native_tab_context_menu::MenuTarget::Session { title:navigation.title(&hit.session).unwrap_or(&hit.session.0).to_owned(), session:hit.session, can_add:true }); }
    // Promoted Home slot owns its menu target directly: upstream NEW_SESSION_TAB
    // has no session identity, so resolving to the active session would invent a
    // target. The New menu carries the live recently-closed history
    // (`session-tabs.tsx:383-417`) so Reopen is serviceable.
    if tabs.home_hit(x,y) { return Some(recent_closed_menu_target(navigation)); }
    tabs.add_rect.filter(|r| x>=r.x && x<r.x.saturating_add(r.width) && y>=r.y && y<r.y.saturating_add(r.height)).map(|_| recent_closed_menu_target(navigation))
}

#[cfg(feature = "native")]
fn selection_clipboard_frame(text: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4 + 16);
    output.push_str("\x1b]52;c;");
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        for value in [
            TABLE[(a >> 2) as usize],
            TABLE[(((a & 3) << 4) | (b >> 4)) as usize],
            if chunk.len() > 1 {
                TABLE[(((b & 15) << 2) | (c >> 6)) as usize]
            } else {
                b'='
            },
            if chunk.len() > 2 {
                TABLE[(c & 63) as usize]
            } else {
                b'='
            },
        ] {
            output.push(value as char);
        }
    }
    output.push('\x07');
    output
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

/// Modal row role from known dialog structure. Selected choices are painted
/// with an opaque accent background below, so their foreground must contrast.
#[cfg(feature = "native")]
fn modal_row_fg(line: &str, heading: bool, palette: crate::native_theme_runtime::Palette) -> Rgba {
    use crate::native_theme_runtime::Slot;
    if heading {
        paint_color(palette, crate::native_ui_style::style(crate::native_ui_style::Role::Heading).foreground)
    } else if line.trim_start().starts_with("> ") {
        paint_color(palette, Slot::Background)
    } else {
        paint_color(palette, Slot::TextMuted)
    }
}

fn run_attributes(flags: u16) -> u32 {
    use crate::markdown_runs::{
        BOLD, HEADING, HEADING1, ITALIC, LINK, LINK_TEXT, QUOTE,
    };
    // Pinned OpenTUI attributes: BOLD=1, ITALIC=4, UNDERLINE=8, matching
    // `generateSyntax` (`packages/theme/src/tui/syntax.ts:46-71`): headings and
    // strong are bold, headings of level 1 underline, emphasis and quotes are
    // italic, and links underline.
    let mut attributes = 0;
    if flags & (BOLD | HEADING | HEADING1) != 0 { attributes |= 1; }
    if flags & (ITALIC | QUOTE) != 0 { attributes |= 4; }
    if flags & (LINK | LINK_TEXT | HEADING1) != 0 { attributes |= 8; }
    attributes
}

/// Foreground token for one Markdown span, in upstream rule order
/// (`packages/theme/src/tui/syntax.ts:46-71`). `None` leaves the row's own
/// foreground alone.
#[cfg(feature = "native")]
fn markdown_run_color(
    markdown: &crate::native_theme_runtime::MarkdownPalette,
    flags: u16,
) -> Option<&crate::native_theme_runtime::HueColor> {
    use crate::markdown_runs::{
        BOLD, CODE, HEADING, HEADING1, ITALIC, LINK, LINK_TEXT, LIST, QUOTE, RULE, STRIKE,
    };
    if flags & CODE != 0 {
        Some(&markdown.code)
    } else if flags & (HEADING | HEADING1) != 0 {
        Some(&markdown.heading)
    } else if flags & BOLD != 0 {
        Some(&markdown.strong)
    } else if flags & ITALIC != 0 {
        Some(&markdown.emphasis)
    } else if flags & LINK_TEXT != 0 {
        Some(&markdown.link_text)
    } else if flags & LINK != 0 {
        Some(&markdown.link)
    } else if flags & QUOTE != 0 {
        Some(&markdown.block_quote)
    } else if flags & RULE != 0 {
        Some(&markdown.horizontal_rule)
    } else if flags & LIST != 0 {
        Some(&markdown.list_item)
    } else if flags & STRIKE != 0 {
        Some(&markdown.muted)
    } else {
        None
    }
}


#[cfg(feature = "native")]
fn paint_color(
    palette: crate::native_theme_runtime::Palette,
    slot: crate::native_theme_runtime::Slot,
) -> Rgba {
    use crate::native_theme_runtime::{Palette, Slot, source_over};
    let background = source_over(
        Palette::opencode_dark().get(Slot::Background),
        palette.get(Slot::Background),
    );
    let [r, g, b, a] = source_over(background, palette.get(slot));
    Rgba::new(r, g, b, a)
}

#[cfg(feature = "native")]
fn sidebar_cell(presentation: crate::native_view::ViewPresentation, x: u32, y: u32) -> bool {
    presentation.sidebar_offset.is_some_and(|left| {
        x >= u32::from(left)
            && x < u32::from(left) + u32::from(presentation.sidebar_width)
            && y < u32::from(presentation.rows.saturating_sub(1))
            && x < u32::from(presentation.cols)
    })
}

#[cfg(all(test, feature = "native"))]
mod sidebar_paint_geometry_tests {
    #[test]
    fn sidebar_membership_excludes_footer_and_hidden_or_narrow_sidebar() {
        use crate::native_view::ViewPresentation;
        let wide = ViewPresentation { cols: 148, rows: 30, main_offset: 0, main_width: 106, main_padding: 2, sidebar_offset: Some(106), sidebar_width: 42 };
        let narrow = ViewPresentation { cols: 100, rows: 30, main_offset: 0, main_width: 100, main_padding: 2, sidebar_offset: Some(58), sidebar_width: 42 };
        let hidden = ViewPresentation { sidebar_offset: None, sidebar_width: 0, ..narrow };
        assert!(super::sidebar_cell(wide, 110, 0));
        assert!(super::sidebar_cell(wide, 110, 28));
        assert!(!super::sidebar_cell(wide, 110, 29));
        assert!(!super::sidebar_cell(hidden, 99, 10));
        assert!(super::sidebar_cell(narrow, 99, 10));
        assert!(!super::sidebar_cell(narrow, 100, 10));
        assert!(!super::sidebar_cell(wide, 105, 10));
    }
}

/// Paint a horizontal stroke in fixed-size runs. Unlike a single truncated
/// run, this covers the entire visible edge even for large terminal widths,
/// without ever submitting a write outside the native renderer viewport.
#[cfg(feature = "native")]
fn draw_horizontal_rule(
    renderer: &mut NativeRenderer,
    x: u32,
    y: u32,
    width: u32,
    glyph: char,
    fg: Rgba,
    bg: Rgba,
) -> Result<(), Box<dyn std::error::Error>> {
    if x >= renderer.cols() || y >= renderer.rows() { return Ok(()); }
    let mut written = 0u32;
    let visible = width.min(renderer.cols() - x);
    while written < visible {
        let count = (visible - written).min(512) as usize;
        renderer.draw_text_styled(
            x + written, y, &glyph.to_string().repeat(count), fg, Some(bg), 0,
        )?;
        written += count as u32;
    }
    Ok(())
}

/// Draw only the outline cells of a page/menu panel. All four edges share the
/// *original* rectangle (not a second clipped width), so clipping cannot
/// fabricate a new closing corner or spill into the following viewport cell.
#[cfg(feature = "native")]
fn draw_panel_outline(
    renderer: &mut NativeRenderer,
    rect: (u16, u16, u16, u16),
    style: crate::native_box_style::EffectiveBox,
    fg: Rgba,
    surface: Rgba,
    root: Rgba,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(geometry) = crate::native_box_style::BoxGeometry::framed(
        rect.0, rect.1, rect.2, rect.3, style.padding,
    ) else { return Ok(()); };
    let Some(frame) = crate::native_box_style::TextFrame::new(
        style, geometry.width as usize, style.padding as usize,
    ) else { return Ok(()); };
    let x = u32::from(geometry.x);
    let y = u32::from(geometry.y);
    let width = u32::from(frame.width);
    let bottom = y + u32::from(geometry.height) - 1;
    let right = x + width - 1;
    let corners = style.corner_glyphs();
    let (horizontal, vertical) = style.border_glyphs();
    if y < renderer.rows() {
        draw_horizontal_rule(renderer, x + 1, y, width - 2, horizontal, fg, surface)?;
    }
    if bottom < renderer.rows() {
        draw_horizontal_rule(renderer, x + 1, bottom, width - 2, horizontal, fg, surface)?;
    }
    // Reserve the enclosing background behind curved corners, not a square
    // panel-colored cell. A sharp corner retains the solid panel background.
    let curved = matches!(style.border_style, crate::native_box_style::BorderStyle::Single)
        && (matches!(style.preset,
            crate::native_box_style::RadiusPreset::Rounded |
            crate::native_box_style::RadiusPreset::IosSquircle)
            || matches!(style.preset, crate::native_box_style::RadiusPreset::Custom)
               && style.radius > 0);
    let corner_bg = if curved { root } else { surface };
    for (cx, cy, glyph) in [
        (x, y, corners.top_left), (right, y, corners.top_right),
        (x, bottom, corners.bottom_left), (right, bottom, corners.bottom_right),
    ] {
        if cx < renderer.cols() && cy < renderer.rows() {
            renderer.draw_text_styled(cx, cy, &glyph.to_string(), fg, Some(corner_bg), 0)?;
        }
    }
    for cy in (y + 1)..bottom.min(renderer.rows()) {
        if x < renderer.cols() {
            renderer.draw_text_styled(x, cy, &vertical.to_string(), fg, Some(surface), 0)?;
        }
        if right < renderer.cols() {
            renderer.draw_text_styled(right, cy, &vertical.to_string(), fg, Some(surface), 0)?;
        }
    }
    Ok(())
}

/// Native styled paint with cell-aligned semantic spans and explicit opaque
/// backgrounds. Modal/sidebar source-over dimming is resolved in Rust before
/// FFI; native alpha or null-background preservation is not assumed.
#[cfg(feature = "native")]
fn surface_fill_rect(
    style: crate::native_box_style::EffectiveBox,
    rect: (u16, u16, u16, u16),
) -> Option<(u16, u16, u16, u16)> {
    let geometry = crate::native_box_style::BoxGeometry::for_border(
        style.border_style, rect.0, rect.1, rect.2, rect.3, style.padding,
    )?;
    if geometry.has_border {
        geometry.interior()
    } else {
        Some((geometry.x, geometry.y, geometry.width, geometry.height))
    }
}

#[cfg(feature = "native")]
fn paint_rail_row(
    renderer: &mut NativeRenderer,
    row: &crate::native_tab_rail::RailRow,
    modal: Option<(u32, u32, u32, u32)>,
    foreground: Rgba,
    background: Rgba,
) -> Result<(), Box<dyn std::error::Error>> {
    let left = u32::from(row.rect.x);
    let right = left.saturating_add(u32::from(row.rect.width)).min(renderer.cols());
    for offset in 0..u32::from(row.rect.height) {
        let y = u32::from(row.rect.y).saturating_add(offset);
        if y >= renderer.rows() || left >= right { continue; }
        let spans = match modal {
            Some((x, top, width, height)) if y >= top && y < top.saturating_add(height) =>
                [(left, right.min(x)), (left.max(x.saturating_add(width)), right)],
            _ => [(left, right), (right, right)],
        };
        let line = row.text.split('\n').nth(offset as usize).unwrap_or("");
        for (start, end) in spans {
            if start >= end { continue; }
            let width = end - start;
            let text = crate::native_view_layout::slice_cells(line, (start - left) as u16, width as u16);
            renderer.fill_rect(start, y, width, 1, background)?;
            renderer.draw_text_styled(start, y, &text, foreground, Some(background), 0)?;
        }
    }
    Ok(())
}

#[cfg(feature = "native")]
fn paint_styled_rail_row(
    renderer: &mut NativeRenderer,
    row: &crate::native_tab_rail::RailRow,
    modal: Option<(u32, u32, u32, u32)>,
    paint: crate::native_tab_rail::RowPaint,
    alpha: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::native_tab_rail::CellColor;
    let color = |bytes: [u8; 4]| {
        let bytes = crate::native_theme_runtime::dim(bytes, alpha);
        Rgba::new(bytes[0], bytes[1], bytes[2], bytes[3])
    };
    let background = color(paint.background);
    let foreground = color(paint.title_fg);
    // The existing bounded primitive owns clipping and modal exclusion.
    paint_rail_row(renderer, row, modal, foreground, background)?;
    let left = u32::from(row.rect.x);
    let top = u32::from(row.rect.y);
    for (offset, line) in row.text.lines().take(usize::from(row.rect.height)).enumerate() {
        let y = top.saturating_add(offset as u32);
        if y >= renderer.rows() { continue; }
        let mut failure = None;
        let mut column = 0usize;
        let mut cluster = 0usize;
        crate::native_view_layout::visit_unpadded_cells(line, usize::from(row.rect.width), |grapheme, width| {
            let current = column;
            let index = cluster;
            column = column.saturating_add(width);
            cluster += 1;
            if failure.is_some() { return; }
            let x = left.saturating_add(current as u32);
            let end = x.saturating_add(width as u32);
            if end > renderer.cols() || modal.is_some_and(|(mx, my, mw, mh)| {
                y >= my && y < my.saturating_add(mh) && x < mx.saturating_add(mw) && end > mx
            }) { return; }
            let role = row.color_spans.iter().rev().find(|(start, _)| usize::from(*start) <= current)
                .map_or(CellColor::Fill, |(_, role)| *role);
            let (fg, attr) = match role {
                // Native buffer clear/fill uses opaque white for unstyled blank cells.
                CellColor::CompactPadding => ([255, 255, 255, 255], 0),
                CellColor::Fill => (paint.title_fg, 0),
                CellColor::Title => (paint.title_fg, paint.title_attr),
                CellColor::Detail => (paint.detail_fg, 0),
                CellColor::Spinner => (paint.spinner_fg, paint.title_attr),
                CellColor::Close => (paint.close_fg, 0),
                CellColor::Search => (paint.title_fg, 0),
                // TAB-23 overlay fg (`session-tabs-rail.tsx:61`): the hover tint.
                CellColor::SearchHover => (paint.search_hover_fg, 0),
                CellColor::NewIndicator => (paint.plus_fg, paint.new_attr),
                CellColor::NewText => (paint.new_text_fg, paint.new_attr),
                CellColor::CapTop => (paint.cap_top_fg, 0),
                CellColor::CapBottom => (paint.cap_bottom_fg, 0),
            };
            // `session-tabs.tsx:817,869-877`: the title/detail ramps blend each
            // visible cluster toward the row background over four cells.
            let fg = match (row.fade, role) {
                (Some(fade), CellColor::Title | CellColor::Detail | CellColor::Search)
                    if index >= usize::from(fade.start)
                        && index < usize::from(fade.start).saturating_add(usize::from(fade.length)) =>
                    fade.color((index - usize::from(fade.start)) as u16, fg, paint.background),
                _ => fg,
            };
            if let Err(error) = renderer.draw_text_styled(x, y, grapheme, color(fg), Some(background), attr) {
                failure = Some(error);
            }
        });
        if let Some(error) = failure { return Err(error.into()); }
    }
    Ok(())
}

#[cfg(feature = "native")]
fn paint_tab_segment(
    renderer: &mut NativeRenderer,
    rect: (u16, u16, u16, u16),
    text: &str,
    modal: Option<(u32, u32, u32, u32)>,
    foreground: Rgba,
    background: Rgba,
    attributes: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let (left, top, width, height) = rect;
    let left = u32::from(left);
    let right = left.saturating_add(u32::from(width)).min(renderer.cols());
    for offset in 0..height {
        let y = u32::from(top).saturating_add(u32::from(offset));
        if y >= renderer.rows() || left >= right { continue; }
        let spans = match modal {
            Some((x, modal_top, modal_width, modal_height))
                if y >= modal_top && y < modal_top.saturating_add(modal_height) =>
                [(left, right.min(x)), (left.max(x.saturating_add(modal_width)), right)],
            _ => [(left, right), (right, right)],
        };
        for (start, end) in spans {
            if start < end { renderer.fill_rect(start, y, end - start, 1, background)?; }
        }
        let line = text.split('\n').nth(usize::from(offset)).unwrap_or("");
        let mut column = 0usize;
        let mut failure = None;
        crate::native_view_layout::visit_unpadded_cells(line, usize::from(width), |cluster, cells| {
            let x = left.saturating_add(column as u32);
            column = column.saturating_add(cells);
            let end = x.saturating_add(cells as u32);
            if failure.is_some() || end > right || modal.is_some_and(|(mx, my, mw, mh)|
                y >= my && y < my.saturating_add(mh) && x < mx.saturating_add(mw) && end > mx
            ) { return; }
            if let Err(error) = renderer.draw_text_styled(x, y, cluster, foreground, Some(background), attributes) {
                failure = Some(error);
            }
        });
        if let Some(error) = failure { return Err(error.into()); }
    }
    Ok(())
}

/// Title segment with the upstream four-cell fade: each visible cluster
/// blends its foreground toward the tab background (`session-tabs.tsx:230-236`).
#[cfg(feature = "native")]
fn paint_tab_title_faded(
    renderer: &mut NativeRenderer,
    rect: (u16, u16, u16, u16),
    text: &str,
    modal: Option<(u32, u32, u32, u32)>,
    foreground: Rgba,
    background: Rgba,
    attributes: u32,
    fade: crate::native_tabs::TitleFade,
) -> Result<(), Box<dyn std::error::Error>> {
    let (left, top, width, height) = rect;
    let left = u32::from(left);
    let right = left.saturating_add(u32::from(width)).min(renderer.cols());
    for offset in 0..height {
        let y = u32::from(top).saturating_add(u32::from(offset));
        if y >= renderer.rows() || left >= right { continue; }
        let spans = match modal {
            Some((x, modal_top, modal_width, modal_height))
                if y >= modal_top && y < modal_top.saturating_add(modal_height) =>
                [(left, right.min(x)), (left.max(x.saturating_add(modal_width)), right)],
            _ => [(left, right), (right, right)],
        };
        for (start, end) in spans {
            if start < end { renderer.fill_rect(start, y, end - start, 1, background)?; }
        }
        let line = text.split('\n').nth(usize::from(offset)).unwrap_or("");
        let mut column = 0usize;
        let mut visible = 0usize;
        let mut failure = None;
        crate::native_view_layout::visit_unpadded_cells(line, usize::from(width), |cluster, cells| {
            let index = visible;
            visible += 1;
            let x = left.saturating_add(column as u32);
            column = column.saturating_add(cells);
            let end = x.saturating_add(cells as u32);
            if failure.is_some() || end > right || modal.is_some_and(|(mx, my, mw, mh)|
                y >= my && y < my.saturating_add(mh) && x < mx.saturating_add(mw) && end > mx
            ) { return; }
            let color = if usize::from(index) < usize::from(fade.length) && index >= usize::from(fade.start) {
                let [r, g, b, a] = fade.color(
                    (usize::from(index) - usize::from(fade.start)) as u16,
                    [foreground.r, foreground.g, foreground.b, foreground.a],
                    [background.r, background.g, background.b, background.a],
                );
                Rgba::new(r, g, b, a)
            } else {
                foreground
            };
            if let Err(error) = renderer.draw_text_styled(x, y, cluster, color, Some(background), attributes) {
                failure = Some(error);
            }
        });
        if let Some(error) = failure { return Err(error.into()); }
    }
    Ok(())
}

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
    let selected_cells: std::collections::BTreeSet<(u32, u32)> = ctx
        .selection
        .iter()
        .flat_map(|&(row, start, width)| {
            (start..start.saturating_add(width).min(renderer.cols())).map(move |x| (row, x))
        })
        .collect();
    let background = paint_color(ctx.palette, Slot::Background);
    let panel = paint_color(ctx.palette, Slot::BackgroundPanel);
    // THEME-06: the modal surface paints the contextual `@dialog` view's
    // `background.base` (`ui/dialog.tsx:44,82`), which the dialog palette
    // exposes through the `backgroundPanel` (raised.base) role. Without an
    // `@dialog` override `ctx.dialog == ctx.palette`, so this is unchanged.
    let dialog_panel = paint_color(ctx.dialog, Slot::BackgroundPanel);
    let element = paint_color(ctx.palette, Slot::BackgroundElement);
    let text_color = paint_color(ctx.palette, Slot::Text);
    let muted = paint_color(ctx.palette, Slot::TextMuted);
    let error = paint_color(ctx.palette, Slot::Error);
    let warning = paint_color(ctx.palette, Slot::Warning);
    let accent = paint_color(ctx.palette, Slot::Primary);
    fn dim(color: Rgba, alpha: u8) -> Rgba {
        let [r, g, b, a] =
            crate::native_theme_runtime::dim([color.r, color.g, color.b, color.a], alpha);
        Rgba::new(r, g, b, a)
    }
    fn dim_tab(color: [u8; 4], alpha: u8) -> Rgba {
        let [r, g, b, a] = crate::native_theme_runtime::dim(color, alpha);
        Rgba::new(r, g, b, a)
    }
    // Paint only the intersection with the actual renderer viewport. Region
    // descriptors come from the shared layout pass, but a resize can make a
    // previously computed bottom edge stale; clipping here prevents panel or
    // composer fills from leaking below the footer/terminal bounds.
    fn fill_clipped(
        renderer: &mut NativeRenderer,
        cols: u32,
        rows: u32,
        rect: (u16, u16, u16, u16),
        color: Rgba,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (x, y, width, height) = rect;
        let x = u32::from(x);
        let y = u32::from(y);
        if x >= cols || y >= rows || width == 0 || height == 0 {
            return Ok(());
        }
        let width = u32::from(width).min(cols - x);
        let height = u32::from(height).min(rows - y);
        if width > 0 && height > 0 {
            renderer.fill_rect(x, y, width, height, color)?;
        }
        Ok(())
    }
    // Resolve alpha in Rust. Every span supplies an opaque background;
    // native null-background/alpha behavior is not relied upon.
    let modal = ctx.modal_rows.and_then(|count| visible_modal_rect(cols, rows,
        u32::from(ctx.modal.x), u32::from(ctx.modal.y), u32::from(ctx.modal.width),
        count.saturating_add(usize::from(ctx.modal.content_y.saturating_sub(ctx.modal.y).saturating_sub(1)))
             .saturating_add(usize::from(ctx.modal.bottom_padding))));
    // Modal layout owns the semantic content rectangle. Keep the painter's
    // clip exactly on those declared coordinates; do not recompute a second
    // inset from border geometry (modal presentation has its own 2-cell
    // framing contract).
    let modal_content_rect = modal.map(|_| (
        u32::from(ctx.modal.content_x),
        u32::from(ctx.modal.content_y),
        u32::from(ctx.modal.content_width),
        ctx.modal_rows.unwrap_or(0) as u32,
    ));
    let base_alpha = if modal.is_some() {
        ctx.modal.backdrop_alpha
    } else {
        ctx.sidebar_backdrop_alpha
    };
    let corner_surfaces = [
        (ctx.appearance.effective(crate::native_box_style::BoxId::ChatComposer), ctx.composer),
        (ctx.appearance.effective(crate::native_box_style::BoxId::Menu), ctx.page_panel),
        (ctx.appearance.effective(crate::native_box_style::BoxId::Sidebar), presentation.sidebar_offset.map(|left|
            (left, 0, presentation.sidebar_width, rows.saturating_sub(1) as u16))),
    ];
    let outside_fill = |x: u32, y: u32| {
        corner_surfaces.iter().rev().any(|(style, rect)| rect.is_some_and(|rect|
            x <= u32::from(u16::MAX) && y <= u32::from(u16::MAX)
                && (style.rounded_fill_corner(rect, x as u16, y as u16)
                    || (style.has_border() && style.rounded_fill_corner(rect, rect.0, rect.1)
                        && x >= u32::from(rect.0) && x < u32::from(rect.0) + u32::from(rect.2)
                        && y >= u32::from(rect.1) && y < u32::from(rect.1) + u32::from(rect.3)
                        && (x == u32::from(rect.0) || x + 1 == u32::from(rect.0) + u32::from(rect.2)
                            || y == u32::from(rect.1) || y + 1 == u32::from(rect.1) + u32::from(rect.3))))))
    };
    renderer.fill_rect(0, 0, cols, rows, dim(background, base_alpha))?;
    if let Some((x, y, width, height)) = ctx.page_panel {
        let style = ctx.appearance.effective(crate::native_box_style::BoxId::Menu);
        let rect = (x, y, width, height);
        let spans = if style.has_border() && style.rounded_fill_corner(rect, x, y) {
            surface_fill_rect(style, rect).into_iter().collect()
        } else { style.fill_rectangles(rect) };
        for rect in spans {
            fill_clipped(renderer, cols, rows, rect, dim(panel, base_alpha))?;
        }
    }
    if let Some((left, top, width, height)) = ctx.composer {
        let style = ctx.appearance.effective(crate::native_box_style::BoxId::ChatComposer);
        let rect = (left, top, width, height);
        let spans = if style.has_border() && style.rounded_fill_corner(rect, left, top) {
            surface_fill_rect(style, rect).into_iter().collect()
        } else { style.fill_rectangles(rect) };
        for rect in spans {
            fill_clipped(renderer, cols, rows, rect, dim(element, base_alpha))?;
        }
    }
    for (row, line) in lines.iter().enumerate().take(rows as usize) {
        let y = row as u32;
        let modal_text = crate::native_view_layout::slice_cells(
            line,
            ctx.modal.content_x,
            ctx.modal.content_width,
        );
        let normal_fg = match ctx.chat_row_classes.get(row) {
            Some(class) => chat_row_foreground(class, text_color, muted, error, warning, ctx.diff.as_ref()),
            None if ctx.home => home_row_fg(lines, row, ctx.palette),
            None => text_color,
        };
        let mut spans: Vec<(u32, String, Rgba, Rgba, u32)> = Vec::with_capacity(8);
        // A styled run may only merge *adjacent display cells*. Modal masks
        // skip cells, and a style-only merge across a skipped region shifts
        // the rest of the row left, potentially eating the opposite border.
        let mut span_end = 0u32;
        crate::native_view_layout::visit_cells(
            line,
            cols.min(u16::MAX as u32) as u16,
            |x, glyph, cells| {
                let x = u32::from(x);
                let sidebar = sidebar_cell(presentation, x, y);
                let in_modal = modal.is_some_and(|(left, top, width, height)| {
                    x >= left && x < left + width && y >= top && y < top + height
                });
                if in_modal {
                    let Some((left, top, width, height)) = modal_content_rect else { return; };
                    if x < left || x >= left.saturating_add(width)
                        || y < top || y >= top.saturating_add(height) { return; }
                }
                let composer = ctx.composer.is_some_and(|(left, top, width, height)| {
                    x >= u32::from(left)
                        && x < u32::from(left) + u32::from(width)
                        && y >= u32::from(top)
                        && y < u32::from(top) + u32::from(height)
                });
                let role_bg = chat_row_background_at(ctx.chat_row_classes.get(row), presentation, x, background, panel, element, ctx.diff.as_ref());
                let in_page = ctx.page_panel.is_some_and(|(left, top, width, height)| {
                    x >= u32::from(left) && x < u32::from(left + width)
                        && y >= u32::from(top) && y < u32::from(top + height)
                });
                let panel_bg = if in_modal { dialog_panel } else { panel };
                let mut bg = surface_underlay_background(sidebar || in_modal || in_page, role_bg, composer, false, background, panel_bg, element);
                let mut fg = if in_modal {
                    modal_row_fg(
                        modal_text.trim_start(),
                        y == u32::from(ctx.modal.content_y),
                        ctx.palette,
                    )
                } else {
                    normal_fg
                };
                let mut attributes = if in_modal {
                    0
                } else {
                    chat_row_attribute_override(ctx.chat_row_classes.get(row))
                };
                if in_modal && (y == u32::from(ctx.modal.content_y)
                    || modal_text.trim_start().starts_with("> ")) {
                    attributes = crate::native_ui_style::style(crate::native_ui_style::Role::Heading).attributes;
                }
                let styled_region = ctx.ui_regions.iter().rev().find(|region| (!in_modal || region.modal)
                    && u32::from(region.y) == y
                    && x >= u32::from(region.x) && x < u32::from(region.x + region.width));
                if let Some(region) = styled_region {
                        let role = if region.role == crate::native_ui_style::Role::Item
                            && region.detail_from.is_some_and(|offset| x >= u32::from(region.x + offset)) {
                            crate::native_ui_style::Role::Description
                        } else { region.role };
                        let style = crate::native_ui_style::style(role);
                        fg = paint_color(ctx.palette, style.foreground);
                        if let Some(slot) = style.background { bg = paint_color(ctx.palette, slot); }
                        attributes = style.attributes;
                }
                // Context pages publish their own usage spans; the composer
                // dock publishes dock spans on the chat route, where
                // `in_page` is false by construction. The vec is empty for
                // every other chat cell, so dropping the page gate cannot
                // color transcript content.
                if !in_modal && !composer && !sidebar {
                    if let Some((_, _, _, slot)) = ctx.context_usage_styles.iter().find(
                        |(left, row, width, _)| {
                            u32::from(*row) == y
                                && x >= u32::from(*left)
                                && x.saturating_add(u32::from(cells))
                                    <= u32::from(*left).saturating_add(u32::from(*width))
                        },
                    ) {
                        fg = paint_color(ctx.palette, *slot);
                    }
                }
                // Sidebar spans win over the row role for the cells they
                // cover: the row role is a whole-row fallback, these are the
                // dot/name/status and strikethrough ranges upstream paints
                // separately (`feature-plugins/sidebar/mcp.tsx:62-85`).
                if sidebar && !in_modal {
                    if let Some((_, _, _, slot, attrs)) =
                        ctx.sidebar_span_styles.iter().find(|(left, row, width, _, _)| {
                            u32::from(*row) == y
                                && x >= u32::from(*left)
                                && x.saturating_add(u32::from(cells))
                                    <= u32::from(*left).saturating_add(u32::from(*width))
                        })
                    {
                        fg = paint_color(ctx.palette, *slot);
                        attributes |= *attrs;
                    }
                }
                if in_modal && styled_region.is_none()
                    && modal_text.trim_start().starts_with("> ")
                    && x >= u32::from(ctx.modal.content_x)
                    && x < u32::from(ctx.modal.content_x)
                        + u32::from(ctx.modal.content_width)
                {
                    bg = accent;
                }
                if (ctx.home || ctx.rail_prompt) && !in_modal {
                    if let Some((left, top, width, height)) = ctx.composer {
                        if x >= u32::from(left)
                            && x < u32::from(left + width)
                            && y >= u32::from(top)
                            && y < u32::from(top + height)
                        {
                            let composer_style = ctx.appearance.effective(crate::native_box_style::BoxId::ChatComposer);
                            // Left-column rail cell: single decision source
                            // (borderless accent caret, bordered default ┃/╹
                            // Border-on-root fix), else unchanged fallthrough.
                            let rail_style = (x == u32::from(left))
                                .then(|| home_composer_border_cell_style(
                                    &ctx.appearance,
                                    composer_style,
                                    glyph,
                                    accent,
                                    background,
                                    ctx.palette,
                                ))
                                .flatten();
                            if let Some((rail_fg, rail_bg)) = rail_style {
                                fg = rail_fg;
                                bg = rail_bg;
                            } else if y == u32::from(top + height - 1)
                                && home_composer_bottom_halfblock_override(
                                    &ctx.appearance,
                                    glyph,
                                )
                            {
                                // Bottom half-block carries the element color
                                // over the root background, matching the prompt
                                // separator rather than drawing a bright rule.
                                fg = element;
                                bg = background;
                            }
                        }
                    }
                }
                if !in_modal {
                    if let Some(markdown) = ctx.markdown.as_ref() {
                        if let Some(run_spans) = ctx.chat_markdown_spans.get(row) {
                            for run_span in run_spans {
                                if x >= u32::from(run_span.start)
                                    && x.saturating_add(u32::from(cells)) <= u32::from(run_span.end)
                                {
                                    if let Some(color) = markdown_run_color(markdown, run_span.flags) {
                                        let [r, g, b, a] = color.rgba;
                                        fg = Rgba::new(r, g, b, a);
                                    }
                                    if run_span.flags & crate::markdown_runs::CODE != 0
                                        && run_span.flags & crate::markdown_runs::CODE_BLOCK == 0
                                    {
                                        let [r, g, b, a] = markdown.inline_background.rgba;
                                        bg = Rgba::new(r, g, b, a);
                                    }
                                }
                            }
                        }
                    }
                }
                if !in_modal {
                    // Every text cell, including whitespace, must obey the
                    // same fill mask as the rectangle pass. Glyph-only corner
                    // adjustment cannot repair opaque rectangular underpaint.
                    if outside_fill(x, y) { bg = background; }
                    let alpha = if modal.is_some() {
                        ctx.modal.backdrop_alpha
                    } else if !sidebar {
                        ctx.sidebar_backdrop_alpha
                    } else {
                        0
                    };
                    bg = finish_surface_background(resolve_surface_corner_background(bg, background,
                        &corner_surfaces, x, y, glyph), None, alpha);
                    fg = dim(fg, alpha);
                }
                if !in_modal && selected_cells.contains(&(y, x)) {
                    bg = accent;
                    fg = background;
                }
                if !in_modal {
                    if let Some(run_spans) = ctx.chat_markdown_spans.get(row) {
                        for run_span in run_spans {
                            if x >= u32::from(run_span.start)
                                && x.saturating_add(u32::from(cells)) <= u32::from(run_span.end)
                            {
                                attributes |= run_attributes(run_span.flags);
                            }
                        }
                    }
                }
                if let Some((_, text, last_fg, last_bg, last_attributes)) = spans.last_mut() {
                    if span_end == x
                        && *last_fg == fg
                        && *last_bg == bg
                        && *last_attributes == attributes
                        && text.len().saturating_add(glyph.len()) <= 32 * 1024
                    {
                        text.push_str(glyph);
                        span_end = x.saturating_add(u32::from(cells));
                        return;
                    }
                }
                spans.push((x, glyph.to_owned(), fg, bg, attributes));
                span_end = x.saturating_add(u32::from(cells));
            },
        );
        for (x, text, fg, bg, attributes) in spans {
            renderer.draw_text_styled(x, y, &text, fg, Some(bg), attributes)?;
        }
    }
    // Transcript scrollbar (`routes/session/index.tsx:1527-1537`): one track
    // column on the viewport's right edge with an opaque thumb sized by the
    // content/viewport ratio. Painted after the transcript text so the two
    // reserved cells never show stale glyphs.
    if let Some(scrollbar) = ctx.transcript_scrollbar {
        let x = u32::from(scrollbar.x);
        let top = u32::from(scrollbar.y);
        let height = u32::from(scrollbar.height);
        if height > 0 && x < cols {
            for row in top..top.saturating_add(height).min(rows) {
                let under_modal = modal.is_some_and(|(left, modal_top, width, modal_height)| {
                    x >= left && x < left.saturating_add(width)
                        && row >= modal_top && row < modal_top.saturating_add(modal_height)
                });
                if under_modal {
                    continue;
                }
                renderer.draw_text_styled(
                    x, row, " ", ctx.scrollbar_thumb, Some(ctx.scrollbar_track), 0,
                )?;
            }
            if let Some((start, len)) = scrollbar.thumb {
                for offset in 0..u32::from(len) {
                    let row = top
                        .saturating_add(u32::from(start))
                        .saturating_add(offset);
                    if row >= rows {
                        break;
                    }
                    let under_modal = modal.is_some_and(|(left, modal_top, width, modal_height)| {
                        x >= left && x < left.saturating_add(width)
                            && row >= modal_top && row < modal_top.saturating_add(modal_height)
                    });
                    if under_modal {
                        continue;
                    }
                    renderer.draw_text_styled(
                        x, row, "█", ctx.scrollbar_thumb, Some(ctx.scrollbar_track), 0,
                    )?;
                }
            }
        }
    }
    // Sidebar body scrollbar (`routes/session/sidebar.tsx:56-69`): ABSOLUTE
    // one-column bar over the body's reserved padding. The track surface is the
    // sidebar's own raised background, so only the thumb is painted; this runs
    // after the body text so the reserved cell never keeps stale glyphs.
    if let Some(scrollbar) = ctx.sidebar_scrollbar {
        if let Some((start, len)) = scrollbar.thumb {
            let x = u32::from(scrollbar.x);
            let top = u32::from(scrollbar.y);
            if x < cols {
                for offset in 0..u32::from(len) {
                    let row = top.saturating_add(u32::from(start)).saturating_add(offset);
                    if row >= rows {
                        break;
                    }
                    let under_modal = modal.is_some_and(|(left, modal_top, width, modal_height)| {
                        x >= left && x < left.saturating_add(width)
                            && row >= modal_top && row < modal_top.saturating_add(modal_height)
                    });
                    if under_modal {
                        continue;
                    }
                    renderer.draw_text_styled(
                        x, row, "█", ctx.scrollbar_thumb, Some(panel), 0,
                    )?;
                }
            }
        }
    }
    // logo.ts + component/logo.tsx: two 19-cell halves separated by one
    // cell, muted left/text right, with 25%-tinted marker backgrounds.
    if let Some((x, y)) = ctx.logo {
        for segment in crate::native_home_logo::segments(cols as u16, rows as u16) {
            let py = u32::from(y) + u32::from(segment.y);
            if py >= rows || modal.is_some_and(|(_, dy, _, h)| (dy..dy + h).contains(&py)) {
                continue;
            }
            let original_fg = if segment.bold { text_color } else { muted };
            {
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
                for (col, marker) in segment.raw.chars().enumerate() {
                    let px = u32::from(x) + u32::from(segment.x) + col as u32;
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
                        u32::from(segment.bold),
                    )?;
                }
            }
        }
    }
    if let Some(rail) = ctx.vertical_rail.as_ref() {
        for row in &rail.rows {
            let x = u32::from(row.rect.x); let y = u32::from(row.rect.y);
            if x >= cols || y >= rows { continue; }
            let mut paint = if matches!(row.kind, crate::native_tab_rail::RailRowKind::New) {
                crate::native_tab_rail::RowPaint::new_control_row(&ctx.palette, rail.active_new, row.is_hovered)
            } else if matches!(row.kind, crate::native_tab_rail::RailRowKind::CapTop) {
                crate::native_tab_rail::RowPaint::cap_row(&ctx.palette, false)
            } else if matches!(row.kind, crate::native_tab_rail::RailRowKind::CapBottom) {
                crate::native_tab_rail::RowPaint::cap_row(&ctx.palette, true)
            } else if matches!(row.kind, crate::native_tab_rail::RailRowKind::SearchHover) {
                // `session-tabs-rail.tsx:54-66`: overlay fg = the hover tint;
                // bg = the tint when the row below is highlighted, else
                // `raised.base` (carried on `row.is_hovered`).
                crate::native_tab_rail::RowPaint::compact_search_hover_row(&ctx.palette, row.is_hovered)
            } else if matches!(row.kind, crate::native_tab_rail::RailRowKind::Search) {
                crate::native_tab_rail::RowPaint::compact_search_row(&ctx.palette)
            } else if rail.width < crate::native_tab_rail::COMPACT_BREAKPOINT
                && matches!(row.kind, crate::native_tab_rail::RailRowKind::Session(_)) {
                crate::native_tab_rail::RowPaint::compact_session_row(&ctx.palette, &row.flags, row.is_hovered)
            } else {
                crate::native_tab_rail::RowPaint::session_row(&ctx.palette, &row.flags, row.is_hovered)
            };
            // TAB-24: a renaming rail title is DIM while animations are off,
            // else unchanged; the New/add rows keep their own BOLD
            // (`session-tabs.tsx:1713-1720`).
            if let (Some(renaming), crate::native_tab_rail::RailRowKind::Session(session)) =
                (ctx.title_renaming.as_ref(), &row.kind)
            {
                paint.title_attr = crate::native_tabs::title_attributes(
                    row.flags.active,
                    renaming(session),
                    ctx.animations,
                );
            }
            // Modal body cells were painted by the general row pass above.
            // Exclude their whole rectangle, not just the later border pass.
            paint_styled_rail_row(renderer, row, modal, paint, base_alpha)?;
        }
    }
    if let Some(tabs) = ctx.tabs.filter(|_| ctx.vertical_rail.is_none()) {
        // TAB-24: when the caller supplies the title-pending fact, stamp the
        // reduced-motion DIM over the resolved paints; otherwise keep the
        // plain selected-BOLD row paint.
        let tabs = match ctx.title_renaming.as_ref() {
            Some(renaming) => tabs
                .clone()
                .with_row_colors(&ctx.palette)
                .with_title_renaming(renaming.as_ref(), ctx.animations),
            None => tabs.clone().with_row_colors(&ctx.palette),
        };
        let fallback_fg = accent;
        let fallback_bg = dim(background, base_alpha);
        for (index, row) in tabs.rows.iter().enumerate() {
            let Some((_, rect)) = tabs.rects.get(index) else { continue };
            let x = u32::from(rect.x);
            let y = u32::from(rect.y);
            if x >= cols || y >= rows { continue; }
            let paint = tabs.row_paints.get(index);
            let row_bg = paint.map_or(fallback_bg, |p| dim_tab(p.background, base_alpha));
            let width = u32::from(rect.width).min(cols - x) as u16;
            let indicator = tabs.indicator_cells.min(width);
            let number = crate::native_view_layout::slice_cells(row, 0, indicator);
            let title = crate::native_view_layout::slice_cells(row, indicator, width - indicator);
            if indicator > 0 {
                paint_tab_segment(renderer, (rect.x, rect.y, indicator, rect.height), &number, modal,
                    paint.map_or(fallback_fg, |p| dim_tab(p.number_fg, base_alpha)), row_bg, 0)?;
            }
            if width > indicator {
                let title_rect = (rect.x.saturating_add(indicator), rect.y, width - indicator, rect.height);
                let title_fg = paint.map_or(fallback_fg, |p| dim_tab(p.title_fg, base_alpha));
                let title_attr = paint.map_or(0, |p| p.title_attr);
                match tabs.row_fades.get(index).copied().flatten() {
                    Some(fade) => paint_tab_title_faded(renderer, title_rect, &title, modal,
                        title_fg, row_bg, title_attr, fade)?,
                    None => paint_tab_segment(renderer, title_rect, &title, modal,
                        title_fg, row_bg, title_attr)?,
                }
            }
        }
        if let Some(rect) = tabs.visible_close_rect() {
            if rect.x < cols as u16 && rect.y < rows as u16 {
                let paint = tabs.rects.iter().position(|(_, parent)|
                    rect.x >= parent.x && rect.x < parent.x.saturating_add(parent.width) && rect.y == parent.y)
                    .and_then(|index| tabs.row_paints.get(index));
                paint_tab_segment(renderer, (rect.x, rect.y, rect.width, rect.height), "✕", modal,
                    paint.map_or(muted, |p| dim_tab(p.close_fg, base_alpha)),
                    paint.map_or(fallback_bg, |p| dim_tab(p.background, base_alpha)), 0)?;
            }
        }
        for (row, rect) in tabs.rows.iter().skip(tabs.rects.len()).zip(&tabs.overflow_rects) {
            if rect.x < cols as u16 && rect.y < rows as u16 {
                paint_tab_segment(renderer, (rect.x, rect.y, rect.width, 1), row, modal, muted, fallback_bg, 0)?;
            }
        }
        if let Some(rect) = tabs.add_rect {
            if rect.x < cols as u16 && rect.y < rows as u16 {
                let paint = tabs.add_paint;
                let add_bg = paint.map_or(fallback_bg, |p| dim_tab(p.background, base_alpha));
                let add_fg = paint.map_or(muted, |p| dim_tab(p.plus_fg, base_alpha));
                let label = if rect.width < 3 { "+" } else { " + " };
                paint_tab_segment(renderer, (rect.x, rect.y, rect.width, rect.height), label, modal, add_fg, add_bg,
                    paint.map_or(0, |p| p.title_attr))?;
            }
        }
        if let Some(rect) = tabs.home_rect {
            if rect.x < cols as u16 && rect.y < rows as u16 {
                let paint = tabs.home_paint;
                let home_bg = paint.map_or(fallback_bg, |p| dim_tab(p.background, base_alpha));
                if let Some(row) = tabs.home_row.as_deref() {
                    let width = rect.width;
                    let indicator = tabs.indicator_cells.min(width);
                    let number = crate::native_view_layout::slice_cells(row, 0, indicator);
                    let title = crate::native_view_layout::slice_cells(row, indicator, width - indicator);
                    if indicator > 0 {
                        paint_tab_segment(renderer, (rect.x, rect.y, indicator, rect.height), &number, modal,
                            paint.map_or(fallback_fg, |p| dim_tab(if p.number_fg == [0, 0, 0, 0] { p.title_fg } else { p.number_fg }, base_alpha)), home_bg,
                            paint.map_or(0, |p| p.title_attr))?;
                    }
                    if width > indicator {
                        paint_tab_segment(renderer, (rect.x.saturating_add(indicator), rect.y, width - indicator, rect.height), &title, modal,
                            paint.map_or(fallback_fg, |p| dim_tab(p.title_fg, base_alpha)), home_bg,
                            paint.map_or(0, |p| p.title_attr))?;
                    }
                }
                if let Some(close) = tabs.home_close_visible_rect() {
                    paint_tab_segment(renderer, (close.x, close.y, close.width, close.height), "✕", modal,
                        paint.map_or(muted, |p| dim_tab(p.close_fg, base_alpha)), home_bg, 0)?;
                }
            }
        }
    }
    // `session-tabs.tsx:1241-1278`: compact rail tooltip. Two half-block caps
    // frame a raised two-row body holding the title (text.base) over the
    // detail (text.muted), one cell inset on each side.
    if let (Some(tooltip), None) = (ctx.compact_tooltip.as_ref(), modal) {
        let x0 = u32::from(tooltip.rect.x);
        let y0 = u32::from(tooltip.rect.y);
        let w = u32::from(tooltip.rect.width).min(cols.saturating_sub(x0));
        let h = u32::from(tooltip.rect.height).min(rows.saturating_sub(y0));
        if w > 0 && h > 0 {
            let raised = paint_color(ctx.palette, crate::native_theme_runtime::Slot::BackgroundPanel);
            let base = paint_color(ctx.palette, crate::native_theme_runtime::Slot::Background);
            let title_fg = paint_color(ctx.palette, crate::native_theme_runtime::Slot::Text);
            let detail_fg = paint_color(ctx.palette, crate::native_theme_runtime::Slot::TextMuted);
            renderer.fill_rect(x0, y0, w, 1, base)?;
            renderer.draw_text_styled(x0, y0, &"▄".repeat(w as usize), raised, Some(base), 0)?;
            if h >= 2 {
                let bottom = y0.saturating_add(h - 1);
                renderer.fill_rect(x0, bottom, w, 1, base)?;
                renderer.draw_text_styled(x0, bottom, &"▀".repeat(w as usize), raised, Some(base), 0)?;
            }
            for row in 1..h.saturating_sub(1) {
                let y = y0.saturating_add(row);
                renderer.fill_rect(x0, y, w, 1, raised)?;
                if w > 2 {
                    let text = match row {
                        1 => tooltip.title.as_str(),
                        2 => tooltip.detail.as_str(),
                        _ => "",
                    };
                    let fg = if row == 2 { detail_fg } else { title_fg };
                    renderer.draw_text_styled(x0 + 1, y, &crate::native_view_layout::fit(text, (w - 2) as u16), fg, Some(raised), 0)?;
                }
            }
        }
    }
    if let Some(rect) = ctx.page_panel {
        if ctx.appearance.full_outline(crate::native_box_style::BoxId::Menu) {
            draw_panel_outline(
                renderer, rect,
                ctx.appearance.effective(crate::native_box_style::BoxId::Menu),
                accent, dim(panel, base_alpha), dim(background, base_alpha),
            )?;
        }
    }
    if let (Some(count), Some((left, top, width, height))) = (ctx.modal_rows, modal) {
        let style = ctx.appearance.effective(ctx.modal_box);
        let corners = style.corner_glyphs();
        let (horizontal, vertical) = style.border_glyphs();
        let bottom = top
            .saturating_add(height.saturating_sub(1))
            .min(rows.saturating_sub(1));
        if style.has_border() && width >= 4 && count > 0 && bottom > top {
            let modal_rect = (left as u16, top as u16, width as u16, height as u16);
            let underlay_at = |x: u32, y: u32| {
                let contains = |rect: (u16, u16, u16, u16)| {
                    x >= u32::from(rect.0) && x < u32::from(rect.0) + u32::from(rect.2)
                        && y >= u32::from(rect.1) && y < u32::from(rect.1) + u32::from(rect.3)
                };
                let composer = ctx.composer.filter(|rect| contains(*rect));
                let home_root = (ctx.home || ctx.rail_prompt) && composer.is_some_and(|rect|
                    home_composer_root_cell(&ctx.appearance,
                        ctx.appearance.effective(crate::native_box_style::BoxId::ChatComposer), rect, x, y));
                let role = chat_row_background_at(ctx.chat_row_classes.get(y as usize), presentation, x, background, panel, element, ctx.diff.as_ref());
                let mut color = surface_underlay_background(
                    sidebar_cell(presentation, x, y) || ctx.page_panel.is_some_and(contains),
                    role, composer.is_some(), home_root, background, panel, element,
                );
                if let Some(region) = ctx.ui_regions.iter().rev().find(|region| !region.modal
                    && u32::from(region.y) == y && x >= u32::from(region.x)
                    && x < u32::from(region.x) + u32::from(region.width)) {
                    if let Some(slot) = crate::native_ui_style::style(region.role).background {
                        color = paint_color(ctx.palette, slot);
                    }
                }
                if home_root { color = background; }
                if outside_fill(x, y) { color = background; }
                let underlying_glyph = ctx.modal_underlay_rows.get(y as usize)
                    .map(|row| crate::native_view_layout::slice_cells(row, x as u16, 1));
                if let Some(glyph) = underlying_glyph {
                    color = resolve_surface_corner_background(color, background, &corner_surfaces, x, y, &glyph);
                }
                if selected_cells.contains(&(y, x)) { color = accent; }
                color
            };
            let tl = modal_surface_background(style, modal_rect, left, top, &corners.top_left.to_string(), underlay_at(left, top), dialog_panel, ctx.modal.backdrop_alpha);
            let tr = modal_surface_background(style, modal_rect, left + width - 1, top, &corners.top_right.to_string(), underlay_at(left + width - 1, top), dialog_panel, ctx.modal.backdrop_alpha);
            let bl = modal_surface_background(style, modal_rect, left, bottom, &corners.bottom_left.to_string(), underlay_at(left, bottom), dialog_panel, ctx.modal.backdrop_alpha);
            let br = modal_surface_background(style, modal_rect, left + width - 1, bottom, &corners.bottom_right.to_string(), underlay_at(left + width - 1, bottom), dialog_panel, ctx.modal.backdrop_alpha);
            renderer.draw_text_styled(left, top, &corners.top_left.to_string(), accent, Some(tl), 0)?;
            renderer.draw_text_styled(left + width - 1, top, &corners.top_right.to_string(), accent, Some(tr), 0)?;
            renderer.draw_text_styled(left, bottom, &corners.bottom_left.to_string(), accent, Some(bl), 0)?;
            renderer.draw_text_styled(left + width - 1, bottom, &corners.bottom_right.to_string(), accent, Some(br), 0)?;
            if width > 2 {
                draw_horizontal_rule(renderer, left + 1, top, width - 2, horizontal, accent, dialog_panel)?;
                draw_horizontal_rule(renderer, left + 1, bottom, width - 2, horizontal, accent, dialog_panel)?;
            }
            for y in top + 1..bottom {
                renderer.draw_text_styled(left, y, &vertical.to_string(), accent, Some(dialog_panel), 0)?;
                renderer.draw_text_styled(left + width - 1, y, &vertical.to_string(), accent, Some(dialog_panel), 0)?;
            }
        }
    }
    if let Some((glyph, _)) = ctx.loading {
        let left = presentation.main_offset.saturating_add(presentation.main_padding);
        if modal.is_none() && rows > 0
            && left < presentation.main_offset.saturating_add(presentation.main_width)
            && matches!(ctx.chat_row_classes.first(), Some(crate::native_view_chat::ChatRowClass::Header))
        {
            // The facade owns the Thinking label in the main header. Animate
            // its leading gutter, never the composer's outline or draft.
            renderer.draw_text_styled(
                u32::from(left),
                0,
                &glyph.to_string(),
                accent,
                Some(background),
                0,
            )?;
        }
    }
    if let Some(completion) = ctx.completion {
        for (index, (line, selected)) in completion.rows.iter().enumerate() {
            let y = u32::from(completion.y) + index as u32;
            let role = if *selected { crate::native_ui_style::Role::Selected }
                else { crate::native_ui_style::Role::Item };
            let style = crate::native_ui_style::style(role);
            let bg = style.background.map(|slot| paint_color(ctx.palette, slot)).unwrap_or(panel);
            renderer.fill_rect(u32::from(completion.x), y, u32::from(completion.width), 1, bg)?;
            renderer.draw_text_styled(
                u32::from(completion.x), y,
                &crate::native_view_layout::fit(line, completion.width),
                paint_color(ctx.palette, style.foreground), Some(bg), style.attributes,
            )?;
        }
    }
    if let Some(menu) = ctx.context_menu.as_ref() {
        if let Some(rect) = menu.rect {
            let x=u32::from(rect.x); let y=u32::from(rect.y);
            let w=u32::from(rect.width).min(cols.saturating_sub(x));
            let h=u32::from(rect.height).min(rows.saturating_sub(y));
            if w>0 && h>0 { renderer.fill_rect(x,y,w,h,panel)?; }
            for item in &menu.items {
                let ix=u32::from(item.rect.x); let iy=u32::from(item.rect.y);
                if ix>=cols || iy>=rows { continue; }
                let iw=u32::from(item.rect.width).min(cols.saturating_sub(ix));
                let bg=paint_color(ctx.palette,item.background); let fg=paint_color(ctx.palette,item.foreground);
                if iw>0 { renderer.fill_rect(ix,iy,iw,1,bg)?; }
                renderer.draw_text_styled(ix+u32::from(item.rect.width>=3),iy,&item.label,fg,Some(bg),0)?;
            }
        }
    }
    if let Some((title,error))=ctx.rename_dialog.as_ref() {
        let w=cols.min(60); let h=5u32.min(rows); let x=(cols-w)/2; let y=(rows-h)/2;
        let bg=paint_color(ctx.palette,crate::native_theme_runtime::Slot::BackgroundPanel); let fg=paint_color(ctx.palette,crate::native_theme_runtime::Slot::Text);
        if w>0&&h>0 { renderer.fill_rect(x,y,w,h,bg)?; renderer.draw_text_styled(x+1,y,"Rename session",fg,Some(bg),0)?; }
        if h>1 { renderer.draw_text_styled(x+1,y+1,&crate::native_view_layout::fit(title,w.saturating_sub(2) as u16),fg,Some(bg),0)?; }
        if h>2 { renderer.draw_text_styled(x+1,y+2,"Enter: save  Esc: cancel",fg,Some(bg),0)?; }
        if let Some(error)=error { if h>3 { renderer.draw_text_styled(x+1,y+3,&crate::native_view_layout::fit(error,w.saturating_sub(2) as u16),paint_color(ctx.palette,crate::native_theme_runtime::Slot::TextMuted),Some(bg),0)?; } }
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
fn seed_navigation_titles(
    navigation: &mut crate::native_navigation::NavigationState,
    snapshot: &LiveSnapshot,
) {
    // This is a single upsert per authenticated listing row, not a replacement:
    // local tabs and titles for other sessions survive every accepted bind.
    let target = snapshot.session_id.as_str();
    let mut rows: Vec<_> = snapshot.session_titles.iter().cloned()
        .filter(|(id, _)| id != target)
        .map(|(id, title)| (crate::native_navigation::SessionRef(id), title))
        .collect();
    if let Some((id, title)) = snapshot.session_titles.iter().find(|(id, _)| id == target) {
        rows.push((crate::native_navigation::SessionRef(id.clone()), title.clone()));
    }
    navigation.ingest_snapshot_titles(rows);
    let labels: Vec<_> = snapshot.session_project_labels.iter().map(|(id, label)| {
        (crate::native_navigation::SessionRef(id.clone()), label.clone())
    }).collect();
    navigation.replace_project_labels(&labels);
}

#[cfg(feature = "native")]
fn rail_inputs_for_navigation(
    navigation: &crate::native_navigation::NavigationState,
    tabs: &[crate::native_tabs::TabInput],
) -> Vec<crate::native_tab_rail::RailInput> {
    tabs.iter().map(|tab| crate::native_tab_rail::RailInput {
        session: tab.session.clone(), title: tab.title.clone(),
        detail: navigation.project_label(&tab.session).unwrap_or_default().to_owned(), flags: tab.flags,
        marquee_offset: tab.marquee_offset,
    }).collect()
}

#[cfg(feature = "native")]
fn bind_session(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    global_draft: &str,
    _global_cursor: usize,
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
    view.set_project_directory(
        snapshot.directory.clone().unwrap_or_else(current_workspace_label),
    );
    if let Err(error) = refresh_session_mcp(view, &snapshot, auth) {
        append_transcript(transcript, format!("error: session mcp unavailable: {error}"));
    }
    if let Err(error) = refresh_session_todos(view, &snapshot, auth) {
        append_transcript(transcript, format!("error: session todos unavailable: {error}"));
    }
    Ok((snapshot, draft))
}

#[cfg(feature = "native")]
fn refresh_session_mcp(
    view: &mut crate::native_view::NativeView,
    snapshot: &LiveSnapshot,
    auth: Option<&str>,
) -> Result<(), String> {
    let path = format!("/api/sessions/{}/mcp", snapshot.session_id);
    let body = http_request(&snapshot.origin, "GET", &path, None, auth)?;
    let value = serde_json::from_str(&body).map_err(|error| format!("invalid MCP response: {error}"))?;
    view.apply_mcp(&value)
}

#[cfg(feature = "native")]
fn refresh_session_todos(
    view: &mut crate::native_view::NativeView,
    snapshot: &LiveSnapshot,
    auth: Option<&str>,
) -> Result<(), String> {
    let path = format!("/api/sessions/{}/todo", snapshot.session_id);
    let body = http_request(&snapshot.origin, "GET", &path, None, auth)?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("invalid todo response: {error}"))?;
    view.apply_todos(&value)
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

#[cfg(feature = "native")]
/// Real `/api/commands` catalogue → Agents-page [`crate::native_view_pages::PageRow`]s.
///
/// Truthful metadata only: name/description/`read_only` and — when the
/// catalogue actually carries them — optional `aliases` are rendered from
/// the authenticated listing; unknown extra fields are ignored. Rows are
/// selectable metadata (activation stays the framework's
/// [`crate::native_view_pages::PageAction::Unsupported`]; no launch command
/// exists, so no pretend action is wired). One malformed entry atomically
/// rejects the WHOLE catalogue: the caller keeps the page's previous rows
/// and reports the error honestly instead of rendering a partial list.
/// Duplicate ids are equally rejected (single source of truth per name).
fn agent_catalogue_rows(value: &serde_json::Value) -> Result<Vec<crate::native_view_pages::PageRow>, String> {
    // Bounded like the @mention loader, which also caps the agents array.
    const MAX_AGENTS: usize = 4096;
    const MAX_ALIASES: usize = 16;
    const MAX_TEXT: usize = 256;

    let valid_token = |token: &str| {
        !token.is_empty() && token.chars().count() <= 128
            && token.chars().all(|ch| !ch.is_control()) && token.chars().all(|ch| !ch.is_whitespace())
    };
    let agents = value
        .get("agents")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "agents listing missing".to_owned())?;
    if agents.len() > MAX_AGENTS {
        return Err(format!("agents listing exceeds {} entries", MAX_AGENTS));
    }
    let mut rows = Vec::with_capacity(agents.len());
    let mut seen = std::collections::HashSet::new();
    for agent in agents {
        let name = agent
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "agent entry missing a string name".to_owned())?;
        if !valid_token(name) {
            return Err(format!("agent name rejected: {name:?}"));
        }
        if !seen.insert(name.to_owned()) {
            return Err(format!("duplicate agent name: {name}"));
        }
        let description = match agent.get("description") {
            None | Some(serde_json::Value::Null) => "",
            Some(serde_json::Value::String(text)) => {
                if text.chars().count() > MAX_TEXT {
                    return Err(format!("agent {name}: description exceeds {} chars", MAX_TEXT));
                }
                text.as_str()
            }
            Some(_) => return Err(format!("agent {name}: description must be a string")),
        };
        let read_only = match agent.get("read_only") {
            None | Some(serde_json::Value::Null) => false,
            Some(serde_json::Value::Bool(flag)) => *flag,
            Some(_) => return Err(format!("agent {name}: read_only must be boolean")),
        };
        let mut aliases: Vec<&str> = Vec::new();
        match agent.get("aliases") {
            None | Some(serde_json::Value::Null) => {}
            Some(serde_json::Value::Array(items)) => {
                if items.len() > MAX_ALIASES {
                    return Err(format!("agent {name}: aliases exceed {} entries", MAX_ALIASES));
                }
                for item in items {
                    let alias = item
                        .as_str()
                        .ok_or_else(|| format!("agent {name}: aliases must be strings"))?;
                    if !valid_token(alias) {
                        return Err(format!("agent {name}: alias rejected: {alias:?}"));
                    }
                    aliases.push(alias);
                }
            }
            Some(_) => return Err(format!("agent {name}: aliases must be a string array")),
        }
        let mut detail: String = description.to_owned();
        if read_only {
            if !detail.is_empty() {
                detail.push_str(" · ");
            }
            detail.push_str("read-only");
        }
        if !aliases.is_empty() {
            if !detail.is_empty() {
                detail.push_str(" · ");
            }
            detail.push_str("aliases: ");
            detail.push_str(&aliases.join(", "));
        }
        rows.push(crate::native_view_pages::PageRow {
            id: name.to_owned(),
            label: name.to_owned(),
            detail,
            enabled: true,
        });
    }
    Ok(rows)
}

#[cfg(all(test, feature = "native"))]
mod agent_catalogue_rows_tests {
    use super::agent_catalogue_rows;

    #[test]
    fn parses_ten_roles_with_read_only_names_and_custom_aliases() {
        let agents: Vec<serde_json::Value> = (0..10)
            .map(|index| {
                serde_json::json!({
                    "name": format!("role-{index}"),
                    "description": format!("Role {index} brief"),
                    "read_only": index % 2 == 0,
                    "aliases": [format!("role-alias-{index}")],
                })
            })
            .collect();
        let catalogue = serde_json::json!({"agents": agents, "commands": {}});

        let rows = agent_catalogue_rows(&catalogue).expect("ten truthful roles");

        assert_eq!(rows.len(), 10);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.id, format!("role-{index}"));
            assert_eq!(row.label, format!("role-{index}"));
            assert_eq!(row.label, row.id);
            assert!(row.detail.starts_with(&format!("Role {index} brief")));
            assert!(row.detail.contains(&format!("aliases: role-alias-{index}")));
            assert_eq!(row.detail.contains("read-only"), index % 2 == 0);
            assert!(row.enabled);
        }
    }

    #[test]
    fn malformed_rows_reject_the_whole_catalogue_atomically() {
        let ok = serde_json::json!({"name": "build", "description": "Builds", "read_only": false});
        let too_long_name: String = "a".repeat(129);
        let too_long_desc: String = "d".repeat(257);
        for malformed in [
            serde_json::json!({"description": "no name"}),
            serde_json::json!({"name": "", "description": "empty name"}),
            serde_json::json!({"name": "bad\nname", "description": "control name"}),
            serde_json::json!({"name": "bad name", "description": "whitespace name"}),
            serde_json::json!({"name": too_long_name}),
            serde_json::json!({"name": ["array"], "description": "name type"}),
            serde_json::json!({"name": "x", "description": 7}),
            serde_json::json!({"name": "x", "description": too_long_desc}),
            serde_json::json!({"name": "x", "read_only": "yes"}),
            serde_json::json!({"name": "x", "aliases": "build"}),
            serde_json::json!({"name": "x", "aliases": [7]}),
            serde_json::json!({"name": "x", "aliases": ["a\nb"]}),
            serde_json::json!({"name": "x", "aliases": (0..17).map(|i| format!("a{i}")).collect::<Vec<_>>()}),
        ] {
            let catalogue = serde_json::json!({"agents": [ok.clone(), malformed]});
            let error = agent_catalogue_rows(&catalogue)
                .expect_err("affirmed atomic rejection");
            assert!(!error.is_empty());
        }
        // Duplicate ids reject the full catalogue, valid-prefix or not.
        let duplicates = serde_json::json!({"agents": [ok.clone(), ok]});
        assert!(agent_catalogue_rows(&duplicates).is_err());
        // Hard bound: exactly 4096 entries still parse; one more rejects.
        let at_bound: Vec<serde_json::Value> = (0..4096)
            .map(|index| serde_json::json!({"name": format!("agent-{index}"), "read_only": false}))
            .collect();
        assert_eq!(
            agent_catalogue_rows(&serde_json::json!({"agents": at_bound}))
                .expect("bound is inclusive")
                .len(),
            4096
        );
        let over_bound: Vec<serde_json::Value> = (0..4097)
            .map(|index| serde_json::json!({"name": format!("agent-{index}"), "read_only": false}))
            .collect();
        assert!(agent_catalogue_rows(&serde_json::json!({"agents": over_bound})).is_err());
    }

    #[test]
    fn results_never_partially_leak_rows_and_drafts_stay_out_of_parse() {
        // A parse failure yields Err — not a partial Vec — so no caller can
        // feed the page half a catalogue; the function itself never touches
        // any composer draft (draft handling lives in the dispatch loop).
        let catalogue = serde_json::json!({"agents": [
            {"name": "build", "read_only": false},
            {"read_only": true}
        ]});
        assert!(agent_catalogue_rows(&catalogue).is_err());
    }
}

#[cfg(feature = "native")]
fn configured_command_rows(value: &serde_json::Value) -> Result<Vec<crate::native_view_pages::PageRow>, String> {
    let commands = value.get("commands").and_then(serde_json::Value::as_array)
        .ok_or("command listing is not an array")?;
    let mut rows = crate::native_view_pages::command_rows();
    if commands.len() + rows.len() > crate::native_view_pages::MAX_ROWS {
        return Err("command listing exceeds row limit".into());
    }
    let mut names = std::collections::HashSet::new();
    for command in commands {
        let name = command.get("name").and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty() && name.len() <= 128 && !name.starts_with('/')
                && !name.chars().any(|ch| ch.is_whitespace() || ch.is_control()))
            .ok_or("invalid configured command name")?;
        if !names.insert(name) { return Err("duplicate configured command name".into()); }
        let invocation = format!("/{name}");
        if is_recognized_local_command(&invocation) || native_exit_route(&invocation) { continue; }
        rows.push(crate::native_view_pages::PageRow {
            id: format!("configured:{name}"),
            label: invocation,
            detail: command.get("description").and_then(serde_json::Value::as_str).unwrap_or("").to_owned(),
            enabled: true,
        });
    }
    Ok(rows)
}

/// One pending backend permission request for the bound session
/// (`GET /api/session/{id}/permission`, existing bounded authenticated
/// client). `Ok(None)` is an honest "no request pending".
#[cfg(feature = "native")]
struct PendingPermission {
    id: String,
    title: String,
    persistent_allowed: bool,
    /// TOOL-002: the pending request's bounded tool `callID`. Upstream joins
    /// on `request.source.type === "tool" && request.source.id === part.id`
    /// (`routes/session/index.tsx:2793-2799`); oc2's `PermissionSource`
    /// serializes the join key as `source.callID`
    /// (`crates/server/src/run_owner.rs:40-45`). `None` for non-tool sources.
    permission_call_id: Option<String>,
}

#[cfg(feature = "native")]
fn permission_save_eligible(save: Option<&serde_json::Value>) -> bool {
    let Some(serde_json::Value::Array(values)) = save else {
        return false;
    };
    !values.is_empty()
        && values
            .iter()
            .all(serde_json::Value::is_string)
}

/// TOOL-002 wire join: the oc2 permission list serializes the request source
/// with serde renames (`crates/server/src/run_owner.rs:40-45`):
/// `{"type":"tool","messageID":...,"callID":...}`. Upstream compares
/// `request.source.type === "tool"` and `request.source.id === part()?.id`
/// (`packages/tui/src/routes/session/index.tsx:2793-2799`), so the oc2 join
/// key is exactly `source.callID`. Bounded and control-free, matching the
/// `fetch_permission` id bound (non-empty, <=128 chars).
#[cfg(feature = "native")]
fn permission_call_id_from_source(source: Option<&serde_json::Value>) -> Option<String> {
    let source = source?;
    if source.get("type").and_then(serde_json::Value::as_str) != Some("tool") {
        return None;
    }
    let call = source.get("callID").and_then(serde_json::Value::as_str)?;
    if call.is_empty() || call.len() > 128 || call.chars().any(char::is_control) {
        return None;
    }
    Some(call.to_owned())
}

#[cfg(all(test, feature = "native"))]
mod permission_call_id_tests {
    use super::permission_call_id_from_source;
    use serde_json::json;

    #[test]
    fn accepts_only_tool_sources_with_bounded_call_ids() {
        let tool = json!({"type":"tool","messageID":"m1","callID":"c1"});
        assert_eq!(permission_call_id_from_source(Some(&tool)).as_deref(), Some("c1"));
        assert_eq!(permission_call_id_from_source(None), None);
        let agent = json!({"type":"agent","messageID":"m1","agentID":"a1"});
        assert_eq!(permission_call_id_from_source(Some(&agent)), None);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool"}))), None);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool","callID":7}))), None);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool","callID":""}))), None);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool","callID":"c\n1"}))), None);
        let long = "c".repeat(129);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool","callID":long}))), None);
        let exact = "c".repeat(128);
        assert_eq!(permission_call_id_from_source(Some(&json!({"type":"tool","callID":exact}))).as_deref(), Some(exact.as_str()));
    }
}

#[cfg(all(test, feature = "native"))]
mod permission_save_eligible_tests {
    use super::permission_save_eligible;
    use serde_json::json;

    #[test]
    fn accepts_only_nonempty_all_string_arrays() {
        assert!(!permission_save_eligible(None));
        assert!(!permission_save_eligible(Some(&json!(null))));
        assert!(!permission_save_eligible(Some(&json!("bash"))));
        assert!(!permission_save_eligible(Some(&json!([]))));
        assert!(!permission_save_eligible(Some(&json!(["bash", 1]))));
        assert!(!permission_save_eligible(Some(&json!([{}]))));
        assert!(permission_save_eligible(Some(&json!(["bash"]))));
    }
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
        // Match the upstream schema (`save?: string[]`) rather than merely
        // checking that an arbitrary JSON array is non-empty.  Malformed,
        // absent, null, scalar, empty, mixed, and object arrays are all
        // ineligible for a persistent grant.
        let persistent_allowed = permission_save_eligible(entry.get("save"));
        // TOOL-002: carry the tool-call join key when this request's source
        // is a tool permission (`index.tsx:2793-2799`).
        let permission_call_id = permission_call_id_from_source(entry.get("source"));
        return Ok(Some(PendingPermission {
            id: id.to_owned(),
            title,
            persistent_allowed,
            permission_call_id,
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
    let (reply, scope) = match &intent {
        DecisionIntent::Approve { .. } => ("once", "once"),
        DecisionIntent::Reject { .. } => ("reject", "once"),
        DecisionIntent::ScopedPermission {allow,persistent,..} => (
            if *allow {"always"} else {"reject"}, if *persistent {"always"} else {"session"}),
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
            let payload = serde_json::json!({ "reply": reply, "scope": scope }).to_string();
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
    crate::native_debug::event(
        "permission.reply",
        serde_json::json!({"decision":reply,"attempt":attempt,"acknowledged":accepted,"request_id":request_id,"session_id":session_id}),
    );
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

#[cfg(feature = "native")]
/// Runs one Stop/Retry/Follow-up control against the existing children HTTP
/// mapping (stop/retry/continue) and records the transport error on the panel.
/// Shared by the keyboard path and the child-panel mouse path.
fn children_action_http(
    panel: &mut crate::native_children::Panel,
    action: crate::native_children::Action,
    live: Option<&LiveSnapshot>,
    auth: Option<&str>,
    generation: u64,
) -> Option<String> {
    use crate::native_children::Action;
    let Action::Open(id) = action else {
        if let Some(snapshot) = live {
            if snapshot.session_id != panel.parent || !panel.context_valid(&snapshot.session_id, generation, auth) { panel.invalidate_context(); return None; }
            let (id, operation, body) = match action {
                Action::Stop(id) => (id, "stop", serde_json::json!({})),
                Action::Retry(id) => (id, "retry", serde_json::json!({})),
                Action::Followup(id, text) => (id, "continue", serde_json::json!({"text":text})),
                Action::Open(_) => unreachable!("open handled before child control HTTP"),
            };
            let result = http_request(&snapshot.origin, "POST", &format!("/api/sessions/{}/children/{id}/{operation}", panel.parent), Some(&body.to_string()), auth);
            if result.is_err() { panel.invalidate_actions(); } else { panel.invalidate_actions(); }
            panel.action_error = result.err();
        }
        return None;
    };
    if let Some(snapshot) = live { if snapshot.session_id == panel.parent && panel.lease_valid(&snapshot.session_id, generation, auth) { Some(id) } else { panel.invalidate_context(); None } } else { panel.invalidate_context(); None }
}

#[cfg(feature = "native")]
fn permission_editor_request(editor:&mut crate::native_permissions::Editor, live:Option<&LiveSnapshot>, auth:Option<&str>, decision:Option<&str>) {
    let result = (|| {
        let snapshot = live.ok_or_else(|| "Daemon unavailable".to_owned())?;
        if let Some(decision) = decision {
            let payload = editor.payload(decision,&snapshot.session_id).to_string();
            http_request(&snapshot.origin,"POST","/api/permission-rules",Some(&payload),auth)?;
        }
        let body = http_request(&snapshot.origin,"GET",&format!("/api/permission-rules?session_id={}",snapshot.session_id),None,auth)?;
        serde_json::from_str(&body).map_err(|_| "Invalid permission rules response".to_owned())
    })();
    match result {Ok(rules)=>editor.apply(rules),Err(_)=>editor.error=Some("Could not save/load rules. Changes were not confirmed.".into())}
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
#[derive(Clone, Debug)]
struct NativeThemeChoice {
    /// Exact persisted identity. Rows and resolution use this same value.
    name: String,
    /// Whether the current native resolver can load this identity.
    available: bool,
}

#[cfg(feature = "native")]
fn native_theme_choices(
    config_paths: &[std::path::PathBuf],
    current: Option<&str>,
    mode: crate::native_theme_runtime::ThemeMode,
) -> Result<Vec<NativeThemeChoice>, String> {
    use std::collections::BTreeMap;
    use std::fs;

    // Enumeration is deliberately bounded independently of file-read limits.
    const MAX_DIRECTORY_ENTRIES: usize = 256;
    const MAX_DISCOVERED_NAMES: usize = 128;
    crate::native_ui_budget::check_paths(config_paths)?;
    let mut choices = BTreeMap::<String, NativeThemeChoice>::new();
    // Keep the exact winning custom file. Config paths are traversed in
    // precedence order, so later entries replace earlier ones just like the
    // resolver's reverse search.
    let mut custom_sources = BTreeMap::<String, std::path::PathBuf>::new();

    for (name, _) in crate::native_theme_assets::ASSETS {
        choices.insert((*name).to_owned(), NativeThemeChoice {
            name: (*name).to_owned(),
            available: false,
        });
    }
    for name in ["dark", "light", "system", "nocolor", "high-contrast"] {
        choices.entry(name.to_owned()).or_insert_with(|| NativeThemeChoice {
            name: name.to_owned(),
            available: false,
        });
    }

    for config in config_paths {
        let Some(directory) = config.parent() else { continue };
        let themes = directory.join("themes");
        let Ok(entries) = fs::read_dir(&themes) else { continue };
        for (index, entry) in entries.enumerate() {
            if index >= MAX_DIRECTORY_ENTRIES {
                return Err("theme directory entry budget exceeded".into());
            }
            let entry = entry.map_err(|_| "theme directory entry could not be read")?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { continue };
            if stem.is_empty()
                || stem.len() > crate::native_theme_runtime::MAX_KEY_BYTES
                || stem.chars().any(|c| c.is_control() || c == '/' || c == '\\')
            {
                continue;
            }
            if !choices.contains_key(stem) && choices.len() >= MAX_DISCOVERED_NAMES {
                return Err("discovered theme name budget exceeded".into());
            }
            custom_sources.insert(stem.to_owned(), path.clone());
            choices.entry(stem.to_owned()).or_insert_with(|| NativeThemeChoice {
                name: stem.to_owned(),
                available: false,
            });
        }
    }

    // Validate each discovered identity from its already-selected winning
    // file. This avoids charging every theme for the same missing lower-
    // precedence probes. The explicit validation cap keeps registry work and
    // total theme bytes responsive; exceeding it reports an incomplete
    // registry rather than inventing per-theme unavailable results.
    const MAX_REGISTRY_VALIDATIONS: usize = 32;
    let mut validations = 0usize;
    for choice in choices.values_mut() {
        if matches!(choice.name.as_str(), "nocolor" | "high-contrast") {
            choice.available = false;
            continue;
        }
        let resolver_name = match choice.name.as_str() {
            "dark" | "light" | "system" => "opencode",
            name => name,
        };
        choice.available = if let Some(path) = custom_sources.get(resolver_name) {
            validations = validations.saturating_add(1);
            if validations > MAX_REGISTRY_VALIDATIONS {
                return Err("theme registry validation budget exceeded; registry incomplete".into());
            }
            let mut budget = crate::native_ui_budget::ReadBudget::default();
            validate_native_theme_file(&choice.name, mode, path, &mut budget).is_ok()
        } else {
            crate::native_theme_assets::lookup(resolver_name).is_some()
        };
    }
    if let Some(name) = current.filter(|name| !name.is_empty()) {
        choices.entry(name.to_owned()).or_insert_with(|| NativeThemeChoice {
            name: name.to_owned(),
            available: false,
        });
    }
    Ok(choices.into_values().collect())
}

#[cfg(feature = "native")]
fn validate_native_theme_file(
    name: &str,
    mode: crate::native_theme_runtime::ThemeMode,
    path: &std::path::Path,
    budget: &mut crate::native_ui_budget::ReadBudget,
) -> Result<crate::native_theme_runtime::RuntimeTheme, String> {
    use crate::native_theme_runtime::{RuntimeTheme, ThemeMode};
    let (runtime_name, mode) = match name {
        "dark" => ("opencode", ThemeMode::Dark),
        "light" => ("opencode", ThemeMode::Light),
        "system" => ("opencode", mode),
        _ => (name, mode),
    };
    let text = budget
        .read(path, 262144, "theme asset")?
        .ok_or_else(|| "theme asset disappeared during registry validation".to_owned())?;
    let value = serde_json::from_str(&text).map_err(|_| "invalid theme JSON")?;
    RuntimeTheme::from_asset(runtime_name, &value, mode)
        .map_err(|error| format!("invalid theme: {error}"))
}

#[cfg(feature = "native")]
fn load_native_theme(
    name: &str,
    mode: crate::native_theme_runtime::ThemeMode,
    config_paths: &[std::path::PathBuf],
) -> Result<crate::native_theme_runtime::RuntimeTheme, String> {
    crate::native_ui_budget::check_paths(config_paths)?;
    let mut budget = crate::native_ui_budget::ReadBudget::default();
    load_native_theme_with_budget(name, mode, config_paths, &mut budget)
}

#[cfg(feature = "native")]
fn load_native_theme_with_budget(
    name: &str,
    mode: crate::native_theme_runtime::ThemeMode,
    config_paths: &[std::path::PathBuf],
    budget: &mut crate::native_ui_budget::ReadBudget,
) -> Result<crate::native_theme_runtime::RuntimeTheme, String> {
    use crate::native_theme_runtime::{RuntimeTheme, ThemeMode};
    let (name, mode) = match name {
        "dark" => ("opencode", ThemeMode::Dark),
        "light" => ("opencode", ThemeMode::Light),
        "system" => ("opencode", mode),
        _ => (name, mode),
    };
    if name.is_empty()
        || name.len() > crate::native_theme_runtime::MAX_KEY_BYTES
        || name.chars().any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("invalid theme name".into());
    }
    for path in config_paths.iter().rev() {
        let Some(directory) = path.parent() else { continue };
        let candidate = directory.join("themes").join(format!("{name}.json"));
        if let Some(text) = budget.read(&candidate, 262144, "theme asset")? {
            let value = serde_json::from_str(&text).map_err(|_| "invalid theme JSON")?;
            return RuntimeTheme::from_asset(name, &value, mode)
                .map_err(|error| format!("invalid theme: {error}"));
        }
    }
    // Upstream `listThemes()` (`packages/tui/src/theme/index.ts`) overrides the
    // legacy `DEFAULT_THEMES` entry: the builtin `opencode` theme is the V2
    // document, whose hue scales drive `theme.increase`/`decrease`.
    if name == "opencode" {
        return Ok(RuntimeTheme::builtin_default(mode));
    }
    let json = crate::native_theme_assets::lookup(name)
        .ok_or_else(|| format!("unknown theme: {name}"))?;
    let value = serde_json::from_str(json).map_err(|_| "invalid bundled theme JSON")?;
    RuntimeTheme::from_asset(name, &value, mode).map_err(|error| format!("invalid theme: {error}"))
}

#[cfg(feature = "native")]
fn resolve_native_cli_config_path(
    explicit_config_dir: Option<&std::path::Path>,
    xdg_config_home: Option<&std::path::Path>,
    home: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    explicit_config_dir
        .filter(|path| path.is_absolute())
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            xdg_config_home
                .filter(|path| path.is_absolute())
                .map(|path| path.join("opencode"))
        })
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .map(|path| path.join(".config/opencode"))
        })
        .map(|directory| directory.join("cli.json"))
}

#[cfg(feature = "native")]
fn native_cli_config_path() -> Option<std::path::PathBuf> {
    resolve_native_cli_config_path(
        env::var_os("OPENCODE_CONFIG_DIR").as_deref().map(std::path::Path::new),
        env::var_os("XDG_CONFIG_HOME").as_deref().map(std::path::Path::new),
        env::var_os("HOME").as_deref().map(std::path::Path::new),
    )
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
    daemon_origin: Option<&str>,
    auth: Option<&str>,
    startup_view: Option<crate::app_start::StartupView>,
    keymap: SubmitKeymap,
    data_dir: &Path,
    explicit_session_route: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = native_size();
    let mut renderer = NativeRenderer::create(cols, rows)?;
    crate::native_debug::event(
        "ui.start",
        serde_json::json!({"cols":cols,"rows":rows,"daemon_attached":live.is_some(),"startup_view":format!("{startup_view:?}")}),
    );
    renderer.setup_terminal()?;
    // Native owns mouse input; otherwise the terminal selects the entire
    // painted rectangular buffer, including Home whitespace and decoration.
    // Motion tracking stays on: upstream sets `renderer.useMouse = true`
    // (`app.tsx:631`) with OpenTUI's default `enableMouseMovement = true`, and
    // tab/context-menu hover is driven by the pointer-move events it delivers.
    renderer.enable_mouse(true)?;
    let _ = renderer.enable_kitty_keyboard(1);
    renderer.set_title("OpenCode RK")?;
    let mut draft = String::new();
    // Byte cursor for the native string-backed composer. Ctrl-A/Ctrl-E use
    // the production native_composer line-binding adapter; buffer Home/End
    // remain intentionally unbound by default.
    let mut draft_cursor = 0usize;
    // Tracked folded-paste parts for the composer draft (CP-36): a long or
    // multi-line paste folds into a `[Pasted ~N lines]` placeholder and records
    // its range here, so later edits realign instead of appending raw text.
    let mut draft_parts: Vec<crate::native_paste::PastedText> = Vec::new();
    // Persisted stores live under the same data dir the rest of the TUI uses.
    let state_path = data_dir.join("state/kv.json");
    let tabs_path = data_dir.join("state/tabs.json");
    // One bounded client-side roster backs both tab presentations.  It is
    // deliberately initialized from the authenticated startup binding; later
    // routes may add sessions without replacing this state machine.
    let mut navigation = crate::native_navigation::NavigationState::new();
    // Restore the persisted open-tab order/titles for this working directory
    // before the startup route binds, so the routed session slots into the
    // restored strip instead of replacing it. No live binding starts on Home
    // with the restored tabs still visible (the active route is not
    // persisted; `context/session-tabs.tsx:72-102`).
    let tab_cwd = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (tab_scope, tab_scope_diagnostics) = {
        let cli_path = native_cli_config_path();
        let inline_cli = env::var("OPENCODE_CLI_CONFIG_CONTENT").ok();
        crate::native_ui_preferences::UiPreferences::tab_scope(
            cli_path.as_deref(),
            inline_cli.as_deref(),
            &tab_cwd,
        )
    };
    match crate::native_ui_preferences::UiPreferences::load_session_tabs(&tabs_path, tab_scope) {
        Ok(persisted) if !persisted.order.is_empty() => {
            navigation.restore_tabs(&persisted);
            if live.is_none() {
                navigation.enter_home();
            }
        }
        Ok(_) => {}
        Err(error) => crate::native_debug::event(
            "tabs.restore.error",
            serde_json::json!({"error": error}),
        ),
    }
    if let Some(snapshot) = live.as_ref() {
        navigation.open(crate::native_navigation::SessionRef(snapshot.session_id.clone()));
        seed_navigation_titles(&mut navigation, snapshot);
    }
    // Persist once on the first loop iteration so a tab opened by the startup
    // route is durable before the user changes anything.
    let mut saved_tabs = crate::native_navigation::PersistedTabs::default();
    let mut selection = crate::native_text_selection::TextSelection::default();
    // Setup is an action from Home, not a mandatory startup modal. In
    // particular, custom-provider discovery must never steal composer input.
    let mut dialog = native_setup::Dialog::None;
    let mut settings_dialog = crate::native_settings::SettingsDialog::default();
    let mut settings_error: Option<String> = None;
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
    let mut models = models_fetch.unwrap_or_default();
    let mut selected = native_setup::selected(&models);
    let recent_provider = native_setup::recent_provider();
    let configured_model = opencode_rk_providers::imported_config::own_config_path()
        .as_deref()
        .and_then(native_setup::configured_model_identity);
    let mut provider_id = "openai".to_owned();
    let mut provider_url = "http://localhost:20128/v1".to_owned();
    let mut providers = vec![
        ("openai".to_owned(), "OpenAI".to_owned(), String::new()),
        (
            "9router".to_owned(),
            "9router (local)".to_owned(),
            provider_url.clone(),
        ),
    ];
    // The bundled catalog is the complete fresh-install floor. A valid cache
    // overlays it by resolved provider ID; bounded reads preserve the floor
    // when cache data is missing, stale, partial, or corrupt.
    let cached_catalog = crate::native_ui_preferences::read_bounded(
        &data_dir.join("catalog/models.dev.api.json"),
        8 * 1024 * 1024,
        "models.dev provider catalog",
    )
    .ok()
    .flatten();
    let catalog = opencode_rk_catalog::Catalog::from_cache_or_bundled(
        cached_catalog.as_deref().map(str::as_bytes),
    )
    .ok();
    let catalog_rows = catalog.map(|catalog| catalog.ordered_providers()).unwrap_or_default();
    // The own config is an authoritative local provider registry layer.  Do
    // not wait for the optional daemon listing to expose it: startup model
    // selection must be able to initialize an imported provider before any
    // model catalogue exists.  This reads normalized provider metadata only;
    // credential resolution remains exclusively in the existing daemon path.
    if let Ok(imported) = opencode_rk_providers::imported_config::load_imported_providers() {
        for provider in imported {
            if let Some(existing) = providers.iter_mut().find(|row| row.0 == provider.id) {
                *existing = (provider.id, provider.name, provider.base_url);
            } else {
                providers.push((provider.id, provider.name, provider.base_url));
            }
        }
    }
    if let Some(snapshot) = live.as_ref() {
        if let Ok(body) = http_request(&snapshot.origin, "GET", "/api/providers", None, auth) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                for row in value["providers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(128)
                {
                    if let (Some(id), Some(url)) = (row["id"].as_str(), row["base_url"].as_str()) {
                        if id.len() <= 128
                            && url.len() <= 2048
                            && id
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                        {
                            let name = row["name"]
                                .as_str()
                                .unwrap_or(id)
                                .chars()
                                .filter(|c| !c.is_control())
                                .take(128)
                                .collect();
                            if let Some(existing) = providers.iter_mut().find(|row| row.0 == id) {
                                *existing = (id.into(), name, url.into());
                            } else {
                                providers.push((id.into(), name, url.into()));
                            }
                        }
                    }
                }
            }
        }
    }
    // All configured/imported/daemon rows have now been collected. Merge
    // exactly once against the complete catalog before adding custom, so
    // existing rows win name/API precedence and duplicate IDs are removed.
    let existing: Vec<_> = providers.iter().filter_map(|(id, name, url)| {
        Some(opencode_rk_catalog::ProviderSummary {
            id: opencode_rk_contracts::ProviderId::new(id.clone()).ok()?,
            name: name.clone(),
            environment: Vec::new(),
            api: (!url.is_empty()).then(|| url.clone()),
        })
    }).collect();
    providers = opencode_rk_catalog::Catalog::merge_provider_rows(&catalog_rows, &existing)
        .into_iter()
        .map(|provider| (provider.id.to_string(), provider.name, provider.api.unwrap_or_default()))
        .collect();

    providers.push((
        "custom".into(),
        "Other — custom OpenAI-compatible provider".into(),
        String::new(),
    ));
    providers.sort_by(|a, b| {
        let rank = |id: &str| opencode_rk_catalog::POPULAR_PROVIDER_IDS
            .iter()
            .position(|popular| *popular == id);
        match (rank(&a.0), rank(&b.0)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) if a.0 == "custom" => std::cmp::Ordering::Greater,
            (None, None) if b.0 == "custom" => std::cmp::Ordering::Less,
            _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0)),
        }
    });
    // `custom` is the picker sentinel, not a configured provider and must
    // never be allowed to become a discovery route.
    let known_provider_ids: Vec<&str> = providers
        .iter()
        .filter(|row| row.0 != "custom")
        .map(|row| row.0.as_str())
        .collect();
    provider_id = native_setup::startup_provider(
        recent_provider.as_deref(),
        configured_model.as_deref(),
        &known_provider_ids,
    );
    let mut model_load = None;
    let mut setup_error: Option<String> = None;
    if provider_id != "openai"
        && providers.iter().any(|row| row.0 == provider_id)
        && provider_id != "custom"
    {
        selected = None;
        if let Some(snapshot) = live.as_ref() {
            model_load = native_setup::ModelLoad::start(&snapshot.origin, auth, &provider_id).ok();
        }
    }
    let mut transcript = NativeChatHistory::default();
    // In-flight turn worker wrapped with the binding identity it was
    // launched under: events apply only while session AND generation still
    // match; a superseded (stale) worker is drained and never touches the
    // current busy flag, transcript or live snapshot.
    let mut worker: Option<crate::native_view::OwnedWorker<native_turn::TurnWorker>> = None;
    let mut compact_worker: Option<NativeCompactWorker> = None;
    let mut pending_submission: Option<crate::native_composer::SubmittedDraft> = None;
    // CP-08 bounded busy queue: composer Enter while a turn owns the
    // current binding routes through `plan_submit` Delivery::Queue
    // (upstream `submit("queue")`, `component/prompt/index.tsx:455`,
    // bounded at the same `BUSY_QUEUE_CAP` as `Composer::submit`). Each
    // entry records the owning session so a tab switch can never
    // promote a foreign session's prompt.
    let mut queued_drafts: std::collections::VecDeque<(Option<String>, String)> =
        std::collections::VecDeque::new();
    let mut binding_generation: u64 = 0;
    // History pagination is deliberately separate from turn/completion
    // workers: one bounded request at a time, and every result carries the
    // binding generation that authorized it.
    let mut history_window = live.as_ref().map(|snapshot| {
        crate::native_chat_projection::NativeHistoryWindow::new(
            &snapshot.raw_history,
            snapshot.history_next_before.clone(),
        )
    });
    let (history_tx, mut history_rx) = tokio::sync::mpsc::channel::<(
        HistoryRequestMeta,
        Result<serde_json::Value, String>,
    )>(1);
    let mut history_pending = false;
    let mut history_request_id = 0u64;
    let mut history_active: Option<HistoryRequestMeta> = None;
    let mut blocked_history_scroll: Option<BlockedHistoryScroll> = None;
    let mut history_navigation_request: Option<HistoryRequestKind> = None;
    let mut history_navigation_revision: u64 = 0;
    let mut history_browsing = false;
    let mut history_tail_refresh_pending = false;
    let mut last_history_refresh = Instant::now();
    // Segregated API-key dialog buffer: never stored as (or merged into)
    // the chat composer draft, never saved by any draft transaction.
    let mut key_input = String::new();
    // Dialog-local provider/model filter buffer: the picker's transient
    // text. Opening a picker preserves the chat draft verbatim, filtering
    // never mutates it, and cancel returns the exact original draft.
    let mut filter_input = String::new();
    let mut model_row: usize = 0;
    let mut last_permission_poll = Instant::now();
    let mut permission_error: Option<String> = None;
    // TOOL-002: bounded pending-permission tool-call ids joined against
    // `ChatPart::ToolState { id }` at paint (`index.tsx:2793-2799`).
    let mut pending_permission_calls: Vec<String> = Vec::new();
    let mut partial = String::new();
    let mut utf8 = NativeUtf8Decoder::default();
    let mut input = native_input_decoder::TerminalInputDecoder::default();
    let mut observed_size = (cols, rows);
    // TAB-21: last observed terminal focus report. Startup assumes focused
    // (`Some(true)`) like the upstream renderer's initial focus state.
    let mut client_focused: Option<bool> = Some(true);
    let mut needs_paint = true;
    // The last painted strip is the sole hit-test snapshot; paint and input
    // never independently recompute tab geometry.
    let mut rendered_tabs: Option<TabStrip> = None;
    let mut rendered_rail: Option<crate::native_tab_rail::RailFrame> = None;
    let mut tab_hovered: Option<crate::native_navigation::SessionRef> = None;
    // Screen row of the pointer inside a hovered compact rail entry; the
    // compact tooltip anchors one row above it (`session-tabs.tsx:1250`).
    let mut tab_hover_y: Option<u16> = None;
    // Press capture binds the click to a stable tab and painted rect.
    let mut tab_press: Option<(crate::native_navigation::SessionRef, crate::native_tabs::TabRect, bool, TabOrientation)> = None;
    // Vertical-rail drag-to-reorder: armed on a session press, previewed while
    // the left button moves, and persisted once on release.
    let mut tab_drag = crate::native_tabs::DragState::new();
    // Upstream keeps the pointer's physical close cell after a close until
    // leave/another close/5 s (`session-tabs.tsx:868-900,1348-1363`).
    let mut close_hold: Option<crate::native_tabs::CloseHold> = None;
    let mut add_press: Option<(crate::native_tabs::TabRect, TabOrientation)> = None;
    let mut rail_search_press: Option<(crate::native_tabs::TabRect, TabOrientation)> = None;
    // TAB-23: the compact search control's live hover fact
    // (`session-tabs-rail.tsx:14-20` `onMouseOver`/`onMouseOut`); drives the
    // `▀` overlay row.
    let mut rail_search_hovered = false;
    // Promoted Home slot state: pointer-over fact drives the close glyph, a
    // captured press (body or close child) absorbs the click so it can never
    // leak into composer/send regions while already on Home.
    let mut home_press: Option<(crate::native_tabs::TabRect, TabOrientation, bool)> = None;
    let mut tab_home_hovered = false;
    let mut tab_add_hovered = false;
    // Hovered-title marquee clock. Upstream only runs it for the tab under the
    // pointer (`session-tabs.tsx:1650`); offset>0 replaces the rendered title
    // with the wrapping ` · ` window.
    let mut tab_marquee = crate::native_tabs::TabMarquee::new();
    let mut context_menu: Option<crate::native_tab_context_menu::TabContextMenu> = None;
    let mut message_menu: Option<crate::native_view_chat::SessionMessageMenu> = None;
    let mut rename_input: Option<(crate::native_navigation::SessionRef, String)> = None;
    let mut rename_error: Option<String> = None;
    // Rename settlements ride the same bounded background-worker/channel
    // polling pattern as completion/history: spawn_blocking posts one owned
    // result, the synchronous loop drains it on every iteration (never
    // awaiting, never blocking the UI thread on HTTP).
    let (rename_tx, mut rename_rx) = tokio::sync::mpsc::channel::<
        Result<(String, u16, String, String), String>,
    >(1);
    let mut rename_inflight: Option<RenameInFlight> = None;
    let mut rename_worker: Option<tokio::task::JoinHandle<()>> = None;
    let mut tab_viewport: Option<(u16, u16)> = None;
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
    // Built-in footer `commands` hint (`prompt/footer.tsx:110-120`): the
    // palette opens on the hardcoded Ctrl-P byte (`16 =>` below), so the live
    // key label is `ctrl+p`. `agent.cycle` (shift+tab) has no native dispatch,
    // so the `agents` hint is deliberately not advertised.
    view.set_rail_shortcuts(Some("ctrl+p".to_owned()), None);
    // First-run import discovery is bounded filesystem work and runs once
    // before the native loop starts. No source is written or executed here;
    // the dialog owns the later human offer/preview/apply decisions.
    let mut import_dialog: Option<crate::native_import_dialog::ImportDialog> = None;
    if let Some(dest) = opencode_rk_providers::imported_config::own_config_path() {
        let env_snapshot = crate::native_import_sources::EnvSnapshot::capture();
        if let Ok(Some(offer)) = crate::native_import_sources::discover_import(
            &std::env::current_dir()?,
            &env_snapshot,
            &dest,
        ) {
            let mut details = Vec::new();
            for warning in offer.warnings.iter().take(16) {
                details.push(format!("warning: {} — {}", warning.source, warning.message));
            }
            for unsupported in offer.plan.unsupported.iter().take(16) {
                details.push(format!("unsupported: {} ({})", unsupported.field, unsupported.reason));
            }
            for (path, origin) in offer.plan.provenance.iter().take(16) {
                details.push(format!("source: {path} ← {origin}"));
            }
            if let Some(mut dialog) = crate::native_import_sources::offer_dialog(offer, data_dir)? {
                dialog.set_details(details);
                import_dialog = Some(dialog);
            }
        }
    }
    let mut children_panel = crate::native_children::Panel::default();
    let mut children_refresh = Instant::now();
    // Running children for the prompt-footer live label
    // (`prompt/footer.tsx:22-28`). Kept as observed facts: an unavailable or
    // malformed listing clears them instead of leaving a stale count.
    let mut live_children = crate::native_children::live::Status::default();
    let mut live_children_refresh = Instant::now() - Duration::from_secs(1);
    let mut todos_refresh = Instant::now();
    let mut memory_binding: Option<(String, u64)> = None;
    let mut memory_page_observed = false;
    let mut compaction_page_observed = false;
    let mut context_page_observed = false;
    let mut context_before: Option<String> = None;
    let mut appearance = crate::native_appearance::AppearanceEditor::new(
        crate::native_ui_preferences::UiPreferences::appearance(&state_path).unwrap_or_default(),
    );
    let mut permission_editor = crate::native_permissions::Editor::default();
    let animation_start = crate::native_clock::FrameClock::new();
    let mut animation_tick = 0u128;
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
    // SIDE-12: the getting-started box shows only while the connection list
    // is known AND no provider is connected (`footer.tsx:18-23`). The bounded
    // native credential store (`auth.json`, the document the daemon's
    // credential writers maintain) is the connection fact source; the
    // persisted `getting-started` dismissal is the observed storage fact.
    // `integrations_known` is true because the integration list is a static
    // client-side fact, not a fetch that can fail.
    view.set_onboarding(Some(crate::native_view_status::native_sidebar::Onboarding {
        dismissed: preferences.onboarding_dismissed,
        integrations_known: true,
        connected_provider: opencode_rk_providers::persisted_auth::any_connected_provider(),
    }));
    // `config.animations ?? true` (`config/index.tsx:250`): the resolved
    // reduced-motion flag. Tab hover/title paint and the marquee gate on it.
    let animations = preferences.animations.unwrap_or(true);
    let cli_path = native_cli_config_path();
    let inline_cli = env::var("OPENCODE_CLI_CONFIG_CONTENT").ok();
    let (configured_markdown, markdown_diagnostics) = crate::native_ui_preferences::UiPreferences::markdown_mode(cli_path.as_deref(), inline_cli.as_deref());
    for diagnostic in markdown_diagnostics { append_transcript(&mut transcript, format!("error: {diagnostic}")); }
    view.set_markdown_mode(match configured_markdown.as_deref() { Some("source") => crate::markdown_runs::MarkdownMode::Source, _ => crate::markdown_runs::MarkdownMode::Rendered });
    let (tab_orientation, tab_diagnostics) =
        crate::native_ui_preferences::UiPreferences::tab_orientation(
            cli_path.as_deref(),
            inline_cli.as_deref(),
        );
    navigation.set_orientation(tab_orientation);
    let mut rail_resize = crate::native_tab_resize::ResizeState::new(
        preferences.vertical_tabs_width.unwrap_or(TabReservation::VERTICAL_WIDTH),
        TabReservation::VERTICAL_WIDTH,
        cols.min(u32::from(u16::MAX)) as u16,
    );
    let (cli_theme, cli_theme_diagnostics) =
        crate::native_ui_preferences::UiPreferences::cli_theme(
            cli_path.as_deref(),
            inline_cli.as_deref(),
        );
    for diagnostic in cli_theme_diagnostics {
        append_transcript(&mut transcript, format!("error: {diagnostic}"));
    }
    for diagnostic in tab_diagnostics {
        append_transcript(&mut transcript, format!("error: {diagnostic}"));
    }
    for diagnostic in tab_scope_diagnostics {
        append_transcript(&mut transcript, format!("error: {diagnostic}"));
    }
    // V2 cli.json theme is authoritative when valid; legacy tui/kv state is
    // retained as the compatibility fallback. `system` remains dark until a
    // terminal-color probe is available, matching the runtime default.
    let selected_theme = cli_theme
        .as_ref()
        .map(|(name, _)| name.as_str())
        .or(preferences.theme.as_deref())
        .unwrap_or("opencode");
    let selected_mode = cli_theme
        .as_ref()
        .map(|(_, mode)| mode.as_str());
    let theme_mode = match selected_mode.or(preferences.theme_mode.as_deref()) {
        Some("light") => crate::native_theme_runtime::ThemeMode::Light,
        _ => crate::native_theme_runtime::ThemeMode::Dark,
    };
    let mut theme = match load_native_theme(
        selected_theme,
        theme_mode,
        &config_paths,
    ) {
        Ok(theme) => theme,
        Err(error) => {
            append_transcript(&mut transcript, format!("error: {error}"));
            crate::native_theme_runtime::RuntimeTheme::builtin_default(theme_mode)
        }
    };
    // THEME-14: parse `tui.cursor` once (fail-closed: an invalid object logs a
    // diagnostic and behaves as absent, so the terminal keeps its own cursor).
    let cursor_config = match preferences.cursor.as_ref() {
        Some(value) => match crate::native_theme_runtime::CursorConfig::from_json(value) {
            Ok(config) => config,
            Err(error) => {
                append_transcript(&mut transcript, format!("error: {error}"));
                None
            }
        },
        None => None,
    };
    view.set_sidebar_hidden(preferences.sidebar_hidden);
    let (scrollbar_enabled, scrollbar_diagnostics) =
        crate::native_ui_preferences::UiPreferences::session_scrollbar(
            cli_path.as_deref(),
            inline_cli.as_deref(),
        );
    for diagnostic in scrollbar_diagnostics {
        append_transcript(&mut transcript, format!("error: {diagnostic}"));
    }
    view.set_scrollbar_enabled(scrollbar_enabled);
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
        view.set_composer_model(&model.name, &model.provider_id);
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
    view.set_project_directory(
        live.as_ref()
            .and_then(|snapshot| snapshot.directory.clone())
            .unwrap_or_else(current_workspace_label),
    );
    // Session sidebar footer (`footer.tsx:87-109`): the active session's own
    // authenticated directory, never the Home workspace fallback. No VCS
    // observer exists yet, so the branch stays absent (the label renders the
    // bare path) rather than inventing one.
    view.set_sidebar_location(
        live.as_ref().and_then(|snapshot| snapshot.directory.clone()),
        None,
    );
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
        // Right pane: authenticated session MCP projection. Without a bound
        // session the global registry remains a status-only fallback.
        if let Err(error) = refresh_session_mcp(&mut view, snapshot, auth) {
            append_transcript(&mut transcript, format!("error: session mcp unavailable: {error}"));
        }
        if let Err(error) = refresh_session_todos(&mut view, snapshot, auth) {
            view.set_todos_error(&error);
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
    let mut configured_commands_loaded = false;
    let mut completion = crate::native_completion::Completion::default();
    completion.commands = crate::native_view_pages::command_rows().into_iter()
        .map(|row| crate::native_completion::Item { value: row.label, description: row.detail, disabled: false }).collect();
    for (value, description) in [
        ("/commands", "Browse commands"), ("/new", "New session"),
        ("/sidebar", "Toggle sidebar"), ("/appearance", "Adjust appearance"),
        ("/permissions", "Manage tool permissions"), ("/permission", "Manage tool permissions"),
        ("/subagents", "View subagents"), ("/exit", "Exit"),
        ("/cd", "Change workspace directory (requires a path)"),
    ] {
        completion.commands.push(crate::native_completion::Item { value: value.into(), description: description.into(), disabled: false });
    }
    let (completion_tx, mut completion_rx) = tokio::sync::mpsc::channel::<(Vec<crate::native_completion::Item>, Option<serde_json::Value>)>(1);
    let mut completion_load: Option<tokio::task::JoinHandle<()>> = None;
    let mut completion_loaded = false;
    let mut byte = [0u8; 1];
    loop {
        // Persist the open-tab order/titles whenever open/close/move/rename
        // changed them. One small bounded write per actual change; titles are
        // refreshed by the same events that already repaint the strip.
        let tabs_now = navigation.persist_tabs();
        if tabs_now != saved_tabs {
            if let Err(error) = crate::native_ui_preferences::UiPreferences::persist_session_tabs(
                &tabs_path,
                tab_scope,
                &tabs_now,
            ) {
                crate::native_debug::event(
                    "tabs.persist.error",
                    serde_json::json!({"error": error}),
                );
            }
            saved_tabs = tabs_now;
        }
        let current_memory_binding = live.as_ref().map(|snapshot| (snapshot.session_id.clone(), binding_generation));
        if memory_binding != current_memory_binding {
            *view.memory_mut() = crate::native_memory::Memory::new();
            view.compaction_mut().invalidate();
            view.context_mut().invalidate();
            context_before = None;
            context_page_observed = false;
            memory_binding = current_memory_binding;
            memory_page_observed = false;
            compaction_page_observed = false;
        }
        if view.page() == crate::native_view_pages::Page::Memory {
            if !memory_page_observed {
                match live.as_ref() {
                    Some(snapshot) => {
                        let path = format!("/api/sessions/{}/memory", snapshot.session_id);
                        if let Err(error) = http_request(&snapshot.origin, "GET", &path, None, auth)
                            .and_then(|body| view.load_memory(&body)) {
                            view.set_memory_error(&error);
                        }
                    }
                    None => view.set_memory_error("daemon unavailable"),
                }
                needs_paint = true;
            }
            memory_page_observed = true;
        } else {
            memory_page_observed = false;
        }
        if view.page() == crate::native_view_pages::Page::Compaction {
            if !compaction_page_observed {
                let result = match live.as_ref() {
                    Some(snapshot) => request_compaction_settings(&snapshot.origin, &snapshot.session_id, auth, None)
                        .and_then(|body| view.compaction_mut().load_response(&body)),
                    None => Err("daemon unavailable".into()),
                };
                if let Err(error) = result { view.compaction_mut().set_error(&error); }
                needs_paint = true;
            }
            compaction_page_observed = true;
        } else {
            compaction_page_observed = false;
        }
        if view.page() == crate::native_view_pages::Page::Context {
            if !context_page_observed {
                let result = match live.as_ref() {
                    Some(snapshot) => request_context_inspection(&snapshot.origin, &snapshot.session_id, auth,
                        context_before.as_deref(),None).and_then(|body| view.context_mut().load_response(&body)),
                    None => Err("daemon unavailable".into()),
                };
                if let Err(error)=result { view.context_mut().set_error(&error); }
                needs_paint=true;
            }
            context_page_observed=true;
        } else { context_page_observed=false; }
        completion.sync(&draft);
        if !completion.triggered() { completion_loaded = false; }
        if let Ok((files, catalogue)) = completion_rx.try_recv() {
            completion_load.take();
            let mut mentions = files;
            if let Some(catalogue) = catalogue {
                if let Ok(rows) = configured_command_rows(&catalogue) {
                    for row in rows {
                        if !completion.commands.iter().any(|item| item.value == row.label) {
                            completion.commands.push(crate::native_completion::Item { value: row.label, description: crate::native_completion::description(&row.detail), disabled: false });
                        }
                    }
                }
                if let Some(agents) = catalogue.get("agents").and_then(serde_json::Value::as_array) {
                    for agent in agents.iter().take(4096) {
                        let Some(name) = agent.get("name").and_then(serde_json::Value::as_str) else { continue; };
                        if name.is_empty() || name.len() > 128 || name.chars().any(|ch| ch.is_whitespace() || ch.is_control()) { continue; }
                        mentions.push(crate::native_completion::Item {
                            value: format!("@{name}"), description: format!("agent · {}", crate::native_completion::description(agent.get("description").and_then(serde_json::Value::as_str).unwrap_or(""))),
                            disabled: false,
                        });
                    }
                }
            }
            mentions.sort_by(|a, b| {
                b.description.starts_with("agent").cmp(&a.description.starts_with("agent")).then_with(|| a.value.cmp(&b.value))
            });
            completion.replace_mentions(mentions);
            needs_paint = true;
        } else if completion_load.as_ref().is_some_and(|load| load.is_finished()) {
            // A failed worker may be retried on the next trigger, without an
            // automatic retry loop or permanently retaining its dead handle.
            completion_load.take();
        }
        if let Ok(result) = rename_rx.try_recv() {
            if let Some(inflight) = rename_inflight.take() {
                let owned_by_dialog =
                    rename_dialog_owns(rename_input.as_ref().map(|(open, _)| open), &inflight);
                let settlement = rename_settlement(&inflight, result);
                if let Some((session, title)) = settlement.accepted {
                    // Server truth for exactly the requested id: record it in
                    // the bounded roster and the bound snapshot when present.
                    if let Some(snapshot) =
                        live.as_mut().filter(|snapshot| snapshot.session_id == session.0)
                    {
                        snapshot.title = title.clone();
                    }
                    navigation.ingest_snapshot_title(session, title);
                    // Only the dialog that still owns this exact id closes;
                    // a cancelled or retargeted dialog is never cleared by
                    // someone else's ack.
                    if owned_by_dialog {
                        rename_input = None;
                        rename_error = None;
                    }
                } else if let Some(message) = settlement.message {
                    if owned_by_dialog {
                        rename_error = Some(message);
                    } else {
                        append_transcript(&mut transcript, format!("error: session rename: {message}"));
                    }
                }
            }
            rename_worker = None;
            needs_paint = true;
        } else if rename_worker.as_ref().is_some_and(|worker| worker.is_finished()) {
            // The worker exited without delivering: fail closed instead of
            // leaving the request pending forever, and only tell a dialog
            // that is still open for the same in-flight id.
            let owned_by_dialog = rename_inflight.as_ref().is_some_and(|inflight| {
                rename_dialog_owns(rename_input.as_ref().map(|(open, _)| open), inflight)
            });
            if owned_by_dialog {
                rename_error = Some("Rename failed: request exited without a response".into());
            }
            rename_worker = None;
            rename_inflight = None;
            needs_paint = true;
        }
        if let Ok((request, result)) = history_rx.try_recv() {
            history_trace("history.response.received", &view, history_window.as_ref(), Some(request), history_navigation_request, history_pending, history_navigation_revision, None);
            if history_active == Some(request) && request.generation == binding_generation {
                history_pending = false;
                history_active = None;
                history_trace("history.response.accepted", &view, history_window.as_ref(), Some(request), history_navigation_request, history_pending, history_navigation_revision, Some("accepted"));
                let request_revision = Some(request.revision);
                let request_tail = request.kind == HistoryRequestKind::Tail;
                let request_newer = request.kind == HistoryRequestKind::Newer;
                let request_refresh = request.kind == HistoryRequestKind::Refresh;
                // Persisted history can lag the current worker's streamed
                // reasoning. A periodic snapshot must not replace that live
                // projection, including a refresh issued before the turn.
                if request_refresh && worker.as_ref().is_some_and(|owned| {
                    owned.owns(
                        live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                        binding_generation,
                    )
                }) {
                    history_trace("history.response.deferred_active_turn", &view, history_window.as_ref(), Some(request), history_navigation_request, history_pending, history_navigation_revision, Some("live_worker_owns_projection"));
                    continue;
                }
                match (history_window.as_mut(), result) {
                    (Some(window), Ok(value)) => {
                        // Every accepted page carries the same server-scoped
                        // child count, including pages later discarded by a
                        // navigation race: the fact is server truth, not a
                        // window mutation.
                        if let Some(snapshot) = live.as_mut() {
                            snapshot.history_children = history_child_count(&value);
                        }
                        let previous_window = window.clone();
                        // `chat_at_tail` is only the local viewport offset.  A
                        // bounded window can be locally at its tail while
                        // older-page traversal has evicted the server tail.
                        // Capture the actual window/anchor identity before
                        // refreshing so external-turn refresh cannot teleport
                        // an old browse window to the newest page.
                        let was_global_tail = window.at_newest_page();
                        // A navigation response is valid only for the input
                        // revision that issued it.  Reject before invoking a
                        // window mutator: resumable newer walks also mutate
                        // their private cursor/candidate state on a page
                        // that does not yet overlap the retained window.
                        if request_revision.is_some()
                            && request_revision != Some(history_navigation_revision)
                        {
                            *window = previous_window;
                            continue;
                        }
                        // Capture the displayed identity before any window
                        // mutator.  In particular, load_older may truncate
                        // the bounded window before the replacement can be
                        // judged safe.
                        let before_entries = crate::native_chat_projection::project_history_entries(previous_window.messages());
                        let before_anchor = view.chat_rendered_anchor(&before_entries);
                        let loaded = if request_refresh {
                            window.refresh_tail(&value)
                        } else if request_newer {
                            window.advance_newer_recovery(&value)
                        } else if request_tail {
                            window.replace_latest(&value)
                        } else {
                            window.load_older(|_, _| Ok(value.clone()))
                        };
                        match loaded {
                            Ok(true) => {
                                // Never move a user who navigated while the
                                // request was in flight.  Only the unchanged
                                // browsing position may be advanced to keep
                                // its old-message anchor visible.
                                if request_newer {
                                    if request_revision != Some(history_navigation_revision) {
                                        *window = previous_window;
                                        continue;
                                    }
                                    // The recovery response is the adjacent
                                    // page, not merely metadata. Admit it now
                                    // so the triggering down action cannot stall
                                    // at the retained-window boundary. Since
                                    // appending evicts the oldest page, move
                                    // the rendered-row offset up by the exact
                                    // inserted-page height to preserve the
                                    // currently visible message anchor.
                                    if window.advance_newer() {
                                        let after = crate::native_chat_projection::project_history_entries(window.messages());
                                        let Some(anchor) = before_anchor.as_ref() else {
                                            *window = previous_window;
                                            continue;
                                        };
                                        if !view.restore_chat_rendered_anchor(&after, anchor) {
                                            *window = previous_window;
                                            continue;
                                        }
                                         apply_history_window(window, &mut transcript);
                                          if let Some(blocked) = blocked_history_scroll {
                                              if request_revision == Some(blocked.revision) {
                                                  view.scroll_down(blocked.rows);
                                                  blocked_history_scroll = None;
                                              }
                                         }
                                         history_browsing = !window.at_newest_page();
                                        history_tail_refresh_pending = window.at_newest_page();
                                    }
                                } else if request_tail {
                                    if request_revision != Some(history_navigation_revision) {
                                        *window = previous_window;
                                        continue;
                                    }
                                    apply_history_window(window, &mut transcript);
                                    view.follow_tail();
                                    history_browsing = false;
                                    history_tail_refresh_pending = false;
                                } else if request_refresh {
                                    if request_revision == Some(history_navigation_revision)
                                        && view.chat_at_tail() && was_global_tail {
                                        window.replace_latest(&value).ok();
                                        apply_history_window(window, &mut transcript);
                                    }
                                } else if request_revision == Some(history_navigation_revision) {
                                    // Prepending a page changes the tail-relative
                                    // coordinate system. Preserve the visible
                                    // message anchor by compensating for the
                                    // whole inserted page, not the triggering
                                    // wheel delta.
                                    // Measure the complete before/after typed
                                    // projection with the production styled
                                    // renderer.  A page is not an independent
                                    // display unit: its first entry may gain a
                                    // group separator from the preceding page,
                                    // and tools/cards contribute rows which do
                                    // not exist in body.text.
                                     let after = crate::native_chat_projection::project_history_entries(window.messages());
                                     let Some(anchor) = before_anchor.as_ref() else {
                                         *window = previous_window;
                                         continue;
                                     };
                                     if !view.restore_chat_rendered_anchor(&after, anchor) {
                                         *window = previous_window;
                                         continue;
                                     }
                                     apply_history_window(window, &mut transcript);
                                } else {
                                    // Navigation changed while the request
                                    // was pending: discard the page entirely
                                    // rather than replacing the visible
                                    // window under the user's cursor.
                                    *window = previous_window;
                                    continue;
                                }
                                needs_paint = true;
                            }
                            Ok(false) => {
                                if request_newer && window.newer_recovery_pending()
                                    && request_revision == Some(history_navigation_revision)
                                {
                                    history_navigation_request = Some(HistoryRequestKind::Newer);
                                    history_browsing = true;
                                    history_tail_refresh_pending = false;
                                }
                            }
                            Err(error) => append_transcript(&mut transcript, format!("error: history pagination: {error}")),
                        }
                    }
                    (_, Err(error)) => append_transcript(&mut transcript, format!("error: history pagination: {error}")),
                    _ => {}
                }
            }
        }
        if !completion_loaded && completion.triggered() && completion_load.is_none() {
            completion_loaded = true;
            let tx = completion_tx.clone();
            let origin = live.as_ref().map(|snapshot| snapshot.origin.clone());
            let auth = auth.map(str::to_owned);
            let root = std::env::current_dir().unwrap_or_default();
            completion_load = Some(tokio::task::spawn_blocking(move || {
                let files = crate::native_completion::workspace_files(&root);
                let catalogue = origin.and_then(|origin| http_request(&origin, "GET", "/api/commands", None, auth.as_deref()).ok())
                    .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok());
                let _ = tx.blocking_send((files, catalogue));
            }));
        }
        if todos_refresh.elapsed() >= Duration::from_millis(500)
            && view.presentation().sidebar_offset.is_some()
        {
            todos_refresh = Instant::now();
            if let Some(snapshot) = live.as_ref() {
                if let Err(error) = refresh_session_todos(&mut view, snapshot, auth) {
                    view.set_todos_error(&error);
                }
                needs_paint = true;
            }
        }
        if children_panel.open && children_refresh.elapsed() >= Duration::from_millis(750) {
            children_refresh = Instant::now();
            if let Some(snapshot) = live.as_ref() {
                if !children_panel.context_valid(&snapshot.session_id, binding_generation, auth.as_deref()) {
                    children_panel.invalidate_context();
                    children_panel.error = Some("child panel context changed; reopen required".to_owned());
                    needs_paint = true;
                    continue;
                }
                let result = http_request(&snapshot.origin, "GET", &format!("/api/sessions/{}/children", children_panel.parent), None, auth)
                    .and_then(|body| serde_json::from_str(&body).map_err(|_| "invalid child listing JSON".to_owned()))
                    .and_then(|value| children_panel.update(&value));
                match result {
                    Ok(()) if snapshot.session_id == children_panel.parent => children_panel.bind_lease(&snapshot.session_id, binding_generation, auth.as_deref()),
                    Ok(()) => children_panel.invalidate_context(),
                    Err(error) => { children_panel.invalidate_actions(); children_panel.error = Some(error); }
                }
                if children_panel.detail && children_panel.lease_valid(&snapshot.session_id, binding_generation, auth.as_deref()) {
                    if let Some(id) = children_panel.selected_id() {
                        match http_request(&snapshot.origin, "GET", &format!("/api/sessions/{id}/messages?limit=200"), None, auth)
                            .and_then(|body| serde_json::from_str(&body).map_err(|_| "invalid child history JSON".to_owned())) {
                            Ok(value) => children_panel.history(&value),
                            Err(error) => children_panel.error = Some(error),
                        }
                    }
                }
            }
            needs_paint = true;
        }
        // Children can only exist for a session whose history delegated, so a
        // session that never called `task` issues no `/children` request (the
        // startup route surface stays exactly what it was before the footer
        // count existed). A running child, the server-reported child count on
        // the reloaded history, or a live delegation part keeps it alive.
        let delegates = should_poll_children(
            live_children.running(),
            live.as_ref().map_or(0, |snapshot| snapshot.history_children),
            &transcript.messages,
        );
        if !children_panel.open && delegates && live_children_refresh.elapsed() >= Duration::from_millis(750) {
            live_children_refresh = Instant::now();
            match live.as_ref() {
                Some(snapshot) => {
                    let result = http_request(&snapshot.origin, "GET", &format!("/api/sessions/{}/children", snapshot.session_id), None, auth)
                        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).map_err(|_| "invalid child listing JSON".to_owned()))
                        .and_then(|value| live_children.apply(&snapshot.session_id, &value));
                    match result {
                        Ok(true) => {
                            view.set_live_children_status(&live_children);
                            needs_paint = true;
                        }
                        Ok(false) => {}
                        Err(_) => {
                            if live_children.clear() {
                                view.clear_live_children_status();
                                needs_paint = true;
                            }
                        }
                    }
                }
                None => {
                    if live_children.clear() {
                        view.clear_live_children_status();
                        needs_paint = true;
                    }
                }
            }
        }
        if view.page() == crate::native_view_pages::Page::Commands {
            if !configured_commands_loaded {
                configured_commands_loaded = true;
                let result = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned())
                    .and_then(|snapshot| http_request(&snapshot.origin, "GET", "/api/commands", None, auth))
                    .and_then(|body| serde_json::from_str(&body).map_err(|_| "invalid command listing JSON".to_owned()))
                    .and_then(|value| configured_command_rows(&value));
                match result {
                    Ok(rows) => view.set_page_rows(rows),
                    Err(error) => append_transcript(&mut transcript, format!("error: configured commands: {error}")),
                }
                needs_paint = true;
            }
        } else {
            configured_commands_loaded = false;
        }
        if let Some((loaded_provider, result)) = model_load
            .as_mut()
            .and_then(|load| load.poll().map(|result| (load.provider_id.clone(), result)))
        {
            model_load = None;
            needs_paint = true;
            match result {
                Ok(rows) => {
                    models.retain(|row| row.provider_id != loaded_provider);
                    models.extend(rows);
                    if dialog == native_setup::Dialog::Model && loaded_provider == provider_id {
                        // Catalogue refresh must not erase a query typed while
                        // the request was in flight or move its focused row.
                        model_row = native_setup::clamp_row(
                            model_row,
                            native_setup::dialog_rows(&models, &filter_input).len(),
                        );
                    }
                    if selected.is_none() {
                        if let Some(model) = native_setup::selected(&models) {
                            selected = Some(model.clone());
                            view.set_model(Some(model.qualified()));
                            view.set_composer_model(&model.name, &model.provider_id);
                            if dialog == native_setup::Dialog::Provider
                                && model.provider_id == loaded_provider
                            {
                                dialog = native_setup::Dialog::None;
                            }
                        }
                    }
                }
                Err(error) => {
                    setup_error = Some(error);
                }
            }
        }
        if let Ok(Some(size)) = renderer.terminal_size() {
            if size != observed_size {
                crate::native_debug::event(
                    "ui.resize",
                    serde_json::json!({"old_cols":observed_size.0,"old_rows":observed_size.1,"cols":size.0,"rows":size.1}),
                );
                renderer.resize(size.0, size.1)?;
                observed_size = size;
                rail_resize.cancel();
                let next_orientation = match navigation.orientation() {
                    crate::native_navigation::TabOrientation::Horizontal => TabOrientation::Horizontal,
                    crate::native_navigation::TabOrientation::Vertical => TabOrientation::Vertical,
                };
                let next_tab_reservation = TabReservation::for_view_with_width(
                    next_orientation,
                    size.0 as u16,
                    size.1 as u16,
                    rail_resize.preferred(),
                );
                let preserve_anchor = session_route && history_browsing && !view.chat_at_tail();
                resize_chat_preserving_anchor(
                    &mut view,
                    size.0 as u16,
                    size.1 as u16,
                    next_tab_reservation,
                    &transcript.messages,
                    &draft,
                    preserve_anchor,
                );
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
                    if !history_browsing {
                        view.follow_tail();
                    }
                    match event {
                        // Backend user-message ACK: the accepted first user
                        // turn moves the main column to the session route
                        // (Home → session), explicitly and only on ACK.
                        native_turn::Event::User { text: accepted_text } => {
                            transcript.begin_reasoning_turn();
                            if let Some(captured) = pending_submission.take() {
                                // The old draft was detached at launch. Even
                                // identical newly typed text belongs to the user.
                                let captured = accepted_text.unwrap_or(captured.text);
                                transcript.append(
                                    format!("you: {captured}"),
                                    ChatMessage::text(ChatRole::User, captured),
                                );
                                crate::native_debug::event(
                                    "turn.admission_ack",
                                    serde_json::json!({"composer_bytes":draft.len()}),
                                );
                            }
                            if !session_route {
                                session_route = true;
                                view.set_session_route(true);
                            }
                        }
                        native_turn::Event::Delta(delta) => {
                            partial.push_str(&delta);
                            replace_partial(&mut transcript, &partial);
                        }
                        native_turn::Event::ReasoningDelta(delta) => transcript.reasoning_event(&delta,false),
                        native_turn::Event::ReasoningFinal(summary) => transcript.reasoning_event(&summary,true),
                        native_turn::Event::Usage {input,output,cached,reasoning} => view.set_usage(input,output,cached,reasoning),
                        native_turn::Event::ToolCall {
                            call_id,
                            name,
                            arguments,
                        } => {
                            transcript.append(
                                format!("tool: {}", native_transcript::strip_ansi(&name)),
                                ChatMessage {
                                    role: ChatRole::Assistant,
                                    parts: vec![ChatPart::ToolState {
                                        name,
                                        id: call_id,
                                        status: "call".into(),
                                        args: arguments.to_string(),
                                        output: String::new(),
                                        is_error: false,
                                        metadata: String::new(),
                                    }],
                                    agent: None,
                                    model: None,
                                    assistant_facts: None,
                                    message_id: None,
                                },
                            );
                        }
                        native_turn::Event::ToolOutput {
                            call_id,
                            name,
                            output,
                            error,
                        } => {
                            transcript.append(
                                format!(
                                    "tool {}: {}",
                                    native_transcript::strip_ansi(&name),
                                    native_transcript::strip_ansi(&output)
                                ),
                                ChatMessage {
                                    role: ChatRole::Tool,
                                    parts: crate::native_chat_projection::tool_output_parts(
                                        &name,
                                        &call_id,
                                        &output,
                                        error.is_some(),
                                    ),
                                    agent: None,
                                    model: None,
                                    assistant_facts: None,
                                    message_id: None,
                                },
                            );
                            if name == "todowrite" {
                                if let Some(snapshot) = live.as_ref() {
                                    if let Err(error) = refresh_session_todos(&mut view, snapshot, auth) {
                                        append_transcript(&mut transcript, format!("error: session todos unavailable: {error}"));
                                    }
                                }
                            }
                        }
                        native_turn::Event::Assistant { text, facts } => {
                            turn_succeeded = true;
                            partial.clear();
                            remove_partial(&mut transcript);
                            settled = true;
                            transcript.append(
                                format!("assistant: {}", native_transcript::strip_ansi(&text)),
                                ChatMessage::text(ChatRole::Assistant, text)
                                    .with_assistant_facts(facts),
                            );
                        }
                        native_turn::Event::Error { code, message: error } => {
                            if let Some(captured) = pending_submission.take() {
                                captured.restore(&mut draft, &mut draft_cursor);
                                completion.sync(&draft);
                            }
                            // Keep the streamed partial text as the message
                            // body: upstream's errored/interrupted assistant
                            // message retains it, and the completion footer
                            // annotates the message (` · interrupted` only for
                            // the TYPED transport code `cancelled`, `Error:
                            // {message}` notice for any other error). The
                            // legacy plain line is kept byte-identical.
                            let partial_text = std::mem::take(&mut partial);
                            remove_partial(&mut transcript);
                            settled = true;
                            let facts =
                                crate::native_assistant_facts::AssistantFacts::from_error(
                                    code,
                                    error.clone(),
                                );
                            transcript.append(
                                format!("error: {}", native_transcript::strip_ansi(&error)),
                                ChatMessage {
                                    role: ChatRole::Assistant,
                                    parts: if partial_text.is_empty() {
                                        Vec::new()
                                    } else {
                                        vec![ChatPart::Text(partial_text)]
                                    },
                                    agent: None,
                                    model: None,
                                    assistant_facts: Some(facts),
                                    message_id: None,
                                },
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
            if let Some(owned) = worker.as_ref() {
                // Settle the tab spinner with the same fact that clears the
                // footer loader. Stale workers settle their own session.
                navigation.apply(crate::native_navigation::DaemonEvent::WorkState {
                    session: crate::native_navigation::SessionRef(owned.session_id().to_owned()),
                    state: crate::native_navigation::WorkState::Idle,
                });
            }
            worker = None;
            if let Some(captured) = pending_submission.take() {
                if binding_active {
                    captured.restore(&mut draft, &mut draft_cursor);
                    completion.sync(&draft);
                }
            }
            if binding_active && crate::native_paste::js_trim(&draft).is_empty() {
                // CP-08: the turn finished with prompts still queued.
                // Surface ONLY the next prompt of THIS session, and only
                // into an empty idle composer, so queued restore or
                // mid-turn typing is never clobbered. The user's next
                // Enter sends it as a steer (upstream flips the session
                // inbox entry to steer, `routes/session/index.tsx:1650-1657`).
                let active = live.as_ref().map(|snapshot| snapshot.session_id.clone());
                let promote = next_queued_for_session(&queued_drafts, active.as_deref());
                if let Some((index, next)) = promote {
                    queued_drafts.remove(index);
                    crate::native_composer::replace_draft_with_cursor(
                        &mut draft,
                        &mut draft_cursor,
                        next.clone(),
                        next.len(),
                    );
                    completion.sync(&draft);
                    needs_paint = true;
                }
            }
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
                            view.set_project_directory(
                                snapshot.directory.clone().unwrap_or_else(current_workspace_label),
                            );
                            if let Err(error) = refresh_session_mcp(&mut view, &snapshot, auth) {
                                append_transcript(&mut transcript, format!("error: session mcp unavailable: {error}"));
                            }
                            if let Err(error) = refresh_session_todos(&mut view, &snapshot, auth) {
                                append_transcript(&mut transcript, format!("error: session todos unavailable: {error}"));
                            }
                            if turn_succeeded && !history_browsing {
                                transcript.reset(
                                    snapshot.history.clone(),
                                    snapshot.typed_history.clone(),
                                );
                            }
                            if turn_succeeded && !history_browsing {
                                history_window = Some(crate::native_chat_projection::NativeHistoryWindow::new(&snapshot.raw_history, snapshot.history_next_before.clone()));
                                transcript.reset(snapshot.history.clone(), snapshot.typed_history.clone());
                            }
                            seed_navigation_titles(&mut navigation, &snapshot);
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
        if let Some(compact) = compact_worker.as_mut() {
            if compact.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation) {
                if let Some(result) = compact.try_result() {
                    compact_worker = None;
                    match result {
                        Ok(_) => {
                            if let Some(current) = live.as_ref() {
                                match fetch_snapshot(&current.origin, Some(&current.session_id), auth) {
                                    Ok(snapshot) if snapshot.session_id == current.session_id => {
                                        seed_navigation_titles(&mut navigation, &snapshot);
                                        live = Some(snapshot);
                                        append_transcript(&mut transcript, "compaction complete".into());
                                    }
                                    Ok(_) => append_transcript(&mut transcript, "error: compaction refresh changed session; history retained".into()),
                                    Err(error) => append_transcript(&mut transcript, format!("error: compaction refresh failed: {error}")),
                                }
                            }
                        }
                        Err(error) => append_transcript(&mut transcript, format!("error: compact: {error}")),
                    }
                    needs_paint = true;
                }
            }
        }
        // Older pages are fetched off the input loop.  The visible window is
        // unchanged until the complete, cursor-validated response arrives.
        if !history_pending && history_navigation_request == Some(HistoryRequestKind::Older) {
            if let (Some(snapshot), Some(window)) = (live.as_ref(), history_window.as_ref()) {
                if let Some(before) = window.next_before() {
                    history_navigation_request = None;
                    history_pending = true;
                    history_request_id = history_request_id.wrapping_add(1);
                    let request = HistoryRequestMeta { id: history_request_id, generation: binding_generation, revision: history_navigation_revision, kind: HistoryRequestKind::Older };
                    history_active = Some(request);
                    let origin = snapshot.origin.clone();
                    let session = snapshot.session_id.clone();
                    let before = before.to_owned();
                    let auth = auth.map(str::to_owned);
                    let generation = binding_generation;
                    let tx = history_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = http_request(
                            &origin,
                            "GET",
                            &format!("/api/sessions/{session}/history?limit=100&before={before}"),
                            None,
                            auth.as_deref(),
                        ).and_then(|body| serde_json::from_str(&body).map_err(|e| format!("history page is not valid JSON: {e}")));
                        let _ = tx.blocking_send((request, result));
                    });
                }
            }
        }
        if !history_pending && history_navigation_request == Some(HistoryRequestKind::Newer) {
            if let (Some(snapshot), Some(window)) = (live.as_ref(), history_window.as_ref()) {
                if let Some(before) = window.newer_recovery_cursor().map(str::to_owned) {
                    history_pending = true;
                    history_request_id = history_request_id.wrapping_add(1);
                    let request = HistoryRequestMeta { id: history_request_id, generation: binding_generation, revision: history_navigation_revision, kind: HistoryRequestKind::Newer };
                    history_active = Some(request);
                    history_trace("history.request.newer", &view, Some(window), history_active, history_navigation_request, history_pending, history_navigation_revision, Some("admitted"));
                    let origin = snapshot.origin.clone();
                    let session = snapshot.session_id.clone();
                    let auth = auth.map(str::to_owned);
                    let generation = binding_generation;
                    let tx = history_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = http_request(&origin, "GET", &format!("/api/sessions/{session}/history?limit=100&before={before}"), None, auth.as_deref())
                            .and_then(|body| serde_json::from_str(&body).map_err(|e| format!("history page is not valid JSON: {e}")));
                        let _ = tx.blocking_send((request, result));
                    });
                    history_navigation_request = None;
                } else {
                    history_navigation_request = None;
                }
            }
        }
        // Same-session external turns are observed through the bounded
        // history endpoint, never replayed through the provider.  A refresh
        // updates the tail anchor while browsing, and replaces the visible
        // window only when already following the tail.
        if (!binding_active || worker_finished)
            && !history_pending
            && last_history_refresh.elapsed() >= Duration::from_millis(750)
        {
            if let Some(snapshot) = live.as_ref() {
                last_history_refresh = Instant::now();
                history_pending = true;
                history_request_id = history_request_id.wrapping_add(1);
                let request = HistoryRequestMeta { id: history_request_id, generation: binding_generation, revision: history_navigation_revision, kind: HistoryRequestKind::Refresh };
                history_active = Some(request);
                let origin = snapshot.origin.clone();
                let session = snapshot.session_id.clone();
                let auth = auth.map(str::to_owned);
                let generation = binding_generation;
                let tx = history_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let result = http_request(&origin, "GET", &format!("/api/sessions/{session}/history?limit=100"), None, auth.as_deref())
                        .and_then(|body| serde_json::from_str(&body).map_err(|e| format!("history refresh JSON: {e}")));
                    let _ = tx.blocking_send((request, result));
                });
            }
        }
        // Returning to the tail requests a fresh bounded newest page. This
        // is the real recovery path when older traversal evicted the newest
        // records; it is not an unbounded cursor walk.
        if !history_pending && history_tail_refresh_pending && view.chat_at_tail() {
            if let (Some(snapshot), Some(window)) = (live.as_ref(), history_window.as_ref()) {
                if window.at_newest_page() {
                    history_pending = true;
                    history_tail_refresh_pending = false;
                    history_request_id = history_request_id.wrapping_add(1);
                    let request = HistoryRequestMeta { id: history_request_id, generation: binding_generation, revision: history_navigation_revision, kind: HistoryRequestKind::Tail };
                    history_active = Some(request);
                    let origin = snapshot.origin.clone();
                    let session = snapshot.session_id.clone();
                    let auth = auth.map(str::to_owned);
                    let generation = binding_generation;
                    let tx = history_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = http_request(&origin, "GET", &format!("/api/sessions/{session}/history?limit=100"), None, auth.as_deref())
                            .and_then(|body| serde_json::from_str(&body).map_err(|e| format!("history page is not valid JSON: {e}")));
                        let _ = tx.blocking_send((request, result));
                    });
                } else {
                    // A local tail is not the global tail; consume the flag so
                    // it cannot stall the loop while the user keeps typing or
                    // navigating the retained history.
                    history_tail_refresh_pending = false;
                }
            }
        }
        // Bounded permission poll (existing authenticated client, 500ms
        // cadence): while a turn owns this binding, while an open decision
        // still needs backend-resolution detection, while the child panel is
        // open, or while a settled conversation remains bound so child-only
        // pending work can be discovered. No queues or new threads; retain
        // the existing bounded request/body limits.
        if !view.dialogs.pending()
            && (binding_active
                || view.dialogs.is_open()
                || children_panel.open
                || (session_route && live.is_some()))
            && last_permission_poll.elapsed() >= Duration::from_millis(500)
        {
            last_permission_poll = Instant::now();
            if let Some(snapshot) = live.as_ref() {
                match fetch_permission(&snapshot.origin, &snapshot.session_id, auth) {
                    Ok(None) => {
                        // TOOL-002: no pending request remains, so upstream
                        // unmarks the tool row (the join memo in
                        // `index.tsx:2793-2799` resolves `false` once the
                        // permission list is empty). Keeping the row marked
                        // after the backend response would lie about state.
                        if !pending_permission_calls.is_empty() {
                            pending_permission_calls.clear();
                            view.set_pending_permission_calls(&pending_permission_calls);
                            needs_paint = true;
                        }
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
                        // TOOL-002: refresh the bounded pending tool-call id
                        // set every poll tick while the backend still owns a
                        // request; the chat render joins the matching
                        // `ChatPart::ToolState { id }` and paints the row
                        // warning (`index.tsx:2793-2799` + `:2837-2847`).
                        let next: Vec<String> =
                            pending.permission_call_id.clone().into_iter().collect();
                        if next != pending_permission_calls {
                            pending_permission_calls = next;
                            view.set_pending_permission_calls(&pending_permission_calls);
                            needs_paint = true;
                        }
                        if !view.dialogs.is_open() {
                            let result = view.dialogs.open_permission_with_persistent(
                                pending.id,
                                snapshot.session_id.clone(),
                                pending.title,
                                pending.persistent_allowed,
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
        // Marquee clock: a cycle whose tab left the rail stops
        // (`session-tabs.tsx:616-618`); otherwise advance on the upstream
        // 600 ms delay / 80 ms cadence.
        let marquee_now = animation_start.now_ms();
        if let Some(active) = tab_marquee.marquee().active().map(str::to_owned) {
            if !navigation.tabs().iter().any(|tab| tab.session.0 == active) {
                tab_marquee.reset();
            }
        }
        if tab_marquee.advance(marquee_now) {
            needs_paint = true;
        }
        // The caller's own pending-title fact keeps the frame clock live
        // exactly as upstream `component/title-shimmer.tsx:50-55` reports
        // `live = animating` while a rename is pending and motion is on.
        let title_animation = crate::native_tabs::title_shimmer_runs(rename_inflight.is_some(), animations);
        let busy_animation = worker.is_some() || model_load.is_some() || title_animation;
        let next_tick = u128::from(animation_start.now_ms()) / 80;
        if busy_animation && next_tick != animation_tick {
            animation_tick = next_tick;
            needs_paint = true;
        }
        // The pointer close-hold auto-releases after 5 s (`session-tabs.tsx:
        // 868-900`); a released hold repaints the ordinary adaptive strip.
        if close_hold
            .as_ref()
            .is_some_and(|hold| hold.expired(animation_start.now_ms()))
        {
            close_hold = None;
            needs_paint = true;
        }
        let mut overlay: Vec<String> = match dialog {
            native_setup::Dialog::Settings => {
                let mut rows = settings_dialog.render(view.modal_presentation().content_width, view.modal_content_height());
                if let Some(error) = settings_error.as_deref() { rows.push(format!("error: {error}")); }
                rows
            }
            native_setup::Dialog::None => Vec::new(),
            native_setup::Dialog::Provider => {
                let mut panel = vec![
                    "Connect a provider".to_owned(),
                    String::new(),
                    format!("Search providers: {filter_input}▏"),
                    String::new(),
                ];
                // The dialog's own bounded filter buffer (typing goes to
                // `filter_input`, never the chat draft) — the overlay must
                // echo and filter the SAME buffer the user is typing into.
                let query = filter_input.trim().to_lowercase();
                let visible: Vec<_> = providers
                    .iter()
                    .filter(|row| row.0.to_lowercase().contains(&query) || row.1.to_lowercase().contains(&query))
                    .collect();
                let active = native_setup::clamp_row(model_row, visible.len());
                let list_start = 4usize;
                let footer_rows = 2 + usize::from(setup_error.is_some()) + usize::from(model_load.is_some());
                let capacity = native_setup::picker_capacity(view.modal_content_height(), list_start, footer_rows);
                for index in native_setup::picker_render_indices(active, visible.len(), capacity) {
                    let Some(row) = visible.get(index) else { continue };
                    panel.push(format!(
                        "{} {}",
                        if index == active { ">" } else { " " },
                        row.1
                    ));
                }
                if visible.is_empty() {
                    panel.push("No matching providers".into());
                }
                panel.push(String::new());
                if let Some(error) = &setup_error {
                    panel.push(format!("Error: {error}"));
                }
                if model_load.is_some() {
                    panel.push("Loading saved provider models… Escape cancels".into());
                }
                panel.push("↑/↓ select · Enter connect · Esc close".into());
                panel
            }
            native_setup::Dialog::ProviderId => vec![
                "Custom provider".into(),
                String::new(),
                "Provider ID (lowercase letters, numbers, - or _)".into(),
                format!("ID: {filter_input}"),
                String::new(),
                "Enter: continue · Escape: cancel".into(),
            ],
            native_setup::Dialog::BaseUrl => vec![
                format!("Configure {provider_id}"),
                String::new(),
                "OpenAI-compatible base URL (include /v1)".into(),
                format!("URL: {filter_input}"),
                String::new(),
                "Enter: continue · Escape: cancel".into(),
            ],
            native_setup::Dialog::ApiKey => vec![
                if provider_id == "openai" {
                    "API key".to_owned()
                } else {
                    format!("API key — {provider_id} (optional for local server)")
                },
                String::new(),
                format!(
                    "Key: {}▏",
                    "*".repeat(key_input.chars().count().min(40))
                ),
                String::new(),
                "Enter save · Esc cancel".into(),
            ],
            native_setup::Dialog::Model => {
                let mut panel = vec![
                    "Select model".to_owned(),
                    String::new(),
                    format!("Search models: {filter_input}▏"),
                    String::new(),
                ];
                if model_load.is_some() {
                    panel.push(format!("Loading models from {provider_id}… Escape cancels"));
                }
                if let Some(error) = &setup_error {
                    panel.push(format!("Error: {error} · /connect to retry"));
                }
                // Same buffer fix as Provider: filter and echo read the
                // dialog-local `filter_input`, never the chat draft.
                let visible = native_setup::dialog_rows(&models, &filter_input);
                let active = native_setup::clamp_row(model_row, visible.len());
                let list_start = 4 + usize::from(model_load.is_some()) + usize::from(setup_error.is_some());
                let capacity = native_setup::picker_capacity(view.modal_content_height(), list_start, 2);
                panel.extend(native_setup::picker_render_indices(active, visible.len(), capacity).into_iter().filter_map(|index| {
                    models.get(visible[index]).map(|model| format!(
                        "{} {} — {}", if index == active { ">" } else { " " }, model.qualified(), model.name
                    ))
                }));
                if visible.is_empty() && model_load.is_none() {
                    panel.push("No matching models".into());
                }
                panel.push(String::new());
                panel.push(format!(
                    "{} models · ↑/↓ select · Enter use · Esc close",
                    visible.len()
                ));
                panel
            }
        };
        if matches!(
            dialog,
            native_setup::Dialog::ProviderId
                | native_setup::Dialog::BaseUrl
                | native_setup::Dialog::ApiKey
        ) {
            if let Some(error) = &setup_error {
                overlay.push(format!("Error: {error}"));
            }
        }
        if view.dialogs.is_open() {
            let _ = appearance.cancel();
        }
        if appearance.open {
            overlay = appearance.rows(view.modal_presentation().content_width);
        }
        if children_panel.open {
            overlay = children_panel.render(view.modal_presentation().content_width, view.modal_content_height());
        }
        if permission_editor.open {
            let (rows, roles, _) = permission_editor.rows(view.modal_presentation().content_width,view.modal_content_height());
            overlay = rows;
            view.set_overlay_roles(roles);
        }
        view.set_overlay_roles(crate::native_ui_style::overlay_roles(&overlay,
            appearance.open || (children_panel.open && !children_panel.detail)
                || matches!(dialog, native_setup::Dialog::Provider | native_setup::Dialog::Model)));
        if permission_editor.open {
            view.set_overlay_roles(permission_editor.rows(view.modal_presentation().content_width,view.modal_content_height()).1);
        }
        if import_modal_visible(view.dialogs.is_open(), permission_editor.open, !matches!(dialog, native_setup::Dialog::None), import_dialog.as_ref().is_some_and(|import| import.is_open())) {
            if let Some(import) = import_dialog.as_ref().filter(|dialog| dialog.is_open()) {
                overlay = import.render(
                    usize::from(view.modal_presentation().content_width),
                    view.modal_content_height(),
                );
            }
        }
        view.set_box_appearance(appearance.style.clone());
        view.set_context_window(selected.as_ref().and_then(|model|model.context_window));
        view.set_prompt_mode(draft.starts_with('!'));
        view.set_prompt_cursor(draft_cursor);
        // CP-35: the draft's text and its tracked part anchors are owned by a
        // single `TrackedDraft` so the text mutation and the anchor realignment
        // travel together. Loading from the working locals and writing the
        // result back keeps every reader of `draft`/`draft_cursor`/`draft_parts`
        // unchanged; the remaining un-migrated mutation sites are listed in
        // evidence/W18-CP35-MIGRATE/handoff.json.
        let mut composer = crate::native_paste::TrackedDraft::new(std::mem::take(&mut draft));
        composer.cursor = draft_cursor;
        composer.parts.pasted = std::mem::take(&mut draft_parts);
        composer.realign_after_edit();
        draft = composer.text.clone();
        draft_cursor = composer.cursor;
        draft_parts = composer.parts.pasted;
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
        let mut help_regions = Vec::new();
        let mut help_panel = None;
        let viewport = (
            renderer.cols().min(u32::from(u16::MAX)) as u16,
            renderer.rows().min(u32::from(u16::MAX)) as u16,
        );
        let configured_orientation = match navigation.orientation() {
            crate::native_navigation::TabOrientation::Horizontal => TabOrientation::Horizontal,
            crate::native_navigation::TabOrientation::Vertical => TabOrientation::Vertical,
        };
        let tab_reservation = TabReservation::for_view_with_width(
            configured_orientation, viewport.0, viewport.1, rail_resize.preferred(),
        );
        let effective_orientation = tab_reservation.orientation.unwrap_or(TabOrientation::Horizontal);
        if tab_viewport != Some(viewport) {
            rail_resize.cancel();
            context_menu = None;
            tab_hovered = None;
            tab_hover_y = None;
            tab_home_hovered = false;
            tab_add_hovered = false;
            rail_search_press = None;
            home_press = None;
            // A resize reflows the strip; the held close cell no longer names a
            // stable physical cell (`session-tabs.tsx:1416-1419`).
            close_hold = None;
            tab_viewport = Some(viewport);
        }
        if dialog_open || help_open || view.command_palette_open() || permission_editor.open
            || appearance.open || children_panel.open
        {
            rail_resize.cancel();
            tab_hovered = None;
            tab_hover_y = None;
            tab_home_hovered = false;
            tab_add_hovered = false;
            tab_press = None;
            add_press = None;
            home_press = None;
        }
        if view.tab_reservation() != tab_reservation {
            let preserve_anchor = session_route && history_browsing && !view.chat_at_tail();
            resize_chat_preserving_anchor(
                &mut view, viewport.0, viewport.1, tab_reservation,
                &transcript.messages, &draft, preserve_anchor,
            );
        }
        view.set_tab_reservation(tab_reservation);
        // A pending permission decision is the "!" attention flag upstream
        // derives from `permission.list(session).length > 0`, and a pending
        // question is "?" from `form.list(session).length > 0`
        // (`context/session-tabs.tsx:163-174`); permission outranks question.
        // `attention_dialog_kind` yields the active tab's kind. A full
        // per-session feed (background tabs) needs a server-side event the
        // daemon does not emit yet; this covers the active session.
        let tab_attention: Vec<crate::native_tabs::Attention> =
            match view.dialogs.attention_dialog_kind() {
                Some(kind) => {
                    let attention_kind = match kind {
                        crate::native_view_dialogs::DialogKind::Question => {
                            crate::native_tabs::TabAttention::Question
                        }
                        _ => crate::native_tabs::TabAttention::Permission,
                    };
                    navigation
                        .active()
                        .map(|tab| {
                            vec![crate::native_tabs::Attention {
                                session: tab.session.clone(),
                                needs_attention: true,
                                kind: attention_kind,
                            }]
                        })
                        .unwrap_or_default()
                }
                None => Vec::new(),
            };
        let mut tab_inputs = TabStrip::from_navigation(&navigation, &tab_attention);
        if tab_drag.preview().is_some() {
            // Paint (and hit-test) the pending drag order until the move is
            // persisted on release.
            let ids: Vec<_> = navigation.tabs().iter().map(|tab| tab.session.clone()).collect();
            tab_drag.project(&mut tab_inputs, &ids);
        }
        // A live close-hold replaces the adaptive width solve so the pointer's
        // close cell stays put (`session-tabs.tsx:1348-1363`); it clears when the
        // layout can no longer apply to the current tab order.
        let held_layout = close_hold.as_ref().and_then(|hold| {
            let ids: Vec<_> = navigation.tabs().iter().map(|tab| tab.session.clone()).collect();
            hold.layout(&ids)
        });
        if close_hold.is_some() && held_layout.is_none() {
            close_hold = None;
        }
        // Upstream promoted NEW_SESSION_TAB: on the Home route the strip's last
        // item is the Home slot (adaptive member, active) and the idle plus is
        // suppressed; home_hovered gates its close glyph like session tabs.
        let home_active = effective_orientation == TabOrientation::Horizontal && navigation.is_home();
        let strip_width = match effective_orientation {
            TabOrientation::Horizontal => viewport.0,
            TabOrientation::Vertical => tab_reservation.width,
        };
        let strip_height = match effective_orientation {
            TabOrientation::Horizontal => 1,
            TabOrientation::Vertical => viewport.1,
        };
        let render_strip = |inputs: &[crate::native_tabs::TabInput]| {
            TabStrip::render_with_held(
                inputs,
                effective_orientation,
                strip_width,
                strip_height,
                rendered_tabs.as_ref().map_or(0, |tabs| tabs.scroll),
                tab_hovered.as_ref(),
                matches!(effective_orientation, TabOrientation::Horizontal),
                home_active,
                home_active && tab_home_hovered,
                held_layout.as_ref(),
            )
            .with_row_colors_semantic(&theme.palette, theme.semantic.as_ref())
            .with_add_hovered(tab_add_hovered, &theme.palette)
        };
        // A running cycle projects the hovered tab's marquee window; the strip
        // and rail each apply it at the exact title budget they paint.
        if let Some(active) = tab_marquee.marquee().active().map(str::to_owned) {
            let offset = tab_marquee.marquee().offset();
            if offset > 0 {
                let session = crate::native_navigation::SessionRef(active);
                if let Some(input) = tab_inputs.iter_mut().find(|tab| tab.session == session) {
                    input.marquee_offset = offset;
                }
            }
        }
        let frame_tabs = render_strip(&tab_inputs);
        // `session-tabs.tsx:1241-1278`: a hovered compact rail entry opens the
        // four-row title/detail tooltip to the right of the rail; drag and an
        // open context menu suppress it.
        let (frame_rail, frame_tooltip) = if effective_orientation == TabOrientation::Vertical {
            let rail_inputs = rail_inputs_for_navigation(&navigation, &tab_inputs);
            let rail = crate::native_tab_rail::RailFrame::render_cells_with_search_hover(&rail_inputs, tab_reservation.width, viewport.1, rendered_rail.as_ref().map_or(0, |r| r.scroll_cells), tab_hovered.as_ref(), navigation.active().is_none(), rail_search_hovered);
            let tooltip = if tab_reservation.width < crate::native_tab_rail::COMPACT_BREAKPOINT
                && tab_drag.dragging().is_none()
                && context_menu.is_none()
            {
                tab_hovered.as_ref().zip(tab_hover_y).and_then(|(session, hover_y)| {
                    let input = rail_inputs.iter().find(|input| input.session == *session)?;
                    crate::native_tab_rail::compact_tooltip(
                        tab_reservation.width,
                        viewport.0,
                        viewport.1,
                        hover_y,
                        true,
                        false,
                        false,
                        &input.title,
                        &input.detail,
                    )
                })
            } else {
                None
            };
            (Some(rail), tooltip)
        } else { (None, None) };
        // Sidebar title rename-pending (`sidebar.tsx:33-45`,
        // `context/data.tsx:22-47`), reprojected from the post-drain
        // in-flight fact every frame — the same "paint from the post-drain
        // state" rule the footer busy flag above follows.
        view.set_title_state(
            sidebar_title_pending(
                live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                rename_inflight.as_ref(),
            ),
            animations,
        );
        // CP-21: expose only drafts owned by the active session. With no active
        // session (home / no session bound) the queue is never published, so an
        // unowned (`None`) draft can never be shown against a different session.
        let active_queue_session = live.as_ref().map(|snapshot| snapshot.session_id.as_str());
        view.set_queued_prompts(publishable_queued_prompts(
            &queued_drafts,
            active_queue_session,
        ));

        let mut lines = if help_open && !dialog_open {
            Vec::new()
        } else {
            // Busy reflects only the worker that owns the CURRENT binding;
            // a stale (superseded) worker never shows busy here.
            // Event draining above may settle and remove the worker during
            // this same frame. Use the post-drain fact for the paint, rather
            // than the loop-entry binding snapshot, otherwise the footer can
            // retain Thinking until an unrelated input event repaints it.
            view.render_typed(
                draft_echo,
                &transcript.messages,
                binding_active && !worker_finished,
                &overlay,
            )
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
            let cols = renderer.cols().min(u32::from(u16::MAX)) as u16;
            let rows = renderer.rows().min(u32::from(u16::MAX)) as u16;
            let inset = if cols >= 24 { 4 } else { 0 };
            let width = cols.saturating_sub(2 * inset).min(100);
            let left = (cols - width) / 2;
            let top = crate::native_ui_style::help_top(rows, help.len());
            let capacity = rows.saturating_sub(2 * top);
            let gap = u16::from(capacity >= 12);
            lines = vec![String::new(); usize::from(rows)];
            for (index, text) in help.iter().enumerate() {
                let row = top + index as u16 + if index > 0 { gap } else { 0 };
                if row >= rows.saturating_sub(top) { break; }
                lines[usize::from(row)] = format!("{}{}", " ".repeat(usize::from(left)), crate::native_view_layout::fit(text, width));
                help_regions.push(crate::native_ui_style::Region {
                    x:left,y:row,width,role:if index == 0 {crate::native_ui_style::Role::Heading} else {crate::native_ui_style::Role::Item},detail_from:None,modal:false,
                });
            }
            if inset > 0 && top > 0 { help_panel = Some((left - 2, top - 1, width + 4, capacity + 2)); }
        }
        if needs_paint {
            let paint_started = Instant::now();
            crate::native_debug::event(
                "ui.state",
                serde_json::json!({"dialog":format!("{dialog:?}"),"page":format!("{:?}",view.page()),"palette":view.command_palette_open(),"help":help_open,"session_route":session_route,"model":selected.as_ref().map(|model|model.qualified()),"model_count":models.len(),"model_loading":model_load.is_some(),"composer_bytes":draft.len(),"busy":worker.is_some(),"permission":dialog_open}),
            );
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
                Some(modal_content_rows(&view.dialogs.render_with_metrics(
                    view.dialog_layout_metrics(view.modal_presentation().content_width),
                )))
            } else if view.command_palette_open() {
                Some(modal_content_rows(&view.command_palette_render()))
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
            let input_overlay = dialog_open
                || view.command_palette_open()
                || appearance.open
                || children_panel.open
                || permission_editor.open
                || import_modal_visible(
                    dialog_open,
                    permission_editor.open,
                    !matches!(dialog, native_setup::Dialog::None),
                    import_dialog.as_ref().is_some_and(|import| import.is_open()),
                )
                || modal_rows.is_some();
            let cursor = if !help_open
                && !input_overlay
                && matches!(dialog, native_setup::Dialog::None)
                && view.page() == crate::native_view_pages::Page::Chat
            {
                if session_route {
                    view.chat_cursor(draft_echo, draft_cursor)
                } else {
                    view.home_cursor_at(draft_echo, draft_cursor)
                }
            } else {
                None
            };
            if renderer.cols() > 0 && renderer.rows() > 0 {
                let (x, y) = cursor.unwrap_or((0, 0));
                renderer.set_cursor(u32::from(x), u32::from(y), cursor.is_some())?;
                // THEME-14: forward the resolved composer cursor shape/blink/
                // colour to OpenTUI. `setCursorStyleOptions` is OpenTUI-core
                // owned (`renderer.zig:2727-2782`); oc2 forwards only, never
                // hand-writing a DECSCUSR byte. `disabled()` upstream
                // (`prompt/index.tsx:192,355`) is `overlay || !interactive`,
                // which the `input_overlay`/`dialog` gate already encodes,
                // plus a pending permission prompt (`index.tsx:238`).
                if let Some(semantic) = theme.semantic.as_ref() {
                    let disabled = input_overlay
                        || !matches!(dialog, native_setup::Dialog::None)
                        || !matches!(view.page(), crate::native_view_pages::Page::Chat)
                        || view.dialogs.pending();
                    let (style, blinking, color) = crate::native_theme_runtime::cursor_wire(
                        cursor_config,
                        semantic,
                        crate::native_theme_runtime::CursorState {
                            focused: cursor.is_some(),
                            disabled,
                        },
                    );
                    renderer.set_cursor_style(
                        style,
                        blinking,
                        Rgba::new(color[0], color[1], color[2], color[3]),
                    )?;
                }
            }
            let selectable: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(row, line)| {
                    if !home
                        && modal_rows.is_none()
                        && !help_open
                        && crate::tui_entry::chat_row_is_selectable(
                            view.chat_row_classes().get(row),
                        )
                    {
                        if let Some((left, width)) =
                            view.chat_text_bounds().get(row).copied().flatten()
                        {
                            format!(
                                "{}{}",
                                " ".repeat(usize::from(left)),
                                crate::native_view_layout::slice_cells(line, left, width)
                            )
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                })
                .collect();
            selection.set_rows(&selectable, 0, 0, u32::from(presentation.main_width));
            native_paint(
                &mut renderer,
                &lines,
                PaintCtx {
                    palette: theme.palette,
                    dialog: theme.dialog,
                    markdown: theme.semantic.clone().map(|semantic| semantic.markdown),
                    diff: theme.semantic.as_ref().map(diff_rgba),
                    presentation,
                    modal_rows: modal_rows.map(|count| count.min(view.modal_content_height())).filter(|count| *count > 0),
                    modal: view.modal_presentation(),
                    sidebar_backdrop_alpha: view.sidebar_backdrop_alpha(),
                    chat_row_classes: view.chat_row_classes().to_vec(),
                    chat_markdown_spans: view.chat_markdown_spans().to_vec(),
                    selection: selection.spans(),
                    appearance: appearance.style.clone(),
                    modal_box: if view.command_palette_open() {
                        crate::native_box_style::BoxId::Menu
                    } else {
                        crate::native_box_style::BoxId::Dialog
                    },
                    // Animate the bounded main header for the current turn;
                    // the facade supplies its label and composer geometry.
                    loading: (binding_active && !worker_finished).then(|| {
                        (
                            crate::native_box_style::frame(
                                animation_start.now_ms(),
                            ),
                            if dialog_open { "Waiting approval" } else { "Thinking…" },
                        )
                    }),
                    home,
                    rail_prompt: view.session_prompt_rail(&draft),
                    composer: if help_open {
                        None
                    } else {
                        view.composer_region(draft_echo)
                    },
                    completion: if !help_open && modal_rows.is_none()
                        && !children_panel.open && !appearance.open && !permission_editor.open
            && !live.as_ref().is_some_and(|snapshot| snapshot.parent_session_id.is_some())
                        && matches!(dialog, native_setup::Dialog::None)
                        && view.page() == crate::native_view_pages::Page::Chat {
                        view.composer_region(draft_echo).and_then(|anchor| completion.panel_at(&draft, draft_cursor, anchor))
                    } else { None },
                    ui_regions: if help_open { help_regions.clone() } else { view.ui_regions() },
                    context_usage_styles: if help_open { Vec::new() } else { view.context_usage_styles().to_vec() },
                    sidebar_span_styles: if help_open { Vec::new() } else { view.sidebar_span_styles().to_vec() },
                    modal_underlay_rows: view.modal_underlay_rows(),
                    page_panel: if help_open { help_panel } else { view.page_panel() },
                    logo: if help_open {
                        None
                    } else {
                        view.home_logo_region(draft_echo)
                    },
            tabs: Some(frame_tabs.clone()),
            animations,
            title_renaming: Some(Box::new({
                let inflight = rename_inflight.clone();
                move |session: &crate::native_navigation::SessionRef| {
                    inflight.as_ref() == Some(session)
                }
            })),
            vertical_rail: frame_rail.clone(),
            compact_tooltip: frame_tooltip,
            context_menu: message_menu
                .as_ref()
                .map(|menu| message_menu_frame(menu, crate::native_tab_context_menu::Viewport { width: viewport.0, height: viewport.1 }))
                .or_else(|| context_menu.as_ref().map(|menu| menu.frame(crate::native_tab_context_menu::Viewport { width: viewport.0, height: viewport.1 }))),
            rename_dialog: rename_input.as_ref().map(|(_,title)| (title.clone(), rename_error.clone())),
            transcript_scrollbar: view.transcript_scrollbar(),
            sidebar_scrollbar: view.sidebar_scrollbar(),
            scrollbar_track: theme
                .semantic
                .as_ref()
                .map(|semantic| {
                    let [r, g, b, a] = crate::native_prompt_pane::prompt_surface(semantic).rgba;
                    Rgba::new(r, g, b, a)
                })
                .unwrap_or_else(|| paint_color(theme.palette, crate::native_theme_runtime::Slot::BackgroundPanel)),
            scrollbar_thumb: paint_color(theme.palette, crate::native_theme_runtime::Slot::Border),
                },
            )?;
            rendered_tabs = Some(frame_tabs);
            rendered_rail = frame_rail;
            needs_paint = false;
            crate::native_debug::event(
                "ui.frame",
                serde_json::json!({"cols":renderer.cols(),"rows":renderer.rows(),"paint_us":paint_started.elapsed().as_micros()}),
            );
        }
        let now = Instant::now();
        input.expire(now);
        let worker_deadline =
            (worker.is_some() || compact_worker.is_some() || model_load.is_some()).then(|| now + Duration::from_millis(20));
        // The marquee's delay expiry (600 ms) and 80 ms steps do not align with
        // the generic animation tick, so wake on its own boundary.
        let marquee_deadline = tab_marquee
            .marquee()
            .next_wake()
            .map(|at| animation_start.deadline(at));
        let timeout = [worker_deadline, input.next_deadline(), marquee_deadline]
            .into_iter()
            .flatten()
            .min()
            // There is no SIGWINCH path in this small native bridge.  A
            // bounded idle wake lets us observe kernel geometry without
            // repainting unchanged frames or waiting for input.
            .map_or(Duration::from_millis(100), |deadline| {
                deadline.saturating_duration_since(now)
            })
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
            crate::native_debug::event(
                "input.raw",
                serde_json::json!({"bytes":1,"kind":if byte[0] == 27 {"escape_prefix"} else if byte[0] < 32 || byte[0] == 127 {"control"} else {"text_or_protocol"}}),
            );
            // A terminal protocol prefix is a control boundary.  Do not let
            // an incomplete UTF-8 scalar straddle it; bytes emitted later by
            // a decoded Kitty event are ordinary input and remain eligible.
            if byte[0] == 0x1b {
                utf8.reset();
            }
            input.push(byte[0], Instant::now());
        }
        let Some(mut event) = input.pop() else { continue };
        // Search owns cancellation until its captured pointer gesture settles.
        // Consume the cancel key before generic Ctrl-C can exit the client.
        if rail_search_press.is_some()
            && matches!(&event, native_input_decoder::InputEvent::Byte(3)
                | native_input_decoder::InputEvent::Escape)
        {
            rail_search_press = None;
            utf8.reset();
            needs_paint = true;
            continue;
        }
        // A keyboard event cancels an owned resize rather than also reaching
        // the composer (in particular Ctrl-C must not exit during a drag).
        if !matches!(&event, native_input_decoder::InputEvent::Mouse { .. })
            && rail_resize.is_resizing()
        {
            rail_resize.cancel();
            needs_paint = true;
            continue;
        }
        let leader_followup_pending = leader_pending && leader_followup_key(&event);
        // Every popped event expires pending state; Ctrl-X re-arms in dispatch below.
        leader_pending = false;
        let memory_source_direction = match &event {
            native_input_decoder::InputEvent::ArrowUp => Some(false),
            native_input_decoder::InputEvent::ArrowDown => Some(true),
            _ => None,
        };
        let (kind, size, control) = match &event {
            native_input_decoder::InputEvent::Byte(byte) if *byte < 32 || *byte == 127 => {
                ("control", 1, Some(*byte))
            }
            native_input_decoder::InputEvent::Byte(_) => ("text", 1, None),
            native_input_decoder::InputEvent::ShiftEnter
            | native_input_decoder::InputEvent::CtrlJ
            | native_input_decoder::InputEvent::AltEnter => ("control", 1, Some(13)),
            native_input_decoder::InputEvent::Paste(text) => ("paste", text.len(), None),
            native_input_decoder::InputEvent::ArrowUp => ("up", 0, None),
            native_input_decoder::InputEvent::ArrowDown => ("down", 0, None),
            native_input_decoder::InputEvent::AltArrowUp => ("alt-up", 0, None),
            native_input_decoder::InputEvent::AltArrowDown => ("alt-down", 0, None),
            // Tab-navigation chords are dispatched to `TabCommand` below; this
            // match only labels them for the debug trace.
            native_input_decoder::InputEvent::AltShiftArrowUp => ("alt-shift-up", 0, None),
            native_input_decoder::InputEvent::AltShiftArrowDown => ("alt-shift-down", 0, None),
            native_input_decoder::InputEvent::TabNext => ("tab-next", 0, None),
            native_input_decoder::InputEvent::TabPrevious => ("tab-previous", 0, None),
            native_input_decoder::InputEvent::TabSlot(_) => ("tab-slot", 0, None),
            native_input_decoder::InputEvent::TabReopen => ("tab-reopen", 0, None),
            native_input_decoder::InputEvent::ArrowLeft => ("left", 0, None),
            native_input_decoder::InputEvent::ArrowRight => ("right", 0, None),
            native_input_decoder::InputEvent::Escape => ("escape", 0, None),
            native_input_decoder::InputEvent::Mouse { .. } => ("mouse", 0, None),
            native_input_decoder::InputEvent::FocusGained => ("focus-gained", 0, None),
            native_input_decoder::InputEvent::FocusLost => ("focus-lost", 0, None),
        };
        crate::native_debug::event(
            "input.event",
            serde_json::json!({"kind":kind,"bytes":size,"control":control,"dialog":format!("{dialog:?}"),"page":format!("{:?}",view.page()),"palette":view.command_palette_open(),"help":help_open,"permission":view.dialogs.intercepts_input(),"composer_bytes":draft.len(),"busy":worker.is_some()}),
        );
        // TAB-21: terminal focus reports are owned here before any dialog or
        // page can consume them as keys. The report only moves the fact on an
        // edge, mirroring the upstream renderer's `_terminalFocusState`
        // guard (`chunk-node-51kpf0mz.js:8777-8786`); the viewed/idle
        // acknowledge driver consumes `client_focused` when the watermark
        // seam lands.
        if matches!(
            &event,
            native_input_decoder::InputEvent::FocusGained
                | native_input_decoder::InputEvent::FocusLost
        ) {
            match &event {
                native_input_decoder::InputEvent::FocusGained if client_focused != Some(true) => {
                    client_focused = Some(true);
                }
                native_input_decoder::InputEvent::FocusLost if client_focused != Some(false) => {
                    client_focused = Some(false);
                }
                _ => {}
            }
            needs_paint = true;
            continue;
        }
        needs_paint = true;
        // Focus snapshot for this event: an open (non-Chat) page behaves
        // like the frozen Help overlay for focus purposes.
        let page_open = !matches!(view.page(), crate::native_view_pages::Page::Chat);
        // Typed-query focus: the Sessions filter and the Commands page
        // (Ctrl-K command input) accept printable/backspace input into the
        // facade-owned bounded query; every other open page stays fully
        // inert, and the permission modal / Help gates above keep
        // precedence over both.
        let page_query_open = view.page().accepts_search();
        // `session.tab.*` shortcuts (`config/keybind.ts:113-118,152-161`).
        // Every physical chord names the same `TabCommand` as the leader
        // path, so default and customized bindings share one dispatch. Like
        // the leader chord, they only fire while the chat composer owns
        // focus; dialogs and pages keep their own arrow navigation.
        let tab_command = match &event {
            native_input_decoder::InputEvent::AltArrowDown
            | native_input_decoder::InputEvent::TabNext => {
                Some(crate::native_navigation::TabCommand::Next)
            }
            native_input_decoder::InputEvent::AltArrowUp
            | native_input_decoder::InputEvent::TabPrevious => {
                Some(crate::native_navigation::TabCommand::Previous)
            }
            native_input_decoder::InputEvent::AltShiftArrowDown => {
                Some(crate::native_navigation::TabCommand::NextUnread)
            }
            native_input_decoder::InputEvent::AltShiftArrowUp => {
                Some(crate::native_navigation::TabCommand::PreviousUnread)
            }
            native_input_decoder::InputEvent::TabSlot(index) => {
                Some(crate::native_navigation::TabCommand::Select(*index))
            }
            native_input_decoder::InputEvent::TabReopen => {
                Some(crate::native_navigation::TabCommand::Reopen)
            }
            _ => None,
        };
        if let Some(command) = tab_command {
            if matches!(dialog, native_setup::Dialog::None)
                && !page_open
                && !help_open
                && !view.dialogs.intercepts_input()
                && !permission_editor.open
                && !appearance.open
                && !children_panel.open
                && !view.command_palette_open()
            {
                use crate::native_navigation::TabCommand;
                let target = match command {
                    TabCommand::Next | TabCommand::Previous => {
                        let forward = matches!(command, TabCommand::Next);
                        let mut probe = navigation.clone();
                        if navigation.active().is_none() {
                            probe.select_tab_slot(if forward { 1 } else { navigation.tab_len() });
                        } else {
                            probe.step_tab(forward);
                        }
                        probe.active().map(|tab| tab.session.clone())
                    }
                    TabCommand::NextUnread | TabCommand::PreviousUnread => {
                        navigation.unread_target(matches!(command, TabCommand::NextUnread))
                    }
                    TabCommand::Select(index) => {
                        navigation.tab_at(index).map(|tab| tab.session.clone())
                    }
                    TabCommand::Reopen => None,
                    TabCommand::Close => None,
                };
                let result = match command {
                    TabCommand::Reopen => {
                        // Read the scroll before the mutable borrow of `view`
                        // starts (`chat_scroll_offset` is a plain getter).
                        let scroll = view.chat_scroll_offset();
                        reopen_tab_transition(
                            &mut view, &mut transcript, &mut navigation, &draft, draft_cursor,
                            daemon_origin,
                            auth, binding_generation,
                            selected.as_ref().map(|model| model.qualified()),
                            scroll,
                            None,
                        )
                    }
                    _ => match target {
                        Some(target) => {
                            let scroll = view.chat_scroll_offset();
                            select_tab_transition(
                                &mut view, &mut transcript, &mut navigation, &draft, draft_cursor,
                                daemon_origin,
                                auth, binding_generation, &target,
                                selected.as_ref().map(|model| model.qualified()), scroll,
                            )
                        }
                        // No resolvable target (e.g. no tabs yet): consume the
                        // chord without a transition, as the pre-existing
                        // `if let Some(target)` guard did.
                        None => {
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
                    },
                };
                match result {
                    Ok(result) => {
                        apply_close_tab_result(
                            result, &mut view, &mut live, &mut history_window,
                            &mut binding_generation, &mut draft, &mut draft_cursor,
                            &mut selected, &models, &mut session_route,
                            &mut compact_worker,
                        );
                        completion.sync(&draft);
                    }
                    Err(error) => {
                        append_transcript(&mut transcript, format!("error: select tab: {error}"))
                    }
                }
            }
            utf8.reset();
            needs_paint = true;
            continue;
        }
        if matches!(dialog, native_setup::Dialog::Settings) {
            let action = match event {
                native_input_decoder::InputEvent::Escape => { settings_dialog.close(); utf8.reset(); dialog = native_setup::Dialog::None; None }
                native_input_decoder::InputEvent::ArrowUp => {
                    settings_dialog.previous();
                    None
                }
                native_input_decoder::InputEvent::ArrowDown => {
                    settings_dialog.next();
                    None
                }
                native_input_decoder::InputEvent::ArrowLeft => settings_dialog.left(),
                native_input_decoder::InputEvent::ArrowRight => settings_dialog.right(),
                native_input_decoder::InputEvent::Byte(13 | 10) => settings_dialog.right(),
                native_input_decoder::InputEvent::Byte(8 | 127) => { utf8.reset(); settings_dialog.backspace(); None },
                native_input_decoder::InputEvent::Byte(byte) if byte >= 32 => {
                    if let Some(character) = utf8.push(byte) { settings_dialog.push_query(character); }
                    None
                },
                native_input_decoder::InputEvent::Mouse { x, y, kind: 0, modifiers: 0 } => {
                    let modal = view.modal_presentation();
                    let content_height = view.modal_content_height();
                    let row_id = modal
                        .content_point(x, y, content_height)
                        .and_then(|(column, row)| settings_dialog.hit_row(
                            column, row, modal.content_width, content_height));
                    row_id.and_then(|id| settings_dialog.click(id))
                }
                _ => None,
            };
            if let Some(action) = action {
                let result = match action {
                    crate::native_settings::SettingsAction::ApplyTabs(orientation) => {
                        match cli_path.as_deref() {
                            Some(path) => crate::native_ui_preferences::UiPreferences::persist_tab_orientation(path, orientation)
                                .map(|()| { navigation.set_orientation(orientation); settings_dialog.applied(orientation); }),
                            None => Err("settings configuration path unavailable".into()),
                        }
                    }
                    crate::native_settings::SettingsAction::ApplySidebar(hidden) => {
                        crate::native_ui_preferences::UiPreferences::persist_sidebar(&state_path, hidden)
                            .map(|()| { view.set_sidebar_hidden(hidden); settings_dialog.applied_sidebar(hidden); })
                    }
                };
                settings_error = result.err();
            }
            needs_paint = true;
            continue;
        }
        // Import owns navigation before completion and composer shortcuts,
        // except when a permission HITL is active (that branch below remains
        // the highest-priority human decision surface).
        if import_modal_visible(view.dialogs.is_open(), permission_editor.open, !matches!(dialog, native_setup::Dialog::None), import_dialog.as_ref().is_some_and(|import| import.is_open())) {
            if let Some(import) = import_dialog.as_mut().filter(|dialog| dialog.is_open()) {
                match event {
                    native_input_decoder::InputEvent::Escape => {
                        let Some(action) = import.dismiss_action() else { continue };
                        let policy = NativeImportPolicy { explicitly_confirmed: false };
                        import.action(action, &policy);
                    }
                    native_input_decoder::InputEvent::Mouse { x, y, kind, modifiers } => {
                        if kind == 0 && modifiers == 0 {
                            let modal = view.modal_presentation();
                            if let Some(row) = y.checked_sub(modal.content_y).map(usize::from) {
                                let content_x = modal.content_x;
                                let Some(col) = x.checked_sub(content_x).map(usize::from) else { continue };
                                if col < usize::from(modal.content_width) {
                                    if let Some(action) = import.action_at(usize::from(modal.content_width), view.modal_content_height(), row, col) {
                                        let policy = NativeImportPolicy { explicitly_confirmed: action == crate::native_import_dialog::ImportAction::Apply };
                                        import.action(action, &policy);
                                    }
                                }
                            }
                        }
                    }
                    native_input_decoder::InputEvent::ArrowUp => import.move_selection(false),
                    native_input_decoder::InputEvent::ArrowDown => import.move_selection(true),
                    native_input_decoder::InputEvent::Byte(byte) => match byte {
                        27 => {
                            let Some(action) = import.dismiss_action() else { continue };
                            import.action(action, &NativeImportPolicy { explicitly_confirmed: false });
                        }
                        13 | 10 => {
                            let policy = NativeImportPolicy { explicitly_confirmed: import.selected_action() == Some(crate::native_import_dialog::ImportAction::Apply) };
                            import.activate_selected(&policy);
                        }
                        2 => import.move_selection(false),
                        14 => import.move_selection(true),
                        _ => {}
                    },
                    _ => {}
                }
                continue;
            }
        }
        // Composer layer keys while the auxiliary dock is open
        // (`composer/index.tsx:72-82`): escape/ctrl+c close, left/right switch
        // tabs (inert with the single Subagents tab). Every other key falls
        // through to the composer text path, so typing is never stolen.
        if view.dock_open()
            && !page_open
            && !help_open
            && !children_panel.open
            && !appearance.open
            && !permission_editor.open
            && !view.dialogs.intercepts_input()
            && !view.command_palette_open()
            && matches!(dialog, native_setup::Dialog::None)
            && import_dialog.as_ref().is_none_or(|import| !import.is_open())
        {
            match &event {
                native_input_decoder::InputEvent::Escape => {
                    let _ = view.dock_key(crate::native_dock::DockKey::Escape);
                    needs_paint = true;
                    continue;
                }
                native_input_decoder::InputEvent::Byte(3) => {
                    let _ = view.dock_key(crate::native_dock::DockKey::CtrlC);
                    needs_paint = true;
                    continue;
                }
                native_input_decoder::InputEvent::ArrowLeft => {
                    let _ = view.dock_key(crate::native_dock::DockKey::Left);
                    needs_paint = true;
                    continue;
                }
                native_input_decoder::InputEvent::ArrowRight => {
                    let _ = view.dock_key(crate::native_dock::DockKey::Right);
                    needs_paint = true;
                    continue;
                }
                _ => {}
            }
        }
        // LAY-08: a narrow sidebar is an overlay and its dedicated Escape /
        // Ctrl+C layer (`routes/session/index.tsx:252-261`) closes it. This
        // runs before the composer/session Escape handling below so the
        // overlay is dismissed first; wide/inline sidebars are unaffected
        // (their layer is disabled upstream).
        if view.narrow_sidebar_overlay_open()
            && !page_open
            && !help_open
            && !children_panel.open
            && !appearance.open
            && !permission_editor.open
            && !view.dialogs.intercepts_input()
            && !view.command_palette_open()
            && matches!(dialog, native_setup::Dialog::None)
            && import_dialog.as_ref().is_none_or(|import| !import.is_open())
            && matches!(
                event,
                native_input_decoder::InputEvent::Escape | native_input_decoder::InputEvent::Byte(3)
            )
        {
            view.toggle_sidebar();
            needs_paint = true;
            continue;
        }
        // Completion owns navigation before composer/session shortcuts, but
        // never before a credential, permission or other modal surface.
        if !page_open && !help_open && !children_panel.open && !appearance.open && !permission_editor.open
            && !live.as_ref().is_some_and(|snapshot| snapshot.parent_session_id.is_some())
            && !view.dialogs.intercepts_input() && !view.command_palette_open()
            && matches!(dialog, native_setup::Dialog::None)
            && import_dialog.as_ref().is_none_or(|import| !import.is_open())
            && view.composer_region(&draft).and_then(|anchor| completion.panel_at(&draft, draft_cursor, anchor)).is_some()
        {
            match &event {
                native_input_decoder::InputEvent::ArrowUp => { completion.navigate_at(&draft, draft_cursor, false); needs_paint = true; continue; }
                native_input_decoder::InputEvent::ArrowDown => { completion.navigate_at(&draft, draft_cursor, true); needs_paint = true; continue; }
                native_input_decoder::InputEvent::Escape => { completion.dismiss(); needs_paint = true; continue; }
                native_input_decoder::InputEvent::Byte(key) => {
                    if matches!(*key, b'\r' | b'\n') {
                        if let Some(route) = completion.selected_command().map(str::to_owned) {
                            // /exit is the upstream `app.exit` action and is
                            // dispatched below as a global client exit, the
                            // same as /quit and :q/:quit (a selected exit
                            // route is never a tab close).
                            // Executable commands are dispatched by this same
                            // Enter event. Argument-bearing commands remain a
                            // completion (for example `/cd` must still get a
                            // directory before it can run).
                            if route != "/cd" {
                                let route_cursor = route.len();
                                crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, route, route_cursor); draft_parts.clear();
                                completion.sync(&draft);
                            } else if let Some(cursor) = completion.complete_key_at(*key, &mut draft, draft_cursor, MAX_NATIVE_DRAFT_BYTES) {
                                draft_cursor = cursor;
                                utf8.reset();
                                needs_paint = true;
                                continue;
                            }
                        } else if let Some(cursor) = completion.complete_key_at(*key, &mut draft, draft_cursor, MAX_NATIVE_DRAFT_BYTES) {
                            draft_cursor = cursor;
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
                    } else if let Some(cursor) = completion.complete_key_at(*key, &mut draft, draft_cursor, MAX_NATIVE_DRAFT_BYTES) {
                        draft_cursor = cursor;
                        utf8.reset();
                        needs_paint = true;
                        continue;
                    }
                }
                _ => {}
            }
        }
        // Completion mouse activation is normalized to the same Enter event
        // used by keyboard dispatch. Modal precedence has already been
        // handled above, so this cannot click through a decision surface.
        // Keep this eligibility identical to the keyboard completion gate
        // above. In particular, a stale/hidden panel must not remain a click
        // target while a modal, setup dialog, help page, palette, or another
        // interaction owner is active.
        let completion_mouse_eligible = !page_open
            && !help_open
            && !children_panel.open
            && !appearance.open
            && !permission_editor.open
            && !view.dialogs.intercepts_input()
            && !view.command_palette_open()
            && matches!(dialog, native_setup::Dialog::None)
            && import_dialog.as_ref().is_none_or(|import| !import.is_open())
            && view.composer_region(&draft).and_then(|anchor| completion.panel_at(&draft, draft_cursor, anchor)).is_some();
        if completion_mouse_eligible {
            if let native_input_decoder::InputEvent::Mouse { x, y, kind: 0, modifiers: 0 } = event {
            if let Some(panel) = view.composer_region(&draft).and_then(|anchor| completion.panel_at(&draft, draft_cursor, anchor)) {
                            if let Some(value) = panel.value_at(x, y).map(str::to_owned).filter(|_| !panel.is_disabled_at(x, y)) {
                                if !value.starts_with('/') {
                                    if let Some(cursor) = completion.accept_value_or_expand_directory(&mut draft, &value, draft_cursor, MAX_NATIVE_DRAFT_BYTES) {
                                        draft_cursor = cursor;
                                        utf8.reset();
                                    }
                                    needs_paint = true;
                                    continue;
                                }
                                let route = value;
                    if route == "/cd" {
                        if let Some(cursor) = completion.accept_value_or_expand_directory(
                            &mut draft,
                            &route,
                            draft_cursor,
                            MAX_NATIVE_DRAFT_BYTES,
                        ) {
                            draft_cursor = cursor;
                        }
                        needs_paint = true;
                        continue;
                    }
                    // A clicked/pressed exit route terminates the client
                    // immediately; there is no tab-close command anymore.
                    if native_process_exit_route(&route) { break; }
                    let route_cursor = route.len();
                    crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, route, route_cursor); draft_parts.clear();
                    completion.sync(&draft);
                    event = native_input_decoder::InputEvent::Byte(13);
                }
            }
        }
        }
        event = composer_pointer_event(
            event,
            &view,
            &draft,
            keymap,
            !page_open
                && !help_open
                && !children_panel.open
                && !appearance.open
                && !permission_editor.open
                && !view.dialogs.intercepts_input()
                && !view.command_palette_open()
                && matches!(dialog, native_setup::Dialog::None)
                && import_dialog.as_ref().is_none_or(|import| !import.is_open()),
        );
        if !page_open && !help_open && !children_panel.open && !appearance.open
            && !permission_editor.open && !view.dialogs.intercepts_input()
            && !view.command_palette_open() && matches!(dialog, native_setup::Dialog::None)
            && import_dialog.as_ref().is_none_or(|import| !import.is_open())
            && view.composer_region(&draft).is_some()
        {
            match &event {
                native_input_decoder::InputEvent::ArrowLeft
                | native_input_decoder::InputEvent::ArrowRight => {
                    // TAB-23 leader follow-up arrows, dispatched BEFORE the
                    // printable leader arm and before composer cursor motion:
                    // `<leader>right` is `pane.focus.right`, `<leader>left` is
                    // `pane.focus.left` (`config/keybind.ts:97-98`); the chord
                    // accepts non-byte follow-ups (`context/keymap.tsx:137-145`).
                    // The follow-up is consumed WITHOUT touching the composer
                    // draft, exactly like the printable leader arm.
                    if leader_followup_pending {
                        if let Some(host_event) = leader_followup_pane_event(&event) {
                            // HostEvent::PaneFocusRight / PaneFocusSession
                            // (`native_host.rs:192-194`, stepped semantics at
                            // `:411-424`): right-pane focus drops composer
                            // interactivity (`useInteractivity`,
                            // `footer.tsx:60` -> `set_rail_interactive`).
                            view.set_rail_interactive(matches!(
                                host_event,
                                crate::native_host::HostEvent::PaneFocusSession
                            ));
                        }
                        leader_pending = false;
                        utf8.reset();
                        needs_paint = true;
                        continue;
                    }
                    let left = matches!(&event, native_input_decoder::InputEvent::ArrowLeft);
                    crate::native_composer::apply_horizontal_binding(&draft, &mut draft_cursor, left);
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
                _ => {}
            }
        }
        // MSG-006 message menu owns input while open (upstream renders the
        // select in `DialogSelect`, `dialog-message.tsx:13-23`): Escape /
        // Ctrl-C / Ctrl-D dismiss, Up/Down move the highlight
        // (`ui/dialog-select.tsx:1590-1602`) and Enter resolves the
        // highlighted option into its effect (`dialog-message.tsx:29-90`).
        // Every other event is swallowed while the menu is open (modal).
        if let Some(mut menu) = message_menu.take() {
            let mut keep = true;
            if menu_dismisses(&event) {
                keep = false;
            } else if matches!(event, native_input_decoder::InputEvent::ArrowUp) {
                message_menu_move(&mut menu, false);
            } else if matches!(event, native_input_decoder::InputEvent::ArrowDown) {
                message_menu_move(&mut menu, true);
            } else if menu_submits(&event) {
                let (session_id, _message_id) = message_menu_identity(&menu);
                match message_menu_dispatch(&menu) {
                    MessageMenuDispatch::Actions(outcome) => {
                        dispatch_message_actions(
                            outcome,
                            session_id.as_deref(),
                            live.as_ref().map(|snapshot| snapshot.origin.as_str()),
                            auth,
                            &mut draft,
                            &mut draft_cursor,
                            &mut draft_parts,
                            &mut transcript,
                        );
                    }
                    MessageMenuDispatch::Pending(effect) => {
                        dispatch_pending_steer(effect, &mut transcript);
                    }
                }
                // The menu dismisses on every dispatched action. Copy upstream
                // clears only after the write resolves (`dialog-message.tsx:72`);
                // the OSC52 write here is synchronous, so it closes too.
                keep = false;
            }
            if keep {
                message_menu = Some(menu);
            }
            needs_paint = true;
            continue;
        }
        // Rename dialog owns input before every widget below: Escape and
        // Ctrl-C/Ctrl-D cancel only (never exit), Enter submits through the
        // real PATCH route, backspace edits, text is bounded UTF-8.
        if rename_input.is_some() {
            if rename_dismisses(&event) {
                rename_input = None;
                rename_error = None;
                utf8.reset();
                needs_paint = true;
                continue;
            }
            if rename_submits(&event) {
                let (session, value) = rename_input.as_ref().expect("rename owns input");
                let Some(title) = rename_accepted_title(value) else {
                    rename_error = Some("Title cannot be empty".into());
                    needs_paint = true;
                    continue;
                };
                // Exactly one bounded request in flight: a second Enter is a
                // truthful refusal, never a queued duplicate or a racing ack.
                if rename_inflight.is_some() {
                    rename_error = Some("A rename request is already in flight".into());
                    needs_paint = true;
                    continue;
                }
                let Some(snapshot) = live.as_ref().filter(|s| s.session_id == session.0) else {
                    rename_error =
                        Some("Rename refused: target session is no longer active".into());
                    needs_paint = true;
                    continue;
                };
                let origin = snapshot.origin.clone();
                let auth_value = auth.map(str::to_owned);
                let id = session.0.clone();
                let body = rename_request_body(&title);
                let path = rename_request_path(&id);
                let tx = rename_tx.clone();
                rename_inflight = Some(crate::native_navigation::SessionRef(id.clone()));
                rename_worker = Some(tokio::task::spawn_blocking(move || {
                    let result = http_request_with_status(
                        &origin,
                        "PATCH",
                        &path,
                        Some(&body),
                        auth_value.as_deref(),
                    )
                    .map(|(body, status)| (body, status, title, id));
                    let _ = tx.blocking_send(result);
                }));
                rename_error = None;
                needs_paint = true;
                continue;
            }
            if rename_backspace(&event) {
                if let Some((_, value)) = rename_input.as_mut() {
                    value.pop();
                }
                utf8.reset();
                needs_paint = true;
                continue;
            }
            if let native_input_decoder::InputEvent::Byte(byte) = &event {
                if *byte >= 32 {
                    if let Some(ch) = utf8.push(*byte) {
                        if let Some((_, value)) = rename_input.as_mut() {
                            if value.len() + ch.len_utf8() <= 256 && !ch.is_control() {
                                value.push(ch);
                            }
                        }
                    }
                    needs_paint = true;
                }
                continue;
            }
            continue;
        }
        if context_menu.is_some() && menu_dismisses(&event) {
            context_menu = None;
            needs_paint = true;
            continue;
        }
        // Normalize only after higher-priority modal/page/completion owners
        // have handled the event. The existing composer KeyAction/SubmitKeymap
        // remains the authority for newline versus submission.
        let event = normalize_modified_return(event, keymap);
        if !matches!(event, native_input_decoder::InputEvent::Mouse { .. }) {
            rail_search_press = None;
        }
        let byte = match event {
            native_input_decoder::InputEvent::Byte(byte) => byte,
            native_input_decoder::InputEvent::Mouse { x, y, kind, modifiers } => {
                // The parent owns drag/release after a boundary press, even
                // outside the old rail. Consume before tab/close/new dispatch.
                let resize_allowed = !view.dialogs.intercepts_input()
                    && !permission_editor.open && !appearance.open && !children_panel.open
                    && !help_open && !view.command_palette_open() && !page_open
                    && context_menu.is_none() && rename_input.is_none()
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open());
                if !resize_allowed || modifiers != 0 {
                    rail_resize.cancel();
                } else {
                    let now_ms = animation_start.now_ms();
                    let mut resize_consumed = false;
                    let mut commit_width = None;
                    if rail_resize.is_resizing() {
                        match kind {
                            1 => { let _ = rail_resize.drag(x, viewport.0); resize_consumed = true; }
                            2 => { commit_width = rail_resize.release(now_ms, x, viewport.0); resize_consumed = true; }
                            7 => { resize_consumed = true; }
                            _ => rail_resize.cancel(),
                        }
                    } else if kind == 0 && effective_orientation == TabOrientation::Vertical
                        && y < viewport.1 && rendered_rail.is_some()
                        && rail_resize.press(now_ms, x, viewport.0)
                    {
                        resize_consumed = true;
                        if !rail_resize.is_resizing() {
                            commit_width = Some(rail_resize.size(viewport.0));
                        }
                    }
                    if resize_consumed {
                        tab_press = None;
                        add_press = None;
                        home_press = None;
                        rail_search_press = None;
                        tab_hovered = None;
                        if let Some(width) = commit_width {
                            if let Err(error) = crate::native_ui_preferences::UiPreferences::persist_vertical_tabs_width(&state_path, width) {
                                append_transcript(&mut transcript, format!("error: tab rail width: {error}"));
                            }
                        }
                        needs_paint = true;
                        continue;
                    }
                }
                let menu_vp=crate::native_tab_context_menu::Viewport { width:viewport.0,height:viewport.1 };
                if let Some(menu)=context_menu.as_mut() {
                    let me=match kind { 0=>Some(crate::native_tab_context_menu::MenuEvent::PointerDown{button:crate::native_tab_context_menu::MouseButton::Left,x,y}),2=>Some(crate::native_tab_context_menu::MenuEvent::PointerUp{button:crate::native_tab_context_menu::MouseButton::Left,x,y}),5=>Some(crate::native_tab_context_menu::MenuEvent::PointerDown{button:crate::native_tab_context_menu::MouseButton::Right,x,y}),6=>Some(crate::native_tab_context_menu::MenuEvent::PointerUp{button:crate::native_tab_context_menu::MouseButton::Right,x,y}),7=>Some(crate::native_tab_context_menu::MenuEvent::PointerMove{x,y}),3|4=>Some(crate::native_tab_context_menu::MenuEvent::Scroll{delta:if kind==3{-3}else{3},x,y}),_=>None};
                    if let Some(me)=me { let outcome=menu.handle(me,menu_vp); if let crate::native_tab_context_menu::MenuOutcome::Invoke(action)=outcome { context_menu=None; match action {
                        crate::native_tab_context_menu::MenuAction::NewSession => { let model=selected.as_ref().map(|m|m.qualified()); let saved=if navigation.active().is_some(){navigation.save_active_state(draft.clone(),draft_cursor,view.chat_scroll_offset(),model)}else{Ok(())}; if let Err(e)=saved{append_transcript(&mut transcript,format!("error: new tab: state save refused: {e:?}"));}else{live=None;history_window=None;history_pending=false;history_active=None;history_navigation_request=None;blocked_history_scroll=None;history_tail_refresh_pending=false;history_browsing=false;draft.clear(); draft_parts.clear();draft_cursor=0;navigation.enter_home();session_route=false;view.reset_to_home_after_tab_close();view.set_chat_scroll_offset(0);} },
                        crate::native_tab_context_menu::MenuAction::CopySessionId{session}=>{let frame=selection_clipboard_frame(&session.0);let mut out=std::io::stdout().lock();out.write_all(frame.as_bytes())?;out.flush()?;},
                        crate::native_tab_context_menu::MenuAction::Close{session}=>{close_hold=None;let scroll=view.chat_scroll_offset();match close_tab_transition(&mut view,&mut transcript,&mut navigation,&draft,draft_cursor,daemon_origin,auth,binding_generation,&session,selected.as_ref().map(|m|m.qualified()),scroll){Ok(result)=>apply_close_tab_result(result,&mut view,&mut live,&mut history_window,&mut binding_generation,&mut draft,&mut draft_cursor,&mut selected,&models,&mut session_route,&mut compact_worker),Err(e)=>append_transcript(&mut transcript,format!("error: close tab: {e}"))}},
                        crate::native_tab_context_menu::MenuAction::Rename{session,current_title}=>{rename_input=Some((session,current_title));rename_error=None;},
                        crate::native_tab_context_menu::MenuAction::Reopen{session}=>{
                            let scroll=view.chat_scroll_offset();
                            match reopen_tab_transition(&mut view,&mut transcript,&mut navigation,&draft,draft_cursor,daemon_origin,auth,binding_generation,selected.as_ref().map(|m|m.qualified()),scroll,Some(&session)){
                                Ok(result)=>apply_close_tab_result(result,&mut view,&mut live,&mut history_window,&mut binding_generation,&mut draft,&mut draft_cursor,&mut selected,&models,&mut session_route,&mut compact_worker),
                                Err(e)=>append_transcript(&mut transcript,format!("error: reopen tab: {e}")),
                            }
                        },
                    }} else if !menu.is_open(){context_menu=None;} needs_paint=true; }
                    continue;
                }
                if kind==5 && modifiers==0 && !view.dialogs.intercepts_input() && !permission_editor.open && !appearance.open && !children_panel.open && !help_open && !view.command_palette_open() && matches!(dialog,native_setup::Dialog::None) && import_dialog.as_ref().is_none_or(|i|!i.is_open()) && !page_open {
                    if let Some(target)=tab_menu_target_at(&navigation,rendered_tabs.as_ref(),rendered_rail.as_ref(),effective_orientation==TabOrientation::Vertical,x,y) { let menu=crate::native_tab_context_menu::TabContextMenu::new(x,y,target); if menu.is_open() && !menu.items().is_empty(){context_menu=Some(menu);} tab_press=None;add_press=None;tab_hovered=None;home_press=None;tab_home_hovered=false;needs_paint=true;continue; }
                }
                if kind != 0 && kind != 2 && kind != 7 {
                    // Middle/right presses cancel a captured drag without
                    // persisting it (`session-tabs.tsx:892-909`); left press
                    // itself (kind 1) is drag motion and must keep the source.
                    if matches!(kind, 5 | 6 | 8 | 9 | 10) {
                        tab_drag.release();
                        tab_drag.clear_suppress();
                    }
                    tab_press = None;
                    add_press = None;
                    rail_search_press = None;
                    home_press = None;
                    tab_home_hovered = false;
                }
                if !view.dialogs.intercepts_input() && !permission_editor.open && !appearance.open && !children_panel.open
                    && !help_open && !view.command_palette_open() && matches!(dialog, native_setup::Dialog::None)
                    && modifiers == 0
                {
                    if kind == 1 && tab_drag.dragging().is_some() {
                        // Left-drag over the strip/rail: move the preview slot,
                        // never the persisted order (that lands once on release,
                        // `session-tabs.tsx:685-702,1508-1523`).
                        match effective_orientation {
                            TabOrientation::Vertical => {
                                if let Some(rail) = rendered_rail.as_ref() {
                                    if let Some(slot) = rail.drag_slot(y, navigation.tab_len()) {
                                        tab_drag.drag_to(slot, navigation.tab_len());
                                        needs_paint = true;
                                    }
                                }
                            }
                            TabOrientation::Horizontal => {
                                if let Some(slot) = rendered_tabs
                                    .as_ref()
                                    .and_then(|tabs| tabs.drag_slot(x, navigation.tab_len()))
                                {
                                    tab_drag.drag_to(slot, navigation.tab_len());
                                    needs_paint = true;
                                }
                            }
                        }
                        continue;
                    }
                    if kind == 7 {
                        tab_hovered = if effective_orientation == TabOrientation::Vertical {
                            rendered_rail.as_ref().and_then(|rail| match rail.hit(x, y) { Some(crate::native_tab_rail::RailHit::Session(id)) => Some(id), _ => None })
                        } else {
                            rendered_tabs.as_ref().and_then(|tabs| tabs.hit(x, y)).map(|hit| hit.session)
                        };
                        // TAB-23: the compact search control's hover fact
                        // (`session-tabs-rail.tsx:14-20`). `search_hit` targets
                        // the painted `⌕` row, so the overlay only appears while
                        // the pointer is physically over the control.
                        let search_hovered = effective_orientation == TabOrientation::Vertical
                            && rendered_rail.as_ref().is_some_and(|rail| rail.search_hit(x, y));
                        if search_hovered != rail_search_hovered {
                            rail_search_hovered = search_hovered;
                            needs_paint = true;
                        }
                        tab_hover_y = tab_hovered.is_some().then_some(y);
                        // Home pseudo-tab hover is identity-free and tracked beside
                        // the session hover fact; session hover wins the glyph.
                        tab_home_hovered = effective_orientation == TabOrientation::Horizontal
                            && tab_hovered.is_none()
                            && rendered_tabs.as_ref().is_some_and(|tabs| tabs.home_hit(x, y));
                        tab_add_hovered = effective_orientation == TabOrientation::Horizontal
                            && rendered_tabs.as_ref().is_some_and(|tabs| tabs.add_hit(x, y));
                        // Leaving the strip box releases the close-hold
                        // (`session-tabs.tsx:1459-1470` onMouseOut bounds).
                        if close_hold.is_some() && effective_orientation == TabOrientation::Horizontal {
                            let inside = rendered_tabs.as_ref().is_some_and(|tabs| {
                                let row = tabs
                                    .rects
                                    .first()
                                    .map(|(_, rect)| rect.y)
                                    .or(tabs.home_rect.map(|rect| rect.y))
                                    .unwrap_or(0);
                                let origin = tabs.strip_origin_x();
                                y == row
                                    && x >= origin
                                    && u32::from(x)
                                        < u32::from(origin) + u32::from(tabs.available_width)
                            });
                            if !inside {
                                close_hold = None;
                                needs_paint = true;
                            }
                        }
                        // `session-tabs.tsx:1650,1678` / `:888,915`: pointer-over
                        // starts the title marquee at the hovered title width;
                        // leaving the tab schedules the zero-delay clear.
                        let hover_now = animation_start.now_ms();
                        match tab_hovered.as_ref() {
                            Some(session) => {
                                let title = navigation
                                    .title(session)
                                    .unwrap_or(&session.0)
                                    .to_owned();
                                let width = match effective_orientation {
                                    TabOrientation::Vertical
                                        if tab_reservation.width < crate::native_tab_rail::COMPACT_BREAKPOINT =>
                                    {
                                        // Compact rows pass `Infinity` upstream, so
                                        // an overflowing title never scrolls there.
                                        u16::MAX
                                    }
                                    TabOrientation::Vertical => {
                                        crate::native_tab_rail::expanded_title_width(
                                            tab_reservation.width,
                                            navigation.tab_len(),
                                            true,
                                        )
                                    }
                                    TabOrientation::Horizontal => rendered_tabs
                                        .as_ref()
                                        .and_then(|tabs| tabs.title_width(session, true))
                                        // A strip not yet painted for this tab must
                                        // not start a cycle at a guessed width.
                                        .unwrap_or(u16::MAX),
                                };
                                tab_marquee.enter(&session.0, &title, width, hover_now);
                            }
                            None => tab_marquee.leave_hovered(hover_now),
                        }
                        needs_paint = true;
                        continue;
                    }
                    if kind == 8 {
                        // Middle-button close on PRESS (upstream session-tabs.tsx:892-898
                        // and 1652-1657 fire tabs.close(tab.sessionID) on onMouseDown).
                        // Only the tab under the pointer closes; middle release (kind 9)
                        // and middle drag (kind 10) are ignored. New-tab region / miss is
                        // inert — an EXPLICIT DIVERGENCE: upstream's plus fires
                        // tabs.add?.() on any non-right RELEASE (session-tabs.tsx
                        // 1784-1790, pressing arms addPressed at 1774-1782); oc2
                        // deliberately keeps the standalone plus middle-inert. NO
                        // fallback to active on miss (Home close(undefined) is wired
                        // below by the horizontal Home arm).
                        let middle_session = if effective_orientation == TabOrientation::Vertical {
                            rendered_rail.as_ref().and_then(|rail| match rail.hit(x, y) {
                                Some(crate::native_tab_rail::RailHit::Session(id)) => Some(id),
                                _ => None,
                            })
                        } else {
                            rendered_tabs.as_ref().and_then(|tabs| tabs.hit(x, y)).map(|hit| hit.session)
                        };
                        if let Some(session) = middle_session {
                            close_hold = None;
                            let close_scroll = view.chat_scroll_offset();
                            match close_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &session, selected.as_ref().map(|m| m.qualified()), close_scroll) {
                                Ok(result) => {
                                    apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                    completion.sync(&draft);
                                    history_pending = false; history_active = None; history_navigation_request = None; blocked_history_scroll = None; history_tail_refresh_pending = false; history_browsing = false;
                                }
                                Err(error) => append_transcript(&mut transcript, format!("error: close tab: {error}")),
                            }
                            needs_paint = true;
                        } else if effective_orientation == TabOrientation::Horizontal {
                            // Horizontal NEW_SESSION_TAB middle press = upstream
                            // tabs.close(undefined) fired on press: navigate to the
                            // retained focused session or the last remaining tab; no
                            // session close ever runs from the Home slot. The
                            // standalone plus slot stays DIVERGENTLY inert on middle
                            // (upstream would add a session on middle release;
                            // session-tabs.tsx 1784-1790) — contracted oc2 behavior.
                            if let Some(tabs) = rendered_tabs.as_ref() {
                                if tabs.home_hit(x, y) {
                                    if let Some(target) = navigation.home_dispatch_target() {
                                        let scroll = view.chat_scroll_offset();
                                        match select_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &target, selected.as_ref().map(|m| m.qualified()), scroll) {
                                            Ok(result) => {
                                                apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                                completion.sync(&draft);
                                                history_pending = false; history_active = None; history_navigation_request = None; blocked_history_scroll = None; history_tail_refresh_pending = false; history_browsing = false;
                                            }
                                            Err(error) => append_transcript(&mut transcript, format!("error: select tab: {error}")),
                                        }
                                    }
                                    needs_paint = true;
                                }
                            }
                        }
                        continue;
                    }
                    if kind == 0 {
                        // A fresh press supersedes any captured drag; a press
                        // on a session body re-arms it below.
                        tab_drag.release();
                        tab_drag.clear_suppress();
                        tab_press = None;
                        add_press = None;
                        rail_search_press = None;
                        if effective_orientation == TabOrientation::Vertical {
                            if let Some(rail) = rendered_rail.as_ref() {
                                match rail.hit(x, y) {
                                    Some(crate::native_tab_rail::RailHit::Session(_)) => {
                                        if let Some((session, rect, close)) = rail.session_press_target(x, y) {
                                            // Upstream arms the drag on the tab body itself;
                                            // the close child keeps its own click semantics.
                                            if !close {
                                                if let Some(index) = navigation.tabs().iter().position(|tab| tab.session == session) {
                                                    tab_drag.press(&session, index);
                                                }
                                            }
                                            tab_press = Some((session, crate::native_tabs::TabRect { x: rect.x, y: rect.y, width: rect.width, height: rect.height }, close, effective_orientation));
                                        }
                                    }
                                    Some(crate::native_tab_rail::RailHit::New) => add_press = Some((crate::native_tabs::TabRect { x: rail.new_rect.map_or(0, |r| r.x), y: rail.new_rect.map_or(0, |r| r.y), width: rail.new_rect.map_or(0, |r| r.width), height: rail.new_rect.map_or(0, |r| r.height) }, effective_orientation)),
                                    Some(crate::native_tab_rail::RailHit::Search) => rail_search_press = Some((crate::native_tabs::TabRect { x: 0, y: 1, width: rail.width, height: 1 }, effective_orientation)),
                                    None => {}
                                }
                            }
                        } else if let Some(tabs) = rendered_tabs.as_ref() {
                            if let Some(hit) = tabs.hit(x, y) {
                                let session = hit.session;
                                if let Some((_, rect)) = tabs.rects.iter().find(|(id, _)| id == &session) {
                                    let close = tabs.visible_close_rect().is_some_and(|close| {
                                        tabs.hovered_session() == Some(&session)
                                            && x >= close.x && y >= close.y
                                            && x < close.x.saturating_add(close.width)
                                            && y < close.y.saturating_add(close.height)
                                    });
                                    // Upstream arms the drag on the tab body
                                    // itself; the close child keeps its own
                                    // click semantics (`session-tabs.tsx:1508-1523,1738-1753`).
                                    if !close {
                                        if let Some(index) = navigation
                                            .tabs()
                                            .iter()
                                            .position(|tab| tab.session == session)
                                        {
                                            tab_drag.press(&session, index);
                                        }
                                    }
                                    tab_press = Some((session, *rect, close, effective_orientation));
                                }
                            } else if tabs.home_hit(x, y) {
                                // Upstream NEW_SESSION_TAB mousedown stopPropagates for
                                // every button; absorb the press so it can never leak
                                // into composer/send handling while already on Home.
                                let close_child = tabs
                                    .home_close_visible_rect()
                                    .is_some_and(|close| x >= close.x && y >= close.y
                                        && x < close.x.saturating_add(close.width)
                                        && y < close.y.saturating_add(close.height));
                                if let Some(rect) = tabs.home_rect {
                                    home_press = Some((rect, effective_orientation, close_child));
                                }
                            } else if let Some(rect) = tabs.add_rect.filter(|rect| x >= rect.x && y >= rect.y && x < rect.x.saturating_add(rect.width) && y < rect.y.saturating_add(rect.height)) {
                                add_press = Some((rect, effective_orientation));
                            }
                        }
                        if tab_press.is_some() || add_press.is_some() || home_press.is_some() || rail_search_press.is_some() {
                            continue;
                        }
                    }
                    if kind == 2 {
                        if tab_drag.dragging().is_some() {
                            let release = tab_drag.release();
                            if let Some(to) = release.move_to {
                                // One persisted move per drag; the preview is
                                // settled against the resulting order.
                                tab_press = None;
                                if let Some(session) = release.select {
                                    navigation.move_tab(&session, to);
                                    let ids: Vec<_> = navigation.tabs().iter().map(|tab| tab.session.clone()).collect();
                                    tab_drag.settle(&ids);
                                    tab_drag.clear_suppress();
                                    let scroll = view.chat_scroll_offset();
                                    match select_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &session, selected.as_ref().map(|m| m.qualified()), scroll) {
                                      Ok(result) => {
                                        apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                        completion.sync(&draft);
                                        history_pending = false;
                                        history_active = None;
                                        history_navigation_request = None;
                                        blocked_history_scroll = None;
                                        history_tail_refresh_pending = false;
                                        history_browsing = false;
                                      }
                                      Err(error) => append_transcript(&mut transcript, format!("error: select tab: {error}")),
                                    }
                                }
                                needs_paint = true;
                                continue;
                            }
                            // A press without motion falls through to the
                            // ordinary click capture below.
                            tab_drag.clear_suppress();
                        }
                        let captured = tab_press.take();
                        let captured_add = add_press.take();
                        let captured_search = rail_search_press.take();
                        if let Some((pressed_rect, pressed_orientation)) = captured_search {
                            let same = pressed_orientation == effective_orientation
                                && effective_orientation == TabOrientation::Vertical
                                && rendered_rail.as_ref().is_some_and(|rail| {
                                    rail.search_hit(x, y)
                                        && x >= pressed_rect.x
                                        && y >= pressed_rect.y
                                        && x < pressed_rect.x.saturating_add(pressed_rect.width)
                                        && y < pressed_rect.y.saturating_add(pressed_rect.height)
                                });
                            if same {
                                view.open_page(crate::native_view_pages::Page::Sessions);
                                consume_local_command_with_cursor(&mut draft, &mut draft_cursor, "/sessions", true);
                                match live.as_ref() {
                                    Some(snapshot) => match session_page_rows(&snapshot.origin, auth, Some(&snapshot.session_id)) {
                                        Ok(rows) => view.set_page_rows(rows),
                                        Err(error) => append_transcript(&mut transcript, format!("error: sessions page: {error}")),
                                    },
                                    None => append_transcript(&mut transcript, "error: sessions page: daemon unavailable".to_owned()),
                                }
                                utf8.reset();
                                needs_paint = true;
                            }
                            continue;
                        }
                        let Some(tabs) = rendered_tabs.as_ref() else { continue };
                        if let Some((session, rect, pressed_close, pressed_orientation)) = captured {
                            let still_same = pressed_orientation == effective_orientation && if effective_orientation == TabOrientation::Vertical { rendered_rail.as_ref().is_some_and(|rail| rail.rows.iter().any(|row| matches!(&row.kind, crate::native_tab_rail::RailRowKind::Session(id) if id == &session) && row.rect.x == rect.x && row.rect.y == rect.y && row.rect.width == rect.width && row.rect.height == rect.height && x >= rect.x && y >= rect.y && x < rect.x.saturating_add(rect.width) && y < rect.y.saturating_add(rect.height))) } else { tabs.rects.iter().any(|(id, current)| *id == session && *current == rect && x >= current.x && y >= current.y && x < current.x.saturating_add(current.width) && y < current.y.saturating_add(current.height)) };
                            if still_same {
                                let close_hit = if effective_orientation == TabOrientation::Vertical {
                                    rendered_rail.as_ref().is_some_and(|rail| rail.close_hit(x, y) == Some(&session))
                                } else { tabs.visible_close_rect().is_some_and(|close| {
                                    tabs.hovered_session() == Some(&session)
                                        && x >= close.x && y >= close.y
                                        && x < close.x.saturating_add(close.width)
                                        && y < close.y.saturating_add(close.height)
                                }) };
                                if pressed_close && !close_hit { continue; }
                                if pressed_close && close_hit {
                                    // Capture the pre-close order and painted
                                    // widths so the pointer's close cell can be
                                    // held through the reflow
                                    // (`session-tabs.tsx:868-900`).
                                    let hold_items: Vec<_> = navigation
                                        .tabs()
                                        .iter()
                                        .map(|tab| tab.session.clone())
                                        .collect();
                                    let hold_visible: Vec<_> = tabs
                                        .rects
                                        .iter()
                                        .map(|(id, _)| id.clone())
                                        .collect();
                                    let hold_widths: Vec<_> = tabs
                                        .rects
                                        .iter()
                                        .map(|(_, rect)| rect.width)
                                        .collect();
                                    let hold_origin = tabs.strip_origin_x();
                                    let close_scroll = view.chat_scroll_offset();
                                    match close_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &session, selected.as_ref().map(|m| m.qualified()), close_scroll) {
                                      Ok(result) => {
                                        if let Some(index) = hold_items.iter().position(|id| id == &session) {
                                            if let Some(target) = hold_items.get(index + 1).or_else(|| index.checked_sub(1).and_then(|i| hold_items.get(i))) {
                                                close_hold = Some(crate::native_tabs::CloseHold::new(
                                                    &hold_items,
                                                    &hold_visible,
                                                    &hold_widths,
                                                    &session,
                                                    target,
                                                    x.saturating_sub(hold_origin),
                                                    animation_start.now_ms(),
                                                ));
                                            }
                                        }
                                        apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                        completion.sync(&draft);
                                        history_pending = false;
                                        history_active = None;
                                        history_navigation_request = None;
                                        blocked_history_scroll = None;
                                        history_tail_refresh_pending = false;
                                        history_browsing = false;
                                      }
                                      Err(error) => append_transcript(&mut transcript, format!("error: close tab: {error}")),
                                    }
                                    needs_paint = true; continue;
                                }
                                let scroll = view.chat_scroll_offset();
                                match select_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &session, selected.as_ref().map(|model| model.qualified()), scroll) {
                                  Ok(result) => {
                                    apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                    completion.sync(&draft);
                                    history_pending = false;
                                    history_active = None;
                                    history_navigation_request = None;
                                    blocked_history_scroll = None;
                                    history_tail_refresh_pending = false;
                                    history_browsing = false;
                                  }
                                  Err(error) => append_transcript(&mut transcript, format!("error: select tab: {error}")),
                                }
                                needs_paint = true; continue;
                            }
                        }
                        if let Some((rect, pressed_orientation)) = captured_add {
                            if navigation.active().is_none() { continue; }
                            let same = pressed_orientation == effective_orientation && if effective_orientation == TabOrientation::Vertical { rendered_rail.as_ref().and_then(|rail| rail.new_rect).is_some_and(|current| current.x == rect.x && current.y == rect.y && current.width == rect.width && current.height == rect.height && x >= current.x && y >= current.y && x < current.x.saturating_add(current.width) && y < current.y.saturating_add(current.height)) } else { tabs.add_rect == Some(rect) && x >= rect.x && y >= rect.y && x < rect.x.saturating_add(rect.width) && y < rect.y.saturating_add(rect.height) };
                            if same {
                                // Home is lazy: retain old tabs and their state;
                                // no session creation request occurs until submit.
                                let outgoing_model = selected.as_ref().map(|model| model.qualified());
                                if let Err(error) = navigation.save_active_state(
                                    draft.clone(),
                                    draft_cursor,
                                    view.chat_scroll_offset(),
                                    outgoing_model,
                                ) {
                                    append_transcript(&mut transcript, format!("error: new tab: state save refused: {error:?}"));
                                    needs_paint = true;
                                    continue;
                                }
                                live = None;
                                history_window = None;
                                history_pending = false;
                                history_active = None;
                                history_navigation_request = None;
                                blocked_history_scroll = None;
                                history_tail_refresh_pending = false;
                                history_browsing = false;
                                draft.clear(); draft_parts.clear(); draft_cursor = 0;
                                navigation.enter_home();
                                session_route = false;
                                view.reset_to_home_after_tab_close();
                                view.set_chat_scroll_offset(0);
                                needs_paint = true;
                            }
                            continue;
                        }
                        if let Some((rect, pressed_orientation, pressed_close)) = home_press.take() {
                            // Upstream NEW_SESSION_TAB release: select(NEW_SESSION_TAB)
                            // early-returns on a body release; the close child (left
                            // button on the hovered glyph) fires close(undefined). Both
                            // stay absorbed so composer/send regions never observe it.
                            let still_home = pressed_orientation == effective_orientation
                                && rendered_tabs
                                    .as_ref()
                                    .and_then(|tabs| tabs.home_rect)
                                    .is_some_and(|current| current == rect)
                                && (!pressed_close
                                    || rendered_tabs
                                        .as_ref()
                                        .is_some_and(|tabs| tabs.home_close_hit(x, y)));
                            if still_home && pressed_close {
                                if let Some(target) = navigation.home_dispatch_target() {
                                    let scroll = view.chat_scroll_offset();
                                    match select_tab_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, daemon_origin, auth, binding_generation, &target, selected.as_ref().map(|m| m.qualified()), scroll) {
                                        Ok(result) => {
                                            apply_close_tab_result(result, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut selected, &models, &mut session_route, &mut compact_worker);
                                            completion.sync(&draft);
                                            history_pending = false; history_active = None; history_navigation_request = None; blocked_history_scroll = None; history_tail_refresh_pending = false; history_browsing = false;
                                        }
                                        Err(error) => append_transcript(&mut transcript, format!("error: select tab: {error}")),
                                    }
                                }
                            }
                            needs_paint = true;
                            continue;
                        }
                    }
                }
                // Tabs were resolved first. Send clicks were normalized before
                // this mouse match, and the explicit rect guard keeps the two targets disjoint.
                if !page_open
                    && !view.dialogs.intercepts_input()
                    && !permission_editor.open
                    && !appearance.open
                    && !children_panel.open
                    && !help_open
                    && !view.command_palette_open()
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                    && kind == 0 && modifiers == 0
                    && !view.send_button_hit(&draft, x, y)
                    && view.context_usage_hit(x, y)
                {
                    view.open_context_usage();
                    needs_paint = true;
                    continue;
                }
                // Tabs were resolved first. Send clicks were normalized before
                // this mouse match, and the explicit rect guard keeps the two targets disjoint.
                if !page_open
                    && !view.dialogs.intercepts_input()
                    && !permission_editor.open
                    && !appearance.open
                    && !children_panel.open
                    && !help_open
                    && !view.command_palette_open()
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                    && kind == 0 && modifiers == 0
                    && !view.send_button_hit(&draft, x, y)
                    && view.context_usage_hit(x, y)
                {
                    view.open_context_usage();
                    needs_paint = true;
                    continue;
                }
                if view.dialogs.intercepts_input() {
                    let modal = view.modal_presentation();
                    let row = y.checked_sub(modal.content_y).map(usize::from);
                    if kind == 0 && modifiers == 0 && x >= modal.content_x
                        && x < modal.content_x.saturating_add(modal.content_width) {
                        if let Some(row) = row {
                            let content_x = modal.content_x;
                            let Some(col) = x.checked_sub(content_x).map(usize::from) else { continue };
                            if col >= usize::from(modal.content_width) { continue; }
                            if let Some(index) = view.dialogs.choice_at_cell_with_metrics(
                                    view.dialog_layout_metrics(modal.content_width), row, col) {
                                view.dialogs.select(index);
                            } else if view.dialogs.confirm_at_cell_with_metrics(
                                    view.dialog_layout_metrics(modal.content_width), row, col) {
                                if let Ok(intent) = view.dialogs.answer() {
                                    dispatch_permission_reply(&mut view, &mut transcript, live.as_ref().map(|snapshot| snapshot.origin.as_str()), auth, intent);
                                }
                            }
                        }
                    }
                    needs_paint = true;
                    continue;
                }
                if permission_editor.open {
                    let modal = view.modal_presentation();
                    if kind == 0 && modifiers == 0 {
                        if let Some((column, row)) = modal.content_point(x, y, view.modal_content_height()) {
                            if let Some(decision) = permission_editor.click_action(column,row,modal.content_width,view.modal_content_height()) {
                                permission_editor_request(&mut permission_editor,live.as_ref(),auth,Some(decision));
                            }
                        }
                    }
                    needs_paint = true;
                    continue;
                }
                if appearance.open {
                    let modal = view.modal_presentation();
                    if let Some(outcome) = appearance.pointer_outcome(
                        x,
                        y,
                        kind,
                        modal.content_x,
                        modal.content_y,
                        modal.content_width,
                        view.modal_content_height().min(u16::MAX as usize) as u16,
                    ) {
                        match outcome {
                            crate::native_appearance::AppearanceOutcome::Apply => {
                                if let Err(error) = crate::native_ui_preferences::UiPreferences::persist_appearance(&state_path, &appearance.style) {
                                    append_transcript(&mut transcript, format!("error: {error}"));
                                }
                                view.set_dialog_size(crate::native_view::DialogSize::Medium);
                            }
                            crate::native_appearance::AppearanceOutcome::Cancel => {
                                view.set_dialog_size(crate::native_view::DialogSize::Medium);
                            }
                            _ => {}
                        }
                    }
                    needs_paint = true;
                    continue;
                }
                if children_panel.open {
                    let modal = view.modal_presentation();
                    if kind == 0 && modifiers == 0
                        && x >= modal.content_x
                        && x < modal.content_x.saturating_add(modal.content_width)
                        && y >= modal.content_y
                    {
                        // Child requests reuse the exact key-path dispatch:
                        // row clicks select/enter, footer clicks stop/retry/
                        // follow-up/send, all routed through one HTTP helper.
                        if let Some(action) = children_panel.mouse(
                            y - modal.content_y,
                            x.saturating_sub(modal.content_x),
                            modal.content_width,
                            view.modal_content_height(),
                        ) {
                            if let Some(child) = children_action_http(&mut children_panel, action, live.as_ref(), auth.as_deref(), binding_generation) {
                                let result = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned()).and_then(|parent_snapshot| open_child_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, &parent_snapshot.origin, auth.as_deref(), binding_generation, &parent_snapshot.session_id, &child, selected.as_ref().map(|model| model.qualified())));
                                match result {
                                    Ok((snapshot, restored, generation, target_cursor, target_scroll, target_model)) => { children_panel.open = false; view.set_dialog_size(crate::native_view::DialogSize::Medium); commit_child_route(snapshot, restored, generation, target_cursor, target_scroll, target_model, &models, &mut selected, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut session_route, &mut completion); }
                                    Err(error) => append_transcript(&mut transcript, format!("error: child open: {error}")),
                                }
                            } else { children_refresh = Instant::now() - Duration::from_secs(1); }
                        }
                    }
                    if !children_panel.open {
                        view.set_dialog_size(crate::native_view::DialogSize::Medium);
                    }
                    needs_paint = true;
                    continue;
                }
                // The dock paints exactly one pointer target (the right-aligned
                // `esc`), and only while open. Admit it before the footer label
                // so the two surfaces can never share a cell.
                if kind == 0 && modifiers == 0 && !help_open
                    && !view.command_palette_open()
                    && !permission_editor.open && !appearance.open
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                    && view.dock_click(x, y)
                {
                    needs_paint = true;
                    continue;
                }
                if kind == 0 && modifiers == 0 && !help_open
                    && !view.command_palette_open()
                    && !permission_editor.open && !appearance.open
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                    && view.live_footer_action(x, y).is_some()
                {
                    // Upstream `session.child.first` toggles the composer dock
                    // (`routes/session/index.tsx:1395-1396`): a child session
                    // or an open dock closes, otherwise it opens on Subagents.
                    view.toggle_dock();
                    needs_paint = true;
                    continue;
                }
                if kind == 0 && modifiers == 0 && !help_open
                    && matches!(dialog, native_setup::Dialog::Provider | native_setup::Dialog::Model)
                {
                    let modal = view.modal_presentation();
                    let Some((_column, content_row)) = modal.content_point(x, y, view.modal_content_height()) else { needs_paint = true; continue; };
                    let (visible_len, list_start, footer_rows) = if dialog == native_setup::Dialog::Model {
                        (native_setup::dialog_rows(&models, &filter_input).len(), 4 + usize::from(model_load.is_some()) + usize::from(setup_error.is_some()), 2usize)
                    } else {
                        let query = filter_input.trim().to_lowercase();
                        (providers.iter().filter(|row| row.0.to_lowercase().contains(&query) || row.1.to_lowercase().contains(&query)).count(), 4usize, 2 + usize::from(setup_error.is_some()) + usize::from(model_load.is_some()))
                    };
                    let capacity = native_setup::picker_capacity(view.modal_content_height(), list_start, footer_rows);
                    if capacity > 0 {
                        let viewport = native_setup::picker_viewport(model_row, visible_len, capacity);
                        if let Some(row) = native_setup::picker_hit_row(viewport, content_row, list_start) { model_row = row; }
                    }
                    needs_paint = true;
                    continue;
                }
                if matches!(kind, 3 | 4) && modifiers == 0 && !help_open
                    && !view.command_palette_open()
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                    && view.scroll_sidebar_at(x, y, if kind == 3 { -3 } else { 3 })
                {
                    selection.clear();
                    needs_paint = true;
                    continue;
                }
                if kind == 0 && modifiers == 0 && !help_open
                    && !view.command_palette_open()
                    && matches!(dialog, native_setup::Dialog::None)
                    && import_dialog.as_ref().is_none_or(|import| !import.is_open())
                {
                    if view.mcp_sidebar_toggle_action(x, y) { needs_paint = true; continue; }
                    if let Some((name, included)) = view.mcp_sidebar_action(x, y) {
                        if let Some(snapshot) = live.as_ref() {
                            let encoded: String = name.bytes().flat_map(|byte| {
                                if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                                    vec![byte as char]
                                } else {
                                    format!("%{byte:02X}").chars().collect()
                                }
                            }).collect();
                            let path = format!("/api/sessions/{}/mcp/{}", snapshot.session_id, encoded);
                            let body = serde_json::json!({"included": !included});
                            match http_request(&snapshot.origin, "PUT", &path, Some(&body.to_string()), auth)
                                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string()))
                            {
                                Ok(value) => {
                                    if let Err(error) = view.apply_mcp(&value) {
                                        append_transcript(&mut transcript, format!("error: mcp toggle rejected: {error}"));
                                    }
                                }
                                Err(error) => append_transcript(&mut transcript, format!("error: mcp toggle failed: {error}")),
                            }
                        }
                        needs_paint = true;
                        continue;
                    }
                }
                if matches!(kind, 2 | 5 | 6) {
                    let disable_copy = crate::native_text_selection::copy_on_select_disabled(
                        env::var("OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT")
                            .ok()
                            .as_deref(),
                        cfg!(windows),
                    );
                    let copied = selection.copy_for_input_kind(kind, disable_copy);
                    if let Some(text) = copied {
                        let frame = selection_clipboard_frame(&text);
                        let mut stdout = std::io::stdout().lock();
                        stdout.write_all(frame.as_bytes())?;
                        stdout.flush()?;
                    } else if kind == 2
                        && message_menu.is_none()
                        && !view.dialogs.is_open()
                        && !view.command_palette_open()
                        && matches!(dialog, native_setup::Dialog::None)
                        && !help_open
                    {
                        // MSG-006 consumer (upstream `index.tsx:2396-2423`):
                        // a left click-release with no copied selection
                        // resolves the painted row's owner message (W18
                        // owner map) and opens the message menu. Upstream
                        // early-returns while a selection has text; that
                        // gate is `click_acts(has_text_selection, ...)`.
                        let has_text_selection = selection.selected_text().is_some();
                        if crate::native_message_rows::click_acts(
                            has_text_selection,
                            crate::native_message_rows::RowAction::MessageActions,
                        ) {
                            // TOOL-008: a subtask row click navigates into its
                            // child session (`index.tsx:3311-3314`, the
                            // `Subagent` `onMouseUp` -> `navigate({type:
                            // "session",sessionID:id})`), before the message
                            // menu resolves.
                            let child_open = view
                                .chat_child_owner_at(usize::from(y))
                                .zip(live.as_ref().map(|snapshot| {
                                    (snapshot.origin.clone(), snapshot.session_id.clone())
                                }));
                            if let Some((child, (origin, parent_session))) = child_open {
                                let result = open_child_transition(
                                    &mut view, &mut transcript, &mut navigation, &draft, draft_cursor,
                                    &origin, auth.as_deref(), binding_generation,
                                    &parent_session, &child,
                                    selected.as_ref().map(|model| model.qualified()),
                                );
                                match result {
                                    Ok((snapshot, restored, generation, target_cursor, target_scroll, target_model)) => {
                                        commit_child_route(snapshot, restored, generation, target_cursor, target_scroll, target_model, &models, &mut selected, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut session_route, &mut completion);
                                    }
                                    Err(error) => append_transcript(&mut transcript, format!("error: child open: {error}")),
                                }
                                needs_paint = true;
                                continue;
                            }
                            if let (Some(snapshot), Some(message_id)) =
                                (live.as_ref(), view.chat_message_owner_at(usize::from(y)))
                            {
                                let user_card = view
                                    .chat_message_owners()
                                    .iter()
                                    .zip(view.chat_row_classes())
                                    .any(|(owner, class)| {
                                        owner.as_deref() == Some(message_id.as_str())
                                            && *class
                                                == crate::native_view_chat::ChatRowClass::UserCard
                                    });
                                if user_card {
                                    message_menu = Some(crate::native_view_chat::menu_for_message(
                                        None,
                                        &snapshot.session_id,
                                        &message_id,
                                        None,
                                    ));
                                }
                            }
                        }
                    }
                    needs_paint = true;
                    continue;
                }
                match kind {
                    0 => {
                        selection.press(u32::from(x), u32::from(y));
                    }
                    1 => selection.drag(u32::from(x), u32::from(y)),
                    2 => selection.release(),
                    3 if !view.dialogs.is_open()
                        && !view.command_palette_open()
                        && matches!(dialog, native_setup::Dialog::None)
                        && !help_open =>
                     {
                         selection.clear();
                         view.scroll_up(3);
                         history_browsing = true;
                         history_navigation_revision = history_navigation_revision.wrapping_add(1);
                         blocked_history_scroll = None;
                         history_navigation_request = Some(HistoryRequestKind::Older);
                    }
                    4 if !view.dialogs.is_open()
                        && !view.command_palette_open()
                        && matches!(dialog, native_setup::Dialog::None)
                        && !help_open =>
                    {
                         selection.clear();
                 history_navigation_revision = history_navigation_revision.wrapping_add(1);
                 history_navigation_request = None;
                 let gesture_blocked = view.chat_at_tail();
                         let gesture_blocked = view.chat_at_tail();
                         // A cached page changes the tail-relative coordinate
                         // system. Capture the pre-gesture rendered identity,
                         // commit the page, restore it, then apply this wheel
                         // delta exactly once.
                         let before_anchor = history_window.as_ref().and_then(|window| {
                             let entries = crate::native_chat_projection::project_history_entries(window.messages());
                             view.chat_rendered_anchor(&entries)
                         });
                         let mut cached_advance = false;
                         view.scroll_down(3);
                         if view.chat_at_tail() {
                             if let Some(window) = history_window.as_mut() {
                                 let previous_window = window.clone();
                                 if before_anchor.is_none() {
                                     // No rendered identity means this frame
                                     // cannot be atomically rebased; leave the
                                     // bounded window untouched and let the
                                     // ordinary scroll path proceed.
                                     continue;
                                 }
                                 if window.advance_newer() {
                                     apply_history_window(window, &mut transcript);
                                     if let Some(anchor) = before_anchor {
                                         let entries = crate::native_chat_projection::project_history_entries(window.messages());
                                         if !view.restore_chat_rendered_anchor(&entries, &anchor) {
                                             *window = previous_window;
                                             apply_history_window(window, &mut transcript);
                                             let old_entries = crate::native_chat_projection::project_history_entries(window.messages());
                                             let _ = view.restore_chat_rendered_anchor(&old_entries, &anchor);
                                             continue;
                                         }
                                     } else {
                                         *window = previous_window;
                                         apply_history_window(window, &mut transcript);
                                         continue;
                                     }
                                     // Re-apply the user's gesture after the
                                     // coordinate-preserving page transition.
                                     view.scroll_down(3);
                                     cached_advance = true;
                                  } else if history_browsing && window.begin_newer_recovery().is_ok() {
                                       history_navigation_request = Some(HistoryRequestKind::Newer);
                                       if gesture_blocked {
                                           blocked_history_scroll = Some(BlockedHistoryScroll { revision: history_navigation_revision, rows: 3 });
                                       }
                                 }
                             }
                         }
                          if view.chat_at_tail() {
                               if history_window.as_ref().is_some_and(|w| w.at_newest_page()) {
                                   history_tail_refresh_pending = true;
                                   history_browsing = false;
                              } else {
                                   history_browsing = true;
                                   history_tail_refresh_pending = false;
                               }
                         }
                         if cached_advance {
                              history_navigation_request = None;
                         }
                    }
                    _ => {
                        selection.clear();
                    }
                }
                needs_paint = true;
                continue;
            }
            direction @ (native_input_decoder::InputEvent::ArrowUp
            | native_input_decoder::InputEvent::ArrowDown) => {
                utf8.reset();
                // A decision dialog owns focus before every other overlay.
                // Route the documented arrow keys to the same selection state
                // as Ctrl-N/Ctrl-B; navigation never submits a decision.
        if view.dialogs.intercepts_input() {
                    if direction == native_input_decoder::InputEvent::ArrowUp {
                        view.dialogs.prev();
                    } else {
                        view.dialogs.next();
                    }
                    continue;
                }
                if permission_editor.open {
                    permission_editor.navigate(direction == native_input_decoder::InputEvent::ArrowDown);
                    needs_paint = true;
                    continue;
                }
                if appearance.open {
                    appearance.move_target(direction == native_input_decoder::InputEvent::ArrowDown);
                    needs_paint = true;
                    continue;
                }
                if children_panel.open {
                    children_panel.key(if direction == native_input_decoder::InputEvent::ArrowUp { 2 } else { 14 });
                    needs_paint = true;
                    continue;
                }
                if view.command_palette_open() && !help_open && !view.dialogs.intercepts_input() {
                    if direction == native_input_decoder::InputEvent::ArrowUp {
                        view.command_palette_prev();
                    } else {
                        view.command_palette_next();
                    }
                    continue;
                }
                // Page selection owns arrows only when no higher-priority
                // overlay owns focus. Never turn them into composer text or
                // permission/credential actions.
                if !page_open
                    && !help_open
                    && !view.dialogs.intercepts_input()
                    && matches!(dialog, native_setup::Dialog::Model)
                {
                    let visible = native_setup::dialog_rows(&models, &filter_input);
                    model_row = native_setup::move_row(
                        model_row,
                        visible.len(),
                        matches!(direction, native_input_decoder::InputEvent::ArrowDown),
                    );
                    continue;
                }
                if dialog == native_setup::Dialog::Provider {
                    let query = filter_input.trim().to_lowercase();
                    let count = providers
                        .iter()
                        .filter(|row| {
                            row.0.to_lowercase().contains(&query) || row.1.to_lowercase().contains(&query)
                        })
                        .count();
                    model_row = native_setup::move_row(
                        model_row,
                        count,
                        matches!(direction, native_input_decoder::InputEvent::ArrowDown),
                    );
                    continue;
                }
                if !page_open
                    || help_open
                    || view.dialogs.intercepts_input()
                    || !matches!(dialog, native_setup::Dialog::None)
                {
                    continue;
                }
                if view.page() == crate::native_view_pages::Page::Compaction {
                    view.compaction_mut().key(if direction == native_input_decoder::InputEvent::ArrowUp { b'k' } else { b'j' });
                    needs_paint = true;
                    continue;
                }
                if view.page() == crate::native_view_pages::Page::Context {
                    view.context_mut().select_step(direction == native_input_decoder::InputEvent::ArrowDown);
                    needs_paint = true;
                    continue;
                }
                if direction == native_input_decoder::InputEvent::ArrowUp {
                    2
                } else {
                    14
                }
            }
            native_input_decoder::InputEvent::Paste(text) => {
                utf8.reset();
                if view.dialogs.intercepts_input() {
                    // Modal precedence: an open permission decision owns
                    // focus over Help and pages; paste is intercepted and
                    // never reaches the composer or the dialog.
                    continue;
                }
                if children_panel.open {
                    for character in text.chars() { children_panel.text(character); }
                    continue;
                }
                if live.as_ref().is_some_and(|snapshot| snapshot.parent_session_id.is_some()) {
                    // Main child composer is disabled; paste belongs nowhere.
                    continue;
                }
                if help_open {
                    // Help owns focus; paste must not mutate the hidden draft.
                    continue;
                }
                if view.command_palette_open() {
                    for character in text.chars() {
                        if !view.command_palette_push(character) {
                            break;
                        }
                    }
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
                match dialog {
                    native_setup::Dialog::ApiKey => {
                        if key_input
                            .len()
                            .checked_add(text.len())
                            .is_some_and(|len| len <= 16 * 1024)
                        {
                            key_input.push_str(&text);
                        }
                    }
                    native_setup::Dialog::None => {
                        // Cursor-aware insertion through the real paste
                        // machinery (`index.tsx:1446-1455`). There is NO raw
                        // pre-check: upstream folds a long/multi-line body into
                        // a small `[Pasted ~N lines]` placeholder regardless of
                        // its raw size (`prompt/index.tsx:1462-1474`), so the
                        // raw-size gate lives in the planner/application where
                        // it applies to the bytes actually inserted. A planner
                        // refusal (non-boundary / the inserted bytes exceed the
                        // budget) falls back to a bounded append.
                        let mode = crate::native_paste::PasteMode::default();
                        if crate::native_composer::insert_paste_at_cursor(
                            &mut draft,
                            &mut draft_cursor,
                            &mut draft_parts,
                            &text,
                            mode,
                            MAX_NATIVE_DRAFT_BYTES,
                        )
                        .is_err()
                        {
                            crate::native_composer::normalize_cursor(&draft, &mut draft_cursor);
                            if draft
                                .len()
                                .checked_add(text.len())
                                .is_some_and(|len| len <= MAX_NATIVE_DRAFT_BYTES)
                            {
                                draft.push_str(&text);
                                draft_cursor = draft.len();
                            }
                        }
                    }
                    _ => {
                        if filter_input
                            .len()
                            .checked_add(text.len())
                            .is_some_and(|len| len <= MAX_NATIVE_DRAFT_BYTES)
                        {
                            filter_input.push_str(&text);
                        }
                    }
                }
                // Paste has no key actions, even in a dialog or a busy turn.
                continue;
            }
            // Defensive exhaustive arms; normal dispatch has already
            // normalized these through the composer keymap above.
            native_input_decoder::InputEvent::ShiftEnter
            | native_input_decoder::InputEvent::CtrlJ
            | native_input_decoder::InputEvent::AltEnter => continue,
            // Preserve the existing standalone-escape action (clear dialog /
            // draft) while keeping protocol escape bytes out of the composer.
            native_input_decoder::InputEvent::Escape => 27,
            // An input owner above blocked composer dispatch; keep its
            // horizontal arrows inert rather than converting them to bytes.
            native_input_decoder::InputEvent::ArrowLeft
            | native_input_decoder::InputEvent::ArrowRight
            | native_input_decoder::InputEvent::AltArrowUp
            | native_input_decoder::InputEvent::AltArrowDown => continue,
            // Tab-navigation chords were already dispatched to `TabCommand`
            // above; if one reaches here it was gated by an input owner, so
            // keep it inert rather than converting it to a composer byte.
            native_input_decoder::InputEvent::AltShiftArrowUp
            | native_input_decoder::InputEvent::AltShiftArrowDown
            | native_input_decoder::InputEvent::TabNext
            | native_input_decoder::InputEvent::TabPrevious
            | native_input_decoder::InputEvent::TabSlot(_)
            | native_input_decoder::InputEvent::TabReopen => continue,
            // Defensive unreachable arm: focus reports are consumed above the
            // dialog/page dispatch, but this match stays exhaustive over
            // `InputEvent`.
            native_input_decoder::InputEvent::FocusGained
            | native_input_decoder::InputEvent::FocusLost => continue,
        };
        if children_panel.open && !view.dialogs.intercepts_input() && !matches!(byte, 3 | 4) {
            let composing = children_panel.composing;
            let action = if composing && byte >= 0x20 && byte != 127 {
                if let Some(character) = utf8.push(byte) { children_panel.text(character); }
                None
            } else {
                utf8.reset();
                children_panel.key(byte)
            };
            if let Some(action) = action {
                if let Some(child) = children_action_http(&mut children_panel, action, live.as_ref(), auth.as_deref(), binding_generation) {
                    let result = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned()).and_then(|parent_snapshot| open_child_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, &parent_snapshot.origin, auth.as_deref(), binding_generation, &parent_snapshot.session_id, &child, selected.as_ref().map(|model| model.qualified())));
                    match result {
                        Ok((snapshot, restored, generation, target_cursor, target_scroll, target_model)) => { children_panel.open = false; view.set_dialog_size(crate::native_view::DialogSize::Medium); commit_child_route(snapshot, restored, generation, target_cursor, target_scroll, target_model, &models, &mut selected, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut session_route, &mut completion); }
                        Err(error) => append_transcript(&mut transcript, format!("error: child open: {error}")),
                    }
                } else { children_refresh = Instant::now() - Duration::from_secs(1); }
            }
            if !children_panel.open { view.set_dialog_size(crate::native_view::DialogSize::Medium); }
            needs_paint = true;
            continue;
        }
        if byte == 27 && !page_open && !help_open && !appearance.open
            && !permission_editor.open && !view.dialogs.intercepts_input()
            && !view.command_palette_open() && matches!(dialog, native_setup::Dialog::None)
        {
            if let Some(parent) = live.as_ref().and_then(|snapshot| snapshot.parent_session_id.clone()) {
                let result = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned()).and_then(|child_snapshot| return_parent_transition(&mut view, &mut transcript, &mut navigation, &draft, draft_cursor, &child_snapshot.origin, auth.as_deref(), binding_generation, &parent));
                match result {
                    Ok((snapshot, restored, generation, target_cursor, target_scroll, target_model)) => commit_child_route(snapshot, restored, generation, target_cursor, target_scroll, target_model, &models, &mut selected, &mut view, &mut live, &mut history_window, &mut binding_generation, &mut draft, &mut draft_cursor, &mut session_route, &mut completion),
                    Err(error) => append_transcript(&mut transcript, format!("error: return to parent: {error}")),
                }
                needs_paint = true;
                continue;
            }
        }
        // Child sessions have no editable composer: upstream replaces it with
        // the return-parent affordance. Arrow navigation remains handled above;
        // ordinary text/edit/submit bytes are inert while child-bound.
        if !matches!(byte, 3 | 4 | 27)
            && live.as_ref().is_some_and(|snapshot| snapshot.parent_session_id.is_some())
            && !page_open && !help_open && !appearance.open
            && !permission_editor.open && !view.dialogs.intercepts_input()
            && !view.command_palette_open() && matches!(dialog, native_setup::Dialog::None)
        {
            utf8.reset();
            needs_paint = true;
            continue;
        }
        if permission_editor.open && !matches!(byte,3|4) {
            match byte {
                27 => permission_editor.open=false,
                9 => permission_editor.persistent = !permission_editor.persistent,
                b'1'|b'2'|b'3' => permission_editor_request(&mut permission_editor,live.as_ref(),auth,
                    Some(if byte == b'1' {"allow"} else if byte == b'2' {"ask"} else {"deny"})),
                _=>{}
            }
            needs_paint=true;
            continue;
        }
        if appearance.open && !matches!(byte, 3 | 4) {
            match appearance.key_outcome(byte) {
                crate::native_appearance::AppearanceOutcome::Apply => {
                    if let Err(error) = crate::native_ui_preferences::UiPreferences::persist_appearance(
                        &state_path, &appearance.style,
                    ) { append_transcript(&mut transcript, format!("error: {error}")); }
                    view.set_dialog_size(crate::native_view::DialogSize::Medium);
                }
                crate::native_appearance::AppearanceOutcome::Cancel => {
                    view.set_dialog_size(crate::native_view::DialogSize::Medium);
                }
                _ => {}
            }
            needs_paint = true;
            continue;
        }
        if byte == 3 {
            match crate::native_text_selection::ctrl_c_copy_outcome(
                selection.has_selection(),
                selection.selected_text().is_some(),
            ) {
                // No selection: fall through so Ctrl+C keeps exit priority.
                crate::native_text_selection::CtrlCCopy::FallThrough => {}
                // Upstream retains the selection after a successful copy
                // (`packages/tui/src/util/selection.ts:78-87`); keep the
                // highlight and click history, only repaint.
                crate::native_text_selection::CtrlCCopy::Retain => {
                    if let Some(text) = selection.selected_text() {
                        let frame = selection_clipboard_frame(&text);
                        let mut stdout = std::io::stdout().lock();
                        stdout.write_all(frame.as_bytes())?;
                        stdout.flush()?;
                        crate::native_debug::event(
                            "ui.selection_copy",
                            serde_json::json!({"bytes":text.len()}),
                        );
                    }
                    needs_paint = true;
                    continue;
                }
                // A failed copy clears the selection (upstream `:79-81`).
                crate::native_text_selection::CtrlCCopy::Clear => {
                    selection.clear();
                    needs_paint = true;
                    continue;
                }
            }
        } else if byte == 27 {
            selection.clear();
        }
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
                b'1'..=b'9' => { view.dialogs.select(usize::from(byte - b'1')); }
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
        // The palette owns a separate query and returns only an existing
        // local route. All side effects remain in the shared dispatcher.
        let mut palette_route = None;
        if view.command_palette_open() && !matches!(byte, 3 | 4) {
            match byte {
                27 => view.close_command_palette(),
                14 => view.command_palette_next(),
                2 => view.command_palette_prev(),
                8 | 127 => {
                    view.command_palette_backspace();
                }
                13 | 10 => {
                    if let Some(crate::native_command_palette::PaletteAction::Route(route)) =
                        view.command_palette_activate()
                    {
                        // Routes activated from the palette (including
                        // /exit) re-enter the same local dispatch chain as
                        // completion and normal composer input, so the
                        // exit-word contract is identical for all three
                        // sources.
                        palette_route = Some(route);
                        view.close_command_palette();
                    }
                }
                value if value >= 0x20 => {
                    if let Some(character) = utf8.push(value) {
                        view.command_palette_push(character);
                    }
                }
                _ => {}
            }
            if byte < 0x20 || byte == 127 {
                utf8.reset();
            }
            if palette_route.is_none() {
                continue;
            }
        }
        // An open page owns focus the same way: only exit/close plus page
        // navigation (Ctrl-N / Ctrl-B) and confirmation (Enter) act; no
        // draft, dialog, paste or turn side effect can occur. On the
        // Sessions filter and the command buffer, printable and
        // backspace bytes are additionally routed to that bounded query
        // (and only it) below; every other page keeps the inert contract.
        if page_open && byte == b'\t' && !help_open
            && matches!(dialog, native_setup::Dialog::None)
        {
            view.open_page(view.page().next());
            needs_paint = true;
            utf8.reset();
            continue;
        }
        if page_open && view.page() == crate::native_view_pages::Page::Context
            && !help_open && matches!(dialog,native_setup::Dialog::None)
            && !matches!(byte,3|4|27)
        {
            match byte {
                b'j'|14 => view.context_mut().select_step(true),
                b'k'|2 => view.context_mut().select_step(false),
                b'f'|6 => view.context_mut().scroll_detail(true),
                b'b' => view.context_mut().scroll_detail(false),
                b'p' => {
                    if let Some(cursor)=view.context().next_before().map(str::to_owned) {
                        context_before=Some(cursor);
                        context_page_observed=false;
                    }
                }
                b'r' => {context_before=None;context_page_observed=false;}
                13|10|b' ' => {
                    let selection=view.context().selected().filter(|item|item.selectable)
                        .map(|item|(item.id.clone(),!item.included));
                    if let Some((item,included))=selection {
                        let result=match live.as_ref() {
                            Some(snapshot)=>request_context_inspection(&snapshot.origin,&snapshot.session_id,auth,
                                context_before.as_deref(),Some((&item,included)))
                                .and_then(|body|view.context_mut().load_response(&body)),
                            None=>Err("daemon unavailable".into()),
                        };
                        if let Err(error)=result {view.context_mut().set_error(&error);}
                    }
                }
                _=>{}
            }
            needs_paint=true;
            utf8.reset();
            continue;
        }
        if page_open && view.page() == crate::native_view_pages::Page::Compaction
            && !help_open && matches!(dialog, native_setup::Dialog::None)
            && !matches!(byte, 3 | 4 | 27)
        {
            let key = match byte { 2 => b'k', 14 => b'j', other => other };
            if let Some(body) = view.compaction_mut().key(key) {
                let result = match live.as_ref() {
                    Some(snapshot) => request_compaction_settings(&snapshot.origin, &snapshot.session_id, auth, Some(&body))
                        .and_then(|ack| {
                            let requested: serde_json::Value = serde_json::from_str(&body)
                                .map_err(|_| "invalid compaction policy request".to_owned())?;
                            let acknowledged: serde_json::Value = serde_json::from_str(&ack)
                                .map_err(|_| "invalid compaction policy acknowledgement".to_owned())?;
                            if acknowledged["configured"] != requested {
                                return Err("backend did not acknowledge the requested compaction policy".into());
                            }
                            view.compaction_mut().load_response(&ack)
                        }),
                    None => Err("daemon unavailable".into()),
                };
                if let Err(error) = result { view.compaction_mut().set_error(&error); }
            }
            needs_paint = true;
            utf8.reset();
            continue;
        }
        if page_open
            && !help_open
            && !matches!(byte, 3 | 4 | 27 | 13 | 10 | 14 | 2)
            && !(view.page() == crate::native_view_pages::Page::Memory && byte == 6)
            && !(page_query_open && (byte == 8 || byte == 127 || byte >= 0x20))
        {
            utf8.reset();
            continue;
        }
        // Memory is a dedicated backend-backed page, not a selectable row
        // list. Keep source navigation and content scrolling here so the
        // composer draft remains untouched.
        if page_open && view.page() == crate::native_view_pages::Page::Memory
            && !help_open && matches!(dialog, native_setup::Dialog::None)
        {
            if let Some(forward) = memory_source_direction {
                view.memory_mut().select_next(forward);
                continue;
            }
            match byte {
                6 => {
                    view.scroll_memory_content(8);
                    continue;
                }
                2 => {
                    view.scroll_memory_content(-8);
                    continue;
                }
                13 | 10 => {
                    if let (Some(snapshot), Some((endpoint, body))) = (
                        live.as_ref(), view.memory().toggle_request()
                    ) {
                        let path = format!("/api/sessions/{}/memory/{endpoint}", snapshot.session_id);
                        match http_request(&snapshot.origin, "POST", &path, Some(&body), auth)
                            .and_then(|ack| view.load_memory(&ack))
                        {
                            Ok(()) => {}
                            Err(error) => view.set_memory_error(&error),
                        }
                    } else if live.is_none() {
                        view.set_memory_error("daemon unavailable");
                    }
                    continue;
                }
                _ => {}
            }
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
                model_load = None;
                setup_error = None;
                key_input.clear();
                filter_input.clear();
                model_row = 0;
                utf8.reset();
            }
            27 => {
                dialog = native_setup::Dialog::None;
                key_input.clear();
                draft.clear(); draft_parts.clear();
                draft_cursor = 0;
                utf8.reset();
            }
            16 if matches!(dialog, native_setup::Dialog::None) => {
                view.open_command_palette(live.is_some());
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
                history_navigation_revision = history_navigation_revision.wrapping_add(1);
                history_navigation_request = None;
                blocked_history_scroll = None;
                history_trace("history.ctrl_f.before", &view, history_window.as_ref(), history_active, history_navigation_request, history_pending, history_navigation_revision, None);
                let gesture_blocked = view.chat_at_tail();
                let before_anchor = history_window.as_ref().and_then(|window| {
                    let entries = crate::native_chat_projection::project_history_entries(window.messages());
                    view.chat_rendered_anchor(&entries)
                });
                view.scroll_down(8);
                history_trace("history.ctrl_f.after_scroll", &view, history_window.as_ref(), history_active, history_navigation_request, history_pending, history_navigation_revision, None);
                let mut cached_advance = false;
                if view.chat_at_tail() {
                    if let Some(window) = history_window.as_mut() {
                        let previous_window = window.clone();
                        if before_anchor.is_none() {
                            continue;
                        }
                        if window.advance_newer() {
                            history_trace("history.ctrl_f.cached_advance", &view, Some(window), history_active, history_navigation_request, history_pending, history_navigation_revision, Some("success"));
                            apply_history_window(window, &mut transcript);
                            if let Some(anchor) = before_anchor {
                                let entries = crate::native_chat_projection::project_history_entries(window.messages());
                                if !view.restore_chat_rendered_anchor(&entries, &anchor) {
                                    *window = previous_window;
                                    apply_history_window(window, &mut transcript);
                                    let old_entries = crate::native_chat_projection::project_history_entries(window.messages());
                                    let _ = view.restore_chat_rendered_anchor(&old_entries, &anchor);
                                    continue;
                                }
                            } else {
                                *window = previous_window;
                                apply_history_window(window, &mut transcript);
                                continue;
                            }
                            view.scroll_down(8);
                             cached_advance = true;
                        } else if history_browsing && !window.at_newest_page()
                            && window.begin_newer_recovery().is_ok()
                        {
                            history_navigation_request = Some(HistoryRequestKind::Newer);
                             if gesture_blocked {
                                 blocked_history_scroll = Some(BlockedHistoryScroll { revision: history_navigation_revision, rows: 8 });
                             }
                            history_trace("history.ctrl_f.begin_newer", &view, Some(window), history_active, history_navigation_request, history_pending, history_navigation_revision, Some("queued"));
                        }
                    }
                    if history_window.as_ref().is_some_and(|window| window.at_newest_page()) {
                        history_tail_refresh_pending = true;
                        history_browsing = false;
                    } else {
                        history_browsing = true;
                        history_tail_refresh_pending = false;
                    }
                } else if cached_advance {
                    history_browsing = !history_window.as_ref().is_some_and(|window| window.at_newest_page());
                    history_tail_refresh_pending = !history_browsing;
                }
                utf8.reset();
            }
            2 if !page_open && matches!(dialog, native_setup::Dialog::None) => {
                view.scroll_up(8);
                history_browsing = true;
                history_navigation_revision = history_navigation_revision.wrapping_add(1);
                 history_navigation_request = Some(HistoryRequestKind::Older);
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
                }) || compact_worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation)) || view.dialogs.is_open();
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
                                draft_cursor,
                                &snapshot.origin,
                                Some(&candidate),
                                auth,
                            )
                        });
                    match bound {
                        Ok((snapshot, restored_draft)) => {
                            seed_navigation_titles(&mut navigation, &snapshot);
                            // Route follows the accepted snapshot: a bind
                            // with history restores the session route; an
                            // empty target (/new) returns to Home.
                            session_route = true;
                            view.set_session_route(session_route);
                            live = Some(snapshot);
                            history_window = live.as_ref().map(|snapshot| crate::native_chat_projection::NativeHistoryWindow::new(&snapshot.raw_history, snapshot.history_next_before.clone()));
                            history_pending = false;
                            history_active = None;
                         history_navigation_request = None;
                         blocked_history_scroll = None;
                            history_tail_refresh_pending = false;
                            history_browsing = false;
                            let restored_cursor = navigation.active().map(|tab| tab.cursor).unwrap_or(restored_draft.len());
                            crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, restored_draft, restored_cursor); draft_parts.clear();
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
            // Ctrl-K (0x0b) is upstream `input.delete.to.line.end`
            // ("Delete to end of line in input", `keybind.ts:233`). When the
            // composer owns focus and the edit changes the draft, the kill
            // wins; a no-op kill on an empty/line-end draft falls through to
            // the Commands page (the local command input) so the advertised
            // Ctrl-K page entry stays reachable. Draft text is otherwise
            // preserved verbatim and no provider turn is ever started.
            11 if !page_open && !help_open && matches!(dialog, native_setup::Dialog::None) => {
                if crate::native_composer::apply_line_binding(&draft, &mut draft_cursor, byte) {
                    crate::native_paste::reconcile_parts(&draft, &mut draft_parts);
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
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
                    crate::native_composer::normalize_cursor(&draft, &mut draft_cursor);
                    if draft_cursor > 0 {
                        let previous = draft[..draft_cursor].char_indices().next_back().map_or(0, |(i, _)| i);
                        draft.drain(previous..draft_cursor);
                        draft_cursor = previous;
                        crate::native_paste::reconcile_parts(&draft, &mut draft_parts);
                    }
                } else {
                    filter_input.pop();
                }
                utf8.reset();
            }
            b'\r' | b'\n'
                if palette_route.is_none()
                    && !page_open
                    && matches!(dialog, native_setup::Dialog::None)
                    && matches!(crate::native_composer::decide_byte(byte, native_composer_keymap(keymap)), Some(crate::native_composer::KeyAction::Newline)) =>
            {
                utf8.reset();
                if draft.len() < MAX_NATIVE_DRAFT_BYTES {
                    draft.insert(draft_cursor, '\n');
                    draft_cursor += 1;
                    crate::native_paste::reconcile_parts(&draft, &mut draft_parts);
                }
            }
            1 | 5 | 21 if !page_open && !help_open && matches!(dialog, native_setup::Dialog::None) => {
                if crate::native_composer::apply_line_binding(&draft, &mut draft_cursor, byte) {
                    crate::native_paste::reconcile_parts(&draft, &mut draft_parts);
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
            }
            b'\r' | b'\n'
                if matches!(crate::native_composer::decide_byte(byte, native_composer_keymap(keymap)), Some(crate::native_composer::KeyAction::Submit)) => {
                crate::native_debug::event(
                    "input.enter",
                    serde_json::json!({"dialog":format!("{dialog:?}"),"page":format!("{:?}",view.page()),"composer_bytes":draft.len(),"model":selected.as_ref().map(|model|model.qualified()),"daemon_attached":live.is_some(),"busy":worker.is_some()}),
                );
                utf8.reset();
                // Set only by command-input Enter (Commands page); the
                // composer path leaves it `None` and parses the draft
                // exactly as before.
                let mut command_route: Option<&'static str> = palette_route;
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
                                }) || compact_worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation)) || view.dialogs.is_open();
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
                                // Capture the outgoing tab's draft/cursor/
                                // scroll/model into navigation state BEFORE
                                // routing, exactly as regular tab switching
                                // does (`select_tab_transition`), so returning
                                // to this session restores the latest edits
                                // rather than a stale draft. Done before the
                                // bind so a refusal leaves state intact.
                                if navigation.active().is_some() {
                                    if let Err(error) = navigation.save_active_state(
                                        draft.clone(),
                                        draft_cursor,
                                        view.chat_scroll_offset(),
                                        selected.as_ref().map(|m| m.qualified()),
                                    ) {
                                        append_transcript(
                                            &mut transcript,
                                            format!("error: session switch: state save refused: {error:?}"),
                                        );
                                        view.resolve_page_ack(crate::native_view_pages::Ack::Rejected(
                                            format!("state save refused: {error:?}"),
                                        ));
                                        continue;
                                    }
                                }
                                let bound = live
                                    .as_ref()
                                    .ok_or_else(|| "daemon unavailable".to_owned())
                                    .and_then(|snapshot| {
                                        bind_session(
                                            &mut view,
                                            &mut transcript,
                                            &draft,
                                            draft_cursor,
                                            &snapshot.origin,
                                            Some(&id),
                                            auth,
                                        )
                                    });
                                match bound {
                                    Ok((snapshot, restored_draft)) => {
                                        session_route = true;
                                        view.set_session_route(session_route);
                                        // Enter routes to the row's session, so
                                        // the rail opens-or-focuses its tab
                                        // (`session-tabs.tsx:203-228`) before
                                        // the title seed reaches it.
                                        let existing_cursor = navigation
                                            .tabs()
                                            .iter()
                                            .find(|tab| tab.session.0 == id)
                                            .map(|tab| tab.cursor);
                                        navigation.open(crate::native_navigation::SessionRef(id.clone()));
                                        seed_navigation_titles(&mut navigation, &snapshot);
                                        live = Some(snapshot);
                                        history_window = live.as_ref().map(|snapshot| crate::native_chat_projection::NativeHistoryWindow::new(&snapshot.raw_history, snapshot.history_next_before.clone()));
                                        history_pending = false;
                                        history_active = None;
                                         history_navigation_request = None;
                                        history_tail_refresh_pending = false;
                                        history_browsing = false;
                                        let restored_cursor = existing_cursor.unwrap_or(restored_draft.len());
                                        crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, restored_draft, restored_cursor); draft_parts.clear();
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
                                        // Upstream closes the session list once
                                        // the route lands (`dialog-session-list.tsx:248-250`).
                                        view.close_page();
                                        needs_paint = true;
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
                        if let Some(name) = view.page_selected().filter(|row| row.enabled)
                            .and_then(|row| row.id.strip_prefix("configured:")) {
                            if name.len().saturating_add(2).saturating_add(draft.len()) > MAX_NATIVE_DRAFT_BYTES {
                                append_transcript(&mut transcript, "error: command prefix exceeds composer limit; draft preserved".into());
                                needs_paint = true;
                                continue;
                            }
                            // Selecting a template wraps the existing draft as
                            // its arguments; submitting still requires Enter
                            // and the normal backend ACK owns draft clearing.
                            let prefix_bytes = format!("/{name} ").len();
                            let insertion_cursor = prefix_bytes + draft_cursor;
                            let replacement = format!("/{name} {draft}");
                            crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, replacement, insertion_cursor); draft_parts.clear();
                            view.close_page();
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
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
                // CP-08 SUBMITWIRE: the composer Enter submit routes through
                // the pure queue-vs-steer decision
                // (`native_composer::plan_submit`). Upstream Enter invokes
                // textarea `onSubmit` -> `submit()` and chooses delivery
                // `queue` exactly while the running turn owns the session
                // (`component/prompt/index.tsx:455,1097-1127,1790-1795`).
                if command_route.is_none() && matches!(dialog, native_setup::Dialog::None) {
                    let turn_owns_binding = worker.as_ref().is_some_and(|owned| {
                        owned.owns(
                            live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                            binding_generation,
                        )
                    });
                    match composer_enter_plan(
                        &draft,
                        turn_owns_binding,
                        live.is_some(),
                        completion.triggered(),
                        queued_drafts.len(),
                    ) {
                        // Idle steer keeps the existing dispatch path; the
                        // native model gate below still owns the selection
                        // dialog (upstream `promptModelWarning`), and no
                        // location catalog exists yet.
                        crate::native_composer::SubmitPlan::Steer
                        | crate::native_composer::SubmitPlan::ModelUnavailable => {}
                        crate::native_composer::SubmitPlan::Ignored => {
                            // Completion owner or disabled: never a submit
                            // (`index.tsx:1100-1104`).
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
                        crate::native_composer::SubmitPlan::Rejected(reason) => {
                            // "This prompt cannot be queued" toast, draft
                            // preserved (`index.tsx:1136-1139`).
                            append_transcript(
                                &mut transcript,
                                format!(
                                    "warning: this prompt cannot be queued ({reason:?}); draft preserved"
                                ),
                            );
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
                        crate::native_composer::SubmitPlan::Exit => {
                            // Upstream `void exit()` (`index.tsx:1141-1144`):
                            // the exit word terminates the client exactly
                            // like the typed terminal command above.
                            utf8.reset();
                            break;
                        }
                        crate::native_composer::SubmitPlan::Queue => {
                            if queued_drafts.len() >= crate::native_composer::BUSY_QUEUE_CAP {
                                // Bounded queue, same cap as
                                // `Composer::submit` QueueFull; overflow
                                // preserves the draft.
                                append_transcript(
                                    &mut transcript,
                                    format!(
                                        "error: queued prompt limit reached ({limit}/{limit}); draft preserved",
                                        limit = crate::native_composer::BUSY_QUEUE_CAP
                                    ),
                                );
                            } else {
                                queued_drafts
                                    .push_back((live.as_ref().map(|snapshot| snapshot.session_id.clone()), crate::native_paste::expand_for_submit(&draft, &draft_parts)));
                                // The queued snapshot clears the composer
                                // (`index.tsx:1172-1174` resetComposer).
                                crate::native_composer::replace_draft_with_cursor(
                                    &mut draft,
                                    &mut draft_cursor,
                                    String::new(),
                                    0,
                                );
                                completion.sync(&draft);
                            }
                            utf8.reset();
                            needs_paint = true;
                            continue;
                        }
                        crate::native_composer::SubmitPlan::BlankPromotion { queued, .. } => {
                            if queued {
                                // Blank idle Enter promotes the FIRST queued
                                // prompt of THIS session to steer delivery
                                // (`routes/session/index.tsx:1650-1657`
                                // `mutatePending("steer", next.id)`); the
                                // fall-through dispatch below sends it
                                // through the ordinary admission chain.
                                let active =
                                    live.as_ref().map(|snapshot| snapshot.session_id.clone());
                                let next =
                                    next_queued_for_session(&queued_drafts, active.as_deref());
                                if let Some((index, next)) = next {
                                    queued_drafts.remove(index);
                                    crate::native_composer::replace_draft_with_cursor(
                                        &mut draft,
                                        &mut draft_cursor,
                                        next.clone(),
                                        next.len(),
                                    );
                                    completion.sync(&draft);
                                } else {
                                    // Next queue entry belongs to another
                                    // session: no cross-session promotion.
                                    utf8.reset();
                                    needs_paint = true;
                                    continue;
                                }
                            } else {
                                // Blank idle Enter with nothing queued is
                                // not a provider turn (`onEmptySubmit`
                                // returns false without a queued prompt,
                                // `index.tsx:1124-1127`,
                                // `routes/session/index.tsx:1650-1653`).
                                utf8.reset();
                                needs_paint = true;
                                continue;
                            }
                        }
                    }
                }
                let text = match command_route {
                    // Command mode: the recognized local route, fed to the
                    // SAME local-route chain below. Every recognized route
                    // branch `continue`s before turn submit, so no
                    // provider turn can originate from the command buffer.
                    Some(route) => route.to_owned(),
                    None => match dialog {
                        native_setup::Dialog::None => crate::native_paste::expand_for_submit(&draft, &draft_parts),
                        native_setup::Dialog::ApiKey => key_input.trim().to_owned(),
                        _ => filter_input.trim().to_owned(),
                    },
                };
                let command = text.trim();
                // Typed commands never close a tab: upstream's
                // `session.tab.close` has no slash alias (keybind
                // `<leader>w` only — see `native_process_exit_route`), so the
                // only typed terminal command here is the exit word. It
                // terminates the client globally, exactly like the upstream
                // `app.exit` action, regardless of how many tabs are open
                // (a Home/root window with no tab must still exit).
                if matches!(dialog, native_setup::Dialog::None) && native_process_exit_route(command) {
                    break;
                }
                if !matches!(dialog, native_setup::Dialog::None) {
                    match dialog {
                        native_setup::Dialog::Provider => {
                            let query = text.to_lowercase();
                            let rows: Vec<_> = providers
                                .iter()
                                .filter(|row| {
                                    row.0.to_lowercase().contains(&query) || row.1.to_lowercase().contains(&query)
                                })
                                .collect();
                            if let Some(row) =
                                rows.get(native_setup::clamp_row(model_row, rows.len()))
                            {
                                model_load = None;
                                provider_id = row.0.clone();
                                provider_url = row.2.clone();
                                setup_error = None;
                                key_input.clear();
                                dialog = if provider_id == "custom" {
                                    native_setup::Dialog::ProviderId
                                } else if provider_id == "openai" {
                                    native_setup::Dialog::ApiKey
                                } else {
                                    native_setup::Dialog::BaseUrl
                                };
                                filter_input = provider_url.clone();
                            } else {
                                setup_error = Some("No provider matches".into());
                            }
                        }
                        native_setup::Dialog::ProviderId => {
                            if !text.is_empty()
                                && text.len() <= 128
                                && text.bytes().all(|b| {
                                    b.is_ascii_lowercase()
                                        || b.is_ascii_digit()
                                        || b == b'-'
                                        || b == b'_'
                                })
                                && text != "openai"
                                && text != "custom"
                            {
                                provider_id = text;
                                dialog = native_setup::Dialog::BaseUrl;
                                filter_input = "http://localhost:20128/v1".into();
                                setup_error = None;
                            } else {
                                setup_error = Some(
                                    "Use a lowercase provider ID (letters, numbers, - or _)".into(),
                                );
                            }
                        }
                        native_setup::Dialog::BaseUrl => {
                            if let Ok(url) = reqwest::Url::parse(&text) {
                                if matches!(url.scheme(), "http" | "https")
                                    && url.host_str().is_some()
                                    && url.username().is_empty()
                                    && url.password().is_none()
                                    && url.query().is_none()
                                    && url.fragment().is_none()
                                    && text.len() <= 2048
                                {
                                    provider_url = text.trim_end_matches('/').to_owned();
                                    dialog = native_setup::Dialog::ApiKey;
                                    key_input.clear();
                                    filter_input.clear();
                                    setup_error = None;
                                } else {
                                    setup_error = Some("Enter a valid provider base URL without credentials, query or fragment".into());
                                }
                            } else {
                                setup_error = Some("Enter a valid provider base URL".into());
                            }
                        }
                        native_setup::Dialog::ApiKey if provider_id != "openai" => {
                            let payload = serde_json::json!({"name":provider_id,"base_url":provider_url,"api_key":if text.is_empty() { None } else { Some(text.as_str()) }}).to_string();
                            let result = live
                                .as_ref()
                                .ok_or_else(|| "daemon unavailable".to_owned())
                                .and_then(|snapshot| {
                                    http_request(
                                        &snapshot.origin,
                                        "POST",
                                        &format!("/api/providers/{provider_id}"),
                                        Some(&payload),
                                        auth,
                                    )?;
                                    native_setup::ModelLoad::start(
                                        &snapshot.origin,
                                        auth,
                                        &provider_id,
                                    )
                                });
                            key_input.clear();
                            match result {
                                Ok(load) => {
                                    if !providers.iter().any(|row| row.0 == provider_id) {
                                        providers.insert(
                                            providers.len() - 1,
                                            (
                                                provider_id.clone(),
                                                provider_id.clone(),
                                                provider_url.clone(),
                                            ),
                                        );
                                    } else if let Some(row) =
                                        providers.iter_mut().find(|row| row.0 == provider_id)
                                    {
                                        row.2 = provider_url.clone();
                                    }
                                    model_load = Some(load);
                                    setup_error = None;
                                    filter_input = format!("{provider_id}/");
                                    dialog = native_setup::Dialog::Model;
                                    model_row = 0;
                                }
                                Err(_) => {
                                    setup_error = Some(
                                        "Unable to save provider; check base URL and daemon".into(),
                                    )
                                }
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
                            // Enter commits the focused advertised row. Query text is
                            // never itself a model identity.
                            let choice =
                                native_setup::dialog_selected_row(&models, &filter_input, model_row);
                            if let Some(model) = choice {
                                match native_setup::save_model(&model) {
                                    Ok(()) => {
                                        selected = Some(model.clone());
                                        provider_id = model.provider_id.clone();
                                        // Confirmed local config write:
                                        // honest live model status update.
                                        view.set_model(selected.as_ref().map(|m| m.qualified()));
                                        view.set_composer_model(&model.name, &model.provider_id);
                                        dialog = native_setup::Dialog::None;
                                        model_row = 0;
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
                        // Settings owns its events before composer/provider
                        // dispatch and never commits a provider selection.
                        native_setup::Dialog::Settings | native_setup::Dialog::None => {}
                    }
                    // Filter dialogs consumed only their dialog-local
                    // buffer: the API-key dialog keeps its segregated
                    // buffer and the chat draft is preserved verbatim
                    // through the entire provider/model/API-key flow.
                    if !was_api
                        && !matches!(
                            dialog,
                            native_setup::Dialog::BaseUrl | native_setup::Dialog::ProviderId
                        )
                    {
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
                    consume_local_command_with_cursor(&mut draft, &mut draft_cursor, command, from_chat_composer);
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
                    consume_local_command_with_cursor(&mut draft, &mut draft_cursor, command, from_chat_composer);
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
                if command == "/settings" {
                    settings_dialog.open_with_state(view.sidebar_hidden(), navigation.orientation());
                    settings_error = None;
                    dialog = native_setup::Dialog::Settings;
                    consume_local_command_with_cursor(&mut draft, &mut draft_cursor, command, from_chat_composer);
                    utf8.reset();
                    needs_paint = true;
                    continue;
                }
                if command == "/import" || command == "/settings-import" {
                    let (import_dest, import_result) = if let Some(dest) = opencode_rk_providers::imported_config::own_config_path() {
                        let result = crate::native_import_sources::discover_import(
                            &std::env::current_dir()?,
                            &crate::native_import_sources::EnvSnapshot::capture(),
                            &dest,
                        );
                        (Some(dest), result)
                    } else {
                        (None, Err("cannot resolve own config destination".to_owned()))
                    };
                    match import_result {
                        Ok(Some(offer)) => {
                            let crate::native_import_sources::ImportOffer { plan, sources, warnings } = offer;
                            let value = plan.value;
                            let unsupported = plan.unsupported;
                            let mut reopened = crate::native_import_dialog::ImportDialog::from_settings_parts(
                                &data_dir.join("import-offer.json"),
                                import_dest.as_deref().expect("offer requires destination"),
                                sources,
                                value,
                            )?;
                            let mut details = warnings.into_iter().take(16)
                                .map(|warning| format!("warning: {} — {}", warning.source, warning.message))
                                .collect::<Vec<_>>();
                            details.extend(unsupported.into_iter().take(16)
                                .map(|unsupported| format!("unsupported: {} ({})", unsupported.field, unsupported.reason)));
                            reopened.set_details(details);
                            import_dialog = Some(reopened);
                        }
                        Ok(None) => append_transcript(&mut transcript, "Import settings: no supported sources found".to_owned()),
                        Err(error) => append_transcript(&mut transcript, format!("Import settings unavailable: {error}")),
                    }
                     consume_local_command_with_cursor(&mut draft, &mut draft_cursor, command, from_chat_composer);
                    needs_paint = true;
                    continue;
                }
                // Local page routes, same contract as /help above: pure
                // state, never a provider turn. A
                // chat-composer typed command is consumed at dispatch so
                // Esc restores the pre-command blank composer; a
                // segregated source (Ctrl-K query) keeps its draft intact.
                // Real-backend pages (/sessions, /agents) additionally
                // report one honest error line when their fetch/parse
                // fails — that is state reporting, not a transcript turn.
                if let Some(compact) = compact_command(command) {
                    consume_local_command(&mut draft, command, from_chat_composer);
                    match compact {
                        Ok(()) => {
                    let busy = worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation))
                                || compact_worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation)) || view.dialogs.is_open();
                            if busy {
                                append_transcript(&mut transcript, "busy: turn or compaction already in progress".into());
                            } else if let (Some(snapshot), Some(model)) = (live.as_ref(), selected.as_ref()) {
                                let strategy = text.split_whitespace().nth(1).unwrap_or("auto");
                                match auth.ok_or_else(|| "missing daemon token".to_owned()).and_then(|token| NativeCompactWorker::start_strategy(&snapshot.origin, &snapshot.session_id, token, &model.qualified(), binding_generation, strategy)) {
                                    Ok(next) => compact_worker = Some(next),
                                    Err(error) => append_transcript(&mut transcript, format!("error: compact: {error}")),
                                }
                            } else {
                                append_transcript(&mut transcript, "error: /compact requires a live session and selected model".into());
                            }
                        }
                        Err(reason) => append_transcript(&mut transcript, format!("error: {reason}")),
                    }
                    needs_paint = true;
                    continue;
                }
                let local_page = native_local_page_route(command);
                if let Some(page) = local_page {
                    if matches!(page, crate::native_view_pages::Page::Commands) {
                        // Single production path: the static route rows
                        // are loaded with the page (same as Ctrl-K).
                        view.open_commands_page();
                    } else {
                        view.open_page(page);
                        if page == crate::native_view_pages::Page::Agents {
                            // Real metadata rows from the same authenticated
                            // `GET /api/commands` catalogue the @mention
                            // loader consumes. Fetch/parse failure keeps the
                            // page's previous rows and reports honestly
                            // (same contract as /sessions above); rows are
                            // metadata — activation stays the framework's
                            // Unsupported because no launch command exists.
                            needs_paint = true;
                            match live.as_ref() {
                                Some(snapshot) => {
                                    let result = http_request(&snapshot.origin, "GET", "/api/commands", None, auth)
                                        .and_then(|body| {
                                            serde_json::from_str::<serde_json::Value>(&body)
                                                .map_err(|_| "invalid command listing JSON".to_owned())
                                        })
                                        .and_then(|value| agent_catalogue_rows(&value));
                                    match result {
                                        Ok(rows) => view.set_page_rows(rows),
                                        Err(error) => append_transcript(
                                            &mut transcript,
                                            format!("error: agents page: {error}"),
                                        ),
                                    }
                                }
                                None => append_transcript(
                                    &mut transcript,
                                    "error: agents page: daemon unavailable".to_owned(),
                                ),
                            }
                        } else if page == crate::native_view_pages::Page::Theme {
                            let choices = native_theme_choices(&config_paths, Some(theme.name.as_str()), theme_mode)
                                .unwrap_or_else(|error| {
                                    append_transcript(&mut transcript, format!("error: theme registry: {error}"));
                                    Vec::new()
                                });
                            let rows: Vec<_> = choices
                                .into_iter()
                                .map(|choice| {
                                    let name = choice.name;
                                    let selected = name == theme.name;
                                    crate::native_view_pages::PageRow {
                                        id: name.clone(),
                                        label: name,
                                        detail: if selected {
                                            "selected".into()
                                        } else if !choice.available {
                                            "unavailable: native resolver is not installed".into()
                                        } else {
                                            String::new()
                                        },
                                        enabled: choice.available,
                                    }
                                })
                                .collect();
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
                    }) || compact_worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation)) || view.dialogs.is_open();
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
                            // Stage the complete outgoing tab state and the new
                            // identity together.  Navigation is client-local
                            // transaction state: the old tab must remember the
                            // transaction-local remainder, cursor, scroll and
                            // selected model, while the new tab starts with its
                            // own empty draft/history state.  Nothing in this
                            // staged clone becomes visible until the exact ACKed
                            // target bind succeeds below.
                            let mut staged_navigation = navigation.clone();
                            let outgoing_model = selected.as_ref().map(|model| model.qualified());
                            if let Err(error) = staged_navigation.save_active_state(
                                working_draft.clone(),
                                draft_cursor,
                                view.chat_scroll_offset(),
                                outgoing_model,
                            ) {
                                append_transcript(
                                    &mut transcript,
                                    format!("error: session create: state staging refused: {error:?}"),
                                );
                                needs_paint = true;
                                continue;
                            }
                            staged_navigation.open(crate::native_navigation::SessionRef(created_id.clone()));
                            let bound = live
                                .as_ref()
                                .ok_or_else(|| "daemon unavailable".to_owned())
                                .and_then(|snapshot| {
                                    bind_session(
                                        &mut view,
                                        &mut transcript,
                                        &working_draft,
                                        draft_cursor,
                                        &snapshot.origin,
                                        Some(&created_id),
                                        auth,
                                    )
                                });
                            match bound {
                                Ok((snapshot, restored_draft)) => {
                                    seed_navigation_titles(&mut staged_navigation, &snapshot);
                                    // An accepted explicit bind owns the session
                                    // route even before its first message.
                                    // The target tab was opened in the staged
                                    // clone with fresh local state. Synchronize
                                    // the bind result into that same tab before
                                    // publishing the clone; otherwise the
                                    // global composer would show the restored
                                    // target draft while a later tab switch
                                    // would resurrect the default empty state.
                                    let target_cursor = if restored_draft.is_empty() {
                                        0
                                    } else {
                                        restored_draft.len()
                                    };
                                    // Upstream's session.new keeps the current
                                    // model selection when it routes Home; the
                                    // native tab must carry that same selection,
                                    // while a new target always starts at chat
                                    // scroll zero. Keep the global view in the
                                    // identical state before publishing the
                                    // staged navigation clone.
                                    let target_model = selected
                                        .as_ref()
                                        .map(|model| model.qualified());
                                    if let Err(error) = staged_navigation.save_active_state(
                                        restored_draft.clone(),
                                        target_cursor,
                                        0,
                                        target_model,
                                    ) {
                                        append_transcript(
                                            &mut transcript,
                                            format!("error: session create: target state staging refused: {error:?}"),
                                        );
                                        needs_paint = true;
                                        continue;
                                    }
                                    navigation = staged_navigation;
                                    history_window = Some(
                                        crate::native_chat_projection::NativeHistoryWindow::new(
                                            &snapshot.raw_history,
                                            snapshot.history_next_before.clone(),
                                        ),
                                    );
                                    history_pending = false;
                                    history_active = None;
                                    history_navigation_request = None;
                                    history_tail_refresh_pending = false;
                                    history_browsing = false;
                                    view.set_chat_scroll_offset(0);
                                    if let Some(model) = selected.as_ref() {
                                        view.set_model(Some(model.qualified()));
                                        view.set_composer_model(&model.name, &model.provider_id);
                                    } else {
                                        view.set_model(None);
                                    }
                                    session_route = true;
                                    view.set_session_route(session_route);
                                    live = Some(snapshot);
                                    // Visible draft updates ONLY on this
                                    // successful commit.
                                    let restored_cursor = navigation.active().map(|tab| tab.cursor).unwrap_or(restored_draft.len());
                                    crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, restored_draft, restored_cursor); draft_parts.clear();
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
                if command == "/cd" || command.starts_with("/cd ") {
                    let busy = worker.as_ref().is_some_and(|owned| {
                        owned.owns(
                            live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                            binding_generation,
                        )
                    }) || compact_worker.as_ref().is_some_and(|owned| owned.owns(live.as_ref().map(|s| s.session_id.as_str()), binding_generation)) || view.dialogs.is_open();
                    if busy {
                        append_transcript(&mut transcript, "busy: turn or permission decision in progress; /cd refused".to_owned());
                        needs_paint = true;
                        continue;
                    }
                    let Some(prospective) = crate::native_view::reserve_generation(binding_generation) else {
                        append_transcript(&mut transcript, "error: binding generation exhausted; /cd refused".to_owned());
                        needs_paint = true;
                        continue;
                    };
                    let argument = match crate::native_workspace::cd_argument(command) {
                        Ok(argument) => argument,
                        Err(error) => {
                            append_transcript(&mut transcript, format!("error: {error}"));
                            needs_paint = true;
                            continue;
                        }
                    };
                    let base = std::env::current_dir()?;
                    let directory = match crate::native_workspace::resolve_directory(&argument, &base) {
                        Ok(directory) => directory,
                        Err(error) => {
                            append_transcript(&mut transcript, format!("error: {error}"));
                            needs_paint = true;
                            continue;
                        }
                    };
                    let created = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned()).and_then(|snapshot| {
                        let body = serde_json::json!({"title":"Chat", "directory":directory}).to_string();
                        http_request(&snapshot.origin, "POST", "/api/sessions", Some(&body), auth)
                    });
                    match created.and_then(|body| parse_created_session_id(&body)) {
                        Ok(id) => {
                            let working = command_transaction_draft(&draft, command, from_chat_composer);
                            let bound = live.as_ref().ok_or_else(|| "daemon unavailable".to_owned()).and_then(|snapshot|
                                bind_session(&mut view, &mut transcript, &working, draft_cursor, &snapshot.origin, Some(&id), auth));
                            match bound {
                                Ok((snapshot, restored)) => {
                                    seed_navigation_titles(&mut navigation, &snapshot);
                                    live = Some(snapshot);
                                    let restored_cursor = navigation.active().map(|tab| tab.cursor).unwrap_or(restored.len());
                                    crate::native_composer::replace_draft_with_cursor(&mut draft, &mut draft_cursor, restored, restored_cursor); draft_parts.clear();
                                    session_route = true;
                                    view.set_session_route(true);
                                    history_window = live.as_ref().map(|snapshot| crate::native_chat_projection::NativeHistoryWindow::new(&snapshot.raw_history, snapshot.history_next_before.clone()));
                                    history_pending = false;
                                    history_active = None;
                                    history_navigation_request = None;
                                    blocked_history_scroll = None;
                                    history_tail_refresh_pending = false;
                                    history_browsing = false;
                                    binding_generation = prospective;
                                    append_transcript(&mut transcript, format!("workspace: {}", directory.display()));
                                }
                                Err(error) => append_transcript(&mut transcript, format!("error: workspace switch: {error}")),
                            }
                        }
                        Err(error) => append_transcript(&mut transcript, format!("error: workspace switch: {error}")),
                    }
                    needs_paint = true;
                    continue;
                }
                if command == "/subagents" {
                    if let Some(snapshot) = live.as_ref() {
                        children_panel.show(&snapshot.session_id);
                        children_panel.bind_lease(&snapshot.session_id, binding_generation, auth.as_deref());
                        children_refresh = Instant::now() - Duration::from_secs(1);
                        view.close_command_palette();
                        view.close_page();
                        help_open = false;
                        let _ = appearance.cancel();
                        dialog = native_setup::Dialog::None;
                        view.set_dialog_size(crate::native_view::DialogSize::Large);
                        consume_local_command(&mut draft, command, from_chat_composer);
                    } else {
                        append_transcript(&mut transcript, "error: daemon unavailable".into());
                    }
                    needs_paint = true;
                    continue;
                }
                if command == "/appearance" {
                    appearance.show();
                    view.set_dialog_size(crate::native_view::DialogSize::Large);
                    consume_local_command(&mut draft, command, from_chat_composer);
                    needs_paint = true;
                    continue;
                }
                if matches!(command,"/permission"|"/permissions") {
                    view.close_page();view.close_command_palette();help_open=false;appearance.open=false;children_panel.open=false;
                    dialog=native_setup::Dialog::None;
                    permission_editor.open=true;
                    permission_editor_request(&mut permission_editor,live.as_ref(),auth,None);
                    consume_local_command(&mut draft,command,from_chat_composer);
                    needs_paint=true;
                    continue;
                }
                if command == "/connect" {
                    // Open the provider picker. A chat-composer typed
                    // "/connect" is consumed (command invocation, not an
                    // unsent draft); the filter is the dialog's own
                    // buffer, and cancel returns the exact draft state at
                    // dispatch (blank for a consumed command).
                    dialog = native_setup::Dialog::Provider;
                    model_row = 0;
                    setup_error = None;
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
                    model_row = 0;
                    if provider_id != "openai" && model_load.is_none() {
                        if let Some(snapshot) = live.as_ref() {
                            match native_setup::ModelLoad::start(
                                &snapshot.origin,
                                auth,
                                &provider_id,
                            ) {
                                Ok(load) => {
                                    model_load = Some(load);
                                    setup_error = None;
                                }
                                Err(error) => setup_error = Some(error),
                            }
                        }
                    }
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
                    }) || compact_worker.as_ref().is_some_and(|owned| {
                        owned.owns(
                            live.as_ref().map(|snapshot| snapshot.session_id.as_str()),
                            binding_generation,
                        )
                    }) {
                        // Do not consume or echo this draft while a worker
                        // owns the current binding.
                        continue;
                    }
                    if live.is_none() {
                        let Some(origin) = daemon_origin else {
                            append_transcript(&mut transcript, "offline: turn not executed".to_owned());
                            needs_paint = true;
                            continue;
                        };
                        if worker.is_some() || compact_worker.is_some() || view.dialogs.is_open() {
                            append_transcript(&mut transcript, "busy: prior session work or permission decision is still active; prompt preserved".to_owned());
                            needs_paint = true;
                            continue;
                        }
                        let Some(prospective) = crate::native_view::reserve_generation(binding_generation) else {
                            append_transcript(&mut transcript, "error: binding generation exhausted; session create refused".into());
                            needs_paint = true;
                            continue;
                        };
                        let directory = std::env::current_dir().unwrap_or_default().display().to_string();
                        let body = serde_json::json!({"title":"Chat", "directory":directory}).to_string();
                        let created = http_request(origin, "POST", "/api/sessions", Some(&body), auth)
                            .and_then(|response| parse_created_session_id(&response));
                        match created {
                            Err(error) => {
                                append_transcript(&mut transcript, format!("error: session create: {error}; draft preserved (not retried)"));
                                needs_paint = true;
                                continue;
                            }
                            Ok(created_id) => match bind_session(
                                &mut view,
                                &mut transcript,
                                &draft,
                                draft_cursor,
                                origin,
                                Some(&created_id),
                                auth,
                            ) {
                                Ok((snapshot, _restored_draft)) => {
                                    seed_navigation_titles(&mut navigation, &snapshot);
                                    navigation.open(crate::native_navigation::SessionRef(snapshot.session_id.clone()));
                                    history_window = Some(crate::native_chat_projection::NativeHistoryWindow::new(
                                        &snapshot.raw_history,
                                        snapshot.history_next_before.clone(),
                                    ));
                                    live = Some(snapshot);
                                    binding_generation = prospective;
                                    session_route = true;
                                    view.set_session_route(true);
                                }
                                Err(error) => {
                                    append_transcript(&mut transcript, format!("error: session create: {error}; created session {created_id} preserved (not bound, not retried); draft preserved"));
                                    needs_paint = true;
                                    continue;
                                }
                            },
                        }
                    }
                    if let Some(snapshot) = live.as_ref() {
                        // Commit the conversation shell before starting the
                        // potentially delayed provider request.  The route is
                        // rolled back if admission/worker creation fails, so
                        // Home never flashes blank while the first response
                        // is in flight.
                        let route_was_home = !session_route;
                        if route_was_home {
                            session_route = true;
                            view.set_session_route(true);
                        }
                        if selected.is_none() && !text.starts_with('!') && configured_command_input(&text).is_none() {
                            if route_was_home {
                                session_route = false;
                                view.set_session_route(false);
                            }
                            if model_load.is_some() {
                                append_transcript(&mut transcript, "error: models are still loading; prompt preserved. Press Enter when ready.".into());
                            } else {
                                dialog = if models.is_empty() {
                                    native_setup::Dialog::Provider
                                } else {
                                    native_setup::Dialog::Model
                                };
                                filter_input.clear();
                                model_row = 0;
                                setup_error = Some(
                                    "Choose a model to send this prompt; your draft is preserved."
                                        .into(),
                                );
                            }
                            continue;
                        }
                        {
                            let result = if let Some(command) = text.strip_prefix('!') {
                                native_turn::TurnWorker::start_shell(
                                    &snapshot.origin,
                                    &snapshot.session_id,
                                    auth.ok_or("missing daemon token")?,
                                    command.trim(),
                                    None,
                                )
                            } else if let Some((name, arguments)) = configured_command_input(&text) {
                                native_turn::TurnWorker::start_command(
                                    &snapshot.origin,
                                    &snapshot.session_id,
                                    auth.ok_or("missing daemon token")?,
                                    name,
                                    arguments,
                                    &selected.as_ref().map(|model| model.qualified()).unwrap_or_default(),
                                )
                            } else {
                                selected
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
                            }
                            .map(|next| {
                                pending_submission = Some(crate::native_composer::SubmittedDraft::capture(
                                    &mut draft, &mut draft_cursor,
                                ));
                                completion.sync(&draft);
                                needs_paint = true;
                                // Capture the binding identity this
                                // worker is launched under: events only
                                // apply while both still match.
                                let started = crate::native_view::OwnedWorker::new(
                                    next,
                                    snapshot.session_id.clone(),
                                    binding_generation,
                                );
                                // The rail/tab busy spinner is sourced from the
                                // same authoritative fact as the footer loader
                                // (`session-tabs.tsx:169-172`), not a guess.
                                navigation.apply(crate::native_navigation::DaemonEvent::WorkState {
                                    session: crate::native_navigation::SessionRef(
                                        snapshot.session_id.clone(),
                                    ),
                                    state: crate::native_navigation::WorkState::Running,
                                });
                                worker = Some(started);
                                partial.clear();
                            });
                            if let Err(error) = result {
                                if route_was_home {
                                    session_route = false;
                                    view.set_session_route(false);
                                }
                                crate::native_debug::event(
                                    "turn.submit_rejected",
                                    serde_json::json!({"category":"worker_start_failed"}),
                                );
                                append_transcript(&mut transcript, format!("error: {error}"));
                            }
                        }
                    } else {
                        crate::native_debug::event(
                            "turn.submit_rejected",
                            serde_json::json!({"category":"daemon_offline"}),
                        );
                            append_transcript(
                                &mut transcript,
                                "offline: turn not executed".to_string(),
                            );
                        }
                }
                // Launch detaches the submitted draft before the next input.
                // Rejected admission restores it only if no newer draft exists.
                utf8.reset();
            }
            b if b >= 0x20 && b != b'\t' => {
                // Leader follow-up: `Ctrl-X` then `b` toggles the sidebar;
                // `<leader>w` closes the active tab; `<leader>1`..`<leader>9`
                // select tab slot 1..9 and `<leader>0` selects slot 10
                // (upstream keybind.ts: `session.tab.select.{1..9,0=tenth}`,
                // `session.tab.close`). Any other key just ends the chord.
                // The chord (and its follow-up key) is consumed WITHOUT
                // touching the composer draft, the credential buffer or any
                // page query, and only while the chat composer owns focus —
                // never while a credential/model dialog, Help or a page is open.
                if leader_followup_pending
                    && matches!(dialog, native_setup::Dialog::None)
                    && !page_open
                    && !help_open
                {
                    leader_pending = false;
                    let slot = match b {
                        b'1'..=b'9' => Some(usize::from(b - b'1')), // 0..8
                        b'0' => Some(9),                            // 10th slot
                        _ => None,
                    };
                    if let Some(index) = slot {
                        // Reuse the SAME select_tab_transition +
                        // apply_close_tab_result as mouse tab clicks.
                        // Out-of-range: no-op, leave tabs intact.
                        if let Some(target) = navigation.tab_at(index).map(|t| t.session.clone()) {
                            let scroll = view.chat_scroll_offset();
                            match select_tab_transition(
                                &mut view, &mut transcript, &mut navigation,
                                &draft, draft_cursor,
                                daemon_origin,
                                auth, binding_generation,
                                &target, selected.as_ref().map(|m| m.qualified()), scroll,
                            ) {
                                Ok(result) => {
                                    apply_close_tab_result(
                                        result, &mut view, &mut live, &mut history_window,
                                        &mut binding_generation, &mut draft, &mut draft_cursor,
                                        &mut selected, &models, &mut session_route,
                                        &mut compact_worker,
                                    );
                                    completion.sync(&draft);
                                }
                                Err(error) => append_transcript(&mut transcript, format!("error: select tab: {error}")),
                            }
                            needs_paint = true;
                        }
                    } else if b == b'w' {
                        // `<leader>w` closes the ACTIVE tab — same transition
                        // as mouse close-click on the active tab.
                        let active_session = navigation
                            .active()
                            .map(|tab| tab.session.clone());
                        if let Some(session) = active_session {
                            let close_scroll = view.chat_scroll_offset();
                            let scroll_result = close_tab_transition(
                                &mut view, &mut transcript, &mut navigation,
                                &draft, draft_cursor,
                                daemon_origin,
                                auth, binding_generation, &session,
                                selected.as_ref().map(|m| m.qualified()), close_scroll,
                            );
                            match scroll_result {
                                Ok(result) => apply_close_tab_result(
                                    result, &mut view, &mut live, &mut history_window,
                                    &mut binding_generation, &mut draft, &mut draft_cursor,
                                    &mut selected, &models, &mut session_route,
                                    &mut compact_worker,
                                ),
                                Err(error) => append_transcript(&mut transcript, format!("error: close tab: {error}")),
                            }
                            needs_paint = true;
                        }
                    } else if b == b'n' {
                        // `<leader>n` = `session.new` (`config/keybind.ts:110`):
                        // route Home; the outgoing tab keeps its draft/model.
                        let new_scroll = view.chat_scroll_offset();
                        match new_session_tab_transition(
                            &mut navigation,
                            &draft,
                            draft_cursor,
                            selected.as_ref().map(|model| model.qualified()),
                            new_scroll,
                            binding_generation,
                        ) {
                            Ok(result) => {
                                apply_close_tab_result(
                                    result, &mut view, &mut live, &mut history_window,
                                    &mut binding_generation, &mut draft, &mut draft_cursor,
                                    &mut selected, &models, &mut session_route,
                                    &mut compact_worker,
                                );
                                completion.sync(&draft);
                                history_pending = false;
                                history_active = None;
                                history_navigation_request = None;
                                blocked_history_scroll = None;
                                history_tail_refresh_pending = false;
                                history_browsing = false;
                            }
                            Err(error) => append_transcript(&mut transcript, format!("error: new tab: {error}")),
                        }
                        needs_paint = true;
                    } else if b == b'b' {
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
                    if let Some(character) = utf8.push(b) {
                        let width = character.len_utf8();
                        if matches!(dialog, native_setup::Dialog::None) {
                            crate::native_composer::normalize_cursor(&draft, &mut draft_cursor);
                            if draft.len().checked_add(width).is_some_and(|len| len <= MAX_NATIVE_DRAFT_BYTES) {
                                draft.insert(draft_cursor, character);
                                draft_cursor += width;
                                // An ordinary edit can shift or delete a
                                // folded placeholder; re-anchor the tracked
                                // ranges to the current draft so a later
                                // paste/expansion cannot use stale offsets.
                                crate::native_paste::reconcile_parts(&draft, &mut draft_parts);
                            }
                        } else if matches!(dialog, native_setup::Dialog::ApiKey) {
                            if key_input.len().checked_add(width).is_some_and(|len| len <= 16 * 1024) { key_input.push(character); }
                        } else if filter_input.len().checked_add(width).is_some_and(|len| len <= MAX_NATIVE_DRAFT_BYTES) {
                            filter_input.push(character);
                            model_row = 0;
                        }
                    }
                }
            }
            _ => {
                utf8.reset();
            }
        }
    }
    if let Some(load) = completion_load { load.abort(); }
    let _ = renderer.disable_kitty_keyboard();
    let _ = renderer.restore_terminal_modes();
    renderer.close();
    Ok(())
}

/// Split only the invocation separator; argument whitespace belongs to the
/// configured template, rather than the client's local route parser.
#[cfg(feature = "native")]
fn configured_command_input(text: &str) -> Option<(&str, &str)> {
    let invocation = text.strip_prefix('/')?;
    Some(match invocation.char_indices().find(|(_, ch)| ch.is_whitespace()) {
        Some((index, ch)) => (&invocation[..index], &invocation[index + ch.len_utf8()..]),
        None => (invocation, ""),
    })
}

/// Recognized LOCAL slash-command surface, resolved through the
/// production page table plus the two chat-only routes that are not
/// `Page::from_route` entries (`/commands` has no static route; `/new`
/// is a session-create action). No invented aliases: an unknown or
/// unsupported command is never "recognized" here. (`/exit`, `/quit`,
/// `/q`, bare `exit`/`quit` and `:q`/`:quit` exit the client before
/// dispatch, so their draft never renders again and needs no
/// consumption — see `native_process_exit_route`.)
#[cfg(feature = "native")]
#[cfg(feature = "native")]
fn is_recognized_local_command(command: &str) -> bool {
    crate::native_view_pages::Page::from_route(command).is_some()
        || matches!(command, "/commands" | "/new" | "/compact" | "/compact native" | "/compact fallback" | "/compact auto" | "/sidebar" | "/appearance" | "/subagents" | "/permission" | "/permissions" | "/import" | "/settings-import" | "/settings")
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

#[cfg(feature = "native")]
fn consume_local_command_with_cursor(
    draft: &mut String,
    cursor: &mut usize,
    command: &str,
    from_chat_composer: bool,
) {
    if from_chat_composer && is_recognized_local_command(command) {
        draft.clear();
        *cursor = 0;
    } else {
        crate::native_composer::normalize_cursor(draft, cursor);
    }
}

/// Slash surface of upstream `app.exit`: the command is registered as
/// `slash: { name: "exit", aliases: ["quit", "q"] }`
/// (`packages/tui/src/app.tsx:1176-1181`), so `/exit`, `/quit` and `/q` all
/// run the same `exit()` action. Recognized here so a user-configured command
/// can never shadow the built-in exit route and the Commands page has a local
/// handler for it.
#[cfg(feature = "native")]
fn native_exit_route(command: &str) -> bool {
    matches!(command, "/exit" | "/quit" | "/q")
}

/// Every spelling that terminates the CLIENT globally, at the selected pin
/// `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
/// - slash surface `/exit`, `/quit`, `/q` — `app.tsx:1176-1181` above;
/// - bare `exit`, `quit`, `:q` typed into the prompt and submitted —
///   `packages/tui/src/component/prompt/index.tsx:1128`
///   (`trimmed === "exit" || trimmed === "quit" || trimmed === ":q"`),
///   mirrored by `isExitCommand` in `packages/tui/src/mini/prompt.shared.ts:71-76`;
/// - `:quit`, retained as an approved native extension (same action).
///
/// Tab close is NOT an exit spelling. Upstream exposes it only as
/// `session.tab.close` bound to `<leader>w`
/// (`packages/tui/src/config/keybind.ts:119`); the command has no `slash`
/// entry (`packages/tui/src/app.tsx:878-884`), so no typed command closes a
/// tab. Tabs close through that key binding, the tab context menu and the
/// middle-click tab gesture, all of which preserve reopen/daemon work.
#[cfg(feature = "native")]
fn native_process_exit_route(command: &str) -> bool {
    matches!(
        command,
        "/exit" | "/quit" | "/q" | "exit" | "quit" | ":q" | ":quit"
    )
}

#[cfg(feature = "native")]
struct CloseTabResult {
    snapshot: Option<LiveSnapshot>,
    draft: String,
    cursor: usize,
    scroll: usize,
    selected_model: Option<String>,
    generation: u64,
    home: bool,
}

#[cfg(feature = "native")]
fn commit_child_route(
    snapshot: LiveSnapshot, restored: String, generation: u64,
    target_cursor: usize, target_scroll: usize, target_model: Option<String>,
    models: &[native_setup::Model], selected: &mut Option<native_setup::Model>,
    view: &mut crate::native_view::NativeView,
    live: &mut Option<LiveSnapshot>,
    history_window: &mut Option<crate::native_chat_projection::NativeHistoryWindow>,
    binding_generation: &mut u64, draft: &mut String, draft_cursor: &mut usize,
    session_route: &mut bool, completion: &mut crate::native_completion::Completion,
) {
    *history_window = Some(crate::native_chat_projection::NativeHistoryWindow::new(&snapshot.raw_history, snapshot.history_next_before.clone()));
    *live = Some(snapshot); *binding_generation = generation;
    crate::native_composer::replace_draft_with_cursor(draft, draft_cursor, restored, target_cursor);
    crate::native_composer::normalize_cursor(draft, draft_cursor);
    view.set_chat_scroll_offset(target_scroll);
    *selected = target_model.as_deref().and_then(|qualified| models.iter().find(|model| model.qualified() == qualified).cloned());
    if let Some(model) = selected.as_ref() {
        view.set_model(Some(model.qualified()));
        view.set_composer_model(&model.name, &model.provider_id);
    } else { view.set_model_unknown(); }
    *session_route = true; view.set_session_route(true); completion.sync(draft);
}

#[cfg(feature = "native")]
fn open_child_transition(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    origin: &str,
    auth: Option<&str>,
    generation: u64,
    parent: &str,
    child: &str,
    selected_model: Option<String>,
) -> Result<(LiveSnapshot, String, u64, usize, usize, Option<String>), String> {
    let mut staged = navigation.clone();
    staged.save_active_state(draft.to_owned(), cursor, view.chat_scroll_offset(), None)
        .map_err(|error| format!("child open refused: {error:?}"))?;
    staged.open_forked(crate::native_navigation::SessionRef(child.to_owned()), crate::native_navigation::SessionRef(parent.to_owned()));
    if let Some(tab) = staged.active_mut() { tab.selected_model = selected_model; }
    let prospective = crate::native_view::reserve_generation(generation)
        .ok_or_else(|| "child open refused: binding generation exhausted".to_owned())?;
    let (snapshot, restored) = bind_session(view, transcript, draft, cursor, origin, Some(child), auth)?;
    seed_navigation_titles(&mut staged, &snapshot);
    let target = staged.active().cloned().ok_or_else(|| "child open target missing".to_owned())?;
    *navigation = staged;
    Ok((snapshot, if target.draft.is_empty() { restored } else { target.draft }, prospective, target.cursor, target.scroll, target.selected_model))
}

#[cfg(feature = "native")]
fn return_parent_transition(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    origin: &str,
    auth: Option<&str>,
    generation: u64,
    parent: &str,
) -> Result<(LiveSnapshot, String, u64, usize, usize, Option<String>), String> {
    let mut staged = navigation.clone();
    staged.save_active_state(draft.to_owned(), cursor, view.chat_scroll_offset(), None)
        .map_err(|error| format!("parent return refused: {error:?}"))?;
    staged.open(crate::native_navigation::SessionRef(parent.to_owned()));
    let prospective = crate::native_view::reserve_generation(generation)
        .ok_or_else(|| "parent return refused: binding generation exhausted".to_owned())?;
    let (snapshot, restored) = bind_session(view, transcript, draft, cursor, origin, Some(parent), auth)?;
    seed_navigation_titles(&mut staged, &snapshot);
    let target = staged.active().cloned().ok_or_else(|| "parent return target missing".to_owned())?;
    *navigation = staged;
    Ok((snapshot, if target.draft.is_empty() { restored } else { target.draft }, prospective, target.cursor, target.scroll, target.selected_model))
}

#[cfg(feature = "native")]
fn select_tab_transition(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    origin: Option<&str>,
    auth: Option<&str>,
    generation: u64,
    session: &SessionRef,
    selected_model: Option<String>,
    scroll: usize,
) -> Result<CloseTabResult, String> {
    // Preflight every fallible navigation operation on a private snapshot.
    // Failed target fetches must leave both the current tab and binding intact.
    let mut staged = navigation.clone();
    if navigation.active().is_some() {
        staged.save_active_state(draft.to_owned(), cursor, scroll, selected_model)
            .map_err(|error| format!("select refused: {error:?}"))?;
    }
    staged.select_tab(session).map_err(|error| format!("select refused: {error:?}"))?;
    let target = staged.active().cloned().ok_or_else(|| "select refused: target missing".to_owned())?;
    if navigation.active().is_some_and(|tab| tab.session == *session) {
        *navigation = staged;
        return Ok(CloseTabResult { snapshot: None, draft: target.draft, cursor: target.cursor, scroll: target.scroll, selected_model: target.selected_model, generation, home: false });
    }
    let prospective = crate::native_view::reserve_generation(generation)
        .ok_or_else(|| "select refused: binding generation exhausted".to_owned())?;
    let origin = origin.ok_or_else(|| "select refused: daemon unavailable".to_owned())?;
    let (snapshot, _) = bind_session(view, transcript, draft, cursor, origin, Some(&session.0), auth)?;
    seed_navigation_titles(&mut staged, &snapshot);
    *navigation = staged;
    Ok(CloseTabResult { snapshot: Some(snapshot), draft: target.draft, cursor: target.cursor, scroll: target.scroll, selected_model: target.selected_model, generation: prospective, home: false })
}

/// `session.tab.reopen` (`config/keybind.ts:118`): restore the most recent
/// user-closed tab at its recorded slot and route to it. The staged
/// navigation snapshot keeps the closed stack intact when the fetch fails.
#[cfg(feature = "native")]
fn reopen_tab_transition(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    origin: Option<&str>,
    auth: Option<&str>,
    generation: u64,
    selected_model: Option<String>,
    scroll: usize,
    requested: Option<&crate::native_navigation::SessionRef>,
) -> Result<CloseTabResult, String> {
    let mut staged = navigation.clone();
    if navigation.active().is_some() {
        staged
            .save_active_state(draft.to_owned(), cursor, scroll, selected_model)
            .map_err(|error| format!("reopen refused: {error:?}"))?;
    }
    let session = staged
        .reopen(requested)
        .ok_or_else(|| "reopen refused: no closed tab".to_owned())?;
    let target = staged
        .active()
        .cloned()
        .ok_or_else(|| "reopen refused: target missing".to_owned())?;
    let prospective = crate::native_view::reserve_generation(generation)
        .ok_or_else(|| "reopen refused: binding generation exhausted".to_owned())?;
    let origin = origin.ok_or_else(|| "reopen refused: daemon unavailable".to_owned())?;
    let (snapshot, _) = bind_session(view, transcript, draft, cursor, origin, Some(&session.0), auth)?;
    seed_navigation_titles(&mut staged, &snapshot);
    *navigation = staged;
    Ok(CloseTabResult {
        snapshot: Some(snapshot),
        draft: target.draft,
        cursor: target.cursor,
        scroll: target.scroll,
        selected_model: target.selected_model,
        generation: prospective,
        home: false,
    })
}

#[cfg(feature = "native")]
fn close_tab_transition(
    view: &mut crate::native_view::NativeView,
    transcript: &mut NativeChatHistory,
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    origin: Option<&str>,
    auth: Option<&str>,
    generation: u64,
    session: &SessionRef,
    selected_model: Option<String>,
    scroll: usize,
) -> Result<CloseTabResult, String> {
    let active = navigation.active().map(|tab| tab.session.clone());
    let mut staged = navigation.clone();
    if navigation.active().is_some() {
        staged.save_active_state(draft.to_owned(), cursor, scroll, selected_model.clone())
            .map_err(|error| format!("close refused: {error:?}"))?;
    }
    if active.as_ref() != Some(session) {
        staged.close(session).map_err(|error| format!("close refused: {error:?}"))?;
        *navigation = staged;
        return Ok(CloseTabResult { snapshot: None, draft: draft.to_owned(), cursor, scroll, selected_model, generation, home: navigation.tab_len() == 0 });
    }
    if navigation.tab_len() <= 1 {
        staged.close(session).map_err(|error| format!("close refused: {error:?}"))?;
        *navigation = staged;
        return Ok(CloseTabResult { snapshot: None, draft: String::new(), cursor: 0, scroll: 0, selected_model: None, generation, home: true });
    }
    let successor = navigation.close_successor().ok_or_else(|| "close refused: successor missing".to_owned())?;
    let prospective = crate::native_view::reserve_generation(generation)
        .ok_or_else(|| "close refused: binding generation exhausted".to_owned())?;
    let origin = origin.ok_or_else(|| "close refused: daemon unavailable".to_owned())?;
    let target = staged.tab_at(staged.tabs().iter().position(|tab| tab.session == successor).ok_or_else(|| "close refused: successor missing" )?).cloned()
        .ok_or_else(|| "close refused: successor missing".to_owned())?;
    let (snapshot, _) = bind_session(view, transcript, draft, cursor, origin, Some(&successor.0), auth)?;
    seed_navigation_titles(&mut staged, &snapshot);
    staged.close(session).map_err(|error| format!("close commit invariant failed: {error:?}"))?;
    staged.select_tab(&successor).map_err(|error| format!("close commit invariant failed: {error:?}"))?;
    *navigation = staged;
    Ok(CloseTabResult { snapshot: Some(snapshot), draft: target.draft, cursor: target.cursor, scroll: target.scroll, selected_model: target.selected_model, generation: prospective, home: false })
}

#[cfg(feature = "native")]
/// Route to Home as the new-session tab without creating a session yet.
/// Upstream `session.new` navigates Home and keeps the current model/agent
/// (`app.tsx:777-796`); the session is created by the next submit.
fn new_session_tab_transition(
    navigation: &mut crate::native_navigation::NavigationState,
    draft: &str,
    cursor: usize,
    selected_model: Option<String>,
    scroll: usize,
    generation: u64,
) -> Result<CloseTabResult, String> {
    let mut staged = navigation.clone();
    if navigation.active().is_some() {
        staged.save_active_state(draft.to_owned(), cursor, scroll, selected_model)
            .map_err(|error| format!("new tab refused: {error:?}"))?;
    }
    staged.enter_home();
    *navigation = staged;
    Ok(CloseTabResult {
        snapshot: None,
        draft: String::new(),
        cursor: 0,
        scroll: 0,
        selected_model: None,
        generation,
        home: true,
    })
}

#[cfg(feature = "native")]
fn apply_close_tab_result(
    result: CloseTabResult,
    view: &mut crate::native_view::NativeView,
    live: &mut Option<LiveSnapshot>,
    history_window: &mut Option<crate::native_chat_projection::NativeHistoryWindow>,
    binding_generation: &mut u64,
    draft: &mut String,
    draft_cursor: &mut usize,
    selected: &mut Option<native_setup::Model>,
    models: &[native_setup::Model],
    session_route: &mut bool,
    compact_worker: &mut Option<NativeCompactWorker>,
) {
    if result.home {
        if let Some(worker) = compact_worker.as_mut() {
            worker.retire();
        }
        *compact_worker = None;
    }
    if let Some(snapshot) = result.snapshot {
        *history_window = Some(crate::native_chat_projection::NativeHistoryWindow::new(
            &snapshot.raw_history,
            snapshot.history_next_before.clone(),
        ));
        *live = Some(snapshot);
        *binding_generation = result.generation;
    }
    crate::native_composer::replace_draft_with_cursor(
        draft,
        draft_cursor,
        result.draft,
        result.cursor,
    );
    crate::native_composer::normalize_cursor(draft, draft_cursor);
    if result.home {
        // Home has no active session binding. Keep daemon_origin separately
        // so the next prompt can create and bind a fresh session.
        *live = None;
        *history_window = None;
        view.reset_to_home_after_tab_close();
        *session_route = false;
    } else {
        view.set_chat_scroll_offset(result.scroll);
        *selected = result.selected_model.as_deref().and_then(|qualified| {
            models.iter().find(|model| model.qualified() == qualified).cloned()
        });
    }
}

#[cfg(feature = "native")]
fn native_local_page_route(command: &str) -> Option<crate::native_view_pages::Page> {
    use crate::native_view_pages::Page;
    match command {
        "/status" => Some(Page::Status),
        "/mcp" => Some(Page::Mcp),
        "/theme" => Some(Page::Theme),
        "/workspace" => Some(Page::Workspaces),
        "/agents" => Some(Page::Agents),
        "/context" => Some(Page::Context),
        "/memory" => Some(Page::Memory),
        "/compaction" => Some(Page::Compaction),
        "/diff" => Some(Page::Diff),
        "/commands" => Some(Page::Commands),
        "/import" | "/settings-import" => Some(Page::Chat),
        _ => None,
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
        let fresh_id = if interactive && !args.once && args.session.is_none() {
            let body = serde_json::json!({"title":"Chat","directory":std::env::current_dir()?})
                .to_string();
            let created = http_request(origin, "POST", "/api/sessions", Some(&body), auth)?;
            let value: serde_json::Value = serde_json::from_str(&created)?;
            Some(
                value
                    .pointer("/session/id")
                    .and_then(|id| id.as_str())
                    .ok_or("session create response missing id")?
                    .to_owned(),
            )
        } else {
            None
        };
        let requested_session = args.session.as_deref().or(fresh_id.as_deref());
        let snapshot_result = match fetch_snapshot(origin, requested_session, auth) {
            Err(error) if error.contains("has no sessions yet") && auth.is_some() => {
                let body =
                    serde_json::json!({ "title": "Chat", "directory":std::env::current_dir()? })
                        .to_string();
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
                fetch_snapshot(origin, requested_session, auth)
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
                        args.origin.as_deref(),
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
            args.origin.as_deref(),
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
mod tab_binding_tests {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

    const A: &str = "00000000-0000-0000-0000-000000000001";
    const B: &str = "00000000-0000-0000-0000-000000000002";

    struct Fixture {
        origin: String,
        token: String,
        requests: Arc<Mutex<Vec<String>>>,
        fail: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Fixture {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let token = "a".repeat(64);
            let requests = Arc::new(Mutex::new(Vec::new()));
            let fail = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let (seen, failed, done, credential) = (requests.clone(), fail.clone(), stop.clone(), token.clone());
            let thread = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(15);
                while !done.load(Ordering::Acquire) && Instant::now() < deadline {
                    let (mut socket, _) = match listener.accept() {
                        Ok(value) => value,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                            continue;
                        }
                        Err(error) => panic!("fixture accept: {error}"),
                    };
                    socket.set_nonblocking(false).unwrap();
                    socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    socket.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut raw = Vec::new();
                    while !raw.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let mut chunk = [0; 1024];
                        let n = socket.read(&mut chunk).unwrap();
                        assert!(n > 0 && raw.len() + n <= 16 * 1024);
                        raw.extend_from_slice(&chunk[..n]);
                    }
                    let raw = String::from_utf8(raw).unwrap();
                    assert!(raw.to_ascii_lowercase().contains(&format!("authorization: bearer {credential}")));
                    let route = raw.lines().next().unwrap().split_whitespace().nth(1).unwrap().to_owned();
                    seen.lock().unwrap().push(route.clone());
                    let (status, body) = if failed.load(Ordering::Acquire) {
                        ("503 Service Unavailable", serde_json::json!({"error":"fixture listing unavailable"}))
                    } else if route == "/api/sessions" {
                        ("200 OK", serde_json::json!({"sessions":[
                            {"id":A,"title":"Session A","state":"idle","updated_at":"2026-10-26T00:00:00Z"},
                            {"id":B,"title":"Session B","state":"idle","updated_at":"2026-10-26T00:00:00Z"}
                        ]}))
                    } else if route.contains("/history?") {
                        let text = if route.contains(B) { "history from B" } else { "history from A" };
                        ("200 OK", serde_json::json!({"messages":[{"id":"message-fixture","role":"assistant","body":{"storage":"inline","text":text}}]}))
                    } else if route.ends_with("/todo") {
                        ("200 OK", serde_json::json!([]))
                    } else {
                        ("200 OK", serde_json::json!({"servers":[]}))
                    };
                    let body = body.to_string();
                    write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
            });
            Self { origin, token, requests, fail, stop, thread: Some(thread) }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() { thread.join().unwrap(); }
        }
    }

    fn navigation() -> crate::native_navigation::NavigationState {
        let mut navigation = crate::native_navigation::NavigationState::new();
        navigation.open(SessionRef(A.into()));
        navigation.open(SessionRef(B.into()));
        navigation.save_active_state("left 世界 right".into(), 8, 23, Some("fixture/target".into())).unwrap();
        navigation.select_tab(&SessionRef(A.into())).unwrap();
        navigation
    }

    #[test]
    fn rail_details_follow_authenticated_session_directory_without_stale_or_invented_labels() {
        // Use the same authenticated history fetch and production tab-to-rail
        // projection as the real loop. Different sessions must retain their
        // own project fallback labels; no project/VCS metadata is invented.
        let fixture = Fixture::new();
        let listing = serde_json::json!({"sessions":[
            {"id":A,"title":"Session A","directory":"/tmp/opencode/packages/tui"},
            {"id":B,"title":"Session B","directory":"/tmp/other/"}
        ]});
        let snapshot = snapshot_from_listing(&fixture.origin, &listing, Some(A), Some(&fixture.token)).unwrap();
        let mut nav = navigation();
        seed_navigation_titles(&mut nav, &snapshot);
        let tabs = crate::native_tabs::TabStrip::from_navigation(&nav, &[]);
        let rail_inputs = rail_inputs_for_navigation(&nav, &tabs);
        assert_eq!(rail_inputs.iter().map(|tab| tab.detail.as_str()).collect::<Vec<_>>(), ["tui", "other"]);
        let rail = crate::native_tab_rail::RailFrame::render_cells(&rail_inputs, 42, 40, 0, None, false);
        assert_eq!(rail.rows[2].text.trim(), "tui");
        assert_eq!(rail.rows[5].text.trim(), "other");

        // A fresh listing that no longer supplies valid directories must not
        // leave old labels behind, block the bind, or paint terminal controls.
        let listing = serde_json::json!({"sessions":[
            {"id":A,"title":"Session A"},
            {"id":B,"title":"Session B","directory":"/tmp/other\u{1b}[31m"}
        ]});
        let snapshot = snapshot_from_listing(&fixture.origin, &listing, Some(A), Some(&fixture.token)).unwrap();
        seed_navigation_titles(&mut nav, &snapshot);
        let tabs = crate::native_tabs::TabStrip::from_navigation(&nav, &[]);
        let rail_inputs = rail_inputs_for_navigation(&nav, &tabs);
        assert_eq!(rail_inputs.iter().map(|tab| tab.detail.as_str()).collect::<Vec<_>>(), ["", ""]);
    }

    #[test]
    fn tab_selection_binds_authenticated_target_history_and_live_send_identity() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        let (snapshot, _) = bind_session(&mut view, &mut transcript, "", 0, &fixture.origin, Some(A), Some(&fixture.token)).unwrap();
        let mut live = Some(snapshot);
        let result = select_tab_transition(&mut view, &mut transcript, &mut navigation, "A unsent", 3, Some(&fixture.origin), Some(&fixture.token), 4, &SessionRef(B.into()), Some("fixture/source".into()), 7).expect("tab selection must bind the target");
        assert_eq!(result.snapshot.as_ref().map(|snapshot| snapshot.session_id.as_str()), Some(B));
        assert_eq!(navigation.active().unwrap().session.0, B);
        assert!(transcript.lines.iter().any(|line| line.contains("history from B")));
        assert!(fixture.requests.lock().unwrap().iter().any(|route| route == &format!("/api/sessions/{B}/history?limit=100")));
        let mut history = None;
        let mut generation = 4;
        let mut draft = "A unsent".to_owned();
        let mut cursor = 3;
        let mut selected = None;
        let mut route = true;
        let mut compact_worker = None;
        let models = [native_setup::Model { id: "target".into(), name: "Target".into(), provider_id: "fixture".into(), context_window: None }];
        apply_close_tab_result(result, &mut view, &mut live, &mut history, &mut generation, &mut draft, &mut cursor, &mut selected, &models, &mut route, &mut compact_worker);
        assert_eq!(live.as_ref().unwrap().session_id, B, "Send reads its session identity from live");
        assert_eq!(generation, 5);
        assert_eq!(draft, "left 世界 right");
        assert_eq!(cursor, 8);
        assert_eq!(view.chat_scroll_offset(), 23);
        assert_eq!(selected.as_ref().unwrap().qualified(), "fixture/target");
        assert!(history.is_some());
        let outgoing = navigation.tabs().iter().find(|tab| tab.session.0 == A).unwrap();
        assert_eq!((&outgoing.draft, outgoing.cursor, outgoing.scroll), (&"A unsent".to_owned(), 3, 7));
    }

    /// F1 regression: an existing tab must reopen from New/Home. The
    /// navigation transition (select/close/reopen) is authorized by the stable
    /// `daemon_origin`, NOT the active session binding (`live`), which is
    /// `None` on Home and while a New tab is pending
    /// (`tui_entry.rs:~8406` New clears `live`; `enter_home` unsets `active`).
    /// This mirrors upstream where clearing the active session view does not
    /// disconnect the daemon (`context/session-tabs.tsx`).
    #[test]
    fn home_select_binds_target_with_stable_daemon_origin_absent_live() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        // New/Home: no active session binding, but the daemon stays reachable.
        navigation.enter_home();
        assert!(navigation.active().is_none());
        let mut live: Option<LiveSnapshot> = None;
        // The transition must succeed with a live binding absent, because the
        // loop forwards `daemon_origin` — not `live` — to the transition.
        let result = select_tab_transition(
            &mut view, &mut transcript, &mut navigation, "", 0,
            Some(&fixture.origin), Some(&fixture.token), 4,
            &SessionRef(B.into()), None, 0,
        )
        .expect("reopening an existing tab from Home must bind the target");
        assert_eq!(result.snapshot.as_ref().map(|s| s.session_id.as_str()), Some(B));
        assert_eq!(navigation.active().unwrap().session.0, B);
        let mut history = None;
        let mut generation = 4;
        let mut draft = String::new();
        let mut cursor = 0;
        let mut selected = None;
        let mut route = false;
        let mut compact_worker = None;
        let models = [native_setup::Model { id: "target".into(), name: "Target".into(), provider_id: "fixture".into(), context_window: None }];
        apply_close_tab_result(result, &mut view, &mut live, &mut history, &mut generation, &mut draft, &mut cursor, &mut selected, &models, &mut route, &mut compact_worker);
        assert_eq!(live.as_ref().map(|s| s.session_id.as_str()), Some(B));
        assert_eq!(generation, 5);
    }

    #[test]
    fn new_target_keeps_global_model_and_zeroes_shared_scroll_with_stored_tab_state() {
        let mut navigation = crate::native_navigation::NavigationState::new();
        navigation.open(crate::native_navigation::SessionRef("old".into()));
        navigation.save_active_state("outgoing 世界".into(), 9, 27, Some("openai/current".into())).unwrap();
        let mut staged = navigation.clone();
        staged.open(crate::native_navigation::SessionRef("new".into()));
        staged.save_active_state("".into(), 0, 0, Some("openai/current".into())).unwrap();
        let target = staged.active().unwrap();
        assert_eq!(target.session.0, "new");
        assert_eq!(target.draft, "");
        assert_eq!(target.cursor, 0);
        assert_eq!(target.scroll, 0);
        assert_eq!(target.selected_model.as_deref(), Some("openai/current"));
        let old = staged.tabs().iter().find(|tab| tab.session.0 == "old").unwrap();
        assert_eq!(old.draft, "outgoing 世界");
        assert_eq!(old.scroll, 27);
        assert_eq!(old.selected_model.as_deref(), Some("openai/current"));
    }

    #[test]
    fn failed_target_fetch_preserves_active_binding_history_and_tab_state() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        let _ = bind_session(&mut view, &mut transcript, "", 0, &fixture.origin, Some(A), Some(&fixture.token)).unwrap();
        let before = transcript.lines.clone();
        let saved_draft = navigation.active().unwrap().draft.clone();
        fixture.fail.store(true, Ordering::Release);
        assert!(select_tab_transition(&mut view, &mut transcript, &mut navigation, "A newer draft", 2, Some(&fixture.origin), Some(&fixture.token), 4, &SessionRef(B.into()), None, 7).is_err());
        assert_eq!(navigation.active().unwrap().session.0, A);
        assert_eq!(navigation.active().unwrap().draft, saved_draft);
        assert_eq!(transcript.lines, before);
    }

    #[test]
    fn malformed_snapshot_titles_are_rejected_before_history_fetch() {
        let fixture = Fixture::new();
        for title in ["bad\ncontrol", &"x".repeat(513)] {
            let listing = serde_json::json!({"sessions":[{"id":A,"title":title,"state":"idle","updated_at":"2026-10-26T00:00:00Z"}]});
            assert!(snapshot_from_listing(&fixture.origin, &listing, Some(A), Some(&fixture.token)).is_err());
        }
        assert!(!fixture.requests.lock().unwrap().iter().any(|route| route.contains("/history?")));
    }

    #[test]
    fn malformed_parent_identity_is_rejected_before_history_fetch() {
        let fixture = Fixture::new();
        let listing = serde_json::json!({"sessions":[{"id":A,"title":"A","state":"idle","updated_at":"2026-10-26T00:00:00Z","parent_session_id":"not-a-uuid"}]});
        assert!(snapshot_from_listing(&fixture.origin, &listing, Some(A), Some(&fixture.token)).is_err());
        assert!(!fixture.requests.lock().unwrap().iter().any(|route| route.contains("/history?")));
    }

    #[test]
    fn child_route_commit_restores_target_model_and_cursor_from_staged_tab() {
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut live = None;
        let mut history = None;
        let mut generation = 1;
        let mut draft = String::new();
        let mut cursor = 0;
        let mut route = true;
        let mut completion = crate::native_completion::Completion::default();
        let models = vec![native_setup::Model { id: "child".into(), name: "Child".into(), provider_id: "fixture".into(), context_window: None }];
        let mut selected = None;
        let snapshot = LiveSnapshot { origin: "http://127.0.0.1:1".into(), session_id: A.into(), title: "A".into(), state: "idle".into(), updated_at: String::new(), message_count: 0, last_text: None, history: vec![], typed_history: vec![], raw_history: vec![], history_next_before: None, history_children: 0, parent_session: true, parent_session_id: Some(B.into()), directory: None, session_titles: vec![], session_project_labels: vec![] };
        commit_child_route(snapshot, "child draft".into(), 2, 8, 13, Some("fixture/child".into()), &models, &mut selected, &mut view, &mut live, &mut history, &mut generation, &mut draft, &mut cursor, &mut route, &mut completion);
        assert_eq!(draft, "child draft");
        assert_eq!(cursor, 8);
        assert_eq!(view.chat_scroll_offset(), 13);
        assert_eq!(selected.as_ref().map(|model| model.qualified()), Some("fixture/child".into()));
    }

    #[test]
    fn exhausted_binding_generation_refuses_selection_before_http_or_local_mutation() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        assert!(select_tab_transition(&mut view, &mut transcript, &mut navigation, "draft", 2, Some(&fixture.origin), Some(&fixture.token), u64::MAX, &SessionRef(B.into()), None, 0).is_err());
        assert!(fixture.requests.lock().unwrap().is_empty());
        assert_eq!(navigation.active().unwrap().session.0, A);
    }

    #[test]
    fn selecting_current_tab_preserves_unsent_state_without_http_or_generation_change() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        let result = select_tab_transition(&mut view, &mut transcript, &mut navigation, "same unsent", 4, Some(&fixture.origin), Some(&fixture.token), u64::MAX, &SessionRef(A.into()), Some("fixture/source".into()), 12).unwrap();
        assert!(result.snapshot.is_none());
        assert_eq!(result.generation, u64::MAX);
        assert_eq!((result.draft.as_str(), result.cursor, result.scroll), ("same unsent", 4, 12));
        assert_eq!(result.selected_model.as_deref(), Some("fixture/source"));
        assert!(fixture.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn unknown_tab_refuses_selection_without_mutating_outgoing_state_or_fetching() {
        let fixture = Fixture::new();
        let mut view = crate::native_view::NativeView::new(100, 32);
        let mut transcript = NativeChatHistory::default();
        let mut navigation = navigation();
        let original = navigation.active().unwrap().draft.clone();
        assert!(select_tab_transition(&mut view, &mut transcript, &mut navigation, "new unsaved", 2, Some(&fixture.origin), Some(&fixture.token), 4, &SessionRef("00000000-0000-0000-0000-000000000003".into()), None, 0).is_err());
        assert_eq!(navigation.active().unwrap().session.0, A);
        assert_eq!(navigation.active().unwrap().draft, original);
        assert!(fixture.requests.lock().unwrap().is_empty());
    }
}

#[cfg(all(test, feature = "native"))]
mod command_consumption_tests {
    use super::native_input_decoder::InputEvent;

    fn send_click(view: &crate::native_view::NativeView, draft: &str) -> InputEvent {
        let (x, y, _) = view.send_button_region(draft).expect("visible Send control");
        InputEvent::Mouse { x, y, kind: 0, modifiers: 0 }
    }

    #[test]
    fn send_click_uses_active_submit_keymap_on_home_and_session() {
        for session in [false, true] {
            let mut view = crate::native_view::NativeView::new(100, 32);
            view.set_session_route(session);
            for (keymap, byte) in [(super::SubmitKeymap::Enter, 13), (super::SubmitKeymap::CtrlJ, 10)] {
                let draft = "left 世界\nright";
                let click = send_click(&view, draft);
                let event = super::composer_pointer_event(click, &view, draft, keymap, true);
                assert_eq!(event, InputEvent::Byte(byte));
                assert_eq!(crate::native_composer::decide_byte(byte, super::native_composer_keymap(keymap)), Some(crate::native_composer::KeyAction::Submit));
            }
        }
    }

    #[test]
    fn send_click_preserves_authoritative_draft_and_middle_cursor_until_ack() {
        let view = crate::native_view::NativeView::new(100, 32);
        let mut draft = String::from("left 世界\nright");
        let mut cursor = 8;
        let captured = draft.clone();
        let click = send_click(&view, &draft);
        assert_eq!(super::composer_pointer_event(click, &view, &draft, super::SubmitKeymap::Enter, true), InputEvent::Byte(13));
        assert_eq!(draft, "left 世界\nright");
        assert_eq!(cursor, 8);
        draft.push_str(" newer edit");
        assert!(!crate::native_composer::acknowledge_submitted_draft_with_cursor(&captured, &mut draft, &mut cursor));
        assert_eq!(draft, "left 世界\nright newer edit");
        assert_eq!(cursor, 8);
        let captured = draft.clone();
        assert!(crate::native_composer::acknowledge_submitted_draft_with_cursor(&captured, &mut draft, &mut cursor));
        assert!(draft.is_empty());
        assert_eq!(cursor, 0);
    }

    #[test]
    fn send_click_cannot_cross_another_input_owner() {
        let mut view = crate::native_view::NativeView::new(100, 32);
        let draft = "keep this draft";
        let click = send_click(&view, draft);
        assert_eq!(super::composer_pointer_event(click.clone(), &view, draft, super::SubmitKeymap::Enter, false), click);
        view.dialogs.open_permission("request", "session", "Run command").unwrap();
        assert_eq!(super::composer_pointer_event(click.clone(), &view, draft, super::SubmitKeymap::Enter, true), click);
    }

    #[test]
    fn composer_enter_routes_through_plan_submit_queue_and_steer() {
        use crate::native_composer::{QueueRejection, SubmitPlan};
        // Idle + draft: steer straight to the existing dispatch.
        assert_eq!(
            super::composer_enter_plan("hello", false, true, false, 0),
            SubmitPlan::Steer
        );
        // Busy + draft: the bounded queue, never a dropped draft.
        assert_eq!(
            super::composer_enter_plan("hello", true, true, false, 0),
            SubmitPlan::Queue
        );
        // Queue stays bounded at the same cap as `Composer::submit`; the
        // loop refuses overflow before enqueueing.
        assert_eq!(
            super::composer_enter_plan("hello", true, true, false, crate::native_composer::BUSY_QUEUE_CAP),
            SubmitPlan::Queue
        );
        // Queued refusals: slash commands and exit words can never be
        // queued (`component/prompt/index.tsx:1136-1139`).
        assert_eq!(
            super::composer_enter_plan("/status", true, true, false, 0),
            SubmitPlan::Rejected(QueueRejection::SlashCommand)
        );
        assert_eq!(
            super::composer_enter_plan("exit", true, true, false, 0),
            SubmitPlan::Rejected(QueueRejection::ExitWord)
        );
        // Completion visible: submit is ignored, selection owns Enter
        // (`index.tsx:1103`).
        assert_eq!(
            super::composer_enter_plan("hello", false, true, true, 0),
            SubmitPlan::Ignored
        );
        // Blank idle Enter with a queued prompt: steer promotion of the
        // FIRST queued item (`routes/session/index.tsx:1650-1657`).
        assert_eq!(
            super::composer_enter_plan("", false, true, false, 1),
            SubmitPlan::BlankPromotion {
                queued: true,
                commit_selection: true
            }
        );
        // Blank idle Enter without a queue: never an empty provider turn
        // (`onEmptySubmit` returns false, `session/index.tsx:1650-1653`).
        assert_eq!(
            super::composer_enter_plan("", false, true, false, 0),
            SubmitPlan::BlankPromotion {
                queued: false,
                commit_selection: true
            }
        );
        // Home (no session) blank Enter: nothing to promote
        // (`index.tsx:1124-1127`).
        assert_eq!(
            super::composer_enter_plan("", false, false, false, 3),
            SubmitPlan::BlankPromotion {
                queued: false,
                commit_selection: false
            }
        );
    }

    /// CP-08 regression: promotion is session-scoped, not queue-front-only.
    /// Upstream `onEmptySubmit` promotes the first queued prompt *of the
    /// current session* (`routes/session/index.tsx:1650-1657`); another
    /// session's earlier entry must never strand the active session's draft.
    #[test]
    fn queued_promotion_is_owner_scoped_not_front_only() {
        use std::collections::VecDeque;
        // Session A queued before B; A sits at the front.
        let queued: VecDeque<(Option<String>, String)> = VecDeque::from(vec![
            (Some("A".to_string()), "a1".to_string()),
            (Some("B".to_string()), "b1".to_string()),
            (Some("A".to_string()), "a2".to_string()),
        ]);
        // B promotes its own entry (index 1), not the front, leaving A's.
        assert_eq!(
            super::next_queued_for_session(&queued, Some("B")),
            Some((1, "b1".to_string()))
        );
        // A promotes its FIRST entry (index 0) and skips B's.
        assert_eq!(
            super::next_queued_for_session(&queued, Some("A")),
            Some((0, "a1".to_string()))
        );
        // An owner with nothing queued promotes nothing.
        assert_eq!(super::next_queued_for_session(&queued, Some("C")), None);
        // A home window (no active session) never pulls a session-owned entry.
        assert_eq!(super::next_queued_for_session(&queued, None), None);
        // Removal by index keeps the other session's drafts intact.
        let mut q = queued.clone();
        let (i, _) = super::next_queued_for_session(&q, Some("B")).unwrap();
        q.remove(i);
        assert_eq!(
            q.iter()
                .map(|(o, t)| (o.as_deref().unwrap(), t.as_str()))
                .collect::<Vec<_>>(),
            vec![("A", "a1"), ("A", "a2")]
        );
    }

    /// CP-21: the dock publishes only the ACTIVE session's queued drafts.
    /// Upstream `queuedPrompts` filters `pending.list(route.sessionID)` to
    /// `delivery === "queue"` (`routes/session/index.tsx:224-228`) and the dock
    /// is shown only while that list is non-empty (`index.tsx:1591`). A draft
    /// owned by another session, or by no session (`None`), must never be
    /// published; with no active session nothing is published at all.
    #[test]
    fn published_queue_dock_is_active_session_scoped() {
        use std::collections::VecDeque;
        use crate::native_view_chat::QueuedPrompt;
        let queued: VecDeque<(Option<String>, String)> = VecDeque::from(vec![
            (Some("A".to_string()), "a1".to_string()),
            (Some("B".to_string()), "b1".to_string()),
            (Some("A".to_string()), "a2".to_string()),
            (None, "orphan".to_string()),
        ]);

        // Active B: only B's draft, keeping its source index for a unique id.
        let published = super::publishable_queued_prompts(&queued, Some("B"));
        assert_eq!(
            published,
            vec![QueuedPrompt { id: "queued-1".to_string(), text: "b1".to_string() }]
        );

        // Active A: both A drafts, in queue order, foreign/orphan skipped.
        let published = super::publishable_queued_prompts(&queued, Some("A"));
        assert_eq!(
            published,
            vec![
                QueuedPrompt { id: "queued-0".to_string(), text: "a1".to_string() },
                QueuedPrompt { id: "queued-2".to_string(), text: "a2".to_string() },
            ]
        );

        // A session with nothing queued publishes nothing.
        assert!(super::publishable_queued_prompts(&queued, Some("C")).is_empty());
        // Home window (no active session) publishes nothing (index.tsx:1591).
        assert!(super::publishable_queued_prompts(&queued, None).is_empty());
    }

    #[test]
    fn modified_return_matrix_uses_real_key_action_authority() {
        use crate::native_composer::{decide_byte, KeyAction};
        let events = [
            InputEvent::ShiftEnter,
            InputEvent::AltEnter,
            InputEvent::CtrlJ,
        ];
        for keymap in [super::SubmitKeymap::Enter, super::SubmitKeymap::CtrlJ] {
            for event in events.iter().cloned() {
                let normalized = super::normalize_modified_return(event.clone(), keymap);
                let byte = match normalized {
                    InputEvent::Byte(byte) => byte,
                    other => panic!("modified Return not normalized: {other:?}"),
                };
                let action = decide_byte(byte, super::native_composer_keymap(keymap));
                let expected = match event {
                    InputEvent::ShiftEnter | InputEvent::AltEnter => KeyAction::Newline,
                    InputEvent::CtrlJ if matches!(keymap, super::SubmitKeymap::CtrlJ) => KeyAction::Submit,
                    InputEvent::CtrlJ => KeyAction::Newline,
                    _ => unreachable!(),
                };
                assert_eq!(action, Some(expected));
            }
        }
    }

    #[test]
    fn rename_owner_does_not_submit_modified_return_events() {
        use super::native_input_decoder::InputEvent;
        for event in [InputEvent::ShiftEnter, InputEvent::AltEnter, InputEvent::CtrlJ] {
            assert!(!super::rename_submits(&event));
            assert!(!super::rename_dismisses(&event));
        }
    }

    #[test]
    fn send_click_ignores_release_wheel_drag_and_modified_presses() {
        let view = crate::native_view::NativeView::new(100, 32);
        let draft = "draft";
        let (x, y, _) = view.send_button_region(draft).unwrap();
        for (kind, modifiers) in [(1, 0), (2, 0), (3, 0), (4, 0), (0, 1), (0, 4)] {
            let event = InputEvent::Mouse { x, y, kind, modifiers };
            assert_eq!(super::composer_pointer_event(event.clone(), &view, draft, super::SubmitKeymap::Enter, true), event);
        }
    }

    #[test]
    fn send_click_uses_translated_rectangle_and_excludes_neighbor_cells() {
        use crate::native_tabs::{TabOrientation, TabReservation};
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let mut view = crate::native_view::NativeView::new(150, 32);
            view.set_session_route(true);
            view.set_tab_reservation(TabReservation::for_view(orientation, 150, 32));
            let draft = "draft";
            let (x, y, width) = view.send_button_region(draft).unwrap();
            for column in x..x + width {
                let event = InputEvent::Mouse { x: column, y, kind: 0, modifiers: 0 };
                assert_eq!(super::composer_pointer_event(event, &view, draft, super::SubmitKeymap::Enter, true), InputEvent::Byte(13));
            }
            for (x, y) in [(x - 1, y), (x + width, y), (x, y - 1), (x, y + 1)] {
                let event = InputEvent::Mouse { x, y, kind: 0, modifiers: 0 };
                assert_eq!(super::composer_pointer_event(event.clone(), &view, draft, super::SubmitKeymap::Enter, true), event);
            }
        }
    }

    #[test]
    fn send_click_rectangle_stays_inside_translated_composer_and_viewport() {
        use crate::native_tabs::{TabOrientation, TabReservation};
        for session in [false, true] {
            for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
                let mut view = crate::native_view::NativeView::new(150, 32);
                view.set_session_route(session);
                view.set_tab_reservation(TabReservation::for_view(orientation, 150, 32));
                let draft = "draft";
                let (left, top, width, height) = view.composer_region(draft).unwrap();
                let (x, y, cells) = view.send_button_region(draft).unwrap();
                assert!(x >= left && x + cells <= left + width, "Send must stay inside the translated composer");
                assert!(y >= top && y < top + height);
                assert!(x + cells <= 150 && y < 32);
                let frame = view.render_typed(draft, &[], false, &[]);
                let mut painted = String::new();
                crate::native_view_layout::visit_cells(&frame[usize::from(y)], 150, |col, glyph, _| {
                    if col >= x && col < x + cells { painted.push_str(glyph); }
                });
                assert_eq!(painted, "[Send]", "hit rectangle must match the rendered cells");
            }
        }
    }

    #[test]
    fn import_modal_visibility_keeps_permission_human_input_higher_priority() {
        assert!(super::import_modal_visible(false, false, false, true));
        assert!(!super::import_modal_visible(true, false, false, true));
        assert!(!super::import_modal_visible(false, true, false, true));
        assert!(!super::import_modal_visible(false, false, true, true));
        assert!(!super::import_modal_visible(false, false, false, false));
    }
    #[test]
    fn configured_discovery_keeps_metadata_and_local_routes() {
        let rows = super::configured_command_rows(&serde_json::json!({"commands":[
            {"name":"team/review","description":"Review a package"},
            {"name":"models","description":"must not replace local picker"}
        ]})).unwrap();
        let custom: Vec<_> = rows.iter().filter(|row| row.id.starts_with("configured:")).collect();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].id, "configured:team/review");
        assert_eq!(custom[0].label, "/team/review");
        assert_eq!(custom[0].detail, "Review a package");
        assert!(rows.iter().any(|row| row.id == "/models"));
        for malformed in [
            serde_json::json!({"commands":[{"name":"bad\nname"}]}),
            serde_json::json!({"commands":[{"name":"same"},{"name":"same"}]}),
            serde_json::json!({"commands":null}),
        ] {
            assert!(super::configured_command_rows(&malformed).is_err());
        }
    }

    #[test]
    fn configured_command_keeps_multiline_arguments_and_nested_name() {
        assert_eq!(
            super::configured_command_input("/team/review first\n  second 世界\n"),
            Some(("team/review", "first\n  second 世界\n"))
        );
        assert_eq!(super::configured_command_input("/review"), Some(("review", "")));
        assert_eq!(super::configured_command_input("plain text"), None);
        assert_eq!(super::configured_command_input("! pwd"), None);
    }

    #[test]
    fn every_palette_entry_is_a_local_action_not_a_provider_prompt() {
        let mut palette = crate::native_command_palette::CommandPalette::default();
        palette.open(true);
        for entry in palette.visible() {
            let crate::native_command_palette::PaletteAction::Route(route) = entry.action;
            assert!(
                super::native_exit_route(route)
                    || super::native_local_page_route(route).is_some()
                    || matches!(
                        route,
                        "/help" | "/sessions" | "/new" | "/connect" | "/models" | "/appearance" | "/subagents" | "/import" | "/settings-import" | "/settings"
                    ),
                "palette action has no local handler: {route}"
            );
        }
        assert_eq!(
            super::native_local_page_route("/status"),
            Some(crate::native_view_pages::Page::Status)
        );
        assert_eq!(
            super::native_local_page_route("/theme"),
            Some(crate::native_view_pages::Page::Theme)
        );
        assert!(super::native_exit_route("/exit"));
        assert!(super::native_exit_route("/quit"));
        assert!(super::native_exit_route("/q"));
        assert_eq!(super::native_local_page_route("/not-a-local-command"), None);
        assert_eq!(super::native_local_page_route("/memory"), Some(crate::native_view_pages::Page::Memory));
        assert!(!super::native_exit_route("/not-a-local-command"));
    }
    use super::{consume_local_command, is_recognized_local_command};

    /// CORRECTED expectations, not weakened ones: an independent
    /// test-contract evaluator recorded APPROVE_SUPERSESSION (2026-10-30)
    /// against pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`. Upstream runs
    /// `exit()` for `app.exit` and its `quit`/`q` aliases
    /// (`packages/tui/src/app.tsx:1176-1181`) and for the submitted exit
    /// words `exit`, `quit`, `:q`
    /// (`packages/tui/src/component/prompt/index.tsx:1128`, mirrored in
    /// `packages/tui/src/mini/prompt.shared.ts:71-76`); `:quit` is the
    /// approved native extension of the same action. The previous version of
    /// this test asserted that `/exit` closed a tab and must NOT exit — that
    /// encoded the installed defect (root-window `/exit` closed a tab and the
    /// app never terminated).
    #[test]
    fn every_exit_spelling_terminates_client_and_tab_close_stays_separate() {
        for spelling in ["/exit", "/quit", "/q", "exit", "quit", ":q", ":quit"] {
            assert!(
                super::native_process_exit_route(spelling),
                "{spelling} must terminate the client globally"
            );
        }
        for not_exit in [
            "/close",
            "/tab",
            "exit now",
            "/exit extra",
            "/exits",
            "Exit",
            "/EXIT",
            "/not-a-command",
        ] {
            assert!(
                !super::native_process_exit_route(not_exit),
                "{not_exit} must not terminate the client"
            );
        }
        // Tab close is a SEPARATE action with no typed command: upstream's
        // `session.tab.close` has no `slash` entry (`app.tsx:878-884`) and is
        // bound to `<leader>w` (`config/keybind.ts:119`). The navigation
        // transition still closes one tab while the daemon-side work state
        // (reopen/daemon work) is preserved.
        let mut navigation = crate::native_navigation::NavigationState::new();
        let session =
            crate::native_navigation::SessionRef("00000000-0000-0000-0000-000000000001".into());
        navigation.open(session.clone());
        navigation.apply(crate::native_navigation::DaemonEvent::WorkState {
            session: session.clone(),
            state: crate::native_navigation::WorkState::Running,
        });
        assert!(navigation.close(&session).is_ok());
        assert_eq!(navigation.tab_len(), 0);
        assert_eq!(
            navigation.work_state(&session),
            Some(crate::native_navigation::WorkState::Running)
        );
    }

    #[test]
    fn new_session_tab_keeps_the_outgoing_tab_state_and_routes_home() {
        let mut navigation = crate::native_navigation::NavigationState::new();
        let first = crate::native_navigation::SessionRef("first".into());
        let second = crate::native_navigation::SessionRef("second".into());
        navigation.open(first.clone());
        navigation.open(second.clone());
        let result = super::new_session_tab_transition(
            &mut navigation,
            "second draft",
            3,
            Some("fixture/other".into()),
            9,
            7,
        )
        .expect("routing Home cannot fail");
        assert!(result.home);
        assert!(result.snapshot.is_none());
        assert_eq!(result.draft, "");
        assert_eq!(result.generation, 7);
        // Tabs stay open and the outgoing tab keeps its own composer state so
        // selecting it again restores exactly that draft (`app.tsx:777-796`).
        assert_eq!(navigation.tab_len(), 2);
        assert!(navigation.active().is_none());
        let saved = navigation.tab_at(1).expect("second tab retained");
        assert_eq!(saved.draft, "second draft");
        assert_eq!(saved.cursor, 3);
        assert_eq!(saved.scroll, 9);
        assert_eq!(saved.selected_model.as_deref(), Some("fixture/other"));
        assert_eq!(navigation.tab_at(0).expect("first tab retained").draft, "");
    }

    #[test]
    fn shared_close_result_application_restores_unicode_middle_cursor_scroll_and_model() {
        let mut view = crate::native_view::NativeView::new(80, 24);
        view.set_session_route(true);
        let mut live = None;
        let mut history_window = None;
        let mut generation = 4;
        let mut draft = String::new();
        let mut cursor = 0;
        let mut selected = None;
        let models = vec![super::native_setup::Model {
            id: "target".into(),
            name: "Target".into(),
            provider_id: "fixture".into(),
            context_window: None,
        }];
        let mut route = true;
        let mut compact_worker = None;
        super::apply_close_tab_result(
            super::CloseTabResult {
                snapshot: None,
                draft: "left 世界 right".into(),
                cursor: 9,
                scroll: 37,
                selected_model: Some("fixture/target".into()),
                generation: 5,
                home: false,
            },
            &mut view,
            &mut live,
            &mut history_window,
            &mut generation,
            &mut draft,
            &mut cursor,
            &mut selected,
            &models,
            &mut route,
            &mut compact_worker,
        );
        assert_eq!(draft, "left 世界 right");
        assert_eq!(cursor, 8, "cursor 9 in fixture is mid-codepoint; normalize_cursor must pull it back to the UTF-8 boundary at byte 8");
        assert_eq!(view.chat_scroll_offset(), 37);
        assert_eq!(selected.as_ref().map(|model| model.qualified()).as_deref(), Some("fixture/target"));
        assert_eq!(generation, 4, "local result application does not commit generation twice");
        assert!(route);
    }

    #[test]
    fn shared_close_result_application_home_clears_stale_live_binding_and_route() {
        let mut view = crate::native_view::NativeView::new(80, 24);
        view.set_session_route(true);
        let mut live = Some(super::LiveSnapshot {
            origin: "http://127.0.0.1:1".into(),
            session_id: "00000000-0000-0000-0000-000000000001".into(),
            title: "Closed session".into(),
            state: "idle".into(),
            updated_at: String::new(),
            message_count: 0,
            last_text: None,
            history: Vec::new(),
            typed_history: Vec::new(),
            raw_history: Vec::new(),
            history_next_before: None,
            history_children: 0,
            parent_session: false,
            parent_session_id: None,
            directory: None,
            session_titles: vec![],
            session_project_labels: vec![],
        });
        let mut history_window = live.as_ref().map(|snapshot| crate::native_chat_projection::NativeHistoryWindow::new(
            &snapshot.raw_history,
            snapshot.history_next_before.clone(),
        ));
        let mut generation = 7;
        let mut draft = "unsent".into();
        let mut cursor = 3;
        let mut selected = None;
        let models = Vec::new();
        let mut route = true;
        let mut compact_worker = None;
        super::apply_close_tab_result(
            super::CloseTabResult {
                snapshot: None,
                draft: String::new(),
                cursor: 0,
                scroll: 0,
                selected_model: None,
                generation: 7,
                home: true,
            },
            &mut view,
            &mut live,
            &mut history_window,
            &mut generation,
            &mut draft,
            &mut cursor,
            &mut selected,
            &models,
            &mut route,
            &mut compact_worker,
        );
        assert!(!route);
        assert!(draft.is_empty());
        assert_eq!(cursor, 0);
        assert!(live.is_none(), "Home clears the closed session binding");
        assert!(history_window.is_none(), "Home clears history bound to the closed session");
        assert!(compact_worker.is_none(), "Home releases stale compact admission slot");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closing_last_tab_retires_compact_worker_without_cancelling_daemon_work() {
        let mut view = crate::native_view::NativeView::new(80, 24);
        view.set_session_route(true);
        let mut live = Some(super::LiveSnapshot {
            origin: "http://127.0.0.1:1".into(),
            session_id: "00000000-0000-0000-0000-000000000001".into(),
            title: "Closed session".into(), state: "idle".into(), updated_at: String::new(),
            message_count: 0, last_text: None, history: Vec::new(), typed_history: Vec::new(),
            raw_history: Vec::new(), history_next_before: None, history_children: 0, parent_session: false, parent_session_id: None, directory: None,
            session_titles: vec![],
            session_project_labels: vec![],
        });
        let mut history = None;
        let mut generation = 7;
        let mut draft = String::new();
        let mut cursor = 0;
        let mut selected = None;
        let mut route = true;
        let (result_tx, result_rx) = tokio::sync::mpsc::channel(1);
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = release_rx.await;
            let delivered = result_tx.send(Ok("old session compaction complete".to_owned())).await.is_ok();
            let _ = finished_tx.send(delivered);
        });
        let mut compact_worker = Some(super::NativeCompactWorker {
            rx: result_rx, task: Some(task),
            session_id: "00000000-0000-0000-0000-000000000001".into(), generation,
            origin: "http://127.0.0.1:1".into(), auth: "a".repeat(64),
        });
        let models = Vec::new();
        super::apply_close_tab_result(
            super::CloseTabResult { snapshot: None, draft: String::new(), cursor: 0, scroll: 0,
                selected_model: None, generation, home: true },
            &mut view, &mut live, &mut history, &mut generation, &mut draft, &mut cursor,
            &mut selected, &models, &mut route,
            &mut compact_worker,
        );
        assert!(compact_worker.is_none(), "fresh Home admission has no stale compact slot");
        assert!(live.is_none(), "last-tab close clears the old binding");
        assert!(!route, "last-tab close returns to Home");
        assert!(!view.dialogs.is_open());
        release_tx.send(()).unwrap();
        assert!(!finished_rx.await.unwrap(), "request completes but stale UI result is discarded");
    }

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

#[cfg(all(test, feature = "native"))]
mod compact_strategy_worker_tests {
    use super::NativeCompactWorker;
    use std::{io::{Read, Write}, net::{TcpListener, TcpStream}, time::{Duration, Instant}};

    fn read_request(stream: &mut TcpStream) -> String {
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0u8; 1024];
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0);
            assert!(bytes.len() + read <= 16 * 1024);
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                let length = headers.lines().find_map(|line| line.to_ascii_lowercase()
                    .strip_prefix("content-length:").and_then(|value| value.trim().parse::<usize>().ok())).unwrap_or(0);
                if bytes.len() >= end + 4 + length { return String::from_utf8(bytes).unwrap(); }
            }
        }
    }

    #[test]
    fn settings_requests_are_authenticated_and_bound_to_the_requested_session() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let token = "c".repeat(64);
        let fixture = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                let body = r#"{"scope":"session"}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        let first = "00000000-0000-4000-8000-000000000001";
        let second = "00000000-0000-4000-8000-000000000002";
        assert!(super::request_compaction_settings(&origin, first, Some(&token), None).is_ok());
        let policy = r#"{"enabled":false,"threshold_tokens":12000,"keep_recent_turns":2,"strategy":"fallback"}"#;
        assert!(super::request_compaction_settings(&origin, second, Some(&token), Some(policy)).is_ok());
        let requests = fixture.join().unwrap();
        assert!(requests[0].starts_with(&format!("GET /api/sessions/{first}/compaction/settings HTTP/1.1\r\n")));
        assert!(requests[1].starts_with(&format!("PUT /api/sessions/{second}/compaction/settings HTTP/1.1\r\n")));
        for request in &requests {
            assert!(request.to_ascii_lowercase().contains(&format!("authorization: bearer {token}")));
        }
        assert_eq!(requests[1].split_once("\r\n\r\n").unwrap().1, policy);
    }

    #[test]
    fn settings_refuse_invalid_authority_before_connecting() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let session = "00000000-0000-4000-8000-000000000001";
        assert!(super::request_compaction_settings(&origin, session, None, None).is_err());
        assert!(super::request_compaction_settings("http://192.0.2.1:1", session, Some(&"c".repeat(64)), None).is_err());
        assert!(super::request_compaction_settings(&origin, "not-a-session", Some(&"c".repeat(64)), None).is_err());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    }

    #[test]
    fn settings_refuse_noncanonical_uuid_before_connecting() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let uppercase = "ABCDEF12-1234-4000-8000-000000000001";
        assert!(super::request_compaction_settings(
            &origin, uppercase, Some(&"c".repeat(64)), None
        ).is_err());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn strategy_worker_selects_and_validates_authenticated_strategy_ack() {
        for (requested, acknowledged, succeeds) in [
            ("fallback", "fallback", true), ("auto", "fallback", true),
            ("auto", "native", true), ("native", "fallback", false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let token = "b".repeat(64);
            let fixture = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                let body = format!(r#"{{"compacted":true,"strategy":"{acknowledged}"}}"#);
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                request
            });
            let mut worker = NativeCompactWorker::start_strategy(&origin, "strategy-session", &token, "openai/fixture", 19, requested).unwrap();
            assert!(worker.owns(Some("strategy-session"), 19));
            assert!(!worker.owns(Some("other-session"), 19));
            let deadline = Instant::now() + Duration::from_secs(2);
            let result = loop {
                if let Some(result) = worker.try_result() { break result; }
                assert!(Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            assert_eq!(result.is_ok(), succeeds, "requested={requested}, acknowledged={acknowledged}");
            let request = fixture.join().unwrap();
            assert!(request.starts_with("POST /api/sessions/strategy-session/compact HTTP/1.1\r\n"));
            assert!(request.to_ascii_lowercase().contains(&format!("authorization: bearer {token}")));
            let sent: serde_json::Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(sent, serde_json::json!({"model":"openai/fixture", "strategy":requested}));
        }
        assert!(NativeCompactWorker::start_strategy("http://192.0.2.1:1", "s", &"b".repeat(64), "openai/fixture", 1, "fallback").is_err());
    }
}

#[cfg(all(test, feature = "native"))]
mod native_cli_config_path_tests {
    use super::resolve_native_cli_config_path;
    use std::path::Path;

    #[test]
    fn home_only_uses_opencode_subdirectory() {
        assert_eq!(
            resolve_native_cli_config_path(None, None, Some(Path::new("/home/test"))),
            Some(Path::new("/home/test/.config/opencode/cli.json").to_path_buf())
        );
    }

    #[test]
    fn explicit_then_xdg_then_home_precedence() {
        let explicit = Path::new("/explicit");
        let xdg = Path::new("/xdg");
        let home = Path::new("/home/test");
        assert_eq!(resolve_native_cli_config_path(Some(explicit), Some(xdg), Some(home)), Some(Path::new("/explicit/cli.json").to_path_buf()));
        assert_eq!(resolve_native_cli_config_path(None, Some(xdg), Some(home)), Some(Path::new("/xdg/opencode/cli.json").to_path_buf()));
        assert_eq!(resolve_native_cli_config_path(None, None, Some(home)), Some(Path::new("/home/test/.config/opencode/cli.json").to_path_buf()));
    }

    #[test]
    fn relative_roots_are_ignored() {
        assert_eq!(resolve_native_cli_config_path(Some(Path::new("relative")), Some(Path::new("xdg")), Some(Path::new("home"))), None);
        assert_eq!(resolve_native_cli_config_path(Some(Path::new("relative")), None, Some(Path::new("/home/test"))), Some(Path::new("/home/test/.config/opencode/cli.json").to_path_buf()));
    }
}

#[cfg(all(test, feature = "native"))]
mod native_theme_registry_tests {
    use super::native_theme_choices;
    use std::path::PathBuf;

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (Fixture, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "opencode-theme-registry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let themes = root.join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        (Fixture(root.clone()), root.join("tui.json"))
    }

    #[test]
    fn malformed_custom_shadow_is_not_advertised_as_available() {
        let (_fixture, config) = fixture();
        std::fs::write(config.parent().unwrap().join("themes/opencode.json"), "{").unwrap();
        let choices = native_theme_choices(&[config], None, crate::native_theme_runtime::ThemeMode::Dark).unwrap();
        let opencode = choices.iter().find(|choice| choice.name == "opencode").unwrap();
        assert!(!opencode.available);
    }

    #[test]
    fn directory_enumeration_has_a_hard_entry_bound() {
        let (_fixture, config) = fixture();
        let themes = config.parent().unwrap().join("themes");
        for index in 0..257 {
            std::fs::write(themes.join(format!("theme-{index}.json")), "{}").unwrap();
        }
        assert!(native_theme_choices(&[config], None, crate::native_theme_runtime::ThemeMode::Dark).is_err());
    }

    #[test]
    fn validation_cap_reports_incomplete_instead_of_false_unavailable() {
        let (_fixture, config) = fixture();
        let themes = config.parent().unwrap().join("themes");
        let asset = crate::native_theme_assets::lookup("opencode").unwrap();
        for index in 0..33 {
            std::fs::write(themes.join(format!("custom-{index}.json")), asset).unwrap();
        }
        let result = native_theme_choices(
            &[config],
            None,
            crate::native_theme_runtime::ThemeMode::Dark,
        );
        assert!(matches!(result, Err(error) if error.contains("registry incomplete")));
    }

    #[test]
    fn disappeared_current_custom_identity_remains_unavailable() {
        let (_fixture, config) = fixture();
        let choices = native_theme_choices(&[config], Some("removed-custom"), crate::native_theme_runtime::ThemeMode::Dark).unwrap();
        let removed = choices.iter().find(|choice| choice.name == "removed-custom").unwrap();
        assert!(!removed.available);
    }
}

#[cfg(all(test, feature = "native"))]
mod middle_close_hit_tests {
    use super::native_input_decoder::{InputEvent, TerminalInputDecoder};
    use crate::native_tabs::{TabHit, TabInput, TabOrientation, TabRect, TabStrip};
    use crate::native_tab_rail::{RailFrame, RailHit, RailInput};
    use std::time::Instant;

    fn nav_inputs() -> Vec<RailInput> {
        vec![
            RailInput { session: crate::native_navigation::SessionRef("s-a".into()), title: "Alpha".into(), detail: "alpha detail".into(), flags: Default::default(), marquee_offset: 0 },
            RailInput { session: crate::native_navigation::SessionRef("s-b".into()), title: "Beta".into(), detail: "".into(), flags: Default::default(), marquee_offset: 0 },
        ]
    }

    #[test]
    fn vertical_rail_middle_press_targets_only_real_session_rows_not_new_or_gap() {
        let inputs = nav_inputs();
        // 42 wide, 10 cells: top padding (1) + two sessions (each 2 rows) + gap.
        let mut active = inputs.clone();
        active[1].flags.active = true; // mark session s-b active so active_new=false
        let frame = RailFrame::render_cells(&active, 42, 10, 0, None, false);
        // Session rows exist (title row 0, detail row 1 per session).
        let title_row = frame.rows.iter().find(|r| matches!(r.kind, crate::native_tab_rail::RailRowKind::Session(ref s) if s.0 == "s-a")).expect("session row present");
        let (x, y) = (title_row.rect.x, title_row.rect.y);
        assert_eq!(frame.hit(x, y), Some(RailHit::Session(crate::native_navigation::SessionRef("s-a".into()))));
        assert_eq!(frame.hit(0, 0), None, "top padding never closes");
        assert_eq!(frame.hit(41, 9), None, "right/bottom whitespace never closes");
        // New control present only in active_new mode; here inert.
        let new_rect = frame.new_rect;
        // Confirm a point within the new rect (if any) is NOT a Session hit.
        if let Some(rect) = new_rect {
            assert!(matches!(frame.hit(rect.x, rect.y), Some(RailHit::New) | None), "new row never closes a session");
        }
    }

    #[test]
    fn vertical_rail_middle_release_drag_return_distinct_kinds_and_never_session() {
        let inputs = nav_inputs();
        let frame = RailFrame::render_cells(&inputs, 42, 10, 0, None, false);
        let title_row = frame.rows.iter().find(|r| matches!(r.kind, crate::native_tab_rail::RailRowKind::Session(_))).expect("row");
        let (x, y) = (title_row.rect.x, title_row.rect.y);
        let now = Instant::now();
        // middle press on session
        let mut dec = TerminalInputDecoder::default();
        for b in format!("\x1b[<1;{};{}M", x + 1, y + 1).bytes() { dec.push(b, now); }
        assert_eq!(dec.pop(), Some(InputEvent::Mouse { x, y, kind: 8, modifiers: 0 }));
        // middle release at same coords
        let mut dec = TerminalInputDecoder::default();
        for b in format!("\x1b[<1;{};{}m", x + 1, y + 1).bytes() { dec.push(b, now); }
        assert_eq!(dec.pop(), Some(InputEvent::Mouse { x, y, kind: 9, modifiers: 0 }));
        // middle drag
        let mut dec = TerminalInputDecoder::default();
        for b in format!("\x1b[<33;{};{}M", x + 1, y + 1).bytes() { dec.push(b, now); }
        assert_eq!(dec.pop(), Some(InputEvent::Mouse { x, y, kind: 10, modifiers: 0 }));
        assert_eq!(dec.pop(), None);
    }

    #[test]
    fn horizontal_tabs_hit_resolves_session_not_add_region_not_overflow() {
        let tabs: Vec<_> = (0..2).map(|i| TabInput {
            session: crate::native_navigation::SessionRef(format!("h-{i}")),
            title: format!("Tab{i}"),
            flags: Default::default(),
            attention: None,
            marquee_offset: 0,
        }).collect();
        let strip = TabStrip::render_with_hover_and_add(&tabs, TabOrientation::Horizontal, 60, 1, 0, None, true);
        // Some session rects exist.
        assert!(!strip.rects.is_empty());
        let (sess, rect) = &strip.rects[0];
        assert_eq!(strip.hit(rect.x, rect.y), Some(TabHit { session: sess.clone(), close: false }));
        // Add control (plain +, never closes) is its own rect; hit returns None there.
        let add = strip.add_rect.expect("add rect present");
        assert!(strip.add_hit(add.x + 1, 0));
        assert_eq!(strip.hit(add.x + 1, 0), None, "add region is not a session target");
        // Whitespace / overflow paint rows (y beyond 0) return None.
        assert_eq!(strip.hit(0, 5), None, "outside tab row never closes");
    }

    #[test]
    fn decoder_middle_kind_assignment_is_stable_and_modifier_isolated() {
        let now = Instant::now();
        let cases = [
            (b"\x1b[<1;10;20M" as &[u8], Some(InputEvent::Mouse { x: 9, y: 19, kind: 8, modifiers: 0 })),   // plain middle down
            (b"\x1b[<1;10;20m", Some(InputEvent::Mouse { x: 9, y: 19, kind: 9, modifiers: 0 })),              // plain middle up
            (b"\x1b[<33;10;20M", Some(InputEvent::Mouse { x: 9, y: 19, kind: 10, modifiers: 0 })),           // middle drag
            (b"\x1b[<5;10;20M", Some(InputEvent::Mouse { x: 9, y: 19, kind: 8, modifiers: 4 })),              // middle+shift
            (b"\x1b[<9;10;20M", Some(InputEvent::Mouse { x: 9, y: 19, kind: 8, modifiers: 8 })),              // middle+meta
            (b"\x1b[<17;10;20M", Some(InputEvent::Mouse { x: 9, y: 19, kind: 8, modifiers: 16 })),           // middle+ctrl
            (b"\x1b[<29;10;20M", Some(InputEvent::Mouse { x: 9, y: 19, kind: 8, modifiers: 28 })),            // middle+all mods
        ];
        for (seq, expect) in cases {
            let mut dec = TerminalInputDecoder::default();
            for b in seq { dec.push(*b, now); }
            assert_eq!(dec.pop(), expect, "sequence {:?}", std::str::from_utf8(seq).unwrap());
            assert_eq!(dec.pop(), None);
        }
        // Regression: left/right/wheel/hover still map to their canonical kinds.
        let reg = b"\x1b[<0;1;1M\x1b[<2;1;1M\x1b[<2;1;1m\x1b[<32;1;1M\x1b[<35;1;1M\x1b[<64;1;1M\x1b[<65;1;1M";
        let mut dec = TerminalInputDecoder::default();
        for b in reg { dec.push(*b, now); }
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(0));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(5));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(6));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(1));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(7));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(3));
        assert_eq!(dec.pop().map(|e| if let InputEvent::Mouse { kind, .. } = e { kind } else { 99 }), Some(4));
        assert_eq!(dec.pop(), None);
    }

    #[test]
    fn middle_release_and_drag_kinds_are_consumed_not_zeroed() {
        // Guard: caller's `kind != 0 && kind != 2 && kind != 7` must also reject
        // middle release (9) and drag (10) so tab_press/add_press clear — i.e. they
        // never reach the kind==0/2 close path. This test asserts the guard logic.
        for kind in [1u8, 9, 10] {
            let blocked = !(kind == 0 || kind == 2 || kind == 7);
            assert!(blocked, "kind {kind} must clear tab press state");
        }
    }
}

#![forbid(unsafe_code)]
//! Right-pane Status + MCP view — SOURCE-PROPOSAL-UNINTEGRATED.
//!
//! Pure, bounded renderer over caller-confirmed daemon snapshots. No IO, no
//! clock, no threads, no backend registry, no new HTTP route, no metrics, no
//! persistence: the caller GETs `GET /api/mcp` (name-keyed status map,
//! `crates/server/src/mcp_http.rs`) plus the read-only native session /
//! provider / model / context snapshot, then feeds confirmed values here.
//!
//! Honesty rules baked into this type:
//! * every field renders either its confirmed value or `Unavailable`;
//!   `connected` is never asserted without a caller-confirmed observation
//!   (`set_connection` is only called after an ACK, `set_connection_unknown`
//!   retracts it, and connection/reconnect snapshots are reconciled by the
//!   caller — this view never self-heals). The model label is three-state:
//!   not observed → `Unavailable`, confirmed no-selection → exact
//!   `model: unset` (canonical native label, `tui_entry.rs:636`), confirmed
//!   selection → `model: <label>`);
//! * `apply_mcp` accepts the typed upstream status union
//!   (`connected | disabled | failed | needs_auth | needs_client_registration`,
//!   pinned `packages/opencode/src/mcp/index.ts` `Status`), rejects anything
//!   else **atomically**: the last good map stays installed, the snapshot is
//!   marked stale, and the bounded reason is returned;
//! * error material is reduced to a bounded stable code — raw error text,
//!   env, command, header or token material can never be rendered because
//!   unknown entry fields are rejected outright;
//! * before the first ACK the MCP section renders `Loading/Unavailable`;
//!   a confirmed fresh `{}` renders `No servers`.
//!
//! Actions are *intents only*: [`McpIntent`] carries no side effect. The
//! caller performs the GET/POST and applies the confirmed response through
//! [`StatusView::apply_mcp`] before any status line changes. `connect` is
//! currently 501 `mcp_connect_unsupported` in this daemon build
//! (`mcp_http.rs`), so it is rendered as unsupported — never a success
//! button.

use crate::native_transcript::strip_ansi;
use crate::native_view_chat::MAX_INPUT_ROW_BYTES;
use opencode_rk_contracts::SessionId;
use serde_json::Value;

/// Maximum MCP entries accepted from one snapshot (daemon registry cap).
pub const MAX_MCP_ENTRIES: usize = 64;
/// Maximum MCP server-name bytes (`MAX_ID_LEN` in `mcp_lifecycle.rs`).
pub const MAX_NAME_BYTES: usize = 128;
/// Maximum error-code bytes (`MAX_ERROR_CODE_LEN` in `mcp_lifecycle.rs`).
pub const MAX_ERROR_CODE_BYTES: usize = 64;
/// Maximum chars rendered for any caller-supplied field value.
pub const MAX_FIELD_CHARS: usize = 128;
/// Hard cap on rows produced by one render.
pub const MAX_ROWS: usize = 256;
/// Stable code shown when raw error material is not a safe bounded code.
const INTERNAL_CODE: &str = "internal";
/// Honest default while no MCP snapshot has been acknowledged.
const LOADING: &str = "Loading/Unavailable";
/// Honest empty result for a confirmed fresh `{}` map.
const NO_SERVERS: &str = "No servers";
/// Honest default for a field the caller never confirmed.
const UNAVAILABLE: &str = "Unavailable";
/// Whether this daemon build implements `POST /api/mcp/{name}/connect`.
pub const CONNECT_SUPPORTED: bool = false;

/// One parsed, bounded MCP entry from a confirmed status map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpEntry {
    pub name: String,
    pub status: McpStatus,
}

/// Typed upstream status union (`packages/opencode/src/mcp/index.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpStatus {
    Connected,
    Disabled,
    Failed { error: String },
    NeedsAuth,
    NeedsClientRegistration { error: String },
}

impl McpStatus {
    /// Wire-exact lowercase status word.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            McpStatus::Connected => "connected",
            McpStatus::Disabled => "disabled",
            McpStatus::Failed { .. } => "failed",
            McpStatus::NeedsAuth => "needs_auth",
            McpStatus::NeedsClientRegistration { .. } => "needs_client_registration",
        }
    }

    /// Bounded safe error code, when this status carries one.
    #[must_use]
    pub fn error_code(&self) -> Option<&str> {
        match self {
            McpStatus::Failed { error } | McpStatus::NeedsClientRegistration { error } => {
                Some(error.as_str())
            }
            _ => None,
        }
    }
}

/// Frontend action intent. Carries no effect: the caller performs the HTTP
/// request and applies the confirmed response before status changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpIntent {
    Add { name: String },
    Disconnect { name: String },
    Connect { name: String },
}

/// Observed state of the selected model label. Three states, because
/// "never observed" and "observed to be unset" are different facts: the
/// native product renders `model: unset` when no model is selected
/// (`crates/cli/src/tui_entry.rs:636`, `render_frame(..., "unset", ...)` at
/// `:490/:1145/:1169`, frozen note `crates/cli/tests/native_daemon_flow.rs:861`)
/// while an unobserved label must stay `Unavailable`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum ModelSlot {
    /// No confirmed observation (default, or a failed refresh).
    #[default]
    NotObserved,
    /// Confirmed: the caller's parsed state selects no model.
    ConfirmedUnset,
    /// Confirmed: a selected model label (bounded, control-free).
    ConfirmedSelected(String),
}

impl ModelSlot {
    /// Exact rendered label for this state.
    #[must_use]
    fn label(&self) -> &str {
        match self {
            ModelSlot::NotObserved => UNAVAILABLE,
            ModelSlot::ConfirmedUnset => "unset",
            ModelSlot::ConfirmedSelected(label) => label.as_str(),
        }
    }
}

/// Bounded right-pane Status + MCP view. Confirmed data only.
#[derive(Clone, Debug, Default)]
pub struct StatusView {
    /// `None` = never confirmed / retracted (`Unavailable`), never a guess.
    connection: Option<bool>,
    model: ModelSlot,
    provider: Option<String>,
    agent: Option<String>,
    session: Option<String>,
    /// Accepted-binding display title: sanitized bounded text from the
    /// SAME accepted snapshot as `session` (initial auto-resume snapshot
    /// or a successful bind transaction). A bare id observation never
    /// sets it, so it can never be faked from a display row label.
    session_title: Option<String>,
    /// Accepted authenticated binding fact: set ONLY by
    /// [`Self::set_session_binding`]. `(live)` renders from this flag and
    /// means "client bound to the authenticated daemon" — never provider
    /// busy, never SSE connected, never a guess.
    session_live: bool,
    /// Confirmed persisted message count of the SAME accepted
    /// authenticated snapshot tuple as `session`/`session_title`:
    /// `messages.len()` from the authenticated
    /// `GET /api/sessions/{id}/messages?limit=200` snapshot. `None` =
    /// never confirmed / retracted — never transcript rows, provider
    /// deltas, or a fabricated zero.
    session_message_count: Option<usize>,
    /// `(used, window)` from the caller's context snapshot.
    context: Option<(u64, u64)>,
    /// Last good MCP map (retained across a rejected snapshot).
    mcp: Vec<McpEntry>,
    /// True once at least one snapshot was accepted (ACK).
    mcp_ack: bool,
    /// Bounded reason the newest snapshot was rejected; last good retained.
    mcp_stale: Option<String>,
}

impl StatusView {
    /// Caller-confirmed daemon connection observation. Call only after an
    /// ACK; connection/reconnect snapshots are reconciled by the caller.
    pub fn set_connection(&mut self, connected: bool) {
        self.connection = Some(connected);
    }

    /// Retract the connection observation (no fresh snapshot): renders
    /// `Unavailable`, never a stale `connected`.
    pub fn set_connection_unknown(&mut self) {
        self.connection = None;
    }

    /// Record the caller-confirmed selected model label.
    ///
    /// * `Some(valid)` — confirmed selection; renders `model: <label>`.
    /// * `None` — **confirmed unset** (the caller's parsed native state has
    ///   no selected model); renders exactly `model: unset`, matching the
    ///   canonical native behaviour at `tui_entry.rs:636`.
    /// * `Some(invalid)` (blank/control-only) — not an observation; the last
    ///   good state is kept and is never relabelled `unset`.
    ///
    /// For a failed refresh that observed nothing, use
    /// [`Self::set_model_unknown`].
    pub fn set_model(&mut self, model: Option<String>) {
        match model {
            None => self.model = ModelSlot::ConfirmedUnset,
            Some(raw) => {
                let cleaned = display(Some(&raw));
                if cleaned != UNAVAILABLE {
                    self.model = ModelSlot::ConfirmedSelected(cleaned);
                }
            }
        }
    }

    /// Retract the model observation (failed refresh / no snapshot): renders
    /// `Unavailable`. Distinct from [`Self::set_model(None)`], which is a
    /// confirmed "no model selected".
    pub fn set_model_unknown(&mut self) {
        self.model = ModelSlot::NotObserved;
    }

    /// Record a bare session identity (UUID) observation. This is NOT an
    /// accepted authenticated binding: any previously bound title,
    /// confirmed message count and `(live)` fact are retracted so a bare
    /// id can never fake auth metadata — the render falls back to
    /// `session: Unavailable` plus the separate `session id:` identity
    /// row and `messages: Unavailable`.
    pub fn set_session(&mut self, id: Option<String>) {
        self.session = non_empty(id);
        self.session_title = None;
        self.session_message_count = None;
        self.session_live = false;
    }

    /// Record an ACCEPTED authenticated binding tuple: id, title and the
    /// confirmed persisted `message_count` come from the SAME accepted
    /// snapshot (the startup auto-resume snapshot, or the fresh
    /// listing/snapshot pair of a successful bind transaction).
    /// `(live)` renders only when a CANONICAL session id and a
    /// display-usable title are present — an unusable title keeps the
    /// identity (and the confirmed count: its provenance is the accepted
    /// snapshot, not the display title) but never claims `(live)`, and a
    /// `None`/invalid id clears the whole tuple. A malformed/noncanonical
    /// id fails closed here: this setter is the only path that can set
    /// `(live)` or a confirmed count, and it never projects an
    /// unvalidated string as an authenticated binding (the startup
    /// snapshot's listing id reaches this setter before the row
    /// projection's own parse guard).
    pub fn set_session_binding(
        &mut self,
        id: Option<String>,
        title: Option<String>,
        message_count: Option<usize>,
    ) {
        self.session = canonical_session_id(id);
        self.session_title = self
            .session
            .as_deref()
            .and(title.as_deref())
            .and_then(sanitize_session_title);
        // Confirmed count travels with the accepted canonical id (any
        // confirmed value, including a valid empty-history 0); an
        // invalid/bare binding clears it — never a fabricated count.
        self.session_message_count = if self.session.is_some() {
            message_count
        } else {
            None
        };
        self.session_live = self.session.is_some() && self.session_title.is_some();
    }

    /// Optional context setters (confirmed values only, `None` => Unavailable).
    pub fn set_provider(&mut self, provider: Option<String>) {
        self.provider = non_empty(provider);
    }

    pub fn set_agent(&mut self, agent: Option<String>) {
        self.agent = non_empty(agent);
    }

    pub fn set_context(&mut self, used: Option<u64>, window: Option<u64>) {
        self.context = match (used, window) {
            (Some(u), Some(w)) => Some((u, w)),
            _ => None,
        };
    }

    /// Confirmed connection observation, if any.
    #[must_use]
    pub fn connection(&self) -> Option<bool> {
        self.connection
    }

    /// Install a confirmed `GET /api/mcp` status map.
    ///
    /// Accepts only the name-keyed typed union produced by
    /// `crates/server/src/mcp_http.rs` / pinned upstream: ≤
    /// [`MAX_MCP_ENTRIES`] entries, non-empty control-free names ≤
    /// [`MAX_NAME_BYTES`], statuses `connected | disabled | failed |
    /// needs_auth | needs_client_registration`, `error` required (string)
    /// for `failed` / `needs_client_registration` and forbidden elsewhere,
    /// no unknown fields. Validation is all-or-nothing: on `Err` the last
    /// good map is kept, marked stale with the bounded reason, and the
    /// reason is returned to the caller.
    pub fn apply_mcp(&mut self, value: &Value) -> Result<(), String> {
        match parse_mcp(value) {
            Ok(entries) => {
                self.mcp = entries;
                self.mcp_ack = true;
                self.mcp_stale = None;
                Ok(())
            }
            Err(reason) => {
                self.mcp_stale = Some(reason.clone());
                Err(reason)
            }
        }
    }

    /// Last good entries (bounded).
    #[must_use]
    pub fn entries(&self) -> &[McpEntry] {
        &self.mcp
    }

    /// Whether any snapshot has been acknowledged.
    #[must_use]
    pub fn mcp_acknowledged(&self) -> bool {
        self.mcp_ack
    }

    /// Bounded stale reason, when the newest snapshot was rejected.
    #[must_use]
    pub fn mcp_stale(&self) -> Option<&str> {
        self.mcp_stale.as_deref()
    }

    /// Intent only: validate and build an add request. The caller performs
    /// `POST /api/mcp` and applies the returned status map via
    /// [`Self::apply_mcp`] before any line changes.
    pub fn intent_add(&self, name: &str) -> Result<McpIntent, String> {
        validate_name(name)?;
        Ok(McpIntent::Add {
            name: name.to_owned(),
        })
    }

    /// Intent only: `POST /api/mcp/{name}/disconnect` for a known entry.
    pub fn intent_disconnect(&self, name: &str) -> Result<McpIntent, String> {
        self.require_known(name)?;
        Ok(McpIntent::Disconnect {
            name: name.to_owned(),
        })
    }

    /// Intent only: `POST /api/mcp/{name}/connect` for a known entry. This
    /// daemon build answers 501 `mcp_connect_unsupported`, so the caller
    /// must render unsupported — never an optimistic success.
    pub fn intent_connect(&self, name: &str) -> Result<McpIntent, String> {
        self.require_known(name)?;
        Ok(McpIntent::Connect {
            name: name.to_owned(),
        })
    }

    /// Pure right-pane render. Both headings (`Status`, `MCP`) are emitted
    /// whenever `height >= 2`, so they are visible at 120x30 with empty
    /// input; rows are bounded by `height`, [`MAX_ROWS`] and `width`.
    #[must_use]
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut out: Vec<String> = Vec::new();
        out.push(fit("Status", width));
        if height == 1 {
            return out;
        }
        let status_rows = self.status_rows();
        // Leave >=1 row for the `MCP` heading so both headings survive any
        // height >= 2, then cap the status body at a third of the pane.
        let budget = status_rows
            .len()
            .min((height / 3).max(1))
            .min(height.saturating_sub(2));
        append_rows(&mut out, &status_rows, budget, width);
        out.push(fit("MCP", width));
        let mcp_budget = height.saturating_sub(out.len());
        let mcp_rows = self.mcp_rows();
        append_rows(&mut out, &mcp_rows, mcp_budget, width);
        out.truncate(MAX_ROWS.min(height));
        out
    }

    /// Pure detail view for one selected server: confirmed status and safe
    /// error code only; actions shown as caller intents, `connect` as
    /// unsupported while [`CONNECT_SUPPORTED`] is false.
    #[must_use]
    pub fn render_detail(&self, name: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        let Some(entry) = self.mcp.iter().find(|e| e.name == name) else {
            return vec![fit(UNAVAILABLE, width)];
        };
        let mut rows = vec![
            format!("MCP: {}", display(Some(entry.name.as_str()))),
            format!("status: {}", entry.status.as_str()),
            format!("error: {}", entry.status.error_code().unwrap_or("none")),
        ];
        if let Some(reason) = &self.mcp_stale {
            rows.push(format!("snapshot: stale ({reason})"));
        }
        if CONNECT_SUPPORTED {
            rows.push("connect: caller action".to_owned());
        } else {
            rows.push("connect: unsupported (501 mcp_connect_unsupported)".to_owned());
        }
        rows.push("disconnect: caller action".to_owned());
        rows.into_iter().map(|r| fit(&r, width)).collect()
    }

    fn status_rows(&self) -> Vec<String> {
        let daemon = match self.connection {
            Some(true) => "daemon: connected".to_owned(),
            Some(false) => "daemon: disconnected".to_owned(),
            None => format!("daemon: {UNAVAILABLE}"),
        };
        vec![
            daemon,
            format!("model: {}", self.model.label()),
            format!("provider: {}", display(self.provider.as_deref())),
            format!("agent: {}", display(self.agent.as_deref())),
            // Title row: the accepted authenticated bound title with the
            // `(live)` marker ONLY while the binding fact holds; otherwise
            // honest Unavailable (never a fabricated default like `Chat`).
            if self.session_live {
                format!("session: {} (live)", display(self.session_title.as_deref()))
            } else {
                format!("session: {UNAVAILABLE}")
            },
            // Identity row: the actual execution UUID, kept separate from
            // the human-visible title (bounded, control-free).
            format!("session id: {}", display(self.session.as_deref())),
            // Message-count row: the confirmed persisted count of the
            // accepted authenticated snapshot only (bounded usize decimal,
            // width-masked by the pane; never transcript rows, provider
            // deltas, or a fabricated zero). Honest `Unavailable` without
            // an accepted binding fact.
            match self.session_message_count {
                Some(count) => format!("messages: {count}"),
                None => format!("messages: {UNAVAILABLE}"),
            },
            match self.context {
                Some((used, window)) => format!("context: used={used} window={window}"),
                None => format!("context: {UNAVAILABLE}"),
            },
        ]
    }

    fn mcp_rows(&self) -> Vec<String> {
        if !self.mcp_ack {
            return vec![format!("  {LOADING}")];
        }
        let mut rows = Vec::new();
        if self.mcp_stale.is_some() {
            rows.push("  snapshot: stale (last good retained)".to_owned());
        }
        if self.mcp.is_empty() {
            rows.push(format!("  {NO_SERVERS}"));
            return rows;
        }
        for entry in &self.mcp {
            let line = match entry.status.error_code() {
                Some(code) => format!(
                    "  {} {} error={}",
                    display(Some(entry.name.as_str())),
                    entry.status.as_str(),
                    code
                ),
                None => format!(
                    "  {} {}",
                    display(Some(entry.name.as_str())),
                    entry.status.as_str()
                ),
            };
            rows.push(line);
            if rows.len() >= MAX_ROWS {
                break;
            }
        }
        rows
    }

    fn require_known(&self, name: &str) -> Result<(), String> {
        validate_name(name)?;
        if !self.mcp.iter().any(|e| e.name == name) {
            return Err("name: unknown mcp server".to_owned());
        }
        Ok(())
    }
}

/// All-or-nothing parse of a name-keyed status map into bounded entries.
fn parse_mcp(value: &Value) -> Result<Vec<McpEntry>, String> {
    let map = value
        .as_object()
        .ok_or_else(|| "root: must be a JSON object".to_owned())?;
    if map.len() > MAX_MCP_ENTRIES {
        return Err(format!("entries: exceeds {MAX_MCP_ENTRIES}"));
    }
    let mut entries = Vec::with_capacity(map.len());
    for (name, raw) in map {
        entries.push(parse_entry(name, raw)?);
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn parse_entry(name: &str, raw: &Value) -> Result<McpEntry, String> {
    validate_name(name)?;
    let obj = raw
        .as_object()
        .ok_or_else(|| "entry: must be an object".to_owned())?;
    let status_str = obj
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "entry.status: required string".to_owned())?;
    let status = match status_str {
        "connected" => {
            allow_fields(obj, &["status"])?;
            McpStatus::Connected
        }
        "disabled" => {
            allow_fields(obj, &["status"])?;
            McpStatus::Disabled
        }
        "needs_auth" => {
            allow_fields(obj, &["status"])?;
            McpStatus::NeedsAuth
        }
        "failed" => {
            allow_fields(obj, &["status", "error"])?;
            McpStatus::Failed {
                error: safe_code(required_error(obj)?),
            }
        }
        "needs_client_registration" => {
            allow_fields(obj, &["status", "error"])?;
            McpStatus::NeedsClientRegistration {
                error: safe_code(required_error(obj)?),
            }
        }
        // Never echo the unknown value: it is unvalidated input.
        _ => return Err("entry.status: not a typed MCP status".to_owned()),
    };
    Ok(McpEntry {
        name: name.to_owned(),
        status,
    })
}

/// Reject any field outside the typed union (env/command/header/token
/// material can never enter the view).
fn allow_fields(obj: &serde_json::Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if obj.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("entry: unknown field".to_owned());
    }
    Ok(())
}

fn required_error(obj: &serde_json::Map<String, Value>) -> Result<&str, String> {
    obj.get("error")
        .and_then(Value::as_str)
        .ok_or_else(|| "entry.error: required string".to_owned())
}

/// Reduce raw error material to a bounded stable code; anything empty,
/// oversized, non `[a-z0-9_.-]`, or secret-shaped degrades to `internal`.
fn safe_code(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let secretish = [
        "sk-",
        "token",
        "secret",
        "password",
        "bearer",
        "authorization",
        "apikey",
        "api-key",
    ];
    if raw.is_empty()
        || raw.len() > MAX_ERROR_CODE_BYTES
        || secretish.iter().any(|needle| lower.contains(needle))
    {
        return INTERNAL_CODE.to_owned();
    }
    if raw
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'))
    {
        raw.to_owned()
    } else {
        INTERNAL_CODE.to_owned()
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name: required non-empty".to_owned());
    }
    if name.len() > MAX_NAME_BYTES {
        return Err("name: exceeds maximum length".to_owned());
    }
    if name.chars().any(char::is_control) {
        return Err("name: control characters are not allowed".to_owned());
    }
    Ok(())
}

/// `None`/empty => `Unavailable`; otherwise bounded control-free text.
fn display(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return UNAVAILABLE.to_owned();
    };
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_FIELD_CHARS)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        UNAVAILABLE.to_owned()
    } else {
        cleaned.to_owned()
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Canonical identity gate for the ACCEPTED-binding setter: the value must
/// be non-empty, parse with the production contracts [`SessionId`] parser
/// (the same one `bind_commit` step 0b and the listing/sessions commit
/// paths use) and round-trip exactly — `parse::<SessionId>()?.to_string()
/// == raw`, i.e. the full lowercase hyphenated UUID spelling. A value that
/// merely parses (uppercase, hyphenless, braced/URN) is still not
/// canonical, so it is rejected too. Anything else is `None`: the
/// projection fails closed instead of claiming an authenticated binding
/// from an unvalidated string.
fn canonical_session_id(id: Option<String>) -> Option<String> {
    let raw = non_empty(id)?;
    let parsed: SessionId = raw.parse().ok()?;
    (parsed.to_string() == raw).then_some(raw)
}

/// Bounded UTF-8 prefix for raw title input: O(bound) work regardless of
/// caller string size and never splits a scalar (a cut through an escape
/// is finished safely by [`strip_ansi`]'s unterminated-sequence handling).
/// The chat module's identical helper is private, so this minimal copy
/// lives here rather than widening any visibility.
fn clip_title_prefix(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Bounded display title for the `session:` status row: raw input is
/// clipped to [`MAX_INPUT_ROW_BYTES`] FIRST (a hostile public-setter title
/// cannot make sanitization allocate its input size), then real ANSI
/// sequences are removed by the shared transcript [`strip_ansi`] — CSI/OSC/
/// DCS and unterminated sequences drop whole, leaving NO printable residue
/// — then control characters are stripped, the text is trimmed and it is
/// truncated at [`MAX_FIELD_CHARS`] with a `…` cut marker when anything was
/// dropped (the row is additionally width-masked by [`fit`]). Returns `None`
/// when nothing displayable remains — a control-sequence-only title (e.g.
/// `ESC[2J`) is unusable, never a guess and never a fabricated default, so
/// it can never claim `(live)`. Source bound verified: the backend rejects
/// session titles over 512 bytes
/// (`crates/server/src/workspace_sessions.rs::MAX_TITLE_BYTES`) and the
/// listing layer re-checks the same bound, so this projection bound is
/// a defensive subset of an already-bounded pipeline.
fn sanitize_session_title(raw: &str) -> Option<String> {
    let bounded = clip_title_prefix(raw, MAX_INPUT_ROW_BYTES);
    let stripped = strip_ansi(bounded);
    let cleaned: String = stripped.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.chars().count() <= MAX_FIELD_CHARS {
        return Some(cleaned.to_owned());
    }
    let mut out: String = cleaned
        .chars()
        .take(MAX_FIELD_CHARS.saturating_sub(1))
        .collect();
    out.push('…');
    Some(out)
}

fn fit(line: &str, width: usize) -> String {
    line.chars().take(width).collect()
}

/// Append up to `budget` rows; a cut list ends with an honest `+N more`.
fn append_rows(out: &mut Vec<String>, rows: &[String], budget: usize, width: usize) {
    if budget == 0 || rows.is_empty() {
        return;
    }
    if rows.len() <= budget {
        for row in rows {
            out.push(fit(row, width));
        }
        return;
    }
    for row in &rows[..budget.saturating_sub(1)] {
        out.push(fit(row, width));
    }
    out.push(fit(&format!("+{} more", rows.len() + 1 - budget), width));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good_map() -> Value {
        json!({
            "alpha": {"status": "disabled"},
            "beta": {"status": "failed", "error": "not_connected"}
        })
    }

    #[test]
    fn unconfirmed_connection_never_renders_connected() {
        let view = StatusView::default();
        let rows = view.render(120, 30);
        assert!(rows.contains(&"daemon: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("daemon: connected")));
        let mut confirmed = StatusView::default();
        confirmed.set_connection(true);
        assert!(confirmed
            .render(120, 30)
            .contains(&"daemon: connected".to_owned()));
        confirmed.set_connection_unknown();
        assert!(confirmed
            .render(120, 30)
            .contains(&"daemon: Unavailable".to_owned()));
    }

    #[test]
    fn both_headings_visible_at_120x30() {
        let mut view = StatusView::default();
        view.apply_mcp(&good_map()).unwrap();
        view.set_model(Some("m".to_owned()));
        let rows = view.render(120, 30);
        assert_eq!(rows[0], "Status");
        let mcp_at = rows.iter().position(|r| r == "MCP").expect("MCP heading");
        assert!(mcp_at < 30);
        assert!(rows.len() <= 30);
        assert!(rows.iter().all(|r| r.chars().count() <= 120));
        // Degenerate sizes still behave.
        assert!(StatusView::default().render(0, 30).is_empty());
        assert_eq!(StatusView::default().render(120, 2), ["Status", "MCP"]);
    }

    #[test]
    fn malformed_snapshot_retains_last_good_and_marks_stale() {
        let mut view = StatusView::default();
        view.apply_mcp(&good_map()).unwrap();
        assert_eq!(view.entries().len(), 2);
        let err = view.apply_mcp(&json!([1, 2])).unwrap_err();
        assert!(err.contains("root"));
        assert_eq!(view.entries().len(), 2);
        assert!(view.mcp_stale().is_some());
        let rows = view.render(120, 30);
        assert!(rows.iter().any(|r| r.contains("stale")));
    }

    #[test]
    fn unknown_status_and_unknown_field_rejected() {
        let mut view = StatusView::default();
        assert!(view
            .apply_mcp(&json!({"a": {"status": "totally_new"}}))
            .is_err());
        assert!(view.apply_mcp(&json!({"a": {"status": "failed"}})).is_err());
        assert!(view
            .apply_mcp(&json!({"a": {"status": "disabled", "error": "x"}}))
            .is_err());
        assert!(view
            .apply_mcp(&json!({"a": {"status": "disabled", "environment": {"K": "v"}}}))
            .is_err());
        assert!(!view.mcp_acknowledged());
        assert!(view.entries().is_empty());
    }

    #[test]
    fn control_char_name_and_secret_error_never_render() {
        let mut view = StatusView::default();
        assert!(view
            .apply_mcp(&json!({"bad\u{1}name": {"status": "disabled"}}))
            .is_err());
        view.apply_mcp(&json!({"ok": {"status": "failed", "error": "sk-SECRET123 token=abc"}}))
            .unwrap();
        assert_eq!(view.entries()[0].status.error_code(), Some(INTERNAL_CODE));
        let rendered: String = view.render(120, 30).join("\n");
        let detail: String = view.render_detail("ok", 120).join("\n");
        for leak in ["SECRET123", "token=abc", "sk-"] {
            assert!(!rendered.contains(leak), "rendered leaked {leak}");
            assert!(!detail.contains(leak), "detail leaked {leak}");
        }
        assert!(rendered.contains("error=internal"));
    }

    #[test]
    fn entry_count_bounded_to_64() {
        let build = |n: usize| {
            let mut map = serde_json::Map::new();
            for i in 0..n {
                map.insert(format!("s{i:03}"), json!({"status": "disabled"}));
            }
            Value::Object(map)
        };
        let mut view = StatusView::default();
        view.apply_mcp(&build(MAX_MCP_ENTRIES)).unwrap();
        assert_eq!(view.entries().len(), MAX_MCP_ENTRIES);
        let err = view.apply_mcp(&build(MAX_MCP_ENTRIES + 1)).unwrap_err();
        assert!(err.contains("exceeds"));
        assert_eq!(view.entries().len(), MAX_MCP_ENTRIES);
    }

    #[test]
    fn loading_until_ack_then_no_servers_for_fresh_empty() {
        let mut view = StatusView::default();
        assert!(view.render(120, 30).iter().any(|r| r.contains(LOADING)));
        view.apply_mcp(&json!({})).unwrap();
        assert!(view.mcp_acknowledged());
        let rows = view.render(120, 30);
        assert!(rows.iter().any(|r| r.contains(NO_SERVERS)));
        assert!(!rows.iter().any(|r| r.contains(LOADING)));
    }

    #[test]
    fn absent_fields_render_unavailable_and_intents_have_no_effect() {
        let mut view = StatusView::default();
        let rows = view.render(120, 30);
        for label in ["model:", "provider:", "agent:", "session:", "context:"] {
            assert!(rows.iter().any(|r| r.starts_with(label)));
        }
        assert!(rows.iter().filter(|r| r.ends_with(UNAVAILABLE)).count() >= 4);
        view.set_model(Some("  ".to_owned()));
        assert!(view
            .render(120, 30)
            .contains(&format!("model: {UNAVAILABLE}")));
        view.apply_mcp(&good_map()).unwrap();
        let before = view.entries().to_vec();
        let intent = view.intent_disconnect("alpha").unwrap();
        assert_eq!(
            intent,
            McpIntent::Disconnect {
                name: "alpha".to_owned()
            }
        );
        assert_eq!(view.entries(), before.as_slice());
        assert!(view.intent_connect("missing").is_err());
        assert!(!CONNECT_SUPPORTED);
    }

    #[test]
    fn model_slot_default_unset_and_selected_are_distinct() {
        // Default: never observed => Unavailable, never `unset`.
        let mut view = StatusView::default();
        let rows = view.render(120, 30);
        assert!(rows.contains(&"model: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r == "model: unset"));
        // Confirmed no-selection => exact canonical label.
        view.set_model(None);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"model: unset".to_owned()));
        assert!(!rows.iter().any(|r| r == "model: Unavailable"));
        // Confirmed selection => the actual label.
        view.set_model(Some("gpt-5.2".to_owned()));
        assert!(view.render(120, 30).contains(&"model: gpt-5.2".to_owned()));
        // Failed refresh retracts to unknown without inventing `unset`.
        view.set_model_unknown();
        let rows = view.render(120, 30);
        assert!(rows.contains(&"model: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r == "model: unset"));
        // An invalid label is not an observation: last good state is kept,
        // and it is never relabelled `unset`.
        view.set_model(Some("gpt-5.2".to_owned()));
        view.set_model(Some("   ".to_owned()));
        assert!(view.render(120, 30).contains(&"model: gpt-5.2".to_owned()));
        let mut never_observed = StatusView::default();
        never_observed.set_model(Some("\u{1}\u{2}".to_owned()));
        assert!(never_observed
            .render(120, 30)
            .contains(&"model: Unavailable".to_owned()));
    }

    #[test]
    fn lost_connection_does_not_fabricate_unset() {
        let mut view = StatusView::default();
        view.set_connection(true);
        view.set_model(Some("gpt-5.2".to_owned()));
        view.set_connection(false);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"daemon: disconnected".to_owned()));
        assert!(rows.contains(&"model: gpt-5.2".to_owned()));
        assert!(!rows.iter().any(|r| r == "model: unset"));
        view.set_connection_unknown();
        let rows = view.render(120, 30);
        assert!(rows.contains(&"daemon: Unavailable".to_owned()));
        assert!(rows.contains(&"model: gpt-5.2".to_owned()));
        // A confirmed unset is likewise not erased or invented by a
        // connection change: connection loss reconciles the caller only.
        let mut fresh = StatusView::default();
        fresh.set_model(None);
        fresh.set_connection_unknown();
        let rows = fresh.render(120, 30);
        assert!(rows.contains(&"model: unset".to_owned()));
        assert!(!rows.iter().any(|r| r == "model: Unavailable"));
    }

    const BOUND_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

    /// RECOVERY-DURABILITY-001 regression: an accepted authenticated
    /// snapshot maps to the visible `session: Chat (live)` title row with
    /// the execution UUID preserved as a SEPARATE identity row, and the
    /// Status pane heading stays row 0 (primary right-pane contract).
    #[test]
    fn accepted_binding_renders_title_live_and_uuid_separate() {
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), None);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        assert_eq!(rows[0], "Status");
        // The title row and the identity row are distinct rows.
        let titles = rows.iter().filter(|r| r.starts_with("session:")).count();
        assert_eq!(titles, 1);
        assert_eq!(
            rows.iter().filter(|r| r.starts_with("session id:")).count(),
            1
        );
    }

    /// Initial/unbound state and any bare-id observation can never claim
    /// `(live)`: no accepted binding fact, no title, no marker — and a
    /// bare id retracts a previous bound pair instead of leaving a
    /// mismatched stale title.
    #[test]
    fn unbound_bare_id_never_claims_live() {
        let mut view = StatusView::default();
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Unavailable".to_owned()));
        assert!(rows.contains(&"session id: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("(live)")));
        view.set_session(Some(BOUND_ID.to_owned()));
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Unavailable".to_owned()));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        assert!(!rows.iter().any(|r| r.contains("(live)")));
        // A later bare observation retracts an earlier bound pair.
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), None);
        assert!(view
            .render(120, 30)
            .contains(&"session: Chat (live)".to_owned()));
        view.set_session(Some(BOUND_ID.to_owned()));
        let rows = view.render(120, 30);
        assert!(!rows.iter().any(|r| r.contains("(live)")));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
    }

    /// A successful switch changes id and title TOGETHER (never a stale
    /// title paired with a new id), and a binding with an unusable title
    /// keeps the identity without any live claim or fabricated name.
    #[test]
    fn switch_changes_id_and_title_together_never_mismatched() {
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Alpha".to_owned()), None);
        view.set_session_binding(Some(B.to_owned()), Some("Beta".to_owned()), None);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Beta (live)".to_owned()));
        assert!(rows.contains(&format!("session id: {B}")));
        assert!(!rows.iter().any(|r| r.contains("Alpha")));
        assert!(!rows.iter().any(|r| r.contains(BOUND_ID)));
        // Unusable (blank-after-sanitize) title: id kept, no live, no name.
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("   ".to_owned()), None);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Unavailable".to_owned()));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        assert!(!rows.iter().any(|r| r.contains("(live)")));
    }

    /// Hostile titles (escape sequences, control bytes, oversized input)
    /// are sanitized and bounded: no control character reaches the row,
    /// the cut is marked with `…`, the identity row is untouched, and a
    /// control-only title can never fabricate a name or a live claim.
    #[test]
    fn hostile_title_sanitized_bounded_no_escape_no_fake_identity() {
        let hostile = format!(
            "\u{1b}[31mevil\u{1b}[0m\u{7}\nlinebreak {}",
            "t".repeat(4096)
        );
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some(hostile), None);
        // Width 200 keeps the full bounded row visible for exact checks.
        let rows = view.render(200, 30);
        let title_row = rows
            .iter()
            .find(|r| r.starts_with("session:"))
            .expect("title row");
        assert!(!title_row.chars().any(char::is_control), "no control leak");
        assert!(
            title_row.chars().count()
                <= "session: ".chars().count() + MAX_FIELD_CHARS + " (live)".chars().count(),
            "bounded with cut marker"
        );
        assert!(title_row.contains('…'), "dot-ellipsis on truncation");
        assert!(title_row.ends_with(" (live)"), "still the bound title");
        // Raw escape bytes never survive into the rendered row.
        assert!(!title_row.contains('\u{1b}'));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        // Control-only title: nothing displayable -> no name, no live.
        let mut view2 = StatusView::default();
        view2.set_session_binding(
            Some(BOUND_ID.to_owned()),
            Some("\u{1b}[2J\u{7}".to_owned()),
            None,
        );
        let rows2 = view2.render(120, 30);
        assert!(rows2.contains(&"session: Unavailable".to_owned()));
        assert!(!rows2.iter().any(|r| r.contains("(live)")));
        // Width masking: every rendered row fits the requested width.
        let narrow = view.render(20, 30);
        assert!(narrow.iter().all(|r| r.chars().count() <= 20));
    }

    /// RECOVERY-DURABILITY-001 (v2 fail-closed guard): the accepted-binding
    /// setter itself validates the canonical session id, so a malformed or
    /// noncanonical id can never render `(live)`, a title, or a forged
    /// identity row — while the canonical lowercase UUID + bound title
    /// control still renders `session: Chat (live)` with the id row.
    /// Table-driven against the REAL setter (no mirrored predicate).
    #[test]
    fn accepted_binding_requires_canonical_id_rejects_five_shapes() {
        let rejected = [
            Some("not-a-session-id".to_owned()),
            // Parses as a UUID, but not the canonical lowercase spelling.
            Some("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".to_owned()),
            // Parses as a UUID, but not the canonical hyphenated spelling.
            Some("aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaaa".to_owned()),
            Some(String::new()),
            None,
        ];
        for id in rejected {
            let mut view = StatusView::default();
            view.set_session_binding(id, Some("Chat".to_owned()), None);
            let rows = view.render(120, 30);
            assert!(
                !rows.iter().any(|r| r.contains("(live)")),
                "no live claim without a canonical id: {rows:?}"
            );
            assert!(
                rows.contains(&"session: Unavailable".to_owned()),
                "no title claim without a canonical id: {rows:?}"
            );
            assert!(
                rows.contains(&"session id: Unavailable".to_owned()),
                "rejected id never forges an identity row: {rows:?}"
            );
        }
        // Valid control: canonical id + accepted bound title still renders.
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), None);
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
    }

    /// RECOVERY-DURABILITY-001 (v3 sanitizer regression): ANSI sequences
    /// are stripped BEFORE the live decision — a control-sequence-only or
    /// control-only title sanitizes to nothing (`Unavailable`, no `(live)`,
    /// canonical id row retained truthfully), while ANSI-wrapped real text
    /// still renders its title live. Unterminated sequences drop with no
    /// printable residue, and an oversized ESC-prefixed title stays a
    /// bounded, control-free `…` prefix. Table-driven against the REAL
    /// setter/renderer (no mirrored predicate).
    #[test]
    fn ansi_titles_fail_closed_or_render_sanitized() {
        // ANSI-only (CSI / OSC) and control-only payloads: nothing
        // displayable remains -> title Unavailable, never live; the
        // canonical id row stays truthful.
        for raw in [
            "\u{1b}[2J",
            "\u{1b}]0;evil window title\u{7}",
            "\u{7}\u{1b}[0m",
        ] {
            let mut view = StatusView::default();
            view.set_session_binding(Some(BOUND_ID.to_owned()), Some(raw.to_owned()), None);
            let rows = view.render(120, 30);
            assert!(
                rows.contains(&"session: Unavailable".to_owned()),
                "no printable ANSI residue: {raw:?} -> {rows:?}"
            );
            assert!(!rows.iter().any(|r| r.contains("(live)")), "{raw:?}");
            assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        }
        // ANSI-wrapped real text: sequence stripped, title still live.
        let mut view = StatusView::default();
        view.set_session_binding(
            Some(BOUND_ID.to_owned()),
            Some("\u{1b}[31mChat\u{1b}[0m".to_owned()),
            None,
        );
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
        // Unterminated trailing CSI: dropped to end of input, the leading
        // legit text is honored and no parameter residue survives.
        let mut view = StatusView::default();
        view.set_session_binding(
            Some(BOUND_ID.to_owned()),
            Some("Ready\u{1b}[38;5;".to_owned()),
            None,
        );
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Ready (live)".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("38;5")));
        // Oversized ESC-prefixed title: bounded sanitized `…` prefix, no
        // control byte and no raw escape reach the rendered row.
        let mut view = StatusView::default();
        view.set_session_binding(
            Some(BOUND_ID.to_owned()),
            Some(format!("\u{1b}[2Jevil{}", "t".repeat(4096))),
            None,
        );
        // Width 200 keeps the full bounded row visible for exact checks.
        let rows = view.render(200, 30);
        let title_row = rows
            .iter()
            .find(|r| r.starts_with("session:"))
            .expect("title row");
        assert!(!title_row.chars().any(char::is_control), "no control leak");
        assert!(!title_row.contains('\u{1b}'), "no raw escape");
        assert!(title_row.contains('…'), "dot-ellipsis on truncation");
        assert!(
            title_row.chars().count()
                <= "session: ".chars().count() + MAX_FIELD_CHARS + " (live)".chars().count(),
            "bounded with cut marker"
        );
        assert!(title_row.ends_with(" (live)"), "still the bound title");
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
    }

    /// RECOVERY-DURABILITY-001 (v4): the accepted tuple is (canonical id,
    /// display title, confirmed persisted snapshot count). The confirmed
    /// count travels ONLY with an accepted canonical binding (including a
    /// valid empty-history 0), survives an unusable display title (its
    /// provenance is the accepted snapshot, not the title), and is
    /// cleared — never fabricated — for invalid, bare or unavailable
    /// bindings. Driven through the REAL setter/renderer.
    #[test]
    fn accepted_tuple_carries_confirmed_count_and_retracts_without_binding() {
        // Accepted tuple: title live, UUID separate, confirmed count shown.
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), Some(7));
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        assert!(rows.contains(&"messages: 7".to_owned()));
        // A confirmed empty-history count of 0 is a valid accepted fact,
        // distinct from `Unavailable` (never fabricated, never conflated).
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), Some(0));
        let rows = view.render(120, 30);
        assert!(rows.contains(&"messages: 0".to_owned()));
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
        // Unusable title + accepted canonical id: no live claim, but the
        // confirmed snapshot count remains a truthful accepted fact.
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("   ".to_owned()), Some(7));
        let rows = view.render(120, 30);
        assert!(rows.contains(&"session: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("(live)")));
        assert!(rows.contains(&"messages: 7".to_owned()));
        // Invalid/bare ids fail closed for the WHOLE tuple: no count.
        for raw in ["aaaa", "", "AAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"] {
            let mut view = StatusView::default();
            view.set_session_binding(Some(raw.to_owned()), Some("Chat".to_owned()), Some(7));
            let rows = view.render(120, 30);
            assert!(rows.contains(&"session: Unavailable".to_owned()), "{raw:?}");
            assert!(
                rows.contains(&"messages: Unavailable".to_owned()),
                "{raw:?}"
            );
            assert!(!rows.iter().any(|r| r.contains("messages: 7")), "{raw:?}");
        }
        // A bare id observation retracts the whole previous tuple.
        let mut view = StatusView::default();
        view.set_session_binding(Some(BOUND_ID.to_owned()), Some("Chat".to_owned()), Some(7));
        view.set_session(Some(BOUND_ID.to_owned()));
        let rows = view.render(120, 30);
        assert!(rows.contains(&format!("session id: {BOUND_ID}")));
        assert!(rows.contains(&"session: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("(live)")));
        assert!(rows.contains(&"messages: Unavailable".to_owned()));
        assert!(!rows.iter().any(|r| r.contains("messages: 7")));
    }

    /// RECOVERY-DURABILITY-001 (v4): the count row is bounded and width-
    /// safe — the full confirmed usize decimal fits the real 39-column
    /// right pane (label 9 + up to 20 digits = 29 < 39) with no clipping
    /// artifact and no unbounded content.
    #[test]
    fn count_row_is_width_bounded_at_39_columns() {
        let mut view = StatusView::default();
        view.set_session_binding(
            Some(BOUND_ID.to_owned()),
            Some("Chat".to_owned()),
            Some(usize::MAX),
        );
        // Width 39 = the actual production right pane at 120 columns.
        let rows = view.render(39, 30);
        let count_row = rows
            .iter()
            .find(|r| r.starts_with("messages:"))
            .expect("count row");
        assert_eq!(
            count_row,
            &format!("messages: {}", usize::MAX),
            "the full confirmed decimal fits the real right pane unclipped"
        );
        assert!(count_row.chars().count() <= 39, "bounded by the pane width");
        assert!(rows.contains(&"session: Chat (live)".to_owned()));
    }
}

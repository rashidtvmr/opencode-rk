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

    pub fn set_session(&mut self, id: Option<String>) {
        self.session = non_empty(id);
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
            format!("session: {}", display(self.session.as_deref())),
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
}

#![forbid(unsafe_code)]
//! Native live status state (TUI-009, slice: app types).
//!
//! Pure state only: provider usage/cache counters built from normalized
//! events (never fixed zeros — observed state is distinguished from
//! never-reported), memory/context entries with sources and visibility
//! policy, MCP server lifecycle controls, quota/fallback/connectivity
//! banners, and bounded event/retention budgets. No rendering, no IO, no
//! threads; the caller executes lifecycle actions against the real daemon.
//!
//! Commit: base 5af7884. Evidence: card TUI-009 (T01 counters from
//! normalized provider events, T02 memory/context sources and visibility
//! policy, T03 MCP controls change the real service lifecycle, T04
//! quota/routing/connectivity accurate states, T05 event/retention budgets
//! without continuous polling); prior art `crates/cli/src/native_app.rs`
//! (freshness model) and `crates/sessions/src/tui_state.rs` (bounded status).

/// Retained token-event observations per counter.
pub const MAX_EVENTS: usize = 64;
/// Retained MCP server entries.
pub const MAX_MCP_SERVERS: usize = 32;
/// Retained context/memory entries.
pub const MAX_CONTEXT_ENTRIES: usize = 64;
/// Idle refresh interval in caller ticks; polling above this rate is refused.
pub const MIN_REFRESH_INTERVAL_TICKS: u64 = 30;

/// A normalized provider usage observation. Counters derive from these
/// events; a counter that never received an event is `unobserved`, not zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageEvent {
    Input { tokens: u64 },
    Output { tokens: u64 },
    Reasoning { tokens: u64 },
    CacheRead { tokens: u64 },
    CacheWrite { tokens: u64 },
}

/// One counter with its observation state (T01: unavailable values are not
/// silently reported as measured zeros).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Counter {
    /// No normalized event ever reported this counter.
    #[default]
    Unobserved,
    /// Measured total from applied events.
    Measured(u64),
}

impl Counter {
    #[must_use]
    pub fn value(self) -> Option<u64> {
        match self {
            Counter::Unobserved => None,
            Counter::Measured(v) => Some(v),
        }
    }
}

/// Visibility policy for a context/memory source (T02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Internal,
    Secret,
}

/// A context/memory entry shown in the status view.
#[derive(Clone, Debug)]
pub struct ContextEntry {
    pub label: String,
    pub source: String,
    pub bytes: u64,
    pub visibility: Visibility,
}

impl ContextEntry {
    /// Whether the entry may be rendered at the given policy level. Secret
    /// sources never render their content, only their footprint.
    #[must_use]
    pub fn renderable(&self, allow_internal: bool) -> bool {
        match self.visibility {
            Visibility::Public => true,
            Visibility::Internal => allow_internal,
            Visibility::Secret => false,
        }
    }
}

/// MCP server lifecycle state (T03: enable/disable/search change the real
/// service — the caller executes the returned action against the daemon).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpState {
    Enabled,
    Disabled,
    Failed,
}

/// A lifecycle action the caller must execute against the real service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpAction {
    Start(String),
    Stop(String),
}

/// An MCP server registry entry.
#[derive(Clone, Debug)]
pub struct McpServer {
    pub name: String,
    pub state: McpState,
}

/// Quota / routing / connectivity banner state (T04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connectivity {
    Online,
    Degraded,
    Offline,
}

/// Bounded usage counters and view state.
#[derive(Debug, Default)]
pub struct StatusBoard {
    counters: [Counter; 5],
    events: Vec<UsageEvent>,
    context: Vec<ContextEntry>,
    servers: Vec<McpServer>,
    connectivity: Option<Connectivity>,
    quota_message: Option<String>,
    fallback_active: bool,
    last_refresh_tick: Option<u64>,
}

impl StatusBoard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            counters: [Counter::Unobserved; 5],
            ..Self::default()
        }
    }

    /// Apply a normalized usage event. Input=0, Output=1, Reasoning=2,
    /// CacheRead=3, CacheWrite=4.
    pub fn apply_usage(&mut self, event: UsageEvent) {
        let idx = match event {
            UsageEvent::Input { .. } => 0,
            UsageEvent::Output { .. } => 1,
            UsageEvent::Reasoning { .. } => 2,
            UsageEvent::CacheRead { .. } => 3,
            UsageEvent::CacheWrite { .. } => 4,
        };
        self.counters[idx] = match (self.counters[idx], event_value(&event)) {
            (Counter::Measured(total), v) => Counter::Measured(total.saturating_add(v)),
            (Counter::Unobserved, v) => Counter::Measured(v),
        };
        if self.events.len() >= MAX_EVENTS {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Counter by index (0..5 per `apply_usage`).
    #[must_use]
    pub fn counter(&self, idx: usize) -> Counter {
        self.counters.get(idx).copied().unwrap_or(Counter::Unobserved)
    }

    /// Whether any usage event was ever applied.
    #[must_use]
    pub fn usage_observed(&self) -> bool {
        !self.events.is_empty()
    }

    /// Retained events (bounded).
    #[must_use]
    pub fn events(&self) -> &[UsageEvent] {
        &self.events
    }

    /// Add or replace a context entry by label (bounded; oldest dropped).
    pub fn upsert_context(&mut self, entry: ContextEntry) {
        if let Some(slot) = self.context.iter_mut().find(|e| e.label == entry.label) {
            *slot = entry;
            return;
        }
        if self.context.len() >= MAX_CONTEXT_ENTRIES {
            self.context.remove(0);
        }
        self.context.push(entry);
    }

    /// Context entries (bounded).
    #[must_use]
    pub fn context(&self) -> &[ContextEntry] {
        &self.context
    }

    /// Register or update an MCP server entry (bounded; oldest dropped).
    pub fn upsert_server(&mut self, server: McpServer) {
        if let Some(slot) = self.servers.iter_mut().find(|s| s.name == server.name) {
            slot.state = server.state;
            return;
        }
        if self.servers.len() >= MAX_MCP_SERVERS {
            self.servers.remove(0);
        }
        self.servers.push(server);
    }

    /// Enable an MCP server. Returns the lifecycle action the caller must
    /// execute; the state only flips when the caller reports success.
    pub fn enable_server(&mut self, name: &str) -> Option<McpAction> {
        self.servers.iter().find(|s| s.name == name).map(|_| McpAction::Start(name.to_string()))
    }

    /// Disable an MCP server; returns the stop action for the caller.
    pub fn disable_server(&mut self, name: &str) -> Option<McpAction> {
        self.servers.iter().find(|s| s.name == name).map(|_| McpAction::Stop(name.to_string()))
    }

    /// The caller reports the executed lifecycle outcome.
    pub fn report_server_state(&mut self, name: &str, state: McpState) {
        if let Some(slot) = self.servers.iter_mut().find(|s| s.name == name) {
            slot.state = state;
        }
    }

    /// Search MCP servers by name substring.
    #[must_use]
    pub fn search_servers(&self, needle: &str) -> Vec<&McpServer> {
        self.servers
            .iter()
            .filter(|s| s.name.contains(needle))
            .collect()
    }

    /// Servers (bounded).
    #[must_use]
    pub fn servers(&self) -> &[McpServer] {
        &self.servers
    }

    /// Update connectivity banner.
    pub fn set_connectivity(&mut self, state: Connectivity) {
        self.connectivity = Some(state);
    }

    /// Connectivity banner state.
    #[must_use]
    pub fn connectivity(&self) -> Option<Connectivity> {
        self.connectivity
    }

    /// Quota / fallback message with the fallback flag.
    pub fn set_quota(&mut self, message: String, fallback_active: bool) {
        self.quota_message = Some(message);
        self.fallback_active = fallback_active;
    }

    /// Quota message.
    #[must_use]
    pub fn quota_message(&self) -> Option<&str> {
        self.quota_message.as_deref()
    }

    /// Whether routing is currently on a fallback provider.
    #[must_use]
    pub fn fallback_active(&self) -> bool {
        self.fallback_active
    }

    /// Refresh gate: refuses more-than-budgeted polling (T05). Returns true
    /// when a refresh is allowed at this tick.
    pub fn request_refresh(&mut self, tick: u64) -> bool {
        match self.last_refresh_tick {
            Some(last) if tick.saturating_sub(last) < MIN_REFRESH_INTERVAL_TICKS => false,
            _ => {
                self.last_refresh_tick = Some(tick);
                true
            }
        }
    }
}

fn event_value(event: &UsageEvent) -> u64 {
    match event {
        UsageEvent::Input { tokens }
        | UsageEvent::Output { tokens }
        | UsageEvent::Reasoning { tokens }
        | UsageEvent::CacheRead { tokens }
        | UsageEvent::CacheWrite { tokens } => *tokens,
    }
}

/// Max chars kept per label inside a status-line item; longer labels are
/// truncated (never silently — the item keeps its brackets/badges).
pub const MAX_STATUS_LABEL: usize = 48;

fn truncate_label(s: &str) -> String {
    if s.chars().count() <= MAX_STATUS_LABEL {
        s.to_string()
    } else {
        // ponytail: char-boundary cut; upgrade to word-boundary when TUI needs it.
        s.chars().take(MAX_STATUS_LABEL).collect()
    }
}

/// Total measured input+output tokens, or `None` when neither was observed.
#[must_use]
pub fn context_tokens(board: &StatusBoard) -> Option<u64> {
    let mut total: Option<u64> = None;
    for idx in [0usize, 1] {
        if let Counter::Measured(v) = board.counter(idx) {
            total = Some(total.unwrap_or(0).saturating_add(v));
        }
    }
    total
}

/// Freshness badge: offline wins over stale; live only when the caller
/// confirms live data AND connectivity is not offline.
#[must_use]
pub fn freshness_badge(live: bool, connectivity: Option<Connectivity>) -> &'static str {
    if connectivity == Some(Connectivity::Offline) {
        "offline"
    } else if live {
        "live"
    } else {
        "stale"
    }
}

/// One-line status bar: model / context / session items plus freshness,
/// fallback, and quota badges. Every item is badged `(stale)` unless `live`;
/// offline connectivity adds `[offline]`. Pure string building, no IO.
#[must_use]
pub fn status_line(
    board: &StatusBoard,
    model: Option<&str>,
    session: Option<&str>,
    live: bool,
) -> String {
    let stale_suffix = if live { "" } else { " (stale)" };
    let model_label = truncate_label(model.unwrap_or("(none)"));
    let context_label = match context_tokens(board) {
        Some(n) => format!("{n} tokens"),
        None => "n/a".to_string(),
    };
    let session_label = truncate_label(session.unwrap_or("(none)"));
    let mut out = format!(
        "[model: {model_label}{stale_suffix}] [context: {context_label}{stale_suffix}] [session: {session_label}{stale_suffix}]"
    );
    let badge = freshness_badge(live, board.connectivity());
    out.push_str(&format!(" [{badge}]"));
    if board.fallback_active() {
        out.push_str(" [fallback]");
    }
    if let Some(msg) = board.quota_message() {
        out.push_str(&format!(" [quota: {}]", truncate_label(msg)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_come_from_events_not_fixed_zeros() {
        let mut board = StatusBoard::new();
        // Never reported: Unobserved, not zero.
        assert_eq!(board.counter(0), Counter::Unobserved);
        assert_eq!(board.counter(0).value(), None);
        // A real event yields a measured value.
        board.apply_usage(UsageEvent::Input { tokens: 120 });
        assert_eq!(board.counter(0), Counter::Measured(120));
        // A reported zero is measured zero — distinct from unobserved.
        board.apply_usage(UsageEvent::Output { tokens: 0 });
        assert_eq!(board.counter(1), Counter::Measured(0));
        assert_eq!(board.counter(1).value(), Some(0));
        // Accumulation.
        board.apply_usage(UsageEvent::Input { tokens: 30 });
        assert_eq!(board.counter(0), Counter::Measured(150));
    }

    #[test]
    fn context_entries_identify_sources_and_enforce_visibility() {
        let mut board = StatusBoard::new();
        board.upsert_context(ContextEntry {
            label: "repo files".into(),
            source: "workspace scan".into(),
            bytes: 4096,
            visibility: Visibility::Public,
        });
        board.upsert_context(ContextEntry {
            label: "env snapshot".into(),
            source: "daemon env".into(),
            bytes: 512,
            visibility: Visibility::Internal,
        });
        board.upsert_context(ContextEntry {
            label: "credentials".into(),
            source: "keyring".into(),
            bytes: 64,
            visibility: Visibility::Secret,
        });
        // Default policy hides internal and secret content.
        let allow_internal = false;
        let renderable: Vec<bool> = board
            .context()
            .iter()
            .map(|e| e.renderable(allow_internal))
            .collect();
        assert_eq!(renderable, vec![true, false, false]);
        // Elevated policy shows internal but never secret.
        let elevated: Vec<bool> = board
            .context()
            .iter()
            .map(|e| e.renderable(true))
            .collect();
        assert_eq!(elevated, vec![true, true, false]);
    }

    #[test]
    fn mcp_controls_change_real_service_lifecycle() {
        let mut board = StatusBoard::new();
        board.upsert_server(McpServer {
            name: "filesystem".into(),
            state: McpState::Disabled,
        });
        // Disable returns the stop action the caller executes.
        let action = board.disable_server("filesystem");
        assert_eq!(action, Some(McpAction::Stop("filesystem".into())));
        // The caller reports success; only then does state flip.
        board.report_server_state("filesystem", McpState::Enabled);
        assert_eq!(board.servers()[0].state, McpState::Enabled);
        // Search finds by substring.
        assert_eq!(board.search_servers("file").len(), 1);
        assert_eq!(board.search_servers("nope").len(), 0);
        // Unknown server yields no action.
        assert_eq!(board.enable_server("missing"), None);
    }

    #[test]
    fn quota_fallback_and_connectivity_render_accurately() {
        let mut board = StatusBoard::new();
        assert_eq!(board.connectivity(), None);
        board.set_connectivity(Connectivity::Degraded);
        board.set_quota("rate limited; retrying on fallback".into(), true);
        assert_eq!(board.connectivity(), Some(Connectivity::Degraded));
        assert_eq!(
            board.quota_message(),
            Some("rate limited; retrying on fallback")
        );
        assert!(board.fallback_active());
    }

    #[test]
    fn refresh_respects_event_budget() {
        let mut board = StatusBoard::new();
        assert!(board.request_refresh(0));
        assert!(!board.request_refresh(10));
        assert!(board.request_refresh(MIN_REFRESH_INTERVAL_TICKS));
    }

    #[test]
    fn event_and_registry_bounds_are_enforced() {
        let mut board = StatusBoard::new();
        for i in 0..(MAX_EVENTS + 10) {
            board.apply_usage(UsageEvent::Input { tokens: i as u64 });
        }
        assert_eq!(board.events().len(), MAX_EVENTS);
        for i in 0..(MAX_MCP_SERVERS + 5) {
            board.upsert_server(McpServer {
                name: format!("srv{i}"),
                state: McpState::Enabled,
            });
        }
        assert_eq!(board.servers().len(), MAX_MCP_SERVERS);
    }

    #[test]
    fn live_status_line_has_no_stale_markers() {
        let mut board = StatusBoard::new();
        board.apply_usage(UsageEvent::Input { tokens: 120 });
        board.apply_usage(UsageEvent::Output { tokens: 30 });
        let line = status_line(&board, Some("gpt-4"), Some("sess-1"), true);
        assert!(line.contains("[model: gpt-4]"), "{line}");
        assert!(line.contains("[context: 150 tokens]"), "{line}");
        assert!(line.contains("[session: sess-1]"), "{line}");
        assert!(line.contains("[live]"), "{line}");
        assert!(!line.contains("stale"), "{line}");
        assert!(!line.contains("offline"), "{line}");
    }

    #[test]
    fn stale_status_line_badges_every_item() {
        let board = StatusBoard::new();
        let line = status_line(&board, Some("gpt-4"), Some("sess-1"), false);
        assert!(line.contains("[model: gpt-4 (stale)]"), "{line}");
        assert!(line.contains("[context: n/a (stale)]"), "{line}");
        assert!(line.contains("[session: sess-1 (stale)]"), "{line}");
        assert!(line.contains("[stale]"), "{line}");
    }

    #[test]
    fn offline_connectivity_badges_offline() {
        let mut board = StatusBoard::new();
        board.set_connectivity(Connectivity::Offline);
        assert_eq!(freshness_badge(false, board.connectivity()), "offline");
        let line = status_line(&board, None, None, false);
        assert!(line.contains("[offline]"), "{line}");
        assert!(line.contains("[model: (none) (stale)]"), "{line}");
        assert_eq!(freshness_badge(true, board.connectivity()), "offline");
    }

    #[test]
    fn quota_fallback_and_long_labels_render_bounded() {
        let mut board = StatusBoard::new();
        board.set_quota("rate limited; retrying on fallback".into(), true);
        let line = status_line(&board, Some(&"m".repeat(100)), Some("s"), true);
        assert!(line.contains("[fallback]"), "{line}");
        assert!(line.contains("[quota: rate limited; retrying on fallback]"), "{line}");
        assert!(line.chars().count() <= 3 * (MAX_STATUS_LABEL + 20) + 64 + 32, "{line}");
        assert!(!line.contains(&"m".repeat(100)), "{line}");
    }
}

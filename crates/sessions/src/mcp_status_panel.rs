//! Bounded MCP status projection for the session information panel.
//!
//! The caller owns the live MCP state and supplies timestamps.  This module
//! only creates a capped, redacted projection, renders it, or performs one
//! non-blocking send through a caller-owned status bus.  It does not start a
//! service, read a clock, touch the network, or retain caller input.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::fmt;

/// Maximum number of tool names included in one panel/event projection.
pub const MAX_STATUS_TOOLS: usize = 16;
/// Alias used by callers that name the visible tool cap directly.
pub const MAX_TOOLS_SHOWN: usize = MAX_STATUS_TOOLS;
/// Maximum error-label length, measured in characters.
pub const MAX_ERROR_LABEL_CHARS: usize = 256;
/// Alias for the bounded diagnostic-label length.
pub const MAX_ERROR_CHARS: usize = MAX_ERROR_LABEL_CHARS;
/// Maximum latency shown or published by this projection.
pub const MAX_LATENCY_MS: u64 = 3_600_000;
/// Alias for the latency ceiling.
pub const MAX_LATENCY: u64 = MAX_LATENCY_MS;
/// Minimum panel width accepted by [`render`].
pub const MIN_PANEL_WIDTH: usize = 20;
/// Default upper bound for the in-memory fixture bus.
pub const MAX_BUS_EVENTS: usize = 64;

const MAX_TOOL_NAME_CHARS: usize = 128;
const TRUNCATION_MARKER: &str = "...[truncated]";
const REDACTED_LABEL: &str = "[redacted]";

/// Caller-supplied MCP state.  Strings are identities and safe diagnostic
/// labels by contract; projection functions still sanitize them at the trust
/// boundary before rendering or publishing.
#[derive(Clone, Eq, PartialEq)]
pub struct McpStatus {
    pub connected: bool,
    pub enabled_count: u64,
    pub disabled_count: u64,
    pub tools: Vec<String>,
    pub last_error: Option<String>,
    pub latency_ms: Option<u64>,
    pub updated_ms: u64,
}

impl fmt::Debug for McpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpStatus")
            .field("connected", &self.connected)
            .field("enabled_count", &self.enabled_count)
            .field("disabled_count", &self.disabled_count)
            .field("tools", &safe_tools(&self.tools).0)
            .field("last_error", &safe_error(self.last_error.as_deref()))
            .field("latency_ms", &bounded_latency(self.latency_ms))
            .field("updated_ms", &self.updated_ms)
            .finish()
    }
}

impl McpStatus {
    /// Construct a status from caller-owned values.
    #[must_use]
    pub fn new<I, S>(
        connected: bool,
        enabled_count: u64,
        disabled_count: u64,
        tools: I,
        last_error: Option<String>,
        latency_ms: Option<u64>,
        updated_ms: u64,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            connected,
            enabled_count,
            disabled_count,
            tools: tools.into_iter().map(Into::into).collect(),
            last_error,
            latency_ms,
            updated_ms,
        }
    }

    #[must_use]
    pub fn summarize(&self) -> McpCounts {
        summarize(self)
    }

    pub fn render(&self, width: usize) -> Result<Vec<String>, McpPanelError> {
        render(self, width)
    }

    pub fn publish<B: McpStatusBus>(&self, bus: &B) -> Result<(), PublishOutcome> {
        publish(bus, self)
    }
}

/// Bounded count projection used by the panel and status event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpCounts {
    pub connected: bool,
    pub enabled: u64,
    pub disabled: u64,
    pub tools_shown: u64,
    pub truncated: bool,
}

/// Safe event payload.  It contains counts, bounded names, codes, latency,
/// and a caller-supplied timestamp only.  It contains no credentials or raw
/// MCP diagnostics.
#[derive(Clone, Eq, PartialEq)]
pub struct McpStatusEvent {
    pub connected: bool,
    pub enabled_count: u64,
    pub disabled_count: u64,
    pub tools: Vec<String>,
    pub last_error: Option<String>,
    pub latency_ms: Option<u64>,
    pub updated_ms: u64,
    pub truncated: bool,
}

impl fmt::Debug for McpStatusEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpStatusEvent")
            .field("connected", &self.connected)
            .field("enabled_count", &self.enabled_count)
            .field("disabled_count", &self.disabled_count)
            .field("tools", &safe_tools(&self.tools).0)
            .field("last_error", &safe_error(self.last_error.as_deref()))
            .field("latency_ms", &bounded_latency(self.latency_ms))
            .field("updated_ms", &self.updated_ms)
            .field("truncated", &self.truncated)
            .finish()
    }
}

/// Result of a non-blocking status send.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    /// The event was accepted by the bus.
    Sent,
    /// The bus could not accept the event without blocking.
    Skipped,
}

/// Closed/full outcomes from an external event bus are represented by the
/// same non-blocking result; this alias makes adapter code self-documenting.
pub type McpPublishOutcome = PublishOutcome;

/// Panel projection failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpPanelError {
    TooNarrow,
}

impl fmt::Display for McpPanelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooNarrow => f.write_str("MCP status panel is too narrow"),
        }
    }
}

impl std::error::Error for McpPanelError {}

/// Minimal bus contract.  Implementations must perform a try-send only and
/// return [`PublishOutcome::Skipped`] when accepting the event would block.
pub trait McpStatusBus {
    /// Preferred adapter spelling.  The default forwards to `try_send`, so a
    /// caller implementing either spelling can remain a try-send-only adapter.
    fn try_publish(&self, event: McpStatusEvent) -> PublishOutcome {
        self.try_send(event)
    }

    /// Compatibility spelling for bounded channel adapters.
    fn try_send(&self, _event: McpStatusEvent) -> PublishOutcome {
        PublishOutcome::Skipped
    }
}

/// Alias matching the generic status-panel terminology used by callers.
pub use McpStatusBus as StatusBus;

/// Small in-memory bounded bus useful for adapters and disposable tests.
/// `RefCell` keeps the bus synchronous and caller-owned; no thread or task is
/// created.  Production adapters can implement [`McpStatusBus`] directly.
pub struct BoundedStatusBus {
    capacity: usize,
    events: RefCell<Vec<McpStatusEvent>>,
}

impl fmt::Debug for BoundedStatusBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundedStatusBus")
            .field("capacity", &self.capacity)
            .field("len", &self.events.borrow().len())
            .finish()
    }
}

impl BoundedStatusBus {
    /// Create a bus with a bounded event capacity.  Zero is clamped to one.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.clamp(1, MAX_BUS_EVENTS);
        Self {
            capacity,
            events: RefCell::new(Vec::with_capacity(capacity)),
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.borrow().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.borrow().is_empty()
    }

    /// Return a bounded copy for a caller-owned test or adapter.
    #[must_use]
    pub fn events(&self) -> Vec<McpStatusEvent> {
        self.events.borrow().clone()
    }

    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }
}

impl Default for BoundedStatusBus {
    fn default() -> Self {
        Self::new(MAX_BUS_EVENTS)
    }
}

impl McpStatusBus for BoundedStatusBus {
    fn try_send(&self, event: McpStatusEvent) -> PublishOutcome {
        let mut events = self.events.borrow_mut();
        if events.len() >= self.capacity {
            return PublishOutcome::Skipped;
        }
        events.push(event);
        PublishOutcome::Sent
    }
}

/// Compatibility aliases for callers that name the local fixture bus
/// explicitly.
pub type McpEventBus = BoundedStatusBus;
pub type FakeStatusBus = BoundedStatusBus;
pub type StatusEvent = McpStatusEvent;

/// Summarize a caller-supplied status without retaining its raw strings.
#[must_use]
pub fn summarize(status: &McpStatus) -> McpCounts {
    let (tools, tools_truncated) = safe_tools(&status.tools);
    McpCounts {
        connected: status.connected,
        enabled: status.enabled_count,
        disabled: status.disabled_count,
        tools_shown: tools.len() as u64,
        truncated: tools_truncated
            || status
                .last_error
                .as_deref()
                .is_some_and(|error| error.chars().count() > MAX_ERROR_LABEL_CHARS),
    }
}

/// Build one safe event projection.
#[must_use]
pub fn event(status: &McpStatus) -> McpStatusEvent {
    let (tools, tools_truncated) = safe_tools(&status.tools);
    let error = safe_error(status.last_error.as_deref());
    let error_truncated = status
        .last_error
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_ERROR_LABEL_CHARS);
    McpStatusEvent {
        connected: status.connected,
        enabled_count: status.enabled_count,
        disabled_count: status.disabled_count,
        tools,
        last_error: error,
        latency_ms: bounded_latency(status.latency_ms),
        updated_ms: status.updated_ms,
        truncated: tools_truncated || error_truncated,
    }
}

/// Alias for adapters that call the wire projection a snapshot.
#[must_use]
pub fn snapshot(status: &McpStatus) -> McpStatusEvent {
    event(status)
}

/// Publish one event without waiting or retrying.
///
/// `Ok(())` means `Sent`; `Err(PublishOutcome::Skipped)` is the intentional
/// full/closed-bus outcome and leaves the caller's turn unblocked.
pub fn publish<B: McpStatusBus>(bus: &B, status: &McpStatus) -> Result<(), PublishOutcome> {
    match bus.try_publish(event(status)) {
        PublishOutcome::Sent => Ok(()),
        PublishOutcome::Skipped => Err(PublishOutcome::Skipped),
    }
}

/// Render a deterministic, non-stale panel fragment.
pub fn render(status: &McpStatus, width: usize) -> Result<Vec<String>, McpPanelError> {
    render_inner(status, width, false)
}

/// Compact-only fallback for callers handling [`McpPanelError::TooNarrow`].
/// The returned line is still width-bounded for every width of at least one.
#[must_use]
pub fn render_counts(status: &McpStatus, width: usize) -> String {
    let projected = summarize(status);
    let state = if projected.connected {
        "connected"
    } else {
        "disconnected"
    };
    fit_line(
        &format!(
            "MCP: {} enabled={} disabled={} tools={}",
            state, projected.enabled, projected.disabled, projected.tools_shown
        ),
        width.max(1),
    )
}

/// Render with an explicit caller-supplied freshness horizon.  No clock is
/// read here.  A status is stale when `now_ms - updated_ms >= horizon_ms`.
pub fn render_at(
    status: &McpStatus,
    width: usize,
    now_ms: u64,
    horizon_ms: u64,
) -> Result<Vec<String>, McpPanelError> {
    let stale = now_ms.saturating_sub(status.updated_ms) >= horizon_ms;
    render_inner(status, width, stale)
}

/// Alias for callers that prefer the freshness terminology.
pub fn render_with_horizon(
    status: &McpStatus,
    width: usize,
    now_ms: u64,
    horizon_ms: u64,
) -> Result<Vec<String>, McpPanelError> {
    render_at(status, width, now_ms, horizon_ms)
}

/// Render using the caller's explicit current time and freshness horizon.
/// This spelling is retained for status-panel integrations that use `now`.
pub fn render_at_time(
    status: &McpStatus,
    width: usize,
    now: u64,
    stale_after_ms: u64,
) -> Result<Vec<String>, McpPanelError> {
    render_at(status, width, now, stale_after_ms)
}

fn render_inner(
    status: &McpStatus,
    width: usize,
    stale: bool,
) -> Result<Vec<String>, McpPanelError> {
    if width < MIN_PANEL_WIDTH {
        return Err(McpPanelError::TooNarrow);
    }

    let projected = event(status);
    let (tools, tools_truncated) = safe_tools(&status.tools);
    let state = if projected.connected {
        "connected"
    } else {
        "disconnected"
    };
    let stale_marker = if stale { " stale" } else { "" };
    let mut lines = if width < 60 {
        vec![
            format!(
                "MCP{} c={} e={} d={} t={}",
                stale_marker,
                state,
                projected.enabled_count,
                projected.disabled_count,
                projected.tools.len()
            ),
            format_tools("tools", &tools, tools_truncated),
            projected
                .last_error
                .as_deref()
                .map_or_else(|| "err: none".to_owned(), |error| format!("err: {error}")),
            format!(
                "lat: {} upd: {}",
                projected
                    .latency_ms
                    .map_or_else(|| "none".to_owned(), |latency| format!("{latency}ms")),
                projected.updated_ms
            ),
        ]
    } else {
        vec![
            format!(
                "MCP{}: {} enabled={} disabled={}",
                stale_marker, state, projected.enabled_count, projected.disabled_count
            ),
            format_tools("Tools", &tools, tools_truncated),
            projected.last_error.as_deref().map_or_else(
                || "Last error: none".to_owned(),
                |error| format!("Last error: {error}"),
            ),
            projected.latency_ms.map_or_else(
                || "Latency: none".to_owned(),
                |latency| format!("Latency: {latency}ms"),
            ),
            format!("Updated: {}", projected.updated_ms),
        ]
    };

    for line in &mut lines {
        *line = fit_line(line, width);
    }
    Ok(lines)
}

fn format_tools(label: &str, tools: &[String], truncated: bool) -> String {
    if tools.is_empty() {
        return if truncated {
            format!("{label}: {TRUNCATION_MARKER}")
        } else {
            format!("{label}: none")
        };
    }
    let marker = if truncated {
        format!(", {TRUNCATION_MARKER}")
    } else {
        String::new()
    };
    format!("{label}: {}{marker}", tools.join(", "))
}

fn bounded_latency(latency_ms: Option<u64>) -> Option<u64> {
    latency_ms.map(|value| value.min(MAX_LATENCY_MS))
}

fn safe_tools(input: &[String]) -> (Vec<String>, bool) {
    let mut output = Vec::with_capacity(input.len().min(MAX_STATUS_TOOLS));
    let mut truncated = input.len() > MAX_STATUS_TOOLS;
    for raw in input {
        let Some(tool) = safe_label(raw, MAX_TOOL_NAME_CHARS) else {
            truncated = true;
            continue;
        };
        if output.len() >= MAX_STATUS_TOOLS {
            truncated = true;
            break;
        }
        output.push(tool);
    }
    (output, truncated)
}

fn safe_error(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| safe_label(raw, MAX_ERROR_LABEL_CHARS))
}

fn safe_label(raw: &str, max_chars: usize) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || is_sensitive(trimmed) {
        return if trimmed.is_empty() {
            None
        } else {
            Some(REDACTED_LABEL.to_owned())
        };
    }

    let mut clean = String::with_capacity(trimmed.len().min(max_chars));
    let mut exceeded = false;
    for (index, character) in trimmed.chars().enumerate() {
        if index >= max_chars {
            exceeded = true;
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '/' | '-') {
            clean.push(character);
        } else {
            clean.push('_');
        }
    }
    if clean.is_empty() {
        return None;
    }
    if exceeded {
        Some(with_marker(&clean, max_chars))
    } else {
        Some(clean)
    }
}

fn is_sensitive(value: &str) -> bool {
    [
        "secret",
        "token",
        "password",
        "passwd",
        "credential",
        "api_key",
        "apikey",
        "authorization",
        "bearer",
    ]
    .iter()
    .any(|marker| {
        value
            .as_bytes()
            .windows(marker.len())
            .any(|window| window.eq_ignore_ascii_case(marker.as_bytes()))
    }) || value.contains('=')
}

fn with_marker(value: &str, max_chars: usize) -> String {
    if max_chars <= TRUNCATION_MARKER.chars().count() {
        return TRUNCATION_MARKER.chars().take(max_chars).collect();
    }
    let prefix_len = max_chars - TRUNCATION_MARKER.chars().count();
    let prefix: String = value.chars().take(prefix_len).collect();
    format!("{prefix}{TRUNCATION_MARKER}")
}

fn fit_line(value: &str, width: usize) -> String {
    if value.len() <= width {
        return value.to_owned();
    }
    with_marker(value, width)
}

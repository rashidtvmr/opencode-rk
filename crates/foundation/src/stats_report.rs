//! REL-006 telemetry report renderer (`stats` output).
//!
//! Pure bounded text renderer over a caller-supplied snapshot: totals,
//! savings, recent entries. No I/O, no clock, no env read. Deterministic:
//! same snapshot + config yields byte-identical output.

#![forbid(unsafe_code)]

/// Upper bound on the byte length of any truncation marker appended by
/// [`render`]. Markers are `... [truncated {n} entries]\n` /
/// `\n... [truncated {n} bytes]\n` with `n: u64` (at most 20 digits),
/// so 64 is a safe ceiling.
pub const MARKER_MAX: usize = 64;

const REDACTED: &str = "[redacted]";
const EMPTY: &str = "[empty]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelemetryEntry {
    pub label: String,
    pub saved_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelemetrySnapshot {
    pub total: u64,
    pub saved_tokens: u64,
    pub total_tokens: u64,
    pub recent: Vec<TelemetryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 5,
            max_bytes: 8192,
        }
    }
}

/// Scrub one label before formatting. Any secret-like substring
/// (`sk-`, `ak-`, `bearer `, `api_key`/`api-key`/`apikey`,
/// `-----BEGIN`, any case) replaces the whole label with `[redacted]`
/// so secret bytes never reach the output. Empty labels render as
/// `[empty]`.
fn scrub_label(label: &str) -> String {
    if label.is_empty() {
        return EMPTY.to_string();
    }
    let lower = label.to_ascii_lowercase();
    let secret = lower.contains("sk-")
        || lower.contains("ak-")
        || lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("api-key")
        || lower.contains("apikey")
        || lower.contains("-----begin");
    if secret {
        REDACTED.to_string()
    } else {
        label.to_string()
    }
}

/// Render the telemetry report. Pure function: no store I/O, no env,
/// no clock. Output is capped at `max_bytes` plus one bounded marker.
#[must_use]
pub fn render(snapshot: &TelemetrySnapshot, cfg: &ReportConfig) -> String {
    if !cfg.enabled {
        return "telemetry disabled (opt-out set)\n".to_string();
    }
    if snapshot.total == 0 {
        return "no telemetry recorded yet\n".to_string();
    }
    let denom = snapshot.total_tokens.max(1) as u128;
    let pct = ((snapshot.saved_tokens as u128 * 100) / denom).min(u64::MAX as u128) as u64;

    let mut out = format!(
        "telemetry: {} compressions, {} tokens saved ({pct}%)\nrecent:\n",
        snapshot.total, snapshot.saved_tokens
    );

    let total_recent = snapshot.recent.len();
    let keep = total_recent.min(cfg.max_entries);
    let dropped_entries = total_recent - keep;
    for entry in snapshot.recent.iter().skip(dropped_entries) {
        out.push_str(&format!(
            "- {}: {} saved\n",
            scrub_label(&entry.label),
            entry.saved_tokens
        ));
    }
    if dropped_entries > 0 {
        out.push_str(&format!("... [truncated {dropped_entries} entries]\n"));
    }

    if out.len() > cfg.max_bytes {
        let mut cut = cfg.max_bytes;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        let dropped_bytes = out.len() - cut;
        out.truncate(cut);
        out.push_str(&format!("\n... [truncated {dropped_bytes} bytes]\n"));
        debug_assert!(
            out.len() <= cfg.max_bytes + MARKER_MAX,
            "byte-cap marker must stay within MARKER_MAX"
        );
    }
    out
}

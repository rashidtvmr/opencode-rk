//! REL-005 compression-stats collector: bounded event counts, tokens-saved
//! and fixed-point-micros cost estimate. Local-only. No clock, no I/O,
//! no locks, no secrets. `seq` is caller-supplied ordering input.
//!
//! Cost basis: `COST_PER_1K_MICROS = 2000` micros per 1K tokens
//! ($0.002/1K blended input/output reference rate, frozen at REL-005
//! implementation time). All cost math integer micros, no float.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::mem::size_of;

/// Bytes assumed per token for the saved-bytes to tokens estimate.
pub const BYTES_PER_TOKEN: usize = 4;
/// Micros (1e-6 USD) per 1000 tokens. Integer math only, no float.
pub const COST_PER_1K_MICROS: u64 = 2000;

const ENTRY_BYTES: usize = size_of::<Entry>();

/// Collector configuration. `Default` is enabled, 64 entries, 64 KiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatsConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 64,
            max_bytes: 65536,
        }
    }
}

impl StatsConfig {
    /// No-op config: `record` returns immediately, allocates nothing (REQ-032).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_entries: 0,
            max_bytes: 0,
        }
    }
}

/// One recorded compression event. `saved = orig.saturating_sub(out)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,
    pub orig: usize,
    pub out: usize,
    pub saved: usize,
}

/// Pure-read aggregate over the collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub total_events: u64,
    pub tokens_saved: u64,
    pub cost_estimate_micros: u64,
    pub retained_entries: usize,
    pub retained_bytes: usize,
}

/// Bounded in-process collector. Plain struct, no hidden mutex; the owner
/// handles sharing. Disabled instance never allocates on `record`.
#[derive(Clone, Debug)]
pub struct Collector {
    cfg: StatsConfig,
    total_events: u64,
    tokens_saved: u64,
    cost_estimate_micros: u64,
    history: VecDeque<Entry>,
    retained_bytes: usize,
}

impl Collector {
    /// Start at zero counts with empty history. Pre-sizes the deque to
    /// `max_entries` once (construction-time alloc only); the disabled
    /// config skips even that so `record` stays zero-alloc.
    #[must_use]
    pub fn new(cfg: StatsConfig) -> Self {
        let history = if cfg.enabled {
            VecDeque::with_capacity(cfg.max_entries.min(1024))
        } else {
            VecDeque::new()
        };
        Self {
            cfg,
            total_events: 0,
            tokens_saved: 0,
            cost_estimate_micros: 0,
            history,
            retained_bytes: 0,
        }
    }

    /// Record one event. Enabled only; disabled returns immediately with no
    /// allocation, no lock, no I/O. Counters always increment; an oversize
    /// single entry is dropped from history but still counted.
    pub fn record(&mut self, seq: u64, orig: usize, out: usize) {
        if !self.cfg.enabled {
            return;
        }
        let saved = orig.saturating_sub(out);
        let tokens = (saved / BYTES_PER_TOKEN) as u64;
        self.total_events = self.total_events.saturating_add(1);
        self.tokens_saved = self.tokens_saved.saturating_add(tokens);
        self.cost_estimate_micros = self
            .cost_estimate_micros
            .saturating_add(tokens.saturating_mul(COST_PER_1K_MICROS) / 1000);
        if ENTRY_BYTES > self.cfg.max_bytes || self.cfg.max_entries == 0 {
            return;
        }
        if self.history.len() >= self.cfg.max_entries {
            if let Some(old) = self.history.pop_front() {
                let _ = old;
                self.retained_bytes = self.retained_bytes.saturating_sub(ENTRY_BYTES);
            }
        }
        self.history.push_back(Entry {
            seq,
            orig,
            out,
            saved,
        });
        self.retained_bytes = self.retained_bytes.saturating_add(ENTRY_BYTES);
        while self.retained_bytes > self.cfg.max_bytes {
            match self.history.pop_front() {
                Some(_) => {
                    self.retained_bytes =
                        self.retained_bytes.saturating_sub(ENTRY_BYTES);
                }
                None => {
                    self.retained_bytes = 0;
                    break;
                }
            }
        }
    }

    /// Pure read, no mutation.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            total_events: self.total_events,
            tokens_saved: self.tokens_saved,
            cost_estimate_micros: self.cost_estimate_micros,
            retained_entries: self.history.len(),
            retained_bytes: self.retained_bytes,
        }
    }

    /// Clone up to `n` entries, oldest-first. `n == 0` returns empty;
    /// oversized `n` returns everything.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<Entry> {
        if n == 0 || self.history.is_empty() {
            return Vec::new();
        }
        let skip = self.history.len().saturating_sub(n);
        self.history.iter().skip(skip).copied().collect()
    }

    /// Clear counters and history back to the `new` state.
    pub fn reset(&mut self) {
        self.total_events = 0;
        self.tokens_saved = 0;
        self.cost_estimate_micros = 0;
        self.history.clear();
        self.retained_bytes = 0;
    }
}

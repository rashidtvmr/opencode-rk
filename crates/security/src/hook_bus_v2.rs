//! Bounded hook bus v2 - max 100 pending with shift-drop, always-emit allowlist, SSRF guard.
#![forbid(unsafe_code)]
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::ssrf::SsrfGuard;

/// Maximum number of pending hooks retained in the bus.
pub const MAX_PENDING: usize = 100;

/// Re-export SsrfBlocked so callers can handle it directly.
pub use crate::ssrf::SsrfBlocked;

/// Error returned by the hook bus.
#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum HookError {
    #[error("SSRF blocked: {0}")]
    SsrfBlocked(#[from] SsrfBlocked),
}

/// A single pending hook entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingHook {
    pub id: u64,
    pub url: String,
    pub payload: String,
    pub created_at: Instant,
}

/// A hook bus with bounded pending queue, always-emit allowlist, and SSRF protection.
#[derive(Debug)]
pub struct HookBusV2 {
    pending: VecDeque<PendingHook>,
    always_emit: Vec<String>,
    guard: SsrfGuard,
    next: u64,
}

impl HookBusV2 {
    /// Create a new bus with the default SSRF guard (localhost blocked).
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: VecDeque::with_capacity(MAX_PENDING),
            always_emit: Vec::new(),
            guard: SsrfGuard::new(),
            next: 1,
        }
    }

    /// Create a bus with a custom SSRF guard (e.g., allow_localhost enabled).
    #[must_use]
    pub fn with_guard(guard: SsrfGuard) -> Self {
        Self {
            pending: VecDeque::with_capacity(MAX_PENDING),
            always_emit: Vec::new(),
            guard,
            next: 1,
        }
    }

    /// Register a URL pattern that should always be emitted immediately, bypassing the queue.
    pub fn add_always_emit(&mut self, pattern: impl Into<String>) {
        self.always_emit.push(pattern.into());
    }

    /// Check if a URL matches an always-emit pattern.
    #[must_use]
    pub fn is_always_emit(&self, url: &str) -> bool {
        self.always_emit
            .iter()
            .any(|pattern| glob_match(pattern, url))
    }

    /// Validate a URL against the SSRF guard.
    pub fn validate_url(&self, url: &str) -> Result<(), SsrfBlocked> {
        self.guard.check_url(url)
    }

    /// Enqueue a hook. If the URL is an always-emit URL it bypasses validation
    /// and queuing (returns Ok with the assigned id and no entry added).
    /// If the queue is full, the oldest entry is shift-dropped.
    pub fn enqueue(&mut self, url: String, payload: String) -> Result<u64, HookError> {
        if self.is_always_emit(&url) {
            // Always-emit URLs bypass SSRF check and queue entirely.
            let id = self.next;
            self.next = self.next.saturating_add(1);
            return Ok(id);
        }

        self.validate_url(&url)?;

        let id = self.next;
        self.next = self.next.saturating_add(1);

        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(PendingHook {
            id,
            url,
            payload,
            created_at: Instant::now(),
        });

        Ok(id)
    }

    /// Drain all pending hooks, leaving the queue empty.
    #[must_use]
    pub fn drain_ready(&self) -> Vec<PendingHook> {
        self.pending.iter().cloned().collect()
    }

    /// Mutable drain: removes all pending hooks and returns them.
    pub fn drain_ready_mut(&mut self) -> Vec<PendingHook> {
        self.pending.drain(..).collect()
    }

    /// Current number of pending hooks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether there are no pending hooks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Maximum capacity of the pending queue.
    #[must_use]
    pub fn capacity(&self) -> usize {
        MAX_PENDING
    }
}

impl Default for HookBusV2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight glob matcher supporting `*` and `?` wildcards (no regex deps).
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_bytes(pattern.as_bytes(), text.as_bytes())
}

fn glob_bytes(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => {
            if rest.first() == Some(&b'*') {
                let mut tail = &rest[1..];
                while tail.first() == Some(&b'*') {
                    tail = &tail[1..];
                }
                (0..=text.len()).any(|i| glob_bytes(tail, &text[i..]))
            } else {
                for i in 0..=text.len() {
                    if glob_bytes(rest, &text[i..]) {
                        return true;
                    }
                    if i == text.len() || text[i] == b'/' {
                        break;
                    }
                }
                false
            }
        }
        Some((b'?', rest)) => !text.is_empty() && text[0] != b'/' && glob_bytes(rest, &text[1..]),
        Some((byte, rest)) => !text.is_empty() && text[0] == *byte && glob_bytes(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> String {
        "https://93.184.216.34/webhook".to_owned()
    }

    #[test]
    fn enqueue_and_drain() {
        let mut bus = HookBusV2::new();
        let id = bus.enqueue(url(), "payload".to_owned()).unwrap();
        assert!(id > 0);
        let drained = bus.drain_ready_mut();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, id);
        assert_eq!(drained[0].url, "https://93.184.216.34/webhook");
        assert_eq!(drained[0].payload, "payload");
        assert!(bus.is_empty());
    }

    #[test]
    fn shift_drop_on_overflow() {
        let mut bus = HookBusV2::new();
        for i in 0..MAX_PENDING {
            let _ = bus
                .enqueue(url(), format!("p{i}"))
                .expect("enqueue should succeed");
        }
        assert_eq!(bus.len(), MAX_PENDING);

        let first_entry = bus.drain_ready();
        let first_id = first_entry.first().map(|h| h.id).unwrap();
        drop(first_entry);

        let new_id = bus
            .enqueue(url(), "p100".to_owned())
            .expect("enqueue should succeed");

        assert_eq!(bus.len(), MAX_PENDING);
        let drained = bus.drain_ready_mut();
        assert!(!drained.iter().any(|h| h.id == first_id));
        assert!(drained.iter().any(|h| h.id == new_id));
    }

    #[test]
    fn always_emit_bypasses_queue() {
        let mut bus = HookBusV2::new();
        bus.add_always_emit("https://93.184.216.34/important/*");

        let id = bus
            .enqueue(
                "https://93.184.216.34/important/event".to_owned(),
                "data".to_owned(),
            )
            .unwrap();
        assert!(id > 0);
        assert!(bus.is_empty(), "always-emit should not add to queue");

        // A normal public IP URL still goes into the queue.
        let _ = bus
            .enqueue("https://8.8.8.8/webhook".to_owned(), "data".to_owned())
            .unwrap();
        assert_eq!(bus.len(), 1);
    }

    #[test]
    fn ssrf_blocks_internal_url() {
        let mut bus = HookBusV2::new();
        let result = bus.enqueue("http://127.0.0.1:8080/hook".to_owned(), "data".to_owned());
        assert!(matches!(result, Err(HookError::SsrfBlocked(_))));
    }

    #[test]
    fn bounded_capacity() {
        let bus = HookBusV2::new();
        assert_eq!(bus.capacity(), MAX_PENDING);
    }
}

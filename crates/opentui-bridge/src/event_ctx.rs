#![forbid(unsafe_code)]
//! Event subscription context (pure, std only, no IO/FFI).
//!
//! Minimal port of `useEvent` subscribe/on closure pattern
//! (`packages/tui/src/context/event.ts:12-30`): subscribers register an
//! owner id per event name; `sync` payload filtering lives at the SDK
//! layer, not here. Bounded fail-closed registry: 64 subs max, 64 bytes
//! max per event name and owner id.

/// Max subscriptions (fail-closed; TS side unbounded).
pub const MAX_SUBS: usize = 64;
/// Max event-name bytes (fail-closed).
pub const MAX_EVENT: usize = 64;
/// Max owner-id bytes (fail-closed).
pub const MAX_OWNER: usize = 64;

/// Bounded (event, owner) subscription registry.
#[derive(Debug, Default, Clone)]
pub struct EventCtx {
    subs: Vec<(String, String)>,
}

impl EventCtx {
    #[must_use]
    pub const fn new() -> Self {
        Self { subs: Vec::new() }
    }

    /// Subscribe `id` to `ev`. False when duplicate, empty/too long,
    /// or registry full.
    pub fn sub(&mut self, ev: &str, id: &str) -> bool {
        if ev.is_empty() || ev.len() > MAX_EVENT {
            return false;
        }
        if id.is_empty() || id.len() > MAX_OWNER {
            return false;
        }
        if self.subs.len() >= MAX_SUBS {
            return false;
        }
        if self.subs.iter().any(|(e, o)| e == ev && o == id) {
            return false;
        }
        self.subs.push((ev.to_string(), id.to_string()));
        true
    }

    /// Unsubscribe `id` from `ev`. False when absent.
    pub fn unsub(&mut self, ev: &str, id: &str) -> bool {
        match self.subs.iter().position(|(e, o)| e == ev && o == id) {
            Some(i) => {
                self.subs.remove(i);
                true
            }
            None => false,
        }
    }

    /// Owner ids subscribed to `ev`, in subscription order.
    #[must_use]
    pub fn emit(&self, ev: &str) -> Vec<String> {
        self.subs
            .iter()
            .filter(|(e, _)| e == ev)
            .map(|(_, o)| o.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_emit_order() {
        let mut c = EventCtx::new();
        assert!(c.sub("session-idle", "a"));
        assert!(c.sub("session-idle", "b"));
        assert!(c.sub("message", "c"));
        assert_eq!(
            c.emit("session-idle"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn unsub_removes() {
        let mut c = EventCtx::new();
        assert!(c.sub("ev", "a"));
        assert!(c.sub("ev", "b"));
        assert!(c.unsub("ev", "a"));
        assert_eq!(c.emit("ev"), vec!["b".to_string()]);
        assert!(!c.unsub("ev", "a"));
    }

    #[test]
    fn missing_emit_empty() {
        let c = EventCtx::new();
        assert!(c.emit("ghost").is_empty());
        let mut c2 = EventCtx::new();
        assert!(c2.sub("ev", "a"));
        assert!(c2.emit("other").is_empty());
    }

    #[test]
    fn dup_sub_false() {
        let mut c = EventCtx::new();
        assert!(c.sub("ev", "a"));
        assert!(!c.sub("ev", "a"));
        assert!(!c.sub("", "a"));
        assert!(!c.sub("ev", ""));
        assert!(!c.sub(&"e".repeat(MAX_EVENT + 1), "a"));
        assert!(!c.sub("ev", &"x".repeat(MAX_OWNER + 1)));
    }

    #[test]
    fn cap_false() {
        let mut c = EventCtx::new();
        for i in 0..MAX_SUBS {
            assert!(c.sub(&format!("ev-{i}"), &format!("owner-{i}")));
        }
        assert!(!c.sub("extra", "one-more"));
    }
}

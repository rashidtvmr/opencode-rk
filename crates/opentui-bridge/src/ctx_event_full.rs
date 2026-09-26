#![forbid(unsafe_code)]
//! Latest context event kind + monotonic sequence (std only).
//!
//! Mirrors payload kind flowing through `subscribe` in
//! `packages/tui/src/context/event.ts:12-19`; `sync` filtered upstream,
//! here only last kind retained with seq bump per emit.

/// Max kind bytes (fail-closed truncate on char boundary).
pub const MAX_KIND: usize = 64;

/// Last emitted event kind with monotonic sequence.
#[derive(Debug, Clone, Default)]
pub struct CtxEvent {
    kind: String,
    seq: u64,
}

impl CtxEvent {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            kind: String::new(),
            seq: 0,
        }
    }

    /// Record `kind` (truncated to [`MAX_KIND`] bytes), bump seq.
    pub fn emit(&mut self, kind: &str) {
        let mut k = kind.to_string();
        while k.len() > MAX_KIND {
            k.pop();
        }
        self.kind = k;
        self.seq = self.seq.wrapping_add(1);
    }

    #[must_use]
    pub fn kind_of(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_bumps_seq() {
        let mut e = CtxEvent::new();
        e.emit("message");
        assert_eq!(e.kind_of(), "message");
        assert_eq!(e.seq(), 1);
        e.emit("idle");
        assert_eq!(e.kind_of(), "idle");
        assert_eq!(e.seq(), 2);
    }

    #[test]
    fn kind_capped_64() {
        let mut e = CtxEvent::new();
        e.emit(&"k".repeat(MAX_KIND + 8));
        assert_eq!(e.kind_of().len(), MAX_KIND);
        assert_eq!(e.seq(), 1);
    }

    #[test]
    fn default_empty_zero() {
        let e = CtxEvent::default();
        assert_eq!(e.kind_of(), "");
        assert_eq!(e.seq(), 0);
    }
}

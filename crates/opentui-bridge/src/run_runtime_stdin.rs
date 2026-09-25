#![forbid(unsafe_code)]
//! Stdin probe companion for the run runtime.
//!
//! Mirrors `packages/opencode/src/cli/cmd/run/runtime.stdin.ts`
//! (`resolveInteractiveStdin`): tty wins, else buffered bytes mean piped,
//! else closed. Thin adapter over [`crate::run_runtime::resolve_stdin`].
use crate::run_runtime::{resolve_stdin, StdinMode};

/// Observed stdin state: tty flag plus buffered pending bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdinProbe {
    pub is_tty: bool,
    pub pending_bytes: usize,
}

impl StdinProbe {
    /// New probe from tty flag and pending byte count.
    pub fn new(is_tty: bool, pending_bytes: usize) -> Self {
        Self {
            is_tty,
            pending_bytes,
        }
    }

    /// Zero probe: not a tty, no bytes buffered (closed).
    pub fn closed() -> Self {
        Self {
            is_tty: false,
            pending_bytes: 0,
        }
    }
}

/// Resolve a probe to [`StdinMode`]; tty wins, bytes mean piped.
pub fn resolve_probe(probe: StdinProbe) -> StdinMode {
    resolve_stdin(probe.is_tty, probe.pending_bytes > 0)
}

/// Stable label for a mode: "tty" / "piped" / "closed".
pub fn probe_label(mode: StdinMode) -> &'static str {
    match mode {
        StdinMode::Tty => "tty",
        StdinMode::Piped => "piped",
        StdinMode::Closed => "closed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_wins_over_bytes() {
        let p = StdinProbe::new(true, 7);
        assert_eq!(resolve_probe(p), StdinMode::Tty);
    }

    #[test]
    fn piped_on_bytes() {
        let p = StdinProbe::new(false, 3);
        assert_eq!(resolve_probe(p), StdinMode::Piped);
    }

    #[test]
    fn closed_when_empty() {
        let p = StdinProbe::new(false, 0);
        assert_eq!(resolve_probe(p), StdinMode::Closed);
    }

    #[test]
    fn probe_zero_is_closed() {
        assert_eq!(resolve_probe(StdinProbe::closed()), StdinMode::Closed);
    }

    #[test]
    fn label_tty() {
        assert_eq!(probe_label(StdinMode::Tty), "tty");
    }

    #[test]
    fn label_piped() {
        assert_eq!(probe_label(StdinMode::Piped), "piped");
    }

    #[test]
    fn label_closed() {
        assert_eq!(probe_label(StdinMode::Closed), "closed");
    }
}

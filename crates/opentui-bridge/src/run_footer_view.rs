#![forbid(unsafe_code)]
//! Unified run footer view.
//!
//! Reconciles the two existing footer shapes without touching them:
//! - `run_footer::FooterView` (`Idle`/`Streaming`/`Done` queue view)
//! - `run_types::FooterPhase` (`Prompt`/`Permission`/`Question`/… busy view)
//!
//! [`FooterSurface`] is the union of both surfaces; [`FooterSnapshot`] pairs
//! it with `busy` and `queue_len` so callers need only one type.

/// Visible footer surface (union of `run_footer` and `run_types` shapes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterSurface {
    Prompt,
    Permission,
    Question,
    Subagent,
    Streaming,
    Done,
    #[default]
    Idle,
}

/// Atomic footer snapshot: active surface plus liveness and queue depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterSnapshot {
    pub surface: FooterSurface,
    pub busy: bool,
    pub queue_len: usize,
}

impl Default for FooterSnapshot {
    fn default() -> Self {
        Self {
            surface: FooterSurface::Idle,
            busy: false,
            queue_len: 0,
        }
    }
}

impl FooterSnapshot {
    #[must_use]
    pub const fn new(surface: FooterSurface, busy: bool, queue_len: usize) -> Self {
        Self {
            surface,
            busy,
            queue_len,
        }
    }

    /// Build from a `run_types` phase name; unknown names fall back to `Idle`.
    #[must_use]
    pub fn from_run_types(phase: &str, busy: bool, queue_len: usize) -> Self {
        let surface = match phase {
            "prompt" => FooterSurface::Prompt,
            "permission" => FooterSurface::Permission,
            "question" => FooterSurface::Question,
            "subagent" => FooterSurface::Subagent,
            "done" => FooterSurface::Done,
            _ => FooterSurface::Idle,
        };
        Self {
            surface,
            busy,
            queue_len,
        }
    }

    /// Build from a `run_footer` streaming view plus queue depth.
    #[must_use]
    pub const fn from_queue(view_is_streaming: bool, queue_len: usize) -> Self {
        if view_is_streaming {
            Self {
                surface: FooterSurface::Streaming,
                busy: true,
                queue_len,
            }
        } else {
            Self {
                surface: FooterSurface::Idle,
                busy: false,
                queue_len,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_phase_maps() {
        let s = FooterSnapshot::from_run_types("prompt", true, 2);
        assert_eq!(s.surface, FooterSurface::Prompt);
        assert!(s.busy);
        assert_eq!(s.queue_len, 2);
    }

    #[test]
    fn permission_phase_maps() {
        let s = FooterSnapshot::from_run_types("permission", true, 0);
        assert_eq!(s.surface, FooterSurface::Permission);
    }

    #[test]
    fn question_phase_maps() {
        let s = FooterSnapshot::from_run_types("question", false, 1);
        assert_eq!(s.surface, FooterSurface::Question);
        assert_eq!(s.queue_len, 1);
    }

    #[test]
    fn subagent_phase_maps() {
        let s = FooterSnapshot::from_run_types("subagent", true, 4);
        assert_eq!(s.surface, FooterSurface::Subagent);
    }

    #[test]
    fn done_phase_maps() {
        let s = FooterSnapshot::from_run_types("done", false, 0);
        assert_eq!(s.surface, FooterSurface::Done);
        assert!(!s.busy);
    }

    #[test]
    fn unknown_phase_maps_to_idle() {
        let s = FooterSnapshot::from_run_types("bogus", false, 0);
        assert_eq!(s.surface, FooterSurface::Idle);
    }

    #[test]
    fn queue_streaming_surface() {
        let s = FooterSnapshot::from_queue(true, 3);
        assert_eq!(s.surface, FooterSurface::Streaming);
        assert!(s.busy);
        assert_eq!(s.queue_len, 3);
    }

    #[test]
    fn queue_empty_idle() {
        let s = FooterSnapshot::from_queue(false, 0);
        assert_eq!(s.surface, FooterSurface::Idle);
        assert!(!s.busy);
        assert_eq!(s.queue_len, 0);
    }
}

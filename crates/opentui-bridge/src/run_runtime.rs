#![forbid(unsafe_code)]
//! Runtime lifecycle + stdin resolver.
//!
//! Mirrors `packages/opencode/src/cli/run/runtime.lifecycle.ts`
//! (`createRuntimeLifecycle`), `runPromptQueue` (cap 64), and
//! `resolveInteractiveStdin` (Tty / Piped / Closed).

/// Lifecycle phase of the run runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePhase {
    Boot,
    Interactive,
    Shutdown,
}

/// How stdin is attached for the interactive session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinMode {
    Tty,
    Piped,
    Closed,
}

/// Boot-to-shutdown lifecycle with a bounded prompt queue.
#[derive(Debug)]
pub struct RuntimeLifecycle {
    phase: RuntimePhase,
    prompt_queue: Vec<String>,
    stdin_mode: StdinMode,
}

/// Max queued prompts (`runPromptQueue` bound).
pub const PROMPT_QUEUE_CAP: usize = 64;

/// Fresh lifecycle in [`RuntimePhase::Boot`].
pub fn boot() -> RuntimeLifecycle {
    RuntimeLifecycle {
        phase: RuntimePhase::Boot,
        prompt_queue: Vec::new(),
        stdin_mode: StdinMode::Closed,
    }
}

/// Resolve stdin mode: tty wins, else buffered bytes mean piped, else closed.
/// Mirrors `resolveInteractiveStdin`.
pub fn resolve_stdin(is_tty: bool, has_bytes: bool) -> StdinMode {
    if is_tty {
        StdinMode::Tty
    } else if has_bytes {
        StdinMode::Piped
    } else {
        StdinMode::Closed
    }
}

impl RuntimeLifecycle {
    /// Current phase.
    pub fn phase(&self) -> RuntimePhase {
        self.phase
    }

    /// Current stdin mode.
    pub fn stdin_mode(&self) -> StdinMode {
        self.stdin_mode
    }

    /// Queued prompt count.
    pub fn queue_len(&self) -> usize {
        self.prompt_queue.len()
    }

    /// Move Boot -> Interactive, recording the resolved stdin mode.
    pub fn enter_interactive(&mut self, stdin_mode: StdinMode) {
        self.phase = RuntimePhase::Interactive;
        self.stdin_mode = stdin_mode;
    }

    /// Queue a prompt; false (dropped) when the 64-entry cap is reached.
    /// Mirrors `runPromptQueue` bound.
    pub fn enqueue(&mut self, prompt: String) -> bool {
        if self.prompt_queue.len() >= PROMPT_QUEUE_CAP {
            return false;
        }
        self.prompt_queue.push(prompt);
        true
    }

    /// Take all queued prompts, leaving the queue empty.
    pub fn drain_queue(&mut self) -> Vec<String> {
        std::mem::take(&mut self.prompt_queue)
    }

    /// Move to terminal [`RuntimePhase::Shutdown`], clearing the queue.
    pub fn shutdown(&mut self) {
        self.phase = RuntimePhase::Shutdown;
        self.prompt_queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_starts_in_boot_phase() {
        let rt = boot();
        assert_eq!(rt.phase(), RuntimePhase::Boot);
        assert_eq!(rt.queue_len(), 0);
    }

    #[test]
    fn enqueue_rejects_past_cap() {
        let mut rt = boot();
        for i in 0..PROMPT_QUEUE_CAP {
            assert!(rt.enqueue(format!("p{i}")));
        }
        assert!(!rt.enqueue("overflow".to_string()));
        assert_eq!(rt.queue_len(), PROMPT_QUEUE_CAP);
    }

    #[test]
    fn drain_clears_queue() {
        let mut rt = boot();
        rt.enqueue("a".to_string());
        rt.enqueue("b".to_string());
        let out = rt.drain_queue();
        assert_eq!(out, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(rt.queue_len(), 0);
        assert!(rt.drain_queue().is_empty());
    }

    #[test]
    fn stdin_modes_resolve() {
        assert_eq!(resolve_stdin(true, false), StdinMode::Tty);
        assert_eq!(resolve_stdin(true, true), StdinMode::Tty);
        assert_eq!(resolve_stdin(false, true), StdinMode::Piped);
        assert_eq!(resolve_stdin(false, false), StdinMode::Closed);
    }

    #[test]
    fn shutdown_is_terminal() {
        let mut rt = boot();
        rt.enter_interactive(StdinMode::Tty);
        assert_eq!(rt.phase(), RuntimePhase::Interactive);
        rt.enqueue("x".to_string());
        rt.shutdown();
        assert_eq!(rt.phase(), RuntimePhase::Shutdown);
        assert_eq!(rt.queue_len(), 0);
    }

    #[test]
    fn enter_interactive_records_stdin() {
        let mut rt = boot();
        rt.enter_interactive(StdinMode::Piped);
        assert_eq!(rt.stdin_mode(), StdinMode::Piped);
        assert_eq!(rt.phase(), RuntimePhase::Interactive);
    }
}

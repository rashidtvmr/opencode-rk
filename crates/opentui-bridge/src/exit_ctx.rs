#![forbid(unsafe_code)]
//! Exit request contract (std-only).
//! TS `context/exit.tsx:1-8` `Exit=(reason?: unknown)=>void`;
//! terminal reason lives in `run_lifecycle.rs` `CloseReason`.

/// Max reason bytes (lifecycle error cap 256).
pub const MAX_REASON: usize = 256;

/// Pending/confirmed exit request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExitCtx {
    code: i32,
    reason: String,
    asked: bool,
}

impl ExitCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn cap(s: &str) -> String {
        let mut t = s.to_string();
        // ponytail: char-boundary cap; byte slicing would panic on multibyte.
        while t.len() > MAX_REASON {
            t.pop();
        }
        t
    }

    /// Record an exit request with reason; marks asked.
    pub fn ask(&mut self, reason: &str) {
        self.reason = Self::cap(reason);
        self.asked = true;
    }

    /// Confirm with exit code; keeps reason, marks asked.
    pub fn confirm(&mut self, code: i32) {
        self.code = code;
        self.asked = true;
    }

    /// Clear pending request (reason + code + asked reset).
    pub fn cancel(&mut self) {
        self.reason.clear();
        self.code = 0;
        self.asked = false;
    }

    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    #[must_use]
    pub fn asked(&self) -> bool {
        self.asked
    }

    /// `"exit <code> <reason> <asked>"`.
    #[must_use]
    pub fn status(&self) -> String {
        format!("exit {} {} {}", self.code, self.reason, self.asked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_empty() {
        let ctx = ExitCtx::new();
        assert_eq!(ctx.code(), 0);
        assert_eq!(ctx.reason(), "");
        assert!(!ctx.asked());
    }

    #[test]
    fn ask_sets_reason() {
        let mut ctx = ExitCtx::new();
        ctx.ask("done");
        assert_eq!(ctx.reason(), "done");
        assert!(ctx.asked());
    }

    #[test]
    fn ask_truncates() {
        let mut ctx = ExitCtx::new();
        ctx.ask(&"x".repeat(300));
        assert_eq!(ctx.reason().len(), MAX_REASON);
        assert!(ctx.asked());
    }

    #[test]
    fn confirm_sets_code() {
        let mut ctx = ExitCtx::new();
        ctx.ask("bye");
        ctx.confirm(2);
        assert_eq!(ctx.code(), 2);
        assert_eq!(ctx.reason(), "bye");
        assert!(ctx.asked());
    }

    #[test]
    fn cancel_clears() {
        let mut ctx = ExitCtx::new();
        ctx.ask("bye");
        ctx.confirm(1);
        ctx.cancel();
        assert_eq!(ctx.code(), 0);
        assert_eq!(ctx.reason(), "");
        assert!(!ctx.asked());
    }

    #[test]
    fn status_parts() {
        let mut ctx = ExitCtx::new();
        ctx.ask("done");
        ctx.confirm(3);
        let s = ctx.status();
        assert!(s.starts_with("exit 3 "), "got {s}");
        assert!(s.contains("done"), "got {s}");
        assert!(s.ends_with(" true"), "got {s}");
    }
}

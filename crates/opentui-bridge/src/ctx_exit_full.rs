#![forbid(unsafe_code)]
//! Exit code request mirroring `exit.tsx:1-8` `Exit=(reason?: unknown)=>void`.
/// One-shot exit code request; `take` yields code once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExitReq {
    pub code: i32,
    pub asked: bool,
}

impl ExitReq {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Record exit code request.
    pub fn ask(&mut self, code: i32) {
        self.code = code;
        self.asked = true;
    }
    /// Consume pending request once; `None` until next `ask`.
    pub fn take(&mut self) -> Option<i32> {
        if self.asked {
            self.asked = false;
            Some(self.code)
        } else {
            None
        }
    }
    /// True when an unclaimed request is pending.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.asked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_not_pending() {
        let r = ExitReq::new();
        assert!(!r.pending());
        assert_eq!(r.code, 0);
    }
    #[test]
    fn ask_pends_take_once() {
        let mut r = ExitReq::new();
        r.ask(2);
        assert!(r.pending());
        assert_eq!(r.take(), Some(2));
        assert!(!r.pending());
        assert_eq!(r.take(), None);
    }
    #[test]
    fn reask_after_take() {
        let mut r = ExitReq::new();
        r.ask(1);
        assert_eq!(r.take(), Some(1));
        r.ask(3);
        assert!(r.pending());
        assert_eq!(r.take(), Some(3));
    }
}

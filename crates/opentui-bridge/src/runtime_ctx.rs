#![forbid(unsafe_code)]
//! Runtime terminal context: dims + readiness flag.
//!
//! Rust mirror of `packages/tui/src/context/runtime.tsx` provider pattern:
//! explicit state, no ambient globals. Zero dims rejected.

/// Terminal dims plus readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCtx {
    pub cols: u16,
    pub rows: u16,
    pub ready: bool,
}

impl RuntimeCtx {
    /// New context, not ready.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            ready: false,
        }
    }

    /// Resize; rejects zero dims, returns false without mutating.
    pub fn resize(&mut self, cols: u16, rows: u16) -> bool {
        if cols == 0 || rows == 0 {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        true
    }

    /// Mark terminal ready.
    pub fn mark_ready(&mut self) {
        self.ready = true;
    }

    /// Current dims.
    pub fn dims(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Readiness flag.
    pub fn is_ready(&self) -> bool {
        self.ready
    }
}

impl Default for RuntimeCtx {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_not_ready() {
        let c = RuntimeCtx::default();
        assert!(!c.is_ready());
    }

    #[test]
    fn zero_cols_false() {
        let mut c = RuntimeCtx::default();
        assert!(!c.resize(0, 24));
        assert_eq!(c.dims(), (80, 24));
    }

    #[test]
    fn zero_rows_false() {
        let mut c = RuntimeCtx::default();
        assert!(!c.resize(80, 0));
        assert_eq!(c.dims(), (80, 24));
    }

    #[test]
    fn resize_ok() {
        let mut c = RuntimeCtx::default();
        assert!(c.resize(120, 40));
        assert_eq!(c.dims(), (120, 40));
    }

    #[test]
    fn ready_flow() {
        let mut c = RuntimeCtx::default();
        c.mark_ready();
        assert!(c.is_ready());
    }

    #[test]
    fn dims_roundtrip() {
        let c = RuntimeCtx::new(100, 30);
        assert_eq!(c.dims(), (100, 30));
        assert!(!c.is_ready());
    }
}

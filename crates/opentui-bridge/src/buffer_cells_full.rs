#![forbid(unsafe_code)]
//! Buffered cell ops for native buffer surface (`buffer.rs:68-140`).
//! `bufferSetCell`->push_op, `bufferClear`->clear_flag,
//! `bufferResize`->resize_ok; DrawBox/Grid/WriteResolvedChars drain in order.
//! ponytail: Vec log only; add grid store when random read needed.

/// One pending `bufferSetCell` write; `char` keeps Unicode scalar-safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellOp {
    pub x: u16,
    pub y: u16,
    pub ch: char,
}

impl CellOp {
    /// Short constructor (keeps call sites under rustfmt struct width).
    pub const fn at(x: u16, y: u16, ch: char) -> Self {
        Self { x, y, ch }
    }
}

/// Max buffered ops; `push_op` returns `false` past this (no growth).
pub const MAX_OPS: usize = 4096;

/// Ordered op log plus pending-clear bit.
#[derive(Debug, Default, Clone)]
pub struct CellOpLog {
    ops: Vec<CellOp>,
    cleared: bool,
}

impl CellOpLog {
    /// Buffer `op`; `false` (dropped) when full.
    pub fn push_op(&mut self, op: CellOp) -> bool {
        if self.ops.len() >= MAX_OPS {
            return false;
        }
        self.ops.push(op);
        true
    }
    /// Buffered op count.
    #[must_use]
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }
    /// Stage (`true`) or unstage a `bufferClear` drain.
    pub fn clear_flag(&mut self, v: bool) {
        self.cleared = v;
    }
    /// Pending-clear bit.
    #[must_use]
    pub fn cleared(&self) -> bool {
        self.cleared
    }
}

/// `bufferResize` bounds: each axis `1..=512`.
#[must_use]
pub fn resize_ok(w: u16, h: u16) -> bool {
    (1..=512).contains(&w) && (1..=512).contains(&h)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_counts() {
        let mut l = CellOpLog::default();
        assert!(l.push_op(CellOp::at(1, 2, 'a')));
        assert!(l.push_op(CellOp::at(0, 0, 'x')));
        assert_eq!(l.op_count(), 2);
    }
    #[test]
    fn cap_drops_past_max() {
        let mut l = CellOpLog::default();
        for _ in 0..MAX_OPS {
            assert!(l.push_op(CellOp::at(0, 0, 'x')));
        }
        assert!(!l.push_op(CellOp::at(0, 0, 'y')));
        assert_eq!(l.op_count(), MAX_OPS);
    }
    #[test]
    fn clear_roundtrip() {
        let mut l = CellOpLog::default();
        l.clear_flag(true);
        assert!(l.cleared());
        l.clear_flag(false);
        assert!(!l.cleared());
    }
    #[test]
    fn resize_bounds() {
        assert!(resize_ok(80, 24) && resize_ok(1, 1) && resize_ok(512, 512));
        assert!(!resize_ok(0, 24) && !resize_ok(80, 0) && !resize_ok(513, 24));
    }
    #[test]
    fn unicode_scalar_safe() {
        assert_eq!(CellOp::at(0, 0, 'x').ch.len_utf8(), 1);
        assert_eq!(CellOp::at(0, 0, 'x').ch, 'x');
    }
}

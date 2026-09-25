#![forbid(unsafe_code)]
//! Minimal native reactive core. Cut: no TTFD/Portal, manual recompute only.
//! [`Cell`]/[`Memo`]/[`Batch`] are plain integers, `Send` + `Sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cell {
    v: i64,
    ver: u64,
}
impl Cell {
    #[must_use]
    pub fn new(v: i64) -> Self {
        Self { v, ver: 0 }
    }
    #[must_use]
    pub fn get(self) -> i64 {
        self.v
    }
    #[must_use]
    pub fn ver(self) -> u64 {
        self.ver
    }
    pub fn set(&mut self, v: i64) {
        self.v = v;
        self.ver = self.ver.saturating_add(1);
    }
}
/// Memo: derived value with manual dirty tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Memo {
    cell: Cell,
    dirty: bool,
}
impl Memo {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn get(self) -> i64 {
        self.cell.get()
    }
    #[must_use]
    pub fn ver(self) -> u64 {
        self.cell.ver()
    }
    #[must_use]
    pub fn dirty(self) -> bool {
        self.dirty
    }
    pub fn mark(&mut self) {
        self.dirty = true;
    }
    pub fn clean(&mut self) {
        self.dirty = false;
    }
    pub fn set(&mut self, v: i64) {
        self.cell.set(v);
    }
}
/// Batch counter for group transactions.
pub struct Batch {
    counter: u32,
}
impl Batch {
    #[must_use]
    pub fn new() -> Self {
        Self { counter: 0 }
    }
    #[must_use]
    pub fn is_batching(self) -> bool {
        self.counter > 0
    }
    pub fn start(&mut self) {
        self.counter = self.counter.saturating_add(1);
    }
    pub fn end(&mut self) {
        self.counter = self.counter.saturating_sub(1);
    }
    #[must_use]
    pub fn value(self) -> u32 {
        self.counter
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cell_roundtrip() {
        let mut c = Cell::new(42);
        c.set(10);
        assert_eq!((c.get(), c.ver()), (10, 1));
    }
    #[test]
    fn cell_ver_saturates() {
        let mut c = Cell::new(0);
        c.ver = u64::MAX;
        c.set(1);
        assert_eq!(c.ver(), u64::MAX);
    }
    #[test]
    fn memo_mark_clean() {
        let mut m = Memo::new();
        m.mark();
        m.set(7);
        m.clean();
        assert!(!m.dirty() && m.get() == 7 && m.ver() == 1);
    }
    #[test]
    fn batch_nests_and_saturates() {
        let mut b = Batch::new();
        b.start();
        b.start();
        b.end();
        b.end();
        b.end();
        assert_eq!(b.value(), 0);
    }
}

#![forbid(unsafe_code)]
//! P0 reactive core (deferred per BRIDGE_MIGRATION_DETAIL.md:310-312).
//!
//! Minimal `Signal`/`Memo`/`Effect` for the `solid_host.rs` boundary
//! (dims/slot-id only; reactivity stays native here until TS wiring lands).
//!
//! Single-owner-thread: all shared state is `Rc<RefCell<..>>`, hence `!Send`
//! by construction. Never move these across threads. `Rc<RefCell>` is allowed
//! ONLY inside this file.
//!
//! Recompute order note: memos recompute explicitly via `recompute()` in
//! creation order (topological: upstream first). No auto-dependency tracking
//! in P0; `batch()` only coalesces the depth counter, callers recompute
//! downstream memos before re-running effects.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static BATCH_DEPTH: Cell<u64> = const { Cell::new(0) };
}

/// Current batch nesting depth (0 outside `batch`).
#[must_use]
pub fn batch_depth() -> u64 {
    BATCH_DEPTH.with(|d| d.get())
}

/// Run `f` inside a batch; nesting increments the counter.
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    BATCH_DEPTH.with(|d| d.set(d.get() + 1));
    let out = f();
    BATCH_DEPTH.with(|d| d.set(d.get() - 1));
    out
}

/// Reactive value cell (`createSignal` P0 stand-in). `!Send` via `Rc`.
#[derive(Debug)]
pub struct Signal<T: Clone> {
    value: Rc<RefCell<T>>,
}

impl<T: Clone> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            value: Rc::clone(&self.value),
        }
    }
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
        }
    }

    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
    }
}

/// Cached derivation; explicit `recompute` in creation order. `!Send`.
pub struct Memo<T: Clone> {
    compute: Rc<dyn Fn() -> T>,
    cache: Rc<RefCell<T>>,
}

impl<T: Clone> Memo<T> {
    pub fn new(compute: impl Fn() -> T + 'static) -> Self {
        let compute: Rc<dyn Fn() -> T> = Rc::new(compute);
        let init = compute();
        Self {
            compute,
            cache: Rc::new(RefCell::new(init)),
        }
    }

    pub fn get(&self) -> T {
        self.cache.borrow().clone()
    }

    pub fn recompute(&self) {
        *self.cache.borrow_mut() = (self.compute)();
    }
}

/// Side-effect with disposal; stale (disposed) effects never re-run. `!Send`.
pub struct Effect {
    run_fn: Rc<dyn Fn()>,
    disposed: Rc<RefCell<bool>>,
    runs: Rc<RefCell<u64>>,
}

impl Effect {
    pub fn new(f: impl Fn() + 'static) -> Self {
        Self {
            run_fn: Rc::new(f),
            disposed: Rc::new(RefCell::new(false)),
            runs: Rc::new(RefCell::new(0)),
        }
    }

    /// No-op when disposed.
    pub fn run(&self) {
        if *self.disposed.borrow() {
            return;
        }
        (self.run_fn)();
        *self.runs.borrow_mut() += 1;
    }

    pub fn dispose(&self) {
        *self.disposed.borrow_mut() = true;
    }

    #[must_use]
    pub fn disposed(&self) -> bool {
        *self.disposed.borrow()
    }

    #[must_use]
    pub fn run_count(&self) -> u64 {
        *self.runs.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_get_set() {
        let s = Signal::new(1);
        assert_eq!(s.get(), 1);
        s.set(2);
        assert_eq!(s.get(), 2);
    }

    #[test]
    fn memo_recompute() {
        let src = Signal::new(2);
        let c = src.clone();
        let m = Memo::new(move || c.get() * 2);
        assert_eq!(m.get(), 4);
        src.set(5);
        assert_eq!(m.get(), 4);
        m.recompute();
        assert_eq!(m.get(), 10);
    }

    #[test]
    fn effect_run() {
        let n = Rc::new(Cell::new(0u32));
        let c = Rc::clone(&n);
        let e = Effect::new(move || c.set(c.get() + 1));
        e.run();
        e.run();
        assert_eq!(e.run_count(), 2);
        assert_eq!(n.get(), 2);
    }

    #[test]
    fn effect_dispose() {
        let e = Effect::new(|| {});
        assert!(!e.disposed());
        e.dispose();
        assert!(e.disposed());
    }

    #[test]
    fn batch_counter() {
        assert_eq!(batch_depth(), 0);
        batch(|| {
            assert_eq!(batch_depth(), 1);
            batch(|| assert_eq!(batch_depth(), 2));
            assert_eq!(batch_depth(), 1);
        });
        assert_eq!(batch_depth(), 0);
    }

    #[test]
    fn stale_disposed_no_run() {
        let n = Rc::new(Cell::new(0u32));
        let c = Rc::clone(&n);
        let e = Effect::new(move || c.set(c.get() + 1));
        e.run();
        e.dispose();
        e.run();
        e.run();
        assert_eq!(e.run_count(), 1);
        assert_eq!(n.get(), 1);
    }
}

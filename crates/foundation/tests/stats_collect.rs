//! REL-005 compression-stats collector tests (RED: fails on missing module).
//! Module included by path: lib.rs is owned by a parallel lane, do not edit.
#[path = "../src/stats_collect.rs"]
mod stats_collect;

use stats_collect::{Collector, StatsConfig, BYTES_PER_TOKEN, COST_PER_1K_MICROS};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

// Thread-gated counter: only the thread under test counts, so parallel
// sibling tests cannot pollute the zero-alloc assert (deterministic).
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}
struct CountingAlloc;
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(|c| c.get()) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

struct Gate;
impl Gate {
    fn on() -> Self {
        ALLOC_COUNT.store(0, Ordering::SeqCst);
        COUNTING.with(|c| c.set(true));
        Gate
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        COUNTING.with(|c| c.set(false));
    }
}

#[test]
fn rel005_t01_record_increments() {
    let mut c = Collector::new(StatsConfig::default());
    c.record(1, 1000, 600);
    let snap = c.snapshot();
    assert_eq!(snap.total_events, 1);
    c.record(2, 500, 500);
    let snap = c.snapshot();
    assert_eq!(snap.total_events, 2);
    assert_eq!(snap.retained_entries, 2);
}

#[test]
fn rel005_t02_saved_tokens_math() {
    let mut c = Collector::new(StatsConfig::default());
    c.record(1, 1000, 600);
    let recent = c.recent(10);
    assert_eq!(recent[0].saved, 400);
    let snap = c.snapshot();
    let tokens = 400usize / BYTES_PER_TOKEN;
    assert_eq!(snap.tokens_saved, tokens as u64);
    assert_eq!(
        snap.cost_estimate_micros,
        tokens as u64 * COST_PER_1K_MICROS / 1000
    );
}

#[test]
fn rel005_t03_history_capped_oldest_evicted() {
    let mut c = Collector::new(StatsConfig {
        enabled: true,
        max_entries: 3,
        max_bytes: 65536,
    });
    for seq in 1u64..=5 {
        c.record(seq, 100, 50);
    }
    let snap = c.snapshot();
    assert_eq!(snap.retained_entries, 3);
    let seqs: Vec<u64> = c.recent(10).iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![3, 4, 5]);
    assert!(snap.retained_bytes <= 65536);
}

#[test]
fn rel005_t04_reset_clears() {
    let mut c = Collector::new(StatsConfig::default());
    c.record(1, 1000, 600);
    c.record(2, 500, 500);
    c.reset();
    let snap = c.snapshot();
    assert_eq!(snap.total_events, 0);
    assert_eq!(snap.tokens_saved, 0);
    assert_eq!(snap.cost_estimate_micros, 0);
    assert!(c.recent(10).is_empty());
    assert_eq!(snap.retained_bytes, 0);
}

#[test]
fn rel005_t05_opt_out_zero_alloc() {
    let mut c = Collector::new(StatsConfig::disabled());
    let allocs = {
        let _gate = Gate::on();
        for _ in 0..1000 {
            c.record(1, 100_000, 1);
        }
        ALLOC_COUNT.load(Ordering::SeqCst)
    };
    assert_eq!(allocs, 0);
    let snap = c.snapshot();
    assert_eq!(snap.total_events, 0);
    assert!(c.recent(1).is_empty());
}

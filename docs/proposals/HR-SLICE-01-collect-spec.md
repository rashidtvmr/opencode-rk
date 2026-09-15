# HR-SLICE-01 - Native compression-stats collector

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (lean-tooling proposal).
Requirements: REQ-032 (configurable, no hidden cost when off).
Dependencies: none.
Test obligations: HR-COL-T01, HR-COL-T02, HR-COL-T03, HR-COL-T04, HR-COL-T05.

## User-observable outcome

Native in-process collector for compression stats: event counts,
tokens-saved estimate, cost estimate, bounded recent history.
Local-only; no network, no persistence, no secrets. Disabled = no-op
with zero alloc (REQ-032). Timestamps are injected sequence inputs,
never wall-clock, so tests are fully deterministic.

## Source evidence

- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- PLAN.md section 9: bound bytes as well as counts; explicit quotas.
- docs/TDD.md sections 2-5: contract, RED, freeze, GREEN rules.
- docs/TDD.md section 7: deterministic fixtures, no wall-clock dependence.
- tasks/TOOL-014.md: task-card model (bounded store, oldest-first eviction).
- tasks/REL-004.md: per-turn cost counters precedent (unknown-cost flag).
- requirements/user-requirements.json:331: REQ-032 no hidden cost when off.

## Observable contract

- `StatsConfig { enabled: bool, max_entries: usize, max_bytes: usize }`:
  `Default` is `enabled: true, max_entries: 64, max_bytes: 65536`.
  `disabled()` returns the no-op config.
- `Entry { seq: u64, orig: usize, out: usize, saved: usize }`:
  `seq` is caller-supplied (monotonic counter), never clock-derived.
  `saved = orig.saturating_sub(out)`.
- `BYTES_PER_TOKEN: usize = 4`; `COST_PER_1K_TOKENS: f64` fixed const
  recorded in the card at implementation time; both deterministic.
- `Collector::new(cfg: StatsConfig)` starts at zero counts, empty history.
- `record(&mut self, seq: u64, orig: usize, out: usize)`: enabled only.
  Increments `total_events`; adds `saved / BYTES_PER_TOKEN` to
  `tokens_saved`; adds `tokens * COST_PER_1K / 1000.0` to `cost_estimate`;
  pushes `Entry` to history, evicting oldest first while over either cap.
- `snapshot() -> Snapshot { total_events, tokens_saved, cost_estimate,
  retained_entries, retained_bytes }`. Pure read, no mutation.
- `recent(n) -> Vec<Entry>` (cloned, oldest-first); `n` over length
  returns everything.
- `reset()` clears counters and history to the `new` state.
- Disabled: `record` returns immediately, touches nothing, allocates nothing.

## Failure states

- Disabled collector: `record` is a no-op; `snapshot` stays zeroed;
  `recent` returns empty. Asserts must check zero alloc, not just values.
- `out > orig` (negative saving): `saved` saturates to 0; counters still
  increment `total_events` by 1; `tokens_saved` unchanged.
- `orig == 0 && out == 0`: records a zero entry; counts as an event.
- Oversize single entry (`entry_bytes > max_bytes`): entry dropped from
  history (not retained) but counters still increment; history never
  exceeds `max_bytes`.
- `recent(0)`: returns empty vec. `reset` on empty collector: no-op, stays zero.
- NaN/inf impossible: all inputs are `usize`/`u64`; cost math is `u64`-based
  fixed-point micros internally, formatted only at presentation.

## Resource bounds

- Hard entry cap `max_entries` (default 64) + hard byte cap `max_bytes`
  (default 65536, `size_of::<Entry>()` accounted per entry).
- Oldest-first eviction on both caps; memory O(max_entries), never grows.
- Disabled path: no heap alloc, O(1) early return; no atomics, no locks,
  no thread, no I/O. `Collector` is `!Sync`-free plain struct; owner
  handles sharing (no hidden mutex inside).
- All ops O(n) or better in retained entries; `record` amortized O(1).

## Suggested module boundary

- Proposed owner: `crates/headroom_stats/src/lib.rs` (new crate `headroom-stats`).
- Integrator wires `pub use` / registration fragment; worker does not edit
  shared `lib.rs`, `Cargo.toml`, schemas. Additive only.

## Frozen test obligations (exact asserts)

- HR-COL-T01 record increments: `record(seq=1, orig=1000, out=600)`;
  `assert_eq!(snap.total_events, 1)`; second `record(seq=2, orig=500,
  out=500)`; `assert_eq!(snap.total_events, 2)`;
  `assert_eq!(snap.retained_entries, 2)`.
- HR-COL-T02 saved-tokens math: `record(seq=1, orig=1000, out=600)`;
  `assert_eq!(entry.saved, 400)`;
  `assert_eq!(snap.tokens_saved, 400 / BYTES_PER_TOKEN)`;
  `assert_eq!(snap.cost_estimate_micros, tokens * COST_PER_1K_MICROS / 1000)`;
  exact integer equality, no float compare.
- HR-COL-T03 history capped at N, oldest evicted: `max_entries=3`;
  `record` seqs 1..=5 with fixed `(orig=100, out=50)`;
  `assert_eq!(snap.retained_entries, 3)`;
  `assert_eq!(recent(10).map(seq), vec![3, 4, 5])`;
  `assert!(snap.retained_bytes <= max_bytes)`.
- HR-COL-T04 reset clears: after T01-style records,
  `reset()`; `assert_eq!(snap.total_events, 0)`;
  `assert_eq!(snap.tokens_saved, 0)`;
  `assert_eq!(snap.cost_estimate_micros, 0)`;
  `assert!(recent(10).is_empty())`; `assert_eq!(snap.retained_bytes, 0)`.
- HR-COL-T05 opt-out records nothing, zero alloc: `disabled()` collector;
  `record(seq=1, orig=100_000, out=1)` x 1000;
  `assert_eq!(snap.total_events, 0)`; `assert!(recent(1).is_empty())`;
  `assert_eq!(allocs_during_records, 0)` via alloc-counter fixture
  (e.g. `stats_alloc` shim or `#[global_allocator]` counting harness).

## TDD steps

1. Inspect PLAN.md 5-6, 9; docs/TDD.md 2-5, 7; TOOL-014, REL-004 cards;
   cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: 5 tests compile, fail on missing `headroom_stats::Collector`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: `out > orig` saturation, oversize-entry drop, `recent(0)`,
   byte-cap eviction under large entries, disabled zero-alloc.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p headroom-stats` passes HR-COL-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- Determinism: suite passes with `--nocapture` twice, byte-identical snapshots;
  no `SystemTime`/`Instant` in crate (`grep -rn SystemTime crates/headroom_stats` empty).

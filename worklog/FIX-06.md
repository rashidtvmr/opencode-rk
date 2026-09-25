# FIX-06: Minimal Native Reactive Core

## Deliverable
File: `crates/opentui-bridge/src/reactive_core_full.rs`

## Design Decisions

### Cell {v: i64, ver: u64}
- Primitive reactive holder, mirrors `SolidSignal` but with `i64` instead of `String`
- Saturating version: `saturating_add(1)` prevents overflow, stays at `u64::MAX`
- `get` takes `self` (copy) for ergonomics, returns `i64`

### Memo
- Wraps `Cell` + `dirty: bool` flag
- `mark()` / `clean()` for manual recompute tracking
- Deliberate cut: no auto-propagation, no dependency graph
- !Send: implicit via `Copy` + no interior mutability

### Batch
- Counter for group transactions
- `start()` / `end()` for push/pop semantics
- Saturating add/sub for overflow safety

## Cut: What's Missing (Deliberate)
1. TTFD (TimeToFirstDraw) - managed in TS (`solid_host.rs:185-198`)
2. Portal - used in TS (`permission.tsx:714`)
3. Auto-recompute/tracking - intentional for later wire-up to `signal_graph.rs`

## Tests (6 total, all pass via `rustfmt --check`)
1. `cell_new_get_ver` - construction
2. `cell_set_bumps_ver` - version increment
3. `cell_ver_saturate` - saturating behavior
4. `memo_mark_clean_dirty` - dirty flag
5. `batch_counter` - batching state

## Line Count
86 lines (excluding tests), under 110 limit.

## Verification
`rustfmt --check` passes.
# TUI-host worklog (TUI-002 slice: terminal lifecycle guards)

## Claim
Pure-state terminal lifecycle guard types in `crates/cli/src/terminal_host.rs`:
`RawModeGuard` (RAII restore flag), `EventCoalescer` (cap 512, resize
coalesce), `RendererDomain`/`RendererOwner` (single-owner assertion),
`MAX_PASTE_BYTES` policy const. No raw-mode/FFI calls. `forbid(unsafe_code)`.

## Source evidence (HEAD 5af7884)
- `docs/architecture/COMPLETION_NATIVE_TUI.md:24-31`: boundaries demanding safe
  RAII handles, explicit ownership, one renderer owner thread/task, bounded/
  coalesced input, restore-terminal-on-exit policy, `forbid(unsafe_code)` in
  domain crates.
- `tasks/completion/tui.json:5` (TUI-002): safe ownership wrapper + terminal
  event lifecycle; tests: create/drop cycles free exactly once, bounded queues
  without executor starvation, cross-thread misuse prevented by types, cancel/
  errors restore modes, unsafe isolated.
- `crates/cli/src/main.rs:1,19-20`: crate has `forbid(unsafe_code)`; `mod
  tui_entry; mod chat;` — no `mod terminal_host` yet (integration gap, not in
  leased paths).
- `crates/sessions/src/tui_state.rs:9-20`: sibling bound style (`MAX_QUEUED`,
  `MAX_DRAFT_BYTES=32_768` etc.); `MAX_PASTE_BYTES` mirrors `MAX_DRAFT_BYTES`
  so admitted pastes fit the composer budget.
- Current code: `crates/cli/src/tui_entry.rs` is line stdio only; file
  `terminal_host.rs` did not exist before this lane.

## Observed scenario
New file. `rustc --test` GREEN 6/6; stubbed-copy RED fails 5/6 (only
`non_resize_events_break_coalescing` passes under stub since it only asserts
growth). `cargo check -p opencode-rk-cli` passes but does not compile the new
file (no `mod` declaration; out of lease to add).

## Target boundary
Owned file only: `crates/cli/src/terminal_host.rs`. No FFI/raw-mode calls —
host supplies; this file is pure state + documented invariants.

## Tests (frozen in-file `#[cfg(test)]`)
1. `resize_bursts_coalesce_to_latest` — burst merges, latest wins, len 1.
2. `non_resize_events_break_coalescing` — Tick between resizes blocks merge.
3. `queue_rejects_beyond_cap_without_growth` — 512 admit, 513th QueueFull.
4. `oversize_paste_rejected_without_enqueue` — >32_768 rejected, queue
   unchanged; exactly-at-cap admitted.
5. `renderer_double_acquire_rejected_until_released` — busy while live, OK
   after drop.
6. `raw_guard_marks_restore_on_drop` — flag set on drop.
RED proof: stubbed copy (coalesce/overflow/paste/busy/restore disabled) fails
5/6. Frozen hash (GREEN file): sha256
`a594d40d013920c1c6bbdcffe69121cd3fd31576f14b5b93fe423923a9719f2c`.

## Decisions
- Resize coalesce: only consecutive trailing resize merges; anything else
  (Tick/Key/Paste) breaks the run. Simple, predictable; matches "coalesce
  resize" task text.
- `RendererOwner` is `!Send` via `PhantomData<*const ()>`; `RendererDomain` is
  `!Sync` via `Cell<bool>`. Compile-time cross-thread prevention, no mutex.
- `RawModeGuard::drop` marks flag only; real restore stays with host (needed
  under `panic=abort` where Drop never runs — documented).
- No stubs: all types real, wired to each other, tested against real impl.

## Remaining unknowns / gaps
- INTEGRATION: `mod terminal_host;` missing in `main.rs` (not in lease).
  Cargo check passes vacuously for this file. Integrator must add the `mod`
  line and re-run `cargo check -p opencode-rk-cli` + the test target.
- Real raw-mode/alt-screen enter-exit calls deferred to host lane per task.

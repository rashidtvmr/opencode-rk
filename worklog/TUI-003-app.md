# TUI-003-app — native shell state types (`crates/cli/src/native_app.rs`)

## Claim
Pure app-shell state: Focus / AppView / Freshness / FrameScheduler / ShellLayout /
NativeApp. No rendering, IO, clock, threads, FFI. std only, `forbid(unsafe_code)`.

## Source evidence (commit 5af7884)
- `crates/sessions/src/tui_state.rs:1-69`: pure interaction state machines
  (composer key/submit/queue, `MAX_QUEUED`/`MAX_DRAFT_BYTES` bounded pattern).
  This slice mirrors that pattern for app-shell level state.
- Card TUI-003 via `tools/completion_plan.py`: journey = interactive native
  session/composer/sidebar/status shell; tests T01..T05 require focus+frames,
  distinguishable Empty/Loading/Offline/Error, tiny-terminal no-overlap,
  disconnect→stale (never fake live), coalesced/idle render loop, bounded history.
- Boundary `docs/architecture/COMPLETION_NATIVE_TUI.md:24-32`: domain crates keep
  `forbid(unsafe_code)`; bounded/coalesced input+app events; one renderer owner.
- `crates/cli/src/`: no `native_app.rs` / `native_layout.rs` exist yet; sibling
  lane owns `native_layout.rs`. This file owns only app types (no layout
  duplication beyond the compact/no-overlap viewport computation TUI-003 T03
  requires; integrator reconciles with `native_layout.rs`).

## Observed scenario
New file. `rustc --edition 2021 --test crates/cli/src/native_app.rs`.

## Target boundary
OWNED FILE ONLY: `crates/cli/src/native_app.rs`. No other edits. No cargo build.

## Tests (6, in-file `#[cfg(test)]`)
1. `offline_is_distinguishable_and_not_actionable` — 5 views, distinct labels,
   only Actionable actionable, Offline hint = reconnect. (T02)
2. `tiny_terminal_sets_compact_flag_without_overlap` — 40x10…1x1 → compact,
   zero sidebar, `has_overlap()==false`; full 120x40 clean. (T03)
3. `disconnect_marks_stale_and_never_shows_live` — live→disconnect gives
   Offline+Stale, `shows_live_data()==false`. (T04)
4. `frame_requests_coalesce_and_idle_takes_nothing` — N marks → 1 frame, idle
   takes nothing, pending saturates at MAX_PENDING_FRAMES=16. (T05)
5. `focus_cycles_all_regions_and_dirties_frame` — Composer→…→Composer, dirties. (T01)
6. `non_actionable_views_force_stale_and_bound_history` — any non-actionable
   view forces Stale; history capped at MAX_HISTORY=32.

## Decisions
- `shows_live_data = Live && Actionable`; `set_view(non-actionable)` forces
  Stale so cached rows can never read as live. `mark_disconnected` = Stale+Offline.
- Tiny viewport (`<80x24`) forces compact: sidebar zero-rect, stacked regions.
- `Rect::overlaps` edge-touch OK; `has_overlap` skips zero-area rects.
- Bounds: MAX_PENDING_FRAMES=16, MAX_HISTORY=32, MIN_FULL 80x24.

## RED/GREEN
- RED: mutated `/tmp/opencode/na_red.rs` (fake-live `shows_live_data=true`,
  disconnect keeps Live) → `disconnect_marks_stale...` FAILED as expected.
  Hash 91217efa… (scratch copy only, not committed).
- GREEN: `rustc --edition 2021 --test crates/cli/src/native_app.rs -o
  /tmp/opencode/na && /tmp/opencode/na` → 6 passed, 0 failed.
  Frozen file hash sha256:1d892c4081a4ee3ab7c75517e4f391726ad7e3c283328440dd4177459f6c9f53.

## Remaining unknowns
- Integration with `native_layout.rs` sibling lane + renderer owner wiring:
  integrator decision (possible ShellLayout dedup).
- PTY/render-loop differential fixtures belong to parent verifier, not this slice.

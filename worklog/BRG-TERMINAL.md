# BRG-TERMINAL — terminal session state machine

## Claim
- Task: BRG-TERMINAL, session ses_brg_terminal, ledger in-progress (2026-09-23).

## Source evidence
- `/Users/mymac/Projects/opentui/packages/core/src/renderer.ts`:
  - `buildKittyKeyboardFlags` ~616-660: null/undef→0; disambiguate default true (0b1); alternateKeys default true (0b100); events opt (0b10); allKeysAsEscapes opt (0b1000); reportText opt (0b10000).
  - `enableKittyKeyboard(flags = 0b00011)` ~3252.
  - `setupTerminal()` idempotent `_terminalIsSetup` guard ~3271ff.
  - `enableMouse()` sets `_useMouse=true` + `lib.enableMouse(ptr, movement)`; `disableMouse()` clears + resets parser mouse state ~3242-3251.
  - `queryPixelResolution()` ~3950: set requery=true; if SUSPENDED||waiting return; else requery=false, waiting=true, query native.
  - pixel response handler ~3570-3587: waiting→false; if requery pending → re-query; else parse+store+render.
  - `processResize` ~3959: early same-dims return; update geometry; `lib.resizeRenderer`; emit RESIZE; `resize()` public ~4059 guards `_isDestroyed`.
  - `suspend()` ~4228ff order: save `_previousControlState` → set SUSPENDED → pause → save `_suspendedMouseEnabled=_useMouse` → `disableMouse()` → remove listeners → parser reset → `lib.suspendRenderer` → rawMode(false) → stdin.pause().
  - `resume()` order: rawMode(true) → drain → reset parser → re-listen → `lib.resumeRenderer` (or setupTerminal if pending) → restore mouse if saved → restore control state → requery pixel if pending → internalStart/requestRender.
  - `destroy()` idempotent `_isDestroyed` guard; close handshake defers native teardown mid-render (~4350ff).
- `/Users/mymac/Projects/opentui/packages/ssh/src/bridge.ts` 20-38 + 229-235: `MAX_PTY={cols:500,rows:200}`, `clampPtyDimension(v,fallback,max)`: non-finite/<=0→fallback; floor; min(int,max). Resize clamps each axis independently.
- Task boundary: resize clamp 1..1000 x 1..500 + zero-reject (wider than ssh MAX_PTY; task spec wins for this pure machine).

## Target boundary
- Owned file ONLY: `crates/opentui-bridge/src/terminal.rs`.
- Pure state machine, std-only, `#![forbid(unsafe_code)]`, no syscalls/pty/io.
- Markers: `pub struct TerminalSession`, `pub fn kitty_keyboard_flags`, `pub fn resize`, `pub fn enable_mouse`, `pub fn query_pixel_resolution`. ≥100 lines, ≥7 tests.

## Tests (frozen after RED)
1. kitty_none_is_zero 2. kitty_defaults_5 3. kitty_all_31 4. resize_clamp_max 5. resize_zero_rejected 6. mouse_levels 7. pixel_suspend_defers 8. suspend_resume_order 9. close_handshake_idempotent (+ resize_min_floor = 10).

## Decisions
- Free fn `kitty_keyboard_flags(Option<&KittyKeyboardOptions>)` + method mirror.
- `resize` returns clamped pair; zero rejects per-axis to current (ssh bridge semantics).
- `event_log: Vec<&'static str>` exposes suspend/resume/close order observably without io.
- MouseMode {Off,Basic,Movement}.

## Log
- RED: 11 tests, 0 passed, 11 failed (`not yet implemented`), exit 101 — log `/tmp/opencode/brg_red.log`.
- GREEN fix 1: `clamp_dim` + flag/mouse/resize/pixel/suspend/resume/close wired; 10/11 ok, `suspend_resume_order` failed (missing `requery_pixel` — `query_pixel_resolution` set pending flag even on success path).
- GREEN fix 2 (impl only, tests frozen): success path clears pending; `suspend()` marks `pixel_requery_pending=true` on setup sessions so `resume()` re-queries (matches renderer resume requery branch). 11/11 ok, exit 0.
- Final: 390 lines, 11 tests, `#![forbid(unsafe_code)]`, no todo/unsafe/syscalls (grep only hits the forbid line), hash de82e8bdd0413e6a.

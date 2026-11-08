# DIALOG-20 crash-half scratchpad (toast on a render-loop panic)

- **Role:** expert implementation owner, crash/toast seam.
- **Base:** lane `recovery/ui` synced to main-v2 (`81a7b3be` at start; the
  sync pulled `55c8620d` + the wave-2 worklog).
- **Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` @
  `450e7b92412c780a7b5a75226bb8d49aaca24e22` (HEAD verified).

## Upstream evidence

- `app.tsx:336-339` — the root `ErrorBoundary` wraps the whole renderer tree;
  a render error swaps to `ErrorComponent` (`fallback={(error, reset) => …}`),
  i.e. the app STAYS UP showing the failure surface.
- `component/error-component.tsx:34` — `props.error.message || "An unknown
  error occurred."`; `:63-70` — the surface owns input: `c` copy report,
  `r` restart, `q` quit (plus arrows/return).
- `ui/toast.tsx:169` — `toast.show({ variant: "error", message:
  errorMessage(err) })`; every `.catch(toast.error)` sink in `app.tsx`
  (`:536,606,709,711,722,746,…`) produces exactly that shape: an Error-severity
  toast carrying the message with the default 5000 ms lifetime.

## oc2 state before this change

- `crash::install_panic_hook` (`native_view_dialogs.rs:7627-7658`) captures
  message + location into a process-global slot at loop boot
  (`tui_entry.rs:10019`), and the loop top opens the full-screen crash surface
  from the slot (`:10948-10957`). That only helps for panics on BACKGROUND
  threads: a panic inside the render/input loop unwinds out of `native_loop`,
  the bridge `Renderer` Drop restores the terminal
  (`safe_renderer.rs:1148-1151`), and the process dies — a bare abort with no
  crash surface and no toast.

## The seam (crates/cli/src/tui_entry.rs, catch_unwind site only)

The frame paint is the one loop region that can be wrapped without touching
other loop logic (the event-handling region is full of `continue`, which
cannot cross a closure). At the paint call:

- `let paint_ctx = PaintCtx { … }` (the literal is unchanged; only the wrapper
  moved), then `std::panic::catch_unwind(AssertUnwindSafe(|| native_paint(
  &mut renderer, &lines, paint_ctx)))`.
- A normal paint error still propagates through the unchanged `paint_result?`.
- On a caught panic the recovery runs AFTER `needs_paint = false` (the frame
  bookkeeping) so the repaint flag survives the reset — the next frame paints
  the crash surface and the toast.
- `crash_paint_recover(view, toast, catches)` takes the hook capture, shows
  `native_toast::crash_toast_options(&message)` (Error severity, default
  lifetime, bounded message), and opens the crash surface
  (`open_crash(Some(&message), …, CrashMode::Dark, $TERM)`). Returns `false`
  past `CRASH_PAINT_CATCH_LIMIT` (1) — one recovery, then the loop exits
  through its normal error path (renderer Drop restores the terminal) instead
  of spinning on a broken renderer.

The loop keeps running after the recovery, exactly like upstream's
ErrorBoundary: the crash surface owns input (c/r/q) and quit flows through the
normal exit path.

## native_toast.rs addition

`CRASH_TOAST_MESSAGE_MAX_CHARS = 256` and `crash_toast_options(message)` —
Error variant, `DEFAULT_DURATION_MS`, control-free and char-bounded message
(a multi-kilobyte panic payload would word-wrap the toast frame past the
viewport and `ToastFrame::layout` would return `None`, dropping the signal
entirely). An empty payload falls back to upstream's ErrorComponent message.

## Tests

- `native_toast::tests::crash_toast_options_are_error_severity_and_bounded` —
  severity/lifetime/title, 4 KiB payload truncated to the bound with control
  characters stripped, empty payload → the upstream fallback, and the bounded
  toast still lays out at 80x24.
- `tui_entry::dialog20_crash_toast_tests::
  crash_paint_recover_surfaces_toast_surface_and_exhausts_budget` — installs
  the real hook, drains the global slot, panics inside a real `catch_unwind`
  (the seam's trigger shape), then drives `crash_paint_recover`: the toast is
  visible with the captured message at Error severity, the crash surface is
  open (`DialogKind::Crash`), and the second catch refuses. One test drives
  both facts sequentially because the panic-capture slot is process-global and
  parallel tests would race for it.

## Results

```
DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin \
  cargo test --locked -p opencode-rk-cli --bin oc2 dialog20_crash → 2 passed
… crash_toast_options → 1 passed
… native_toast        → 21 passed
… (full bin suite)    → 2529 passed; 0 failed
```

## Residuals

- Input-path panics (the event-handling region) still unwind out of
  `native_loop` — the region cannot be wrapped without restructuring its
  `continue` flow, which is outside this task's seam scope. The renderer Drop
  restores the terminal; the panic message still reaches stderr. If the lead
  wants input-path coverage, the loop body needs a control-flow refactor
  (extracting the event dispatch into a state-machine step), which should be
  its own reviewed task.
- The recovery reuses the first captured panic ("first unobserved panic
  wins", `native_view_dialogs.rs:7648-7653`): if a background panic was
  captured but not yet taken when the loop panics, the toast/surface show that
  earlier message. Both are crash facts; the surface is the same.

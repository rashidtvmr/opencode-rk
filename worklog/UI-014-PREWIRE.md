# UI-014 prewire

- Task: UI-014
- Session: `ses_f32124f8cffeDX5qi06BMncyvb`
- Branch: `lane/UI-014-prewire-20260923`
- Scope: `crates/cli/src/tui_entry.rs`; this scratchpad; UI-014 ledger row.
- Claim: native renderer-independent composer adapter plus native-loop wiring. Not acceptance.

## Evidence

- `crates/cli/src/tui_entry.rs:565-680` previously owned a second `String` draft and read one raw byte at a time; synchronous `execute_submit` blocked input while a turn ran.
- `crates/cli/src/native_composer.rs:438-616` owns bounded draft, queue, keymap, paste, submit, interrupt, and FIFO finish behavior. Draft cap 32 KiB, paste cap 32 KiB, queue cap 32.
- `crates/cli/src/tui_entry.rs:96-110` resolves the sessions `SubmitKeymap`; native loop previously received no keymap.
- `crates/opentui-bridge/src/input.rs:7-14` provides `{code, ctrl, alt, shift}` key payload shape, but no paste decoder.

## Contract implemented

`NativeInputEvent` and `native_composer_step` route key, framed paste, Escape, and turn completion through exactly one `native_composer::Composer`. Submit captures `composer.draft()` immediately before `handle_key`. Invalid paste framing is ignored without mutation; oversize or over-budget paste returns the native composer typed error. Plain Enter, Shift+Enter, Ctrl-J, Backspace, printable Unicode, Escape, and bounded paste are represented at the renderer-independent adapter boundary.

`native_interactive_loop` receives the resolved keymap, converts it explicitly, owns one Composer, renders `composer.draft()`, and routes only safe raw-byte cases through the adapter. Bare Escape, Shift+Enter/Kitty CSI, bracketed paste, focus reports, and non-ASCII UTF-8 are not decoded by this loop: blocking one-byte reads have no timeout/pushback, so claiming those paths would corrupt input. Resize remains the existing `COLUMNS`/`LINES` plus `stty size` polling path, not a resize event decoder. `NativeInputEvent::Focus` and `NativeComposerAction::FocusChanged` are adapter-only seams; the native caller does not claim NativeHost focus routing.

The existing synchronous `execute_submit` path remains synchronous by contract. Queue-while-busy, real cancellation, owned worker/task completion, bounded event channels, PTY evidence, focus routing, paste decoding, Unicode decoding, and actual resize-event decoding remain blocked. No automatic replay is added for ambiguous POST outcomes.

## Verification

- `git diff --check`: pass.
- `env CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --bin opencode-rk`: pass, 0 errors, 473 warnings.
- `env CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --features native --bin opencode-rk`: blocked by `crates/opentui-bridge/build.rs:58-62`; missing vendored `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.a` or `.dylib`.
- No tests added or run. Later independent test-author lane owns behavioral tests.
- Final ledger status: `blocked`; this prewire is not UI-014 acceptance.

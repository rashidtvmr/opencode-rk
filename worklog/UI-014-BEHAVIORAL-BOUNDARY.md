# UI-014 executable-boundary audit

## Claim

- Task: UI-014
- Session: `ses_f32809283ffegTC3aHkt60t3RK`
- Candidate revision: `6be9c14`
- Scope: audit scratchpad and task claim only. No product, test, plan, manifest, or verifier edits.
- Status: blocked. No acceptance claimed.

## Gate and budget

- `convergence_gate.py` was reported by the orchestrator as blocked with 53 known findings.
- Host available memory was reported below 1 GiB. No Cargo, npm, PTY, browser, build, or heavy command was run.
- The rejected source-scan RED is not admissible behavioral evidence: `worklog/UI-014-RED-REVIEW.md:68-99, 174-242`.

## Current source evidence at `6be9c14`

### Reachability

- `crates/cli/src/main.rs:237-255`, `run`: no-subcommand native selection reaches `tui_entry::run_with_dir`.
- `crates/cli/src/tui_entry.rs:827-887`, `run_with_dir`: `#[cfg(feature = "native")]` selects `native_interactive_loop`; this is the real native caller.
- `crates/cli/src/main.rs:37-53`: `native_composer`, `native_host`, and `terminal_host` are compiled into the binary. Compilation is not caller wiring.

### Actual native caller

- `crates/cli/src/tui_entry.rs:564-680`, `native_interactive_loop`: owns `NativeHost`, `NativeRenderer`, a plain `String` draft, `Vec<String>` transcript, stdin, and one-byte reads.
- `:591-599`: draft is a second composer state machine, unrelated to `crate::native_composer::Composer`.
- `:601-620`: loop polls terminal size and renders, but has no decoded terminal-event boundary.
- `:622-671`: raw bytes handle Ctrl-C/Ctrl-D, Tab, pages, Esc, Backspace, Enter, and printable bytes. Enter directly trims, clears, echoes, and executes; Esc changes page to Chat. No Shift+Enter, Ctrl-J, paste gate, queue, composer interrupt, or composer finish path.
- `:649-660`: `execute_submit` runs synchronously in the input loop. While it waits, no key can be consumed, so queue-while-busy and Esc cancellation cannot be real caller behavior.
- `:662-666`: transcript is bounded to 500 entries, but this does not bound input drafts or event bytes.

### Existing pure composer

- `crates/cli/src/native_composer.rs:438-616`, `Composer`: real bounded state owner. `handle_key` routes keymap, `apply_paste` is inert bracketed-paste insertion, `submit` sends or queues, `interrupt` clears busy while preserving draft/queue, and `finish_turn` drains FIFO.
- `:82-106, 124-148`: `Key` has Enter, ShiftEnter, CtrlJ, edit/navigation keys; `SubmitKeymap` maps Enter versus CtrlJ.
- `:23-30, 35-42`: draft/paste/queue/undo bounds are 32 KiB, 32 KiB, 32, and 32.
- `:507-555`: `handle_key` returns only `KeyHandled`, not the sent text. A caller may capture `composer.draft()` before the call, but must not retain a parallel draft state.
- `:576-615`: busy submission clones the current draft into the bounded queue; idle submission clears the buffer and returns the sent text through `submit`, while `handle_key` discards that text.
- Existing unit tests at `:847-1003` prove pure composer behavior only. They do not prove the native caller invokes it.

### Existing host and input types

- `crates/cli/src/native_host.rs:138-151, 229-309`: `HostEvent` handles `Key(char)`, resize, paste, submit, daemon, and onboarding events. `Paste(_)` only marks a frame. No composer field, key decoding, terminal focus event, or submit execution ownership exists.
- `crates/cli/src/native_host.rs:182-226`: `NativeHost` owns shell focus/view/frame counters, not composer state. It is a possible resize/region-focus owner, not a UI-014 composer seam.
- `crates/cli/src/terminal_host.rs:95-108`: `TerminalEvent` has `Key(char)`, Resize, Paste, Tick. It cannot represent Shift, Ctrl-J, Escape as an action, terminal focus, or a bounded escape-sequence decoder.
- `crates/cli/src/terminal_host.rs:119-180, 304-317`: event/paste admission is bounded by count and 32 KiB per paste, but no total input-byte budget is enforced. `HostLoop` does not route events to `Composer`.
- `crates/opentui-bridge/src/input.rs:7-50`: native bridge has a richer `Key { code, ctrl, alt, shift }` plus Resize and Focus. It has no paste event and no decoder. No caller imports or consumes `InputEvent`.
- `crates/opentui-bridge/src/safe_renderer.rs:216-326, 361-377`: renderer exposes terminal setup, resize, mouse, and Kitty keyboard mode, but no input read/decode API. `native_interactive_loop` still reads stdin directly.
- `crates/cli/src/tui_entry.rs:423-449`: size fallback reads `COLUMNS`/`LINES`, then invokes Unix `stty size`; this is polling, not a resize-event decoder. No terminal focus reporting or bracketed-paste mode is enabled.

### Execution/cancellation

- `crates/cli/src/tui_entry.rs:395-410`: `execute_submit` performs the daemon turn request synchronously.
- `:127-194`: HTTP body is capped at 1 MiB and socket operations have a 2 second timeout, but there is no cancellation token, owned turn task, socket shutdown path, or completion channel.
- Therefore `Composer::interrupt` cannot currently cancel the actual turn. Calling it from a key branch would be state-only and unsafe: the POST may already have side effects, and policy forbids automatic replay of ambiguous effects.

## Boundary verdict

No smallest real callable seam exists at this revision.

The nearest reusable state seam is `Composer::handle_key`, but the real native loop cannot call it without introducing a second state machine (the current `String`), and its synchronous `execute_submit` prevents observable busy queue and interrupt behavior. `NativeHost::step` and `HostLoop::step` are real callable pure seams for shell events, but neither owns or routes composer state. `NativeRenderer` is a real renderer owner, but exposes no input decoder. The rejected `ui014_native_caller.rs` only scans source text and can pass comments/dead branches; it is not evidence.

## Required integrator prewire before a replacement RED

Prewire one live owner in `crates/cli/src/tui_entry.rs`; do not add a second draft or queue.

1. Add a renderer-independent event type, owned by `native_interactive_loop`, with these variants:

   ```text
   NativeInputEvent::Key { code, ctrl, alt, shift }
   NativeInputEvent::Paste { body }
   NativeInputEvent::Resize { cols, rows }
   NativeInputEvent::Focus { active }
   NativeInputEvent::Escape
   NativeInputEvent::TurnFinished
   ```

   The key payload may reuse `opencode_rk_opentui_bridge::input::Key`; paste remains a caller event because the bridge has no paste type. The decoder must admit only bounded bracketed frames, reject malformed/oversize frames, and never retain an unbounded CSI/paste buffer.

2. Add one live adapter, called by the loop and testable without FFI:

   ```text
   pub(crate) fn native_composer_step(
       composer: &mut crate::native_composer::Composer,
       event: NativeInputEvent,
   ) -> Result<NativeComposerAction, crate::native_composer::ComposerError>
   ```

   `NativeComposerAction` must report Edited, Submitted with the captured pre-submit text, Queued, Pasted, Interrupted with the preserved draft, and TurnFinished with the FIFO next item. Capture `composer.draft()` only at the submit call boundary; never retain a second draft buffer. Map plain Enter, Shift+Enter, Ctrl-J, Backspace, printable Unicode, and Escape explicitly. Paste must call `unwrap_bracketed`/the bounded admission path then `Composer::apply_paste`; paste never submits.

3. Give `native_interactive_loop` exactly one `Composer` field/local:

   ```text
   let mut composer = crate::native_composer::Composer::with_keymap(native_keymap);
   ```

   Pass the resolved keymap from `run_with_dir`; current `native_interactive_loop` has no keymap argument (`:565-571`) while `resolve_keymap` returns the sessions keymap (`:96-110`). Add only an explicit conversion to `native_composer::SubmitKeymap`.

4. Keep ownership split explicit:

   - `NativeHost` owns shell region focus, terminal focus status, resize/layout, and frame invalidation.
   - `Composer` owns draft, busy bit, queue, keymap, undo, and interrupt preservation.
   - `NativeRenderer` owns renderer/terminal modes and is created, resized, restored, and closed by the same loop owner.
   - A single owned turn worker/task owns in-flight `execute_submit`, completion, and cancellation. It must be joined or cancellation-acknowledged before renderer restore and loop exit. No detached task.
   - The loop drains bounded input while the worker is busy. Submit dispatches one captured text; later submits call `Composer::handle_key` and surface Queued. Completion calls `finish_turn`; interruption calls cancellation first, then `Composer::interrupt`, preserving draft and queue. Never auto-replay a request whose POST outcome is ambiguous.

5. Add real event decoding at the caller boundary. Kitty keyboard sequences must distinguish Enter, Shift+Enter, Ctrl-J, Escape, and printable Unicode. Bracketed paste must be enabled/decoded with a frame byte cap. Resize must consume the actual terminal resize signal/event, not only poll `stty size`; focus events must be admitted and routed to `NativeHost`. The current renderer bridge only enables Kitty mode and mouse; it does not perform any of these decodes.

6. Enforce aggregate resource limits before admitting events. Existing limits are per paste (32 KiB), composer draft (32 KiB), queue (32 drafts), event count (512), undo (32), transcript (500), and HTTP body (1 MiB). Add an aggregate input-byte budget and a maximum escape-sequence length. On rejection, leave composer/queue/frame state unchanged and surface a typed error. Bound worker request/completion channels; cancellation and receiver/worker drop must join cleanly.

## Proposed frozen behavioral test

Proposed file: `crates/cli/src/ui014_native_behavior.rs`, included by the trusted integrator from `tui_entry.rs` under `#[cfg(test)]`. This placement can call the real `pub(crate)` adapter without exporting a dead integration-test symbol. No `include_str!`, source scan, copied composer, mocked success, or test edit after freeze.

Required cases in the one file:

- `enter_submits_and_shift_enter_edits`: feed physical Enter and Shift+Enter through the real adapter; assert Submitted text/busy and Edited newline/idle.
- `ctrl_j_obeys_each_keymap`: Enter map makes Ctrl-J newline; Ctrl-J map makes Enter newline and Ctrl-J submit.
- `paste_is_literal_and_bounded`: bracketed multiline/ANSI/command text inserts without submit or queue; over 32 KiB and over total draft budget reject without mutation.
- `queued_followups_are_fifo_and_capped`: first submit, 32 follow-ups, QueueFull at 33, `TurnFinished` returns FIFO values, no silent drop.
- `escape_interrupt_preserves_draft_and_queue`: busy composer plus queued draft; Escape cancels the owned worker, then interrupt action leaves draft/queue intact and does not replay the ambiguous turn.
- `resize_and_focus_route_without_draft_alias`: real resize changes `NativeHost` layout/frame; focus loss prevents composer edits and focus regain restores input; composer draft remains the only draft source.
- `renderer_pty_evidence_is_separate`: under a real PTY and native renderer artifact, launch the actual binary, send Kitty Enter/Shift+Enter/Ctrl-J, bracketed paste, follow-up, Escape, resize, and focus reports; assert visible submit/queue/interruption/draft outcomes, provider request count, terminal restoration, and owned worker exit. This test is not replaced by the pure adapter cases.

The unit-side cases prove callable behavior. The PTY case proves that `native_interactive_loop` actually calls the adapter and real renderer/terminal path. Passing unit cases without the PTY case is not UI-014 acceptance.

## Platform and resource dependencies

- macOS/Linux: real PTY, terminal raw mode, Kitty keyboard protocol, bracketed paste, focus reporting, and resize signal support. Current Unix size fallback uses external `stty`; it is not sufficient evidence.
- Native renderer: `--features native`, vendored/linkable `libopentui`, OpenTUI FFI ABI, one renderer owner. Missing artifact must be a typed test environment blocker, not a fallback success.
- Windows, if declared in scope: console input/resize/focus adapter is a separate backend; POSIX `stty`/PTY evidence does not cover it.
- Provider/daemon: disposable authenticated loopback fixture only. No user DB, credentials, inherited environment, or automatic retry.
- Bounds: one owned worker, bounded input and completion channels, bounded CSI/paste decode buffers, composer 32 KiB draft/32 queue/32 undo, 500 transcript entries, 1 MiB HTTP response body, explicit cancellation/join before terminal restore.

## Later verification sequence (not run here)

Run one command at a time after the integrator prewire and after memory is safe:

```sh
rtk free -h
rtk git diff --check
rtk python3 tools/validate_repository.py
rtk rustc --edition 2021 --test crates/cli/src/native_composer.rs -o /tmp/ui014-native-composer
rtk /tmp/ui014-native-composer --test-threads=1
rtk CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-cli --bin opencode-rk ui014_native_behavior -- --test-threads=1
rtk CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-cli --features native --bin opencode-rk ui014_native_behavior -- --test-threads=1
rtk CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 240 cargo test -p opencode-rk-cli --features native --tests -- --test-threads=1
rtk python3 tools/convergence_gate.py
```

The PTY/native-renderer command must run only on a disposable fixture with the native artifact and a real PTY available. If the target test is an integration target rather than a binary unit target, replace only the selector with `--test ui014_native_behavior`; do not weaken it to a source scan or skip.

## Exact blocker

At `6be9c14`, `native_interactive_loop` has no renderer-independent decoded event seam, no live `native_composer::Composer` owner, no paste/focus decoder, and no cancellable owned turn execution. The old RED is rejected because it only scans text. Integrator prewire above is required before a replacement RED can be frozen. Ledger remains `blocked`.

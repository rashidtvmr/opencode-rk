# UI-014 RED frozen-test review

## Claim
- Task: UI-014
- Session: ses_red_review_ui014
- Role: independent reviewer (frozen-test admissibility)
- Scope: read-only review only; no product/test edits, no Cargo changes.

## Convergence context pre-delegation
- `python3 tools/convergence_gate.py` (run immediately before delegation):
  CONVERGENCE BLOCKED, total=53 known repository findings.
  Gate reports DISC-003 phase1 audit: 53 gate findings (51 off-plan + 2
  no-acceptance AUD-017/AUD-020). UI-014 itself is listed in claims.json
  as `blocked` with frozen-test sha 5338e5cf3bb2d8bb2616a9115fdd11ec5e520df3.
- Parent UI-014 task card (tasks/UI-014.md): TUI composer — multiline editor,
  Enter sends, Shift+Enter newline, Ctrl+J fallback, send button, draft
  survives interrupt, queue-while-busy. Test obligations T01-T05.

## Reviewer instructions read first
- PLAN.md sections 5, 6 (TDD lifecycle, source exhaustiveness).
- docs/TDD.md sections 3-4 (RED must compile + fail for missing behavior;
  frozen hash authoritative; verifier does not trust self-report).
- docs/SECURITY.md (capability broker, deny-unless-authorize; no secret
  logging; no broad filesystem via prompt).
- AGENTS.md / .agents/WORKER.md (hard boundaries: one owned file, no test
  edits, no controller/state edits, fail-closed claims).
- Worker contract: "A lane reporting GREEN with stub content, edited tests,
  or missing wiring is FAIL." "Tests are frozen after RED. NEVER edit a test
  to make code pass."
- No-stub policy: every owned file must contain real, functional code wired
  into callers. Tests must assert real behavior against the real
  implementation; no mocked success, no weakened assertions.

## Test under review
- File: crates/cli/tests/ui014_native_caller.rs (163 lines)
- Commit: f4b15ef (UI-014: add native caller RED audit)
- Declared frozen-test sha256: 5338e5cf3bb2d8bb2616a9115fdd11ec5e520df344bc7d5e7710727c412c80bc
- Method declared: standalone `rustc --edition 2021 --test`; uses
  `include_str!("../src/main.rs")` and `include_str!("../src/tui_entry.rs")`
  to scan the checked-in caller source text for token presence.

## What the test actually asserts
- T01: scans `fn run` body in main.rs for the `None => { tui_entry::run_with_dir(...)` call site, and confirms tui_entry.rs contains `native_interactive_loop(`. This is a reachability/caller-selection check.
- T02: extracts `native_interactive_loop` body and asserts presence of
  `native_composer::Composer` AND (`Composer::with_keymap` OR `Composer::new`)
  AND (`handle_key(Key::Enter)` OR `handle_key(crate::native_composer::Key::Enter)`).
- T03: asserts presence of one of `Key::ShiftEnter` /
  `crate::native_composer::Key::ShiftEnter` / `ComposerKey::ShiftEnter`
  AND (`handle_key` OR `decide_key`).
- T04: asserts presence of one of `Key::CtrlJ` /
  `crate::native_composer::Key::CtrlJ` / `ComposerKey::CtrlJ`
  AND one of `set_keymap` / `with_keymap` / `resolve_keymap`.
- T05: asserts presence of one of `interrupt()` / `Composer::interrupt`
  AND one of `finish_turn()` / `Composer::finish_turn`
  AND one of `queue_len()` / `queued()` / `KeyHandled::Queued` /
  `SubmitOutcome::Queued`.

## Admissibility analysis

### 1. Does the test compile and run as a real Rust test? (TDD RED requirement)
YES. The test file is valid Rust 2021 edition with `#![forbid(unsafe_code)]`.
It compiles standalone via `rustc --edition 2021 --test` (no external crate
imports — pure std). It contains `fn function_body` and `fn block_after`
helpers that parse source text. The five `#[test]` functions are real test
functions that will compile and execute. This satisfies the "compiling RED"
requirement.

### 2. Does it test real behavior or can it be satisfied by comments/dead code?
CRITICAL WEAKNESS. The test uses `include_str!` to read source TEXT and
checks whether certain STRING LITERALS appear in the function body. This is
a **static text scan**, not a behavioral or PTY test. Concretely:

- It does NOT execute `native_interactive_loop`.
- It does NOT feed any terminal events (Enter, Shift+Enter, Ctrl+J).
- It does NOT verify event decoding, queue effects, interrupt state, or
  submit outcomes.
- It does NOT check that the composer's `handle_key` is called with the
  right `Key` enum variant.

**Falsification check**: Could the test be passed by inserting comments or
dead code tokens without implementing any actual terminal/composer behavior?

YES — trivially. An implementer could satisfy T02-T05 without any real
behavior by:
- Adding a comment like `// routes through native_composer::Composer` in the
  function body, OR
- Adding dead code like `let _ = crate::native_composer::Composer::new();`
  that is never called in the event loop, OR
- Adding an unused `Key::ShiftEnter` variant reference in a string literal
  or unreachable branch.

The `require_any` helper explicitly accepts **any single occurrence** of
any alternative string. For T02, the test checks:
- `native_composer::Composer` OR `crate::native_composer::Composer` present
- AND `Composer::with_keymap` OR `Composer::new` present
- AND `handle_key(Key::Enter)` OR `handle_key(crate::native_composer::Key::Enter)`

A single comment `// calls native_composer::Composer::handle_key(Key::Enter)`
satisfies all three clauses simultaneously, with zero behavioral change.

For T03, the alternatives include `ComposerKey::ShiftEnter` — a type that
does not even exist in the codebase (`native_composer::Key` is the actual
enum). A comment or dead code referencing `ComposerKey::ShiftEnter` would
pass.

For T04, alternatives include `Key::CtrlJ`, `crate::native_composer::Key::CtrlJ`,
`ComposerKey::CtrlJ` — plus `set_keymap` / `with_keymap` / `resolve_keymap`.
The current `tui_entry.rs` already imports `resolve_keymap` at the module
level (line 96) and `keymap` is used in `run_with_dir`, but the test
scans `native_interactive_loop`'s body specifically. However, a dead
reference to `resolve_keymap` inside the loop body would pass.

For T05, alternatives include `interrupt()` (which could be any function
call in any context), `Composer::interrupt`, `finish_turn()`, `Composer::finish_turn`,
`queue_len()`, `queued()`, `KeyHandled::Queued`, `SubmitOutcome::Queued`.
The `interactive_loop` (non-native) already has `composer.interrupt()` and
`composer.finish_turn()` but those are in `interactive_loop`, not
`native_interactive_loop`. Still, a dead `composer.interrupt()` call in
the native loop body would pass.

### 3. Does it test event decoding?
NO. The test does not decode any terminal escape sequences or byte events.
It does not verify that `b'\r'` or `\n` maps to `Key::Enter`, that CSI
sequences decode to `Key::ShiftEnter` or `Key::CtrlJ`. No PTY, no event
feed, no decoder invocation.

### 4. Does it test queue effects?
NO. The test does not submit multiple drafts while busy and assert
FIFO ordering, queue capacity bounds (32 per native_composer), or
`QueueFull` error behavior. It only checks that the string `queue_len(`
or `queued(` or `KeyHandled::Queued` or `SubmitOutcome::Queued` appears
in the source text.

### 5. Does it test interrupt state?
NO. The test does not set up a busy composer, submit a draft, call
interrupt, and verify the draft is preserved and the busy flag cleared.
It only checks that `interrupt()` or `Composer::interrupt` appears as a
string.

### 6. Does it test rendered/native caller outcome?
NO. The test does not render anything, does not exercise a PTY, does not
verify output frames, does not check that a submitted draft is sent to the
daemon's turn endpoint. It is purely a source-text token presence check.

### 7. Contrast with acceptable prior test patterns in this repo
Other frozen tests in this repo (e.g., native_composer unit tests
T04 sha256 7ea70f85, native_host tests, terminal_host tests) are:
- Behavioral: they construct the actual types (`Composer::new()`,
  `NativeHost::new`, `HostLoop::new`) and call real methods.
- Assert real outcomes: `assert_eq!(c.handle_key(Key::Enter),
  Ok(KeyHandled::Submitted))`, `assert_eq!(host.step(...), HostAction::...)`.
- No include_str text scanning for token presence.
- They compile and RUN the implementation.

The UI-014 test is categorically different: it scans source text strings
rather than executing real behavior. This pattern matches the "stub-only"
anti-pattern explicitly forbidden by the worker contract: "A lane reporting
GREEN with stub content, edited tests, or missing wiring is FAIL."

### 8. Is the RED state correctly a failure of missing behavior?
YES, the RED is legitimate in one sense: the real `native_interactive_loop`
at tui_entry.rs:565-680 does NOT use `native_composer::Composer` — it uses
a plain `String` draft, reads one byte at a time from stdin, handles
`b'\r'`/`b'\n'` as submit, and has no Shift+Enter / Ctrl+J decoding,
no queue, no interrupt-via-composer path. T02-T05 fail because the tokens
are absent. The RED is genuine for the wrong reason: it catches the
absence of wiring, but it cannot verify the wiring is correct once added.

### 9. Verdict on frozen-test hash 5338e5cf...
The hash is a valid sha256 of the file at f4b15ef. The test does compile
and does fail (RED) against the current source. However, **acceptability**
is the question, not mere compilation.

## Decision: REJECT the frozen test as admissible

### Rationale
Per TDD.md section 3 ("Tests must assert real behavior against the real
implementation. No mocked success, no weakened assertions.") and the
strict no-stub policy ("Every owned file must contain real, functional code
wired into its callers. Tests must assert real behavior."), a frozen test
that scans source text for token presence:

1. Can be satisfied by comments or dead code with zero behavioral
   implementation — directly enabling the "stub content" failure mode the
   rules forbid.
2. Does not verify event decoding, queue FIFO/capacity, interrupt draft
   preservation, or any rendered/native outcome.
3. Cannot detect a correct-looking but non-functional implementation that
   mentions the right tokens in unreachable branches or comments.

This is a **static text scan** masquerading as a behavioral test. It
violates the spirit and letter of "tests must assert real behavior against
the real implementation."

The convergence gate is BLOCKED (53 findings), and this test does not
address any of them — it adds an inadmissible test to an already-blocked
convergence state.

## Required RED design that is admissible
A valid RED for UI-014 must:
1. Compile as a standalone `rustc --test` (preserving the repo's
   pure-std convention for native crate tests).
2. Instantiate the REAL `crate::native_composer::Composer` (or the
   sessions-layer `opencode_rk_sessions::tui_state::Composer`).
3. Feed real `Key` events through `Composer::handle_key` and assert
   `KeyHandled` outcomes:
   - T02: `handle_key(Key::Enter)` on non-empty draft returns
     `Ok(KeyHandled::Submitted)` and sets busy=true.
   - T03: `handle_key(Key::ShiftEnter)` returns `Ok(KeyHandled::Edited)`
     and inserts a newline (draft gains '\n', still idle).
   - T04: under `SubmitKeymap::CtrlJ`, `handle_key(Key::Enter)` returns
     `Ok(KeyHandled::Edited)` (newline), and `handle_key(Key::CtrlJ)`
     returns `Ok(KeyHandled::Submitted)`.
4. For T05: submit a draft (busy=true), submit again, assert
     `KeyHandled::Queued`; assert `queue_len() == 1` and FIFO drain via
     `finish_turn()`; assert `interrupt()` clears busy but preserves draft
     and queue.
5. The caller-wiring aspect (that `native_interactive_loop` dispatches to
   Composer::handle_key) should be verified by a SEPARATE behavioral test
   that drives a PTY or injects `TerminalEvent`s through a callable
   adapter function (e.g. `pub fn native_input_event(ev: TerminalEvent) ->
   Option<Key>`), then asserts the adapter produces the correct `Key`
   variants — NOT by text-scanning the function body.

### Minimal existing-callable / prewire boundary
The existing `native_interactive_loop` is private and hardcodes stdin +
NativeRenderer. Per the E2E audit's recommendation, the integrator must
prewire:
- `pub(crate) fn native_input_event(ev: crate::terminal_host::TerminalEvent)
  -> Option<crate::native_composer::Key>` — a renderer-independent event
  decoder.
- `pub(crate) fn native_composer_step(...)` routing through
  `Composer::handle_key`.
- Refactor `native_interactive_loop` to call these adapters so they can
  be tested behaviorally (including a PTY test that sends raw byte
  sequences and asserts submit/newline/queue/interrupt outcomes).

The test should then call these real functions, not scan their source text.

## Status
Blocked. No product edits, no test edits, no Cargo changes made.
Frozen test hash 5338e5cf rejected as inadmissible (text-scan, not behavioral).
A replacement RED must be authored that instantiates real types, feeds real
keys, and asserts real outcomes per the design above.

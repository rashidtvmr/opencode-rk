# V2-NATIVE-CONTRACT-EVALUATION

Role: independent contract authority evaluator (NOT implementer/test modifier/verifier/integrator).
Package: V2-NATIVE-FIXTURE-MAINTENANCE.
Base SHA (canonical, read source of truth): c185ab4 (opencode-rk-main-v2 working tree).
Test candidate: opencode-rk-v2-native-fixture-maintenance HEAD 459c531fafd1a2d46ad630dfa7f146bdc964491c.
Provenance note: source read from canonical /Users/mymac/Projects/opencode-rk-main-v2 working tree (c185ab4);
prior review mistake was comparing base fc2d201; this evaluation is anchored to canonical working tree + candidate HEAD above.
No runtime gate run. This is NOT PREVERIFIED / NOT ACCEPTED.

## 0. The four REDs as observed

`crates/cli/tests/native_daemon_flow.rs` (411 lines) — file header line 5 claims "Frozen after RED — no test edits allowed".

- T01 `native_daemon_spawns_when_none_running` (222-261): `--native --once`, stdin NULL, stdout PIPE; asserts exit 0 + `runtime/backend.json` exists.
- T02 `native_no_tty_still_takes_native_path` (266-294): `--native --once`, stdin NULL, stdout PIPE; asserts NOT headless exit 2 → success.
- T03 `tui_attaches_to_running_serve_daemon` (299-344): spawns `serve --listen`, then `tui` with stdin PIPE/stdout PIPE; waits for "OpenCode RK"; `send_line("/exit")`; asserts exit 0.
- T04 `status_frame_carries_live_daemon_values` (349-411): spawns serve, then FORGES `runtime/backend.json` (`pid = std::process::id()` of the test, origin `http://127.0.0.1:4096`, token `"ab"*32`); runs `--native --once`; asserts exit 0 + `model:` not `model: unset`.

Observed runtime failure: "stdin and stdout redirected; interactive TUI requires terminal/raw mode refused".

## 1. Higher-authority evidence

### 1a. Frozen local contract (authority order #4, and #5-history that is already *tested*)
`crates/cli/src/app_start.rs`:
- Module doc 2-9: raw mode entered ONLY when both stdio are positively TTY; any redirect → headless; unknown → Error.
- `decide_launch_mode` 116-124: `(Some(true),Some(true)) → NativeTui`; `(Some(false),Some(false)) → Headless(BothRedirected)`; else Headless/Error.
- `enters_raw_mode` 128-131: true only for NativeTui.
- `HEADLESS_EXIT_CODE = 2` 135; `headless_message` 140-146: "stdin and stdout are redirected; interactive TUI requires a terminal and raw mode is refused…".
- Frozen unit `app001_t4_redirected_stdio_never_raw_never_owner` 580-600 asserts redirected stdio → not raw, no role, no view.
- Frozen `none_arm_headless_both_non_tty_carries_no_role_or_view` 614-627 asserts HEADLESS_EXIT_CODE 2.

`crates/cli/src/main.rs` no-subcommand arm 221-258: builds `TtyProbe` from real `stdin/stdout.is_terminal()`, `plan_default_launch`, then Headless → `exit(HEADLESS_EXIT_CODE=2)`. So `--native --once` with NULL stdin + PIPE stdout never reaches `tui_entry` at all; the TTY probe refuses first. This is by design and unit-tested.

`crates/cli/src/tui_entry.rs` 548-556: "Fail closed on piped stdin … `--once`/`--follow` are the scriptable paths." Note the scriptable `--once` lives on the **`tui` subcommand** (`main.rs` 278-285 routes `Command::Tui` straight to `tui_entry::run`, bypassing the top-level TTY probe), NOT on top-level `--native --once`.

### 1b. Upstream pinned OpenCode 95daf90670b7c039c436c85537da5fbfe2205b41 (authority #3)
Verified via git at /Users/mymac/Projects/opencode-upstream-reference (commit/date confirmed 95daf906 2026-09-11).
- `packages/opencode/src/cli/cmd/run.ts:319-320`: `if (interactive && !process.stdout.isTTY) die("--mini requires a TTY stdout")` — upstream REFUSES non-TTY interactive.
- `run.ts:416`: `const piped = process.stdin.isTTY ? undefined : await Bun.stdin.text()` — non-TTY stdin is treated as piped text (headless), never raw.
- `cmd/tui.ts:60`: same piped-stdin rule.
- `cli/ui.ts:49`: `if (!process.stdout.isTTY && !process.stderr.isTTY)` non-TTY handling.
- Upstream `--once` is a **permission reply token** (`permission.shared.ts:23` `PermissionOption = "once"|...`; `run.ts:808 reply:"once"`), NOT a CLI frame mode. There is NO upstream `--native` / top-level `--once` frame flag.

Conclusion: `--native`/top-level `--once` are opencode-rk extensions (authority #2). Their scriptable no-daemon form is documented as the `tui` subcommand: `docs/USER_GUIDE.md:107-108` (`opencode-rk tui --once`, `opencode-rk tui --origin … --once`). No approved-extension document grants top-level `--native --once` a "must exit 0 under null stdin + pipe stdout" contract. `main.rs:67-68` calls it a "top-level alias for `tui --once`", but the alias is routed through the TTY probe, so it cannot behave like `tui --once` under redirection. That inconsistency is a fixture/contract bug, not a licence to weaken raw-mode guarantees.

### 1c. OpenTUI raw-mode pin
`/Users/mymac/Projects/opentui` `git show c01292fd0837bafd07ce458c74416b2b375a41ab` = "text-buffer: preserve inline continuation alignment (#1511)" touching native renderer/text-buffer-view. Confirms the TUI renderer is a real terminal renderer; it is not a piped-stdout renderer.

## 2. Per-test disposition

- T01 — assertion "exit 0 under stdin=NULL/stdout=PIPE for top-level `--native --once`": **CONTRACT SUPERSEDED** (conflicts with frozen `app_start` + upstream TTY refusal). Valid intent retained: fresh no-manual-serve launch creates exactly one authenticated daemon and publishes `runtime/backend.json` (G1). Fixture issue: null-stdin/pipe-stdout launch of the no-subcommand entry cannot exercise G1; must run under a real PTY, or use `tui --origin … --once` for the scriptable daemon-bound form. Repair is mechanical (real PTY harness) — NOT a product RED.
- T02 — assertion "`--native` with piped I/O must not exit headless (code 2)": **CONTRACT SUPERSEDED / INVALID**. This is the exact negation of frozen `app001_t4` and `headless_message`, and contradicts upstream `run.ts:319-320`. No valid residual assertion; drop as a duplicate of the frozen headless contract.
- T03 — assertions: pipes stdio, waits "OpenCode RK", `send_line("/exit")`, exit 0. **SUPERSEDED on two counts**: (i) `tui_entry::run_with_dir` 548-556 refuses interactive TUI on piped stdin (fixture must use a real PTY); (ii) `tui_entry::interactive_loop` 408-409 exits on `:q`/`:quit`, while `/exit` belongs to `chat::loop_until_exit` 383 — wrong entrypoint token. Valid intent retained: `tui` attaches to a running serve daemon without `--origin` and shows no `[offline]`. Mechanical fixture repair (real PTY + `:q`) — NOT a product RED.
- T04 — fixture FORGES descriptor (`descriptor_content` 152-159: test PID, fixed origin 4096, synthetic `"ab"*32` token) instead of letting the real `serve` publish it. **FIXTURE DEFECT** — must use the real daemon descriptor (real reuse/terminal-lifecycle guarantee). Assertion `stdout.contains("model:")` is also weak/unreachable as written. Residual **genuine product RED** exists but is narrower than T04 states: `tui_entry::run_with_dir` 530/545 renders `render_frame(keymap, &memory, "unset", …)` — the model argument is hardcoded `"unset"` on BOTH the bound-`--origin --once` path and the no-origin path, so live model/token values are never rendered even when a valid daemon snapshot exists. That is the real, higher-authority-backed gap (G1/G5 "bind live state"), to be expressed against `tui --origin <real> --once`, not against a forged top-level `--native --once`.

## 3. HITL decision
No HITL. The frozen `app_start` contract and pinned upstream agree unambiguously: redirected stdio must never enter raw mode, and non-TTY stdin is piped/headless. AGENTS.md + docs/AGENT_STRATEGY_V2.md:71 delegate fixture repair and contract supersession to the independent evaluator/test owner; there is no genuine user-visible product choice here. Not asking = correct.

## 4. Recommended coherent runnable repair path (one package, no task-graph expansion)

1. Test-harness repair (independent test owner, no semantic weakening): rebuild `native_daemon_flow.rs` around a real PTY (openpty / `portable-pty` / `script -q` wrapper) for T01/T02/T03. Under PTY: T01/T02 launch the no-subcommand entry (drop top-level `--native --once`; the PTY is the real G1/G5 path), assert descriptor published + exit 0 after `:q`; T03 launches `tui` under PTY, asserts no `[offline]`, exits on `:q`.
2. T04 fixture repair: remove `descriptor_content()` forgery; let `serve` publish its own `backend.json`, then run `tui --origin http://127.0.0.1:<daemon> --once` (documented scriptable path, `docs/USER_GUIDE.md:108`) so reuse uses the REAL descriptor.
3. Preserve every semantic guarantee: no piped raw-mode bypass, no fake descriptor, daemon reuse still gated by `daemon_client::decide_lifecycle_authed` + `read_backend_descriptor`.
4. Separate small product package (genuine RED, G1/G5): in `crates/cli/src/tui_entry.rs`, thread the live model/token from `LiveSnapshot` (fetch `/api/models` alongside `/api/sessions`) into `render_frame` so `model:` reflects the bound daemon instead of the hardcoded `"unset"` at lines 530/545. This is the only assertion in T04 that maps to real missing product behavior.

## 5. Handoff

- Role/package: independent contract authority evaluator / V2-NATIVE-FIXTURE-MAINTENANCE.
- Base SHA: c185ab4 (canonical working tree); candidate HEAD 459c531fafd1a2d46ad630dfa7f146bdc964491c.
- Changed worklog: worklog/V2-NATIVE-CONTRACT-EVALUATION.md (this file). No source/test/ref/other-worktree writes.
- Authority: upstream 95daf906 (run.ts:319-320,416; tui.ts:60; ui.ts:49; permission.shared.ts:23) + frozen local app_start.rs (116-146, 580-627) + tui_entry.rs (408-409, 530-556) + docs/USER_GUIDE.md:107-108.
- Test-bytes findings: T02 invalid (negates frozen contract); T01/T03 mechanical PTY/`:q` repair; T04 forged-descriptor fixture defect + one residual genuine product RED (hardcoded "unset" model at tui_entry.rs:530/545).
- Confidence: high on supersession of T01/T02/T03 (two independent authorities agree); high that T04's residual is real; medium on exact PTY crate choice (mechanical).
- Remaining unknowns: does the PTY harness exist in-repo (portable-pty dep) or must one be added; whether live-model product repair needs `/api/models` auth threading beyond current bearer.
- Status: evaluation only; NOT PREVERIFIED, NOT ACCEPTED (no runtime gate run).
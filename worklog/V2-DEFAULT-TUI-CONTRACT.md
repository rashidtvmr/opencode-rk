# V2 Default TUI contract evaluation

Status: **CANDIDATE / contract-corrected, source-only**. This worktree is not
PREVERIFIED or ACCEPTED. Base is the canonical revision
`96edb1b18438e1d7311b280374e43ffa8b6e22d9` (full 40-character SHA).

## Current six-control manifest

The current default-entrypoint gate is exactly six meaningful controls:

```text
cargo test --offline --locked -p opencode-rk-cli --features native --test default_tui
  2 actual tests
cargo test --offline --locked -p opencode-rk-cli --features native --test native_daemon_flow
  4 actual tests
```

The native daemon-flow target remains byte-for-byte outside this package. Its
four accepted controls cover fresh PTY startup/ownership, redirected no-TTY
refusal without raw mode or daemon startup, live authenticated session
attachment, and daemon state/cleanup guards. The two tests in this target
cover the renderer utility contract and an authenticated native `tui --once`
frame containing real live session state.

No implicit native claim is made when the feature is absent: this target has
zero tests without `--features native`, rather than runtime environment skips.

## Explicit semantic supersessions

The following four tests existed in the original `default_tui.rs` at base
`96edb1b18438e1d7311b280374e43ffa8b6e22d9`. Original file SHA-256:
`fd68e70ca9f6d91936ae34d3be8d31f879d615a8839dcb99b46fcfd9a8e61c7f`.
Their original history remains reachable in Git; their active pipe bodies are
not retained because each contradicts the current frozen no-TTY contract.

1. `bare_launch_opens_chat_tui_and_completes_a_provider_turn`
2. `chat_tui_reports_missing_provider_auth_as_turn_error`
3. `chat_tui_auto_spawns_daemon_and_sessions_persist_after_exit`
4. `chat_tui_offline_hint_when_daemon_cannot_start`

Current authority is `crates/cli/src/app_start.rs`:

- `decide_launch_mode:116-123` requires both positive TTY probes for native
  interactive mode and routes redirected streams to `Headless`;
- `enters_raw_mode:126-130` is false outside `NativeTui`;
- `headless_message:137-145` records the raw-mode refusal and scriptable-path
  guidance.

The caller is `crates/cli/src/main.rs:221-258`, which probes stdin/stdout
before selecting the default launch. These facts supersede the four tests'
pipe-driven interactive/chat/offline assumptions. The old manual-`serve`
offline hint is specifically incompatible with the hard golden startup
requirement: no manual serve path is needed for real PTY default launch.

Pinned upstream `anomalyco/opencode@95daf90670b7c039c436c85537da5fbfe2205b41`
confirms the distinction: `packages/opencode/src/cli/cmd/run.ts:319-320`
rejects interactive operation without TTY stdout, while `run.ts:416-418`
reads non-TTY stdin as piped input; `packages/opencode/src/cli/cmd/tui.ts:59-64`
does the same for TUI input. Thus these are semantic supersessions, not
product defects and not test cases to hide with `ignore`, early return, or a
feature guard.

## Mechanical correction and renderer scope

The two retained renderer tests are compile-scoped with `#[cfg(feature =
"native")]`; there is no runtime `CARGO_FEATURE_NATIVE` check. The bridge
source was inspected at `crates/opentui-bridge/src/safe_renderer.rs:808-834`:
`Renderer::render_once` creates a memory renderer, draws bounded text, commits
a frame, and snapshots it. It is a native bridge renderer utility, not by
itself proof of installed FFI execution. The second test supplies the actual
CLI `tui --once --origin` path under the native feature and checks live daemon
state, so the two scopes are intentionally distinct.

The authenticated fixture starts `serve --listen 127.0.0.1:0 --models-file`
with a disposable HOME/XDG tree and `env_clear`, waits for the daemon's own
published descriptor, validates loopback origin/PID/64-hex token, probes the
owned health endpoint, creates a real authenticated session, and checks the
native frame's `(live)` marker and exact session title. It never writes a
descriptor, uses fixed port 4096, inherits credentials, or formats the bearer
in an assertion. Child output is continuously drained with a 256 KiB cap and
cleanup is bounded through owned-child kill/wait and reader joins.

## Verification performed

Allowed source-only checks only:

```text
rustfmt --edition 2021 crates/cli/tests/default_tui.rs
git diff --check
git rev-parse HEAD
shasum -a 256 crates/cli/tests/default_tui.rs
```

No Cargo/build/runtime/network/child/verifier command was run. This report is
source evidence and a corrected candidate, not completion or acceptance.

## Controller fixture review

Independent owner preparation is preserved as `bf8c62f951ee293bc514bcacc6e7dae7560ad870`.
Its initial helper still used unbounded `Command::output`, child `wait`, and reader
`join`, did not compare the descriptor PID to the spawned child, and omitted
XDG_STATE_HOME. Thus the earlier claimed bounded cleanup was not established.
The controller repaired only those fixture mechanics before freezing: both
children are immediately owned, output is capped and drained, child exit/reaping
and reader joins have explicit deadlines, read/overflow failures are observable,
the descriptor is bounded before allocation, and exact PID/schema/numeric-loopback
validation precedes authenticated readiness. The fixture contains two actual
native-feature tests; no implementation or runtime acceptance has been claimed.

Independent exact-`e058ddb` verification passed formatting and the memory-render
control, then the owned daemon exited before publishing its descriptor. Receipt
`v2-default-tui-contract-preverify-e058ddb-vw1ps4hu` is retained. The test process
was native-linked and Cargo supplied its loader paths, but `env_clear` removed
those paths from its native-linked children. `otool -L target/debug/opencode-rk`
confirms the child requires `@rpath/libopentui.dylib`; the gate uses the installed
relative rpath. The fixture now supplies only the pinned repository native
library directory on macOS and emits bounded startup stderr on an early exit.
This is fixture-loader maintenance, not a product or assertion change; the same
two-plus-four native gate remains required before preverification.

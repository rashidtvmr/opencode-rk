# V2-NATIVE-LIFECYCLE (G5 renderer package, candidate only)

Base: `fc2d201`. Branch: `v2/native-lifecycle-salvage` (isolated worktree only).
Raw-input repair base: `3692743b0c2fd717efcabd7b491a4e48b7d46c1e`.
Canonical integration context: `b323e2b18217c58231e68aa01ef0533b8d7e0ea0`.

## Observed raw-input failure and repair

The real native target compiled and ran at `3692743`, with four PTY cases
reporting `live PTY was not raw`. The pinned native library configures ANSI
terminal modes, while the host wrapper owns POSIX stdin raw mode. Exact pin
evidence from `c01292fd0837bafd07ce458c74416b2b375a41ab`,
`packages/core/src/renderer.ts`: raw activation at 3592–3593, suspend restoration
at 4295–4296, resume activation at 4303–4304, and destroy restoration at 4470–4472.
The main session independently inspected this exact Git revision.

The Rust host now uses the existing workspace `rustix` dependency's safe termios
API to capture an owned CLOEXEC duplicate of stdin and its exact original
attributes, activate raw input, restore it on explicit restoration/close/drop,
and restore/reactivate it across suspend/resume. The single-renderer invariant
bounds the ownership slot. No-TTY input has no termios state to alter; genuine
capture/set/restore failures return `BridgeError::TerminalFailed`, with Drop
reporting a restoration error rather than panicking during unwinding.

Implementation changed paths: `crates/opentui-bridge/src/safe_renderer.rs`,
`crates/opentui-bridge/Cargo.toml`, and the single bridge dependency edge in
`Cargo.lock` (serialized by the integrator). The optional dependency is enabled
by the native feature on Unix.

## Independent PTY harness maintenance

Independent test owner: Xkiro DeepSeek V4.1 Flash Free.
The old readiness helper used an empty wire needle in an AND predicate, so its
loop returned immediately instead of waiting for the trace readiness marker.
The trace marker is bytes, requiring a byte-oriented trace read. The test owner
repairs those readiness mechanics while preserving raw-mode/restoration
assertions, protocol markers/order, deadlines, and output caps. The original
test SHA-256 `09fff5f534325d8df5509368a401bcaea200b1d32561981aec8edbae402cf3ef`
remains preserved in Git. Final independently repaired fixture SHA-256:
`3799fb3444d0ac20f3b20c40404543c93bf85790aaa8060f053a8b678c011940`.
The owner also added bounded PTY draining through child exit and owned-process
cleanup. The eight-second deadline, one-MiB cap, raw/restoration assertions and
protocol ordering remain unchanged.

For a valid compiling RED, the exact corrected fixture is also applied to the
disposable detached baseline at `3692743` under the approved temporary directory.
That baseline retains the implementation without POSIX raw input ownership.
With that final fixture, the baseline produces four clean `live PTY was not raw`
failures; the candidate passes all ten target tests (five real PTY scenarios).
The native/default library gates also pass, as recorded below. Independent exact
candidate verification and exact integrated verification are required next.

Darwin restoration was investigated separately: switching raw input back to the
captured canonical state adds the kernel-derived `PENDIN=536870912` bit. Repeated
`tcsetattr` and a zero-length read do not clear it; an input flush does. The
implementation uses the safe nonblocking `tcflush(..., QueueSelector::IFlush)`
only when this bit was absent on entry and appeared during restoration. An
entry-time PENDIN bit is preserved.

## Salvaged (selective, no unrelated drift)

`crates/opentui-bridge/src/safe_renderer.rs`: exact `fc2d201..ffa52dd`
(lane/TUI-015) lifecycle delta, which itself contains the `6100c0e`
(prod/native-tui-parity) 8-symbol surface with restore-once ownership:

- extern: `restoreTerminalModes/suspendRenderer/resumeRenderer/enableMouse/
  disableMouse/enableKittyKeyboard/disableKittyKeyboard/clearTerminal`
  (audited full-surface decls already exist in `renderer.rs:61-103`).
- `LIFECYCLE_HANDLE`/`LIFECYCLE_FLAGS` atomics + `TERMINAL_ACTIVE/
  MOUSE_ENABLED/KITTY_KEYBOARD_ENABLED`; single-live-renderer invariant
  (`CLAIMED`) bounds the slot, no handle map, no struct change.
- methods: `restore_terminal_modes/suspend/resume/enable_mouse/
  disable_mouse/enable_kitty_keyboard/disable_kitty_keyboard/clear_terminal`.
- `release()` restores only caller-owned modes once (flag-gated) before the
  exactly-once `destroyRenderer`; `setup_terminal/enable_mouse/
  enable_kitty_keyboard` set flags; `restore_terminal_modes/disable_*` clear
  them. Chosen over prod `6100c0e` unconditional restore (double-restore on
  explicit-restore-then-drop).
- Existing frozen unit tests untouched (72 lib tests pass).

`crates/opentui-bridge/tests/native_terminal_lifecycle.rs`: byte-exact copy
of `ffa52dd` (strongest TUI-015 ref), original sha256
`09fff5f534325d8df5509368a401bcaea200b1d32561981aec8edbae402cf3ef`.
5 PTY cases via python3 supervisor (`TUI015_NATIVE_LIB_DIR` + staged
`libopentui.dylib` + `DYLD_LIBRARY_PATH` child env, nonblocking handshake,
1MiB cap, 8s deadline, process-group reap). The independent mechanical repairs
and final freeze hash are documented above.

## Native library provenance: RECOVERED from preserved Git blob

- Git objects, not filesystem: blob
  `0352894af7d7bbcc30f56b0848fd91f7dc31c02a` (6863648 bytes) exists in
  preserved commits `6eba7ba` (vendor arm64 dylib), `79840f880ab3`
  (`lane/TUI-011-native-artifacts`), `83b38276d59a` (artifact matrix),
  `2263e912644e` (static-macos). Refs untouched.
- Recovered via `git cat-file blob 0352894a > crates/opentui-bridge/native/lib/
  aarch64-apple-darwin/libopentui.dylib`. `git hash-object` of staged file
  reproduces `0352894a`; sha256
  `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`
  matches `artifacts.json` (blob `758e20dd`, schema 1) at all four commits.
- Manifest pin (read-only, no lock edited): OpenTUI source
  `github.com/rashidtvmr/opentui.git` commit
  `c01292fd0837bafd07ce458c74416b2b375a41ab` (tree `261e8ea4`),
  zig `0.16.0` (`zig-aarch64-macos-0.16.0.tar.xz` sha256 `b23d70de...`,
  target `aarch64-macos.13.0`), `ReleaseSafe`, install-name
  `@rpath/libopentui.dylib` (no LC_RPATH — see `f0b5a27` RED rationale;
  the PTY test passes the dir via `DYLD_LIBRARY_PATH` instead).
- Local verify: `file` → Mach-O 64-bit arm64 dylib; `nm -gU` confirms all
  14 required symbols incl. `restoreTerminalModes/suspendRenderer/
  resumeRenderer/enableMouse/disableMouse/enableKittyKeyboard/
  disableKittyKeyboard/clearTerminal`; `otool -L` confirms
  `@rpath/libopentui.dylib` + system frameworks only.

## Verification (bridge only, jobs2, offline, locked)

Native fixture directory:
`/Users/mymac/Projects/opencode-rk-v2-native-lifecycle/crates/opentui-bridge/native/lib/aarch64-apple-darwin`.
The DYLD variables are set **after** `/usr/bin/arch` so Darwin does not strip
them before Cargo/test execution.

- `cargo test --locked --offline -p opencode-rk-opentui-bridge --lib`
  → 72 passed, 0 failed.
- `cargo test --locked --offline -p opencode-rk-opentui-bridge
  --test native_terminal_lifecycle --no-run` (default features)
  → compiles (10 dead-code warnings for native-only consts, pre-existing
  pattern); links without native lib as expected.
- Final corrected baseline at the preserved detached `3692743` worktree:
  `rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1
  TUI015_NATIVE_LIB_DIR=<native-dir> DYLD_LIBRARY_PATH=<native-dir>
  cargo test --offline --locked -p opencode-rk-opentui-bridge --features native
  --test native_terminal_lifecycle -- --nocapture`
  → RED, four `live PTY was not raw` failures, six tests passed.
- Same exact command on the raw-input repair working tree → GREEN, ten passed,
  including all five real PTY cases.
- `rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1
  DYLD_LIBRARY_PATH=<native-dir> cargo test --offline --locked
  -p opencode-rk-opentui-bridge --features native --lib`
  → GREEN, 73 passed, including real native memory rendering.
- `rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1
  cargo test --offline --locked -p opencode-rk-opentui-bridge --lib`
  → GREEN, 72 passed.
- `git diff --check` → PASS.
- Xkiro GPT-6 Luna independent read-only ownership/error-handling review:
  `ses_f0e9b1063ffeLllJb19CpH9TGi` → no focused blocking defect. No Cargo run
  was performed in that review; exact candidate preverification is still required.
- Broader canonical review remains RED: `cargo fmt --all -- --check` reports
  existing workspace formatting drift; `cargo clippy --offline --locked
  --all-targets -- -D warnings` reports 14 existing security-crate errors.
  Neither broad formatting nor lint suppressions were used to obtain GREEN.

## Remaining G5 failures

1. This package has candidate GREEN evidence and a focused independent source
   review; it is not yet accepted on an integrated revision.
2. Release-built CLI golden journey and user-visible provider/session flows
   remain separate observed gates; renderer lifecycle verification alone does
   not complete G5 or G8.

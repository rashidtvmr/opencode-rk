# V2-NATIVE-LIFECYCLE (G5 salvage, candidate only)

Base: `fc2d201`. Branch: `v2/native-lifecycle-salvage` (isolated worktree only).

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
of `ffa52dd` (strongest TUI-015 ref), sha256
`09fff5f534325d8df5509368a401bcaea200b1d32561981aec8edbae402cf3ef`.
5 PTY cases via python3 supervisor (`TUI015_NATIVE_LIB_DIR` + staged
`libopentui.dylib` + `DYLD_LIBRARY_PATH` child env, nonblocking handshake,
1MiB cap, 8s deadline, process-group reap). No mechanical edits made.

## Native library provenance: ABSENT

- opentui lock: `sources/upstream.lock.json` pins the opencode app only
  (`95daf90`); no OpenTUI native-library pin/provenance exists in-tree.
- Approved attested path `temp/native-build-attest/wt/.../aarch64-macos/
  libopentui.dylib` does not exist (`/Users/mymac/Projects/temp/` absent).
- Read-only sweep of known worktrees found only
  `native/lib/x86_64-unknown-linux-gnu/libopentui.so`; no mac `.dylib`/`.a`.
- Nothing staged under `native/lib/aarch64-apple-darwin/`. No fake
  backend/lib created, no fetch, no pin invented. A source patch staging the
  real attested dylib is deferred until the artifact is produced/provided.

## Verification (bridge only, jobs2, offline, locked)

- `cargo test --locked --offline -p opencode-rk-opentui-bridge --lib`
  → 72 passed, 0 failed.
- `cargo test --locked --offline -p opencode-rk-opentui-bridge
  --test native_terminal_lifecycle --no-run` (default features)
  → compiles (10 dead-code warnings for native-only consts, pre-existing
  pattern); links without native lib as expected.
- `... --features native --test native_terminal_lifecycle --no-run`
  → FAILS at `build.rs:58` link gate: native artifact missing for
  `aarch64-apple-darwin`. Infrastructure block, not semantic RED.
  Gated by the missing dylib above; PTY runtime not runnable here.

## Remaining G5 failures

1. No `aarch64-apple-darwin` libopentui artifact → native link + all 5 PTY
   cases unrunnable. Prior lane observation (unverified here): child reached
   READY but parent reported `live PTY was not raw`.
2. `suspend/resume/clear_terminal` paths unexercised beyond compile.
3. Full native loop/input redesign explicitly out of scope.

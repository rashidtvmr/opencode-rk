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

## Native library provenance: RECOVERED from preserved Git blob

- Git objects, not filesystem: blob
  `0352894af7d7bbcc30f56b0848fd91f7dc31c02a` (6863648 bytes) exists in
  preserved commits `6eba7ba` (vendor arm64 dylib), `79840f880ab3`
  (`lane/TUI-011-native-artifacts`), `83b38276d59a` (artifact matrix),
  `2263e912644e` (static-macos). Refs untouched.
- Recovered via `git cat-file blob 0352894a > native/lib/
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

- `cargo test --locked --offline -p opencode-rk-opentui-bridge --lib`
  → 72 passed, 0 failed.
- `cargo test --locked --offline -p opencode-rk-opentui-bridge
  --test native_terminal_lifecycle --no-run` (default features)
  → compiles (10 dead-code warnings for native-only consts, pre-existing
  pattern); links without native lib as expected.
- `... --features native --test native_terminal_lifecycle --no-run`
  → previously FAILED at `build.rs:58` link gate (missing dylib). Now
  UNRUN here: no Cargo builds per current instruction (heavy Rust with
  G4 owner). Link gate inputs now present; integrator/verifier to rerun.

## Remaining G5 failures

1. Native artifact now staged (verified blob); native link + 5 PTY cases
   NOT yet run here (no-build instruction). Prior lane observation
   (unverified here): child reached READY but parent reported
   `live PTY was not raw` — expect RED or failure on first run.
2. `suspend/resume/clear_terminal` paths unexercised beyond compile.
3. Full native loop/input redesign explicitly out of scope.

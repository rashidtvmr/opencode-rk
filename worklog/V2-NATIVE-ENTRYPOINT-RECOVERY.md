# V2 native entrypoint recovery

Candidate source handoff, not an integration or acceptance claim.

## Renderer ABI repair follow-up

Pinned OpenTUI `c01292fd` defines `currentRenderBuffer` as the presented
buffer and `nextRenderBuffer` as the writable scene (`renderer.zig`:
`getCurrentBuffer`, `getNextBuffer`, and `render`). The persistent native
frame was incorrectly painting current, so the renderer diff saw an unchanged
next buffer and emitted only blank cells. The bridge now paints next and the
one-shot memory helper commits one frame before reading its snapshot.

Candidate follow-up commit: bridge-owned `safe_renderer.rs` only.

## Validation after ABI repair

Focused command (shared target, offline locked native feature):

```text
CARGO_TARGET_DIR=/Users/mymac/Projects/opencode-rk-main-v2/target \
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 \
TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp \
RUSTFLAGS='-C link-arg=-Wl,-rpath,@executable_path/../lib' \
/usr/bin/arch -arm64 cargo test --offline --locked \
-p opencode-rk-cli --features native --test native_daemon_flow -- --nocapture
```

Result: **4/4 GREEN** (`native_daemon_spawns_when_none_running`,
`native_no_tty_entry_routes_headless_without_raw_mode_or_daemon`,
`status_frame_carries_live_daemon_values`,
`tui_attaches_to_running_serve_daemon_without_origin`).

Installed/native PTY command using the candidate binary and worktree dylib:

```text
OC2_NATIVE_BINARY=/Users/mymac/Projects/opencode-rk-main-v2/target/debug/oc2 \
MAC_OPENTUI_FIXTURE=$PWD/crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib \
python3 tests/e2e/native_interactive_pty.py
```

Result: **1/1 GREEN**. This is candidate-local evidence only; parent must
rerun both gates on the exact integrated SHA.

## Authority and base

- Base: `0b00ae4` (candidate worktree branch `v2/native-entrypoint-recovery`).
- Upstream evidence: pinned `95daf906`, `packages/cli/src/tui.ts` and
  `packages/tui/src/config/keybind.ts` require an event-driven TUI runner.
- Historical salvage: `2263e91` default native routing and daemon preparation;
  `53845c2` persistent OpenTUI renderer/input loop; `425d617` bridge-owned raw
  input capture/restoration.
- Frozen REDs supplied by parent: installed PTY native entrypoint (`ICANON=256`)
  and native daemon-flow T03 live-bound-frame failure. Tests were not edited.

## Candidate changes

- `crates/cli/src/main.rs`: native-enabled `LaunchMode::NativeTui` now routes
  no-subcommand launches through `tui_entry::run_with_dir`, without requiring
  `--native`.
- `crates/cli/src/chat.rs`: added `DaemonLease`/`prepare_daemon`, reusing the
  existing validated descriptor, credential-bound lifecycle decision, health
  check, bounded spawn readiness, and owned-child cleanup.
- `crates/cli/src/tui_entry.rs`: no-origin TUI acquires the same daemon lease;
  native builds use persistent `Renderer::create`/`setup_terminal`, raw byte
  input, redraws, Ctrl-C/Ctrl-D and `/exit`/`/quit`/`:q` exits, and bridge-owned
  restoration. Live submits use authenticated `/api/sessions/{id}/turns`.

## Verification status

Source-only candidate. No Cargo/build/test commands were run by this worker per
the parent resource gate. Parent must independently compile and run the frozen
PTY and daemon-flow tests on the integrated SHA.

## Known limits requiring parent review

- The native loop currently uses bounded synchronous turn requests rather than
  `/turns/stream`; it preserves the real turn engine but does not claim provider
  streaming progress.
- Input handling is intentionally byte-oriented and bounded; full escape-key,
  UTF-8, resize, and mouse decoding remain future parity work.
- `--follow` remains the existing scriptable polling path and `--once` remains
  snapshot-only.

# LANE-TUI-LAND — Native TUI Landing

## Claim
- Task: LANE-TUI-LAND
- Session: ses_worker_tui_land
- Status: completed

## Source Evidence
- `crates/cli/src/main.rs:52-65` — Cli struct with `--native` flag
- `crates/cli/src/main.rs:201-218` — run() dispatch: `--native` → `tui_entry::run(TuiArgs{native:true})`
- `crates/cli/src/tui_entry.rs:601-606` — `run()` checks `args.native` → `run_native_boot`
- `crates/cli/src/native_host.rs:130-261` — `run_native()` creates Renderer, `--once` path renders single frame
- `crates/opentui-sys/src/bridge.rs:124-172` — `Renderer::create` fails closed without `native` feature
- `crates/opentui-sys/build.rs:138-192` — build.rs gates linking on `native` feature, checks artifact + symbols
- `native/lib/x86_64-linux/libopentui.so` — 25.4M vendored Zig artifact present

## What Changed
1. `crates/cli/src/main.rs` — Added `#[arg(long, global = true)] native: bool` to `Cli` struct; added dispatch in `None` arm: when `cli.native`, constructs `TuiArgs{native:true,..Default}` and calls `tui_entry::run(args)`
2. `crates/cli/tests/native_launch.rs` — NEW frozen test file:
   - Scenario A: default launch (no flags) satisfies line-mode contract (banner, /exit 0)
   - Scenario B: `--native --once` piped mode either renders+exit0 (artifact present) or exit non-zero with actionable message (artifact absent)

## Gate Results
- RED sha256: `e93be6673b1984f06d176e64aaf7cc88a5b13a302da67604edf634b2ff0d8982`
- GREEN sha256: `e93be6673b1984f06d176e64aaf7cc88a5b13a302da67604edf634b2ff0d8982` (byte-identical, zero test edits)
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 400 rtk cargo test -p opencode-rk-cli --test native_launch` → 2 passed
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 600 rtk cargo test -p opencode-rk-cli` → 333+9+2 passed; 1 pre-existing default_tui failure (port 4096 conflict)

## Build Artifact State
- `native/lib/x86_64-linux/libopentui.so` present (25.4M)
- opentui-sys `native` feature gates FFI linking; without it, `Renderer::create` returns `BridgeError::CreateFailed`
- The `--native` flag dispatches to `tui_entry::run` which calls `native_host::run_native`
- With `native` feature + artifact: renders a frame and exits 0
- Without `native` feature or artifact: exits non-zero with "renderer failed: CreateFailed"

## Decisions
- Used `tui_entry::run` as the dispatch target (reuses existing `--native`/`--once`/`--origin`/`--session` infrastructure) instead of wiring directly to `native_host::run_native`
- Default path (no `--native`) remains `chat::run(&data)` — zero change to line-mode behavior
- No `--once` on the default dispatch — interactive native TUI is the default when `--native` is used

## Remaining Unknowns
- Zig artifact rebuild instructions are in `native/opentui-fork/PIN` (orchestrator concern, not lane scope)
- `chat_tui_offline_hint_when_daemon_cannot_start` fails due to pre-existing port 4096 conflict — not caused by this lane

# V2 native entrypoint recovery

Candidate source handoff, not an integration or acceptance claim.

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

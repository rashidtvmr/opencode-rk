# DISC-102 — Installed opencode2 command inventory and binary identity

## Claim
- Task: DISC-102 (dep AUD-001 completed). Ledger claim OK: session
  `ses_f41f2c27bffe5WV94bXE46wlLc`, status `in-progress`,
  scratchpad `worklog/DISC-102.md`.
- Owned file: `crates/cli/src/install_commands.rs`. `main.rs` touch allowed
  only as single-use mod-wiring line; `mod install_commands;` already present
  at `crates/cli/src/main.rs:31`, so no `main.rs` edit made.
- Actual HEAD at work time: `9a9a9ab` (orchestrator brief said `1614754`;
  tree advanced via AUD-020 refresh; card content identical).

## Source evidence
- `crates/cli/src/install_commands.rs:18` — `BINARY_NAME = "oc2"`. Canonical
  per commit `b58384c` (packaged identity `opencode2` -> `oc2`); see
  `worklog/APP-010.md` (`oc2` canonical, `opencode-rk` dev-only cargo alias).
  Do NOT rename back.
- `crates/cli/src/install_commands.rs:56-66` — `command_inventory()` =
  `[doctor, session, models, serve, web, tui, run]`, mirrors
  `crates/cli/src/main.rs:76-90` `Command` enum order exactly.
- `crates/cli/src/main.rs:31` — `mod install_commands;` declared.
- `crates/cli/src/main.rs:57` — clap `name = "opencode-rk"` (legacy dev name
  in packaged `--help`; deviation, integrator one-liner pending, see below).
- `crates/cli/src/main.rs:221-259` — `None` arm runs
  `app_start::plan_default_launch` + `daemon_client` + chat/native TUI but
  never calls `install_commands::no_subcommand_entry`.
- `crates/cli/src/main.rs:209-219` — `Cli::parse()` exits inside clap on
  unknown subcommand (code 2, clap text); never calls
  `install_commands::unknown_subcommand_exit` (contract: exit 64, no daemon).
- `crates/cli/Cargo.toml:9-18` — bins `oc2` (ship) + `opencode-rk` (dev/test
  alias, same source). Release scripts `scripts/install-oc2.*` (APP-010).
- Card: `python3 tools/completion_plan.py --card DISC-102` (T01-T05 =
  no-subcommand TUI / inventory / oc2 identity / checksums fail-closed /
  unknown-subcommand nonzero).

## Observed scenario (RED-check)
- Cmd: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 180 cargo test -p
  opencode-rk-cli --bin oc2 install_commands`
- Result pre-repair: 5/5 PASS (`disc102_t01..t05`), 0 failed, 297 filtered.
  Pure boundary module pre-complete; RED N/A — the gap is integration
  (zero callers of `install_commands::` in `main.rs`, clap name legacy),
  which is outside the one-file lane boundary.
- `cargo check -p opencode-rk-cli --bins`: dead-code warnings reference
  `install_commands.rs` consts/fns (zero callers) — expected until
  integrator lands the caller patch below.

## Target boundary
- Repair inside owned file only: caller-wiring contract docs naming the
  exact `main.rs` call sites + values. Zero behavior change, zero test
  edits (test module lines 173-218 byte-untouched).
- `main.rs` caller wiring + clap `name` fix are integrator-owned (need
  multi-line `main.rs` edits: `try_parse` fallback, `None`-arm call).

## Tests
- Frozen in-file T01-T05 (`install_commands.rs:174-218`), buf hash recorded
  at GREEN run. Command:
  `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 180 cargo test -p
  opencode-rk-cli --bin oc2 install_commands` → 5 passed, 0 failed.

## Decisions
- Keep `BINARY_NAME="oc2"` (b58384c + APP-010 canonical). Card title says
  "opencode2" but packaged artifact is `oc2` (installed as `opencode2` per
  install scripts); deviation documented here, not renamed.
- No `main.rs` edit: mod-wiring line already exists; clap literal rename and
  caller wiring need integrator multi-line patch (below). Lane rule:
  one owned file.
- Docs-only repair in owned file (no functional drift risk to frozen tests).

## Integrator patch (exact, not applied by this lane)
1. `main.rs:57`: `name = "opencode-rk"` → `name = "oc2"` (clap needs a
   literal; must mirror `install_commands::BINARY_NAME`).
2. `main()`: `Cli::parse()` → `Cli::try_parse()`, on `ErrorKind::
   UnknownArgument/InvalidSubcommand` extract the name, call
   `install_commands::unknown_subcommand_exit(&name)`, print help text,
   `exit(help.exit)` (64, no daemon).
3. `run()` `None` arm: `debug_assert_eq!(install_commands::
   no_subcommand_entry(), install_commands::NoSubcommandEntry::
   OpenNativeTui)` (pins T01 wiring; daemon/TUI behavior already owned by
   `app_start` + `daemon_client` + `tui_entry`).

## Remaining unknowns
- None for this lane. Parent acceptance waits on integrator patch + frozen
  parent journey (`installed_default_entrypoint.rs`, currently RED todos).

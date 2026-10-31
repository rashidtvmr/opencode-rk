# RENAME-001 - Rebrand opencode-rk to ocm (binary, data dirs, env, display)

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none newly introduced (rebrand of existing surfaces; no REQ change).
Dependencies: CONV-001 (Phase 1 scope must be closed first); owner-recorded contract revision for 8 display-string test files (see Frozen-test handling; gate, do not start without it).
Test obligations: RENAME-001-T01, RENAME-001-T02, RENAME-001-T03, RENAME-001-T04, RENAME-001-T05, RENAME-001-T06.
Program: yes. Owned paths: crates/cli/Cargo.toml (bin section only), crates/cli/src/main.rs (name/data-dir/env sites), crates/cli/src/install_commands.rs (LEGACY_BINARY_NAME + identity consts), crates/cli/src/tui_entry.rs (brand display sites), crates/cli/src/chat.rs (brand display site), crates/foundation/src/effect.rs (temp-file prefix), scripts/install-ocm.sh, scripts/install-ocm.ps1. Integration gate: none (single integrator lane; MUST NOT fan out — a partial rename is a broken product). Merge order: after CONV-001, before any GUI-001 leaf.
Ownership locks: listed Owned paths only; worker never edits other shared lib.rs, other Cargo.toml files, schemas, migrations, tools/, tests/bootstrap/, frozen test files (the 8 re-baselined files are edited ONLY under the owner-recorded contract revision, as a rebrand re-baseline with a new frozen hash — not as a test fix).
Suggested module: no new module; edits are confined to existing brand/name/data-dir sites.

## User-observable outcome

`ocm` is the product name. `ocm --version` prints the ocm version string. A fresh install writes to `~/.local/share/ocm` (Linux/macOS) or `%USERPROFILE%\.ocm` (Windows) and reads `OCM_*` env vars. `opencodem` works as an alias. Existing users lose nothing: on first run the daemon finds a legacy `~/.local/share/opencode-rk` directory, migrates it atomically, leaves a `.migrated-from` marker, and every `OPENCODE_RK_*` var still resolves with a once-per-var deprecation warning.

## Source evidence

- crates/cli/Cargo.toml:8 `[[bin]] name = "oc2"` (current primary); :17 `[[bin]] name = "opencode-rk"` (compat alias; 26 `CARGO_BIN_EXE_opencode-rk` refs in 9 frozen test files depend on this bin existing at compile time).
- crates/cli/src/install_commands.rs:31 `LEGACY_BINARY_NAME = "opencode-rk"`.
- crates/cli/src/main.rs:63 `OPENCODE_RK_HOME`; :390 `println!("OpenCode RK {}", ...)`; :753 unix data dir `~/.local/share/opencode-rk`; :756 windows data dir `~/.opencode-rk`; :758 error string naming `OPENCODE_RK_HOME`.
- crates/cli/src/chat.rs:164 `println!("OpenCode RK")`.
- crates/cli/src/tui_entry.rs:335,470,471,604 four `OpenCode RK` display sites.
- scripts/install-oc2.sh:9 `BIN="oc2"`; :108 rejects `*opencode-rk*` archives; install-oc2 identity rule ("identify as oc2 and never as the legacy dev name").
- crates/foundation/src/effect.rs:171 `opencode-rk-effect-{}-{}-{}.bin` temp prefix.
- 16 further `OPENCODE_RK_*` env vars, ~110 use sites (counted 2026-09-26 via git grep).
- Crate package names `opencode-rk-*` (12) and Rust identifiers `opencode_rk_*` (~510) are explicitly OUT of scope (owner decision 2026-09-27): internal names stay, user-facing surface renames.
- worklog/ and existing tasks/ cards keep old-name references: history is an accurate record of what shipped under the old name; no history rewrite.

## Observable contract

- Release bins: `ocm` (primary), `opencodem` (alias), `opencode-rk` (legacy, RETAINED so the 26 `CARGO_BIN_EXE_opencode-rk` refs keep compiling with zero test edits), `oc2` (legacy).
- `--version` output contains the primary name `ocm`; install-script identity check accepts the new names and rejects `opencode-rk` (mirroring today's oc2 rule).
- `pub const BRAND: &str = "ocm"` exported from the CLI crate; all ~8 display sites route through it. Future rebrands are a one-line change.
- Data-dir resolution order: `--data-dir` > `OCM_HOME` > legacy `OPENCODE_RK_HOME` (warn) > `~/.local/share/ocm` (unix) / `~/.ocm` (windows).
- Env resolution order per var: `OCM_<X>` > legacy `OPENCODE_RK_<X>` (warn once per var per process) > built-in default. Warning text never contains the variable value (secret safety).
- Migration: new dir absent AND legacy dir present -> atomic rename, one WARN line, write `.migrated-from` marker. New dir already present -> use it, leave legacy untouched, no warning spam. Same-filesystem rename only; cross-device -> fail closed with instructions. Never delete legacy data. Never copy a full DB (ADR-004 byte-discipline).
- In-flight effect temp files under the old prefix (`effect.rs:171`): migration sweeps BOTH prefixes for cleanup.
- Explicitly excluded: crate package renames, Rust identifier renames, worklog/task-history rewrites, docs beyond README + install docs.

## State and persistence transitions

- Data dir: absent-new + present-legacy -> present-new (renamed) + marker. Absent-new + absent-legacy -> created fresh, no marker, no warning.
- Env: resolution is read-only; the only new state is the once-per-var warning flag (process-local, no persistence).
- Daemon identity: `ocm` primary; all alias bins resolve to the same data dir and discovery file.

## Failure states

- Unreadable/undeletable legacy dir -> FAIL CLOSED: refuse to start with an actionable error. Never silently start empty and orphan a user's SQLite history. (The one unacceptable outcome.)
- Both `OCM_<X>` and legacy set with conflicting values -> `OCM_<X>` wins, warn the legacy var was ignored (value never logged).
- Cross-device rename impossible -> fail closed with explicit instructions, leave both dirs intact.
- Secret safety: env warnings carry names only, never values; captured-log test asserts this.

## Resource bounds

- Single `stat`/`rename` on daemon startup; bounded, once, fail-closed. No new queues, threads, background tasks, or retained output. No allocation growth on any hot path.

## Frozen-test handling (contract revision, owner authority)

Two independent couplings, different severity:

- (a) Compile-time, 26 refs / 9 files (`ci_ext.rs` 5, `default_tui.rs` 4, `native_daemon_flow.rs` 7, `doctor_next_steps.rs` 2, `native_launch.rs` 2, `installed_default_entrypoint.rs` 2, `ci_mode.rs` 1, `native_tui_parity.rs` 2, `doctor_diagnostics.rs` 1): ZERO test edits — the retained `opencode-rk` bin keeps `env!("CARGO_BIN_EXE_opencode-rk")` compiling.
- (b) Assertion-level, 17 refs / 8 files (`default_tui.rs` 7, `native_tui_parity.rs` 3, `native_launch.rs` 2, `tui_entrypoint.rs`, `web_entrypoint.rs`, `native_daemon_flow.rs`, `daemon_long_path.rs`, `web_assets.rs` 1 each): owner-recorded contract revision. Per AGENTS.md this is NOT an implementer or orchestrator action. Required before lane start: recorded authorization, then re-baseline the 8 files to assert via the `BRAND` constant, new frozen hash logged as a product contract change.

## Test obligations (frozen)

- RENAME-001-T01 (bin identity): `ocm --version`, `opencodem --version`, `oc2 --version`, `opencode-rk --version` all run; install identity check accepts new names, rejects legacy.
- RENAME-001-T02 (data-dir precedence): `--data-dir` > `OCM_HOME` > legacy `OPENCODE_RK_HOME` (warn) > `~/.local/share/ocm` / `~/.ocm` default.
- RENAME-001-T03 (legacy migration): legacy-only -> migrated + marker + history intact; both-present -> new wins, legacy untouched; unwritable legacy -> fail closed with actionable error, no silent empty start.
- RENAME-001-T04 (env precedence): `OCM_*` wins; legacy-only warns exactly once per var per process; conflict warns legacy ignored; warnings contain zero secret bytes (asserted on captured logs).
- RENAME-001-T05 (brand surface): every display site emits via `BRAND`; the 8 re-baselined files assert through the constant with the new recorded hash.
- RENAME-001-T06 (purity): zero network; zero writes outside the disposable fixture dir; `fs_calls` confined to the migration path.

## TDD steps

1. Contract review -> recorded owner authorization for the 8-file re-baseline. GATE; do not start without it.
2. Author T01..T06; establish compiling RED (fail: no `ocm` bin, old data-dir default).
3. Freeze T01..T06 hash + command manifest, and the 8 re-baselined files' new hash.
4. Implement in order: bins -> `BRAND` const -> env precedence + once-warn -> data-dir + migration -> display sites -> install scripts (`install-ocm.sh/.ps1`; keep `install-oc2.*` as legacy wrappers).
5. GREEN, refactor, rerun. Re-run `python3 tools/validate_repository.py`.
6. Docs: README + install docs only. No worklog/tasks history rewrite.

## Verification

```bash
git status --short -- tasks/RENAME-001.md
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-cli rename
python3 tools/validate_repository.py
cargo check --workspace
```

## Ownership note

Task-card model mirrored from tasks/ACP-001.md. Single-integrator lane by design; no fan-out. The 8 re-baselined files are owner-authorized only. Follows the program rule: GUI-001 leaves may not start until this card is accepted and `BRAND` is published (ordering, not scope).

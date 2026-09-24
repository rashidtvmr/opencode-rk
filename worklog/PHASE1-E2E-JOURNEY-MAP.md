# PHASE1-E2E-JOURNEY-MAP

## Claim

- Task: `PHASE1-E2E-JOURNEY-MAP`
- Session: `ses_f2e037479ffeOuZl2LUxXEnpmC`
- Branch: `plan/e2e-journey`
- Starting revision: `1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9`
- Reclaim evidence: prior holder `ses_f2e23ff78ffeHaSE8HprkKa3XT` left `in-progress` with pending-stub worklog only, no accepted work, no commits; second session `ses_f2e2a8212ffeE84PKnb529qg7I` exhausted before writing. Reclaimed via `cc.reclaim` with recorded note, then `cc.claim` by this session.
- Owned paths: this scratchpad; `tasks/completion/claims.json` row for this task only.
- Scope: audit/write/claim/commit/push only. No product, test, controller, policy, fixture edits. No Cargo runs.

## Source evidence (HEAD 1f4a9e6, branch plan/e2e-journey)

- `crates/cli/src/main.rs:225-271` `run()` no-subcommand arm: TTY probe, `discover_presence`, `creds_configured`, `plan_default_launch`, routes `plan.view` into `tui_entry::run_default`. Reads current, not stale.
- `crates/cli/src/app_start.rs:295-321` `plan_default_launch`; `:312-315` creds `Some(false)|None` routes `StartupView::Setup`; `:337-339` `needs_setup`; `:344-346` `setup_message`.
- `crates/cli/src/daemon_client.rs:646-648` `is_wellformed_token` (64-hex); `:665-700` `parse_authenticated_descriptor`; `:748-773` `discover_presence`; `:776-791` `creds_configured` (env-var presence, ignores stored providers); `:796-804` `try_become_owner` startup election.
- `crates/cli/src/chat.rs:100-138` `prepare_daemon` singleton discover/spawn, `DaemonLease` owns child handle, drop detaches without killing shared daemon; `:141-158` compat `run` (offline banner path).
- `crates/cli/src/tui_entry.rs:1436-1448` `run_default` maps `Setup`/`Main` to flags; `:1450-1495` `run_with_startup` setup arm (TTY-guarded, refuses piped stdin); `:1509-1514` `create_first_session` calls `fetch_or_create_snapshot_async`; `:457-480` same POSTs `/api/sessions` `{"title":"New session"}` then refetches; `:1611-1628` `resolve_origin_bearer` validates descriptor token; `:1225-1235` `worker.shutdown` + `renderer.restore_terminal_modes`.
- `crates/cli/src/turn_worker.rs:307` `InterruptResult`; `:447-466` `interrupt()`; `:536` `shutdown()`; `:849-868` interrupt-uncertain-without-replay test documents no-ambiguous-replay rule.
- `crates/cli/src/native_approvals.rs:37-60` `OperationDigest`, `ApprovalScope`, `Unconfirmed` never approves destructive/human-only.
- `crates/security/src/lib.rs:63-77` `Decision::{Allow, Deny{reason}, RequireHuman{approval_id,reason}}`; `is_mandatory` true for Deny/RequireHuman.
- `crates/tools/src/file_ops.rs:13` `MAX_READ_BYTES = 64*1024`; `:161-183` `execute_authorized` broker-gated read.
- `crates/cli/src/shutdown.rs:3-25` `RestoreGuard` Drop-restores terminal; abort/SIGKILL ceiling documented.
- `crates/cli/src/main.rs:541-547` `open_sessions` via `Storage::open(StoragePaths::under(data))`; `:382` `embedded_sqlite: true` in doctor output.
- `crates/cli/tests/installed_default_entrypoint.rs:382-571` (macOS-only, `#![cfg(target_os="macos")]`) five frozen installed journeys: `bare_no_command_enters_planned_native_route`, `single_authenticated_daemon_reused_by_second_client`, `missing_credentials_open_setup_without_offline_instruction_or_secret`, `native_renderer_emits_frame_and_restores_alternate_screen`, `rerun_same_executable_and_revision_is_deterministic` (needs `OC2_E2E_REVISION` env or compile-time `GIT_COMMIT`).
- `crates/cli/tests/packaging_identity.rs` + `scripts/install-oc2.sh:58-192` install/verify/uninstall (`--uninstall` keeps user data `:154-172`; identity gate staged before publish `:327`).
- `worklog/TUI-011.md:207-265` exact-revision static-closure + packaged-process proof on lane `TUI-011-static-macos` (product-spine `2568eec4...`): release `oc2` 10,866,592 B, archive 6,337,512 B, installed PTY 1/1 via `OC2_E2E_BIN`, second-client shared-daemon survival with unchanged PID; bare fresh-HOME setup/first-session stayed RED there.
- `worklog/INSTALLED-DEFAULT-CONTRACT-INTEGRATION.md` cherry-pick `26da874` as `15381e3`: without receipt 4/5, with `OC2_E2E_REVISION=$(git rev-parse HEAD)` 5/5, SHA-256 `fec2fdb9...`.
- This branch has no `crates/cli/build.rs` (`ls` fails) and `install-oc2.sh` has no `OC2_E2E_REVISION`/`GIT_COMMIT` injection. Product-spine commits `c4325e4` (build.rs receipt, 214 lines) and `5d66683` (APP-010 integration evidence) exist in object store but are NOT ancestors of HEAD `1f4a9e6` here.

## Observed scenario

Installed `oc2` journey is proven in fragments across lanes, never end to end on one installed revision on this branch. Source-built PTY proof is strong (5/5 frozen with receipt on product-spine); packaged proof covers install, identity, static closure, single PTY rerun, second-client daemon survival; fresh-HOME setup/first-session/provider-turn/brokered-read/approval/interrupt/history/restart are unwired or unit-only on this branch.

## Target boundary

Map only. Each row names observable contract, current proof tier, OS, exact source symbol. No implementation. No acceptance claim.

## Matrix (journey x proof tier x OS)

Proof tiers: in-process (unit/lib test), source-built (cargo-built bin under PTY), packaged (release archive installed to disposable prefix), installed (bare `oc2` from fresh HOME, no Cargo env). OS: macOS arm64 proven where noted; Linux/Windows/MSVC not run from here.

| # | Journey | Observable contract | Current proof | OS |
|---|---|---|---|---|
| J01 | Fresh HOME/setup | Bare `oc2`, empty HOME+data, TTY: opens `StartupView::Setup`, no `offline`/`serve` text, no secret echo | source-built: `installed_default_entrypoint.rs:463-490` (with receipt); packaged bare-HOME was RED on TUI-011 lane (no setup marker) | macOS |
| J02 | Secret entry/persistence | Typed key masked, persisted to provider store or env contract, never in logs/capture | in-process only: `app005_setup_secret_red.rs` RED; `creds_configured` reads env only (`daemon_client.rs:776`), no stored-credential path on this branch | macOS |
| J03 | First session | Authenticated empty daemon auto-creates one session, frame renders it | source-built: `installed_default_entrypoint.rs:382-412` enters planned route; auto-create path `fetch_or_create_snapshot_async` (`tui_entry.rs:457`) has no installed proof here | macOS |
| J04 | Provider turn | Prompt dispatches one provider POST, streams result, no ambiguous replay | in-process + source-built unit only: `turn_worker.rs:849` documents interrupt uncertainty; packaged UI-014 single delayed POST ran on product-spine lane, not this branch | macOS |
| J05 | Brokered read | Tool `read` executes only via `PermissionBroker`, 64KiB cap | in-process: `file_ops.rs:13,161-183` + APP-012 lanes; no installed journey proof | any |
| J06 | Protected denial | Denied op returns typed denial, nothing executes | in-process: `Decision::Deny` (`security/lib.rs:63`), `registry_dispatch.rs:67` `authorize`; no installed journey proof | any |
| J07 | Human approval/resume | `RequireHuman` pauses, explicit grant resumes exactly one digest | in-process: `native_approvals.rs:37-60`, `app_policy.rs`; no installed journey proof | any |
| J08 | Concurrent client | Second client attaches same daemon PID, same token, health 200 | packaged (other lane) + source-built: `installed_default_entrypoint.rs:414-461`; `prepare_daemon` election `chat.rs:100-138` | macOS |
| J09 | Interruption | `:i`/Ctrl-C requests interrupt, draft preserved, no replay | in-process: `turn_worker.rs:447-466`, `chat.rs:512-556`; packaged UI-014 queued-input/interrupt ran on product-spine lane only | macOS |
| J10 | Durable history | Turns persist in embedded sqlite, listed after restart | in-process: `sessions` storage lib; `open_sessions` `main.rs:541`; no installed restart proof | any |
| J11 | Daemon restart/resume | Kill daemon, restart, sessions resume, token rotation safe | none: no RED file, no lane; `discover_presence` Stale path (`daemon_client.rs:768-771`) untested installed | any |
| J12 | Terminal restoration | Alt-screen enter `1049h`, exit restores `1049l`, exit 0 | source-built: `installed_default_entrypoint.rs:492-528`; guard `shutdown.rs:3-25` | macOS |
| J13 | Daemon survival | Last client `:q` leaves shared daemon alive, health 200 | source-built + packaged (other lane): `installed_default_entrypoint.rs:437-443`; `DaemonLease` drop semantics `chat.rs:77-86` | macOS |
| J14 | Install/upgrade/uninstall | Archive installs, `--version` has `oc2` not legacy, uninstall removes bin keeps data | packaged: `install-oc2.sh` + `packaging_identity.rs` + APP-010 T1-T5; upgrade path untested; receipt injection missing on this branch | macOS |

## Missing RED files and single-owner lanes

Each lane owns exactly one RED file. No shared-file edits.

- LANE-J01-MAP owns `crates/cli/tests/j01_installed_setup_first_run.rs` (fresh-HOME setup marker + first auto-created session, installed bin, macOS PTY). Depends on LANE-RECEIPT.
- LANE-J02-MAP owns `crates/cli/tests/j02_installed_secret_persist.rs` (masked entry, persisted cred, no secret in capture). Depends on LANE-J01-MAP.
- LANE-J04-MAP owns `crates/cli/tests/j04_installed_provider_turn.rs` (one stub-provider POST, streamed result, interrupt-during-turn). Depends on LANE-J02-MAP.
- LANE-J05J06-MAP owns `crates/cli/tests/j05j06_installed_broker_deny.rs` (brokered read + typed denial over installed daemon). Depends on LANE-J04-MAP.
- LANE-J07-MAP owns `crates/cli/tests/j07_installed_approval_resume.rs` (RequireHuman pause/resume one digest). Depends on LANE-J05J06-MAP.
- LANE-J10J11-MAP owns `crates/cli/tests/j10j11_installed_history_restart.rs` (history durable across daemon kill/restart). Depends on LANE-J04-MAP.
- LANE-RECEIPT owns `scripts/install-oc2.sh` change only (inject truthful `OC2_E2E_REVISION` or `GIT_COMMIT` at package time) plus receipt assertion doc in owned worklog. Depends on nothing; blocks J01, J03 determinism rerun. Port of product-spine `c4325e4` build.rs approach is integrator decision, not this map.
- LANE-J14-MAP owns `crates/cli/tests/j14_installed_upgrade.rs` (install old, upgrade, version identity, uninstall keeps data). Depends on LANE-RECEIPT.

J03/J08/J09/J12/J13 need no new RED: covered by frozen `installed_default_entrypoint.rs` once LANE-RECEIPT lands on this branch.

## Dependency graph

```
LANE-RECEIPT
 +- LANE-J01-MAP (setup + first session)
     +- LANE-J02-MAP (secret persist)
         +- LANE-J04-MAP (provider turn)
             +- LANE-J05J06-MAP (broker read + denial)
             |   +- LANE-J07-MAP (approval resume)
             +- LANE-J10J11-MAP (history + restart)
LANE-RECEIPT +- LANE-J14-MAP (upgrade)
```

## Safe parallel wave (≤14 breadth, convergence-first)

Wave A (1 lane): LANE-RECEIPT alone; it unblocks determinism assertions everywhere.
Wave B (3 lanes): LANE-J01-MAP, LANE-J14-MAP, plus one verifier lane re-running frozen `installed_default_entrypoint` 5/5 with receipt on the merged tree.
Wave C (4 lanes): LANE-J02-MAP after J01 green; LANE-J04-MAP after J02; LANE-J05J06-MAP and LANE-J10J11-MAP after J04; LANE-J07-MAP after J05J06.
Reserve: 4 integration-spine + 2 verifier lanes stay free per AGENTS.md; this map consumes at most 4 breadth lanes.

## Critical path

LANE-RECEIPT -> LANE-J01-MAP -> LANE-J02-MAP -> LANE-J04-MAP -> LANE-J05J06-MAP -> LANE-J07-MAP. J10J11 and J14 run off-path in parallel. Longest chain is six lanes; J01 is the current gate (bare-HOME setup marker absent on packaged runs).

## Revision receipt accounting

- Product-spine `c4325e4` landed compile-time `GIT_COMMIT` receipt (`crates/cli/build.rs`, 214 lines) and `5d66683` recorded post-push GREEN; both present in object store, neither an ancestor of this branch HEAD `1f4a9e6`.
- Package binding stays open here: no `crates/cli/build.rs`, no `OC2_E2E_REVISION` in `scripts/install-oc2.sh`, so `rerun_same_executable_and_revision_is_deterministic` cannot pass installed on this branch without env injection.
- LANE-RECEIPT is the single owner for closing this gap; no other lane touches packaging.

## Tests and commands

- No Cargo run per task orders.
- `rtk git diff --check`: clean (evidence below at commit time).
- Ledger: `cc.reclaim` + `cc.claim` by `ses_f2e037479ffeOuZl2LUxXEnpmC`; status stays `in-progress` until orchestrator integration (map is audit artifact, `completed` requires frozen RED suite which this lane does not author).

## Decisions

- No coverage ratios stated; denominators differ per tier and OS.
- Source-built evidence never represented as installed-package evidence; product-spine packaged results cited as other-lane, not this-branch proof.
- Unit/component evidence never represented as real installed `oc2` journey evidence.
- Linux/Windows/MSVC rows marked not-run; MSVC has no pinned Zig target (`worklog/TUI-011.md:35,108`).

## Remaining unknowns

- Stored (non-env) provider credential path: `creds_configured` checks env only; setup persistence target file/format undefined.
- Stub-provider harness for J04 installed turn: no agreed fake endpoint contract.
- Upgrade semantics: version ordering, data migration, rollback on identity failure.
- Windows installer parity (`install-oc2.ps1`) and signing/notarization authority: outside any defined lane.

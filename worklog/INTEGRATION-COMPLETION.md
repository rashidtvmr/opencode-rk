# INTEGRATION-COMPLETION — completion-plan integrator evidence

Base: 9cc0a34 scope, fan-out HEAD 5af7884 (AUD-001..020). This lane = integrator only.
No test file modified anywhere in this session. No ralph.json / FEATURES.md / PLAN.md edit.

## What was integrated

0. Second sweep found 16 more unwired lane files from parallel sessions; all
   wired and verified (no dead code lands):
   - agents: agent_executor.rs (PAR-004 real-loop scaffold)
   - cli: headless_engine.rs (APP-008), native_layout.rs (TUI-layout)
   - security: platform_matrix.rs
   - tools: app_extensions.rs (PAR-007)
   - storage: backup_v2.rs (DB-backup)
   - server (10): remote_approvals, remote_connector, remote_files, remote_pty,
     remote_recovery, remote_revocation, remote_sessions, remote_turns,
     web_turn_adapter, workspace_sessions (NET-005..013 / PAR-008; lane
     worklogs explicitly deferred lib.rs wiring to the integrator)
   - remote_sync.rs stays #[path]-included by tests/remote_sync.rs (lane pattern)
1. Wired 24 further Wave1/Wave2 lane modules into their crates' lib/main:
   - cli (13): app_start, daemon_client, diagnostics, native_app, native_composer,
     native_palette, native_status, native_transcript, onboarding, pair,
     service_commands, shutdown, terminal_host
   - server (4): app_protocols, turn_service, event_cursor, app_runtime
   - sessions (2): app_history, session_membership
   - providers (2): app_routing, account_setup
   - agents (1): app_delegation
   - security (1): app_policy; tools (1): app_services
   - driver_lane.rs / delegation_lane.rs stay #[path]-included by their test
     targets (established repo pattern), not lib.rs-wired.
2. Wrote the 4 missing TUI lane modules (pure state, forbid(unsafe_code),
   card-cited headers, frozen in-file scenario tests):
   - native_approvals.rs (TUI-008), native_navigation.rs (TUI-007),
     native_status.rs (TUI-009), native_palette.rs (TUI-006)
3. ralph.completion.json includes += tasks/completion/discovered.json
   (19 DISC-101..119 stories, schema-identical to local.json). Plan check:
   SPEC OK additions=109, legacy=258, scenarios=545.
4. tools/plan_model.py PHASE_RANK += legacy reconciliation families
   (SYNC/RUN/ACP=2, WSX/SDK=5, HEAD=1) per COORD-008's local table
   (tools/completion_reconcile.py). Fixes plan-load PlanError
   "Unknown task prefix for 'SYNC-001'" — 11 accepted legacy stories had no
   rank. Bootstrap failures: 19 -> 8.

## Gate results (this tree)

| Gate | Result |
|---|---|
| cargo check cli/server/security/tools + sessions/providers/agents | 0 errors |
| cargo test -p opencode-rk-agents | 39/39 |
| cargo test -p opencode-rk-providers | 381/381 |
| cargo test -p opencode-rk-sessions | 300/300 |
| cargo test -p opencode-rk-security -p opencode-rk-tools | 492/492 (first pass) |
| cargo test -p opencode-rk-foundation | 168/168 |
| cargo test -p opencode-rk-storage (incl. backup_v2) | 177/177 |
| cargo test -p opencode-rk-server (incl. 10 remote lanes + web/workspace adapters) | 270/270 parallel; session_turn_stream_api 2/2 with --test-threads=1 (documented serial mandate, TURN-STREAM-GATE env race) |
| cargo test -p opencode-rk-cli -p security -p tools (final pass, all wiring in) | 665/665 |
| pytest tests/completion | 38/38 |
| Python self-checks (reconcile/integration/leases/ownership/verification/budgets) | all exit 0 |
| completion_plan.py --check / --export | OK / OK |
| cargo build --release -p opencode-rk-cli | OK (1m16s, 0 errors) |
| Release smoke: ./target/release/opencode-rk doctor / --help | exit 0; actionable next-steps + full command surface (doctor/session/models/serve/web/tui) |
| tools/validate_repository.py | FAIL backlog exhaustion (pre-existing controller-owned RED, see below) |

Rust totals this session: 39+381+300+168+177+272+665 = 2002 passed, 0 failed.

## Remaining RED (pre-existing, controller-owned — not introduced here)

- tests/bootstrap/test_auto_controller asserts the pre-flip plan: exactly 219
  stories, zero authored dependencyIds. ralph.json carries 258 stories since
  248f519 (incl. 21 with authored deps), so 8 auto_controller/validate_plan
  tests stay RED and backlog-exhaustion (backlog-ledger projection) fails
  closed with them. This is the parked controller acceptance-flip
  reconciliation (worklog/ACCEPTANCE-FLIP-PROPOSAL.md). tests are frozen;
  flipping statuses/backlog is a controller decision, not an integrator action.
- DISC-003 manifest drift (test_ci_enforcement) is part of the same
  reconciliation: manifest inputs include ralph.json's SHA.

## Boundaries honored

- No edits to tests/, verifier code, ralph.json, FEATURES.md, PLAN.md.
- One cargo at a time, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=2 (8 GB budget).
- No acceptance claims: statuses unchanged; verifier decides integration.

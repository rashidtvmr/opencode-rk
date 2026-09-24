# PHASE1-ROADMAP-20260923: dependency graph to unsigned Phase 1 release

## Claim

- Task: `PHASE1-ROADMAP-20260923`, session `ses_f2e2b0c72ffeT2x01toLrwBRBC`.
- Branch: `plan/phase1-roadmap`, base `1f4a9e6` (local) vs `origin/main 8a91a7b`.
- Owned files ONLY: `worklog/PHASE1-ROADMAP-20260923.md` + own ledger row. No product/test/controller edits.
- Status: `in-progress` (analysis lane, no acceptance claimed).

## Source evidence (exact revisions)

- Local HEAD `1f4a9e6 integrate installed default contract evidence`; `origin/main 8a91a7b Complete-app wave 21-Sep`.
- Plan scope: 89 stories across `tasks/completion/{local,delivery,discovered,parity,remote,tui}.json`.
  Ledger-vs-plan accounting (local tree): completed 47, not-started 40, in-progress 1, blocked 1.
- Gate: `python3 tools/convergence_gate.py` = CONVERGENCE BLOCKED, total=86
  (78 off-plan completed alias rows + AUD-017/AUD-020 bad-note rows + INSTALLED-DEFAULT-CONTRACT-INTEGRATION missing-note).
- Guard: `python3 tools/validate_repository.py` = FAIL `validate_backlog_exhaustion: 51 error(s)`
  (accepted/unknown stories, stale ownership-gap notes for ROUTE/OPS/REL/EXT/SHARE/INT families).
- Ledger (local): 149 completed / 33 blocked / 24 in-progress. Origin: 123 / 18 / 20.
  157 ledger-only IDs (not plan tasks); most completed are verify/repair/LANE aliases.
- `docs/CONVERGENCE.md:10-24` hard boundary: installed `opencode2` no-subcommand journey,
  real daemon/broker/tool/persistence, two clients, denial/interrupt/restart, frozen E2E rerun.
- `tasks/completion/local.json`: APP-012 deps APP-008+APP-010+APP-011+TUI-010 (all completed).
- `tasks/completion/delivery.json`: SHIP-001 deps APP-010+TUI-011; SHIP-002 deps APP-012+SHIP-001;
  SHIP-008 deps SHIP-001..007+COORD-008.

## Mandatory parents (must stay open, evidence)

1. APP-012 (in-progress, `worklog/APP-012.md`): golden installed journey open. Blockers: trusted RED
   being authored; frozen `installed_default_entrypoint.rs` has five immutable `todo!()` bodies
   (controller contract review required); APP-001/APP-005 rows admit unwired wiring; executor only
   runs bash/shell/echo (`tools/src/executor.rs:105-119`), RequireHuman converts to terminal error
   (no approval-resume channel); `install-oc2.ps1:54-66` lacks native closure + bounds; no release workflow.
2. TUI-011 (blocked, `worklog/TUI-011.md`): builder matrix done (macOS arm64+x86_64, linux-gnu
   x86_64+aarch64, windows-gnu DLL+import-lib via zig dlltool, MSVC fail-closed by design).
   Remaining: vendor/copy lanes, macOS signing/notarization (external), frozen
   `native_artifact_manifest` conflicts (SHA-digit predicate :95, 64KiB dylib bound :122, see
   `worklog/TUI-011-FROZEN-CONTRACT-REVIEW.md`).
3. DISC-003 (blocked, `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md`): 80 findings on integrated
   candidate (78 alias retire + 2 bad-note rows); controller authority required, no acceptance.
4. SESS-008 (blocked, `worklog/SESS-008-CALLER-REVIEW.md`): `persist.rs` unwired state-only;
   controller must retire/decompose SESS-008/011.
5. PROV-023 (blocked, `worklog/PROV-023-TRANSPORT-GAP.md`): live transport posts `/responses` but
   catalog documents only chat-completions; needs reviewed Responses entry + RED child.
6. PROV-024 (blocked, `worklog/PROV-024-FIXTURE-RED.md`): frozen sha 750e3a32, missing-root RED gap.
7. REL-002 (blocked, `worklog/REL-002-STRONG-RED.md`): 11/12 pass, executable planning CI step fails;
   blocked on protected workflow review.
8. UI-014 (blocked, `worklog/UI-014-TURN-WORKER-PTY-RED.md`): product PTY GREEN but parent RED
   (fresh-HOME setup/first session absent, five todo! bodies).
9. WEB-009 (blocked, `worklog/WEB-009-EMBEDDED-VERIFY.md`): embedded proof GREEN, blocked on
   activity unification + external provider/browser evidence.
10. INT-010 (blocked): no ownership lock; duplicate impl files; needs single owned-file assignment.
11. ROUTE-009 (blocked): no card/worklog/impl; stale ownership-gap note.
12. INSTALLED-DEFAULT-CONTRACT (blocked): 4/5 without receipt; GREEN 5/5 only with explicit
    `OC2_E2E_REVISION`; packaging must inject truthful receipt (frozen sha fec2fdb9...).
13. APP-001-REPAIR-RED (blocked): macOS PTY RED, restoration needs ESC `[?1049l`; Linux/Windows out of scope.
14. APP-005-SECURE-RED (blocked): setup repaint echoes raw sentinel, masking required.
15. FIX-MCP-STUBS (blocked): frozen `tests/mcp_config.rs:76,87` json!-repeat uncompilable + `:95,109`
    missing HashMap; test-owner fix required.
16. FIX-STREAM (blocked per ledger; orchestrator addendum 2026-09-20 claims human-approved env_lock
    fix with 3x parallel GREEN; needs independent verifier confirmation before unblock).
17. FIX-CI-APPROVAL (blocked): full ci_mode hangs in ci_t01 fixture join (pre-existing).
18. AUD-016 (blocked): MOB-006 needs real devices/signing + DISC-118 freeze not-started.
19. Missing-artifact diagnoses (each blocked, needs rebuild lane): AUD-004-ENTRY, AUD-008-LEDGER,
    TUI-010-CAPS, WEB-ARTIFACT-EXEC, WEB-ATTACH-TRANSMIT, WEB-LIBRARY, WEB-PINS-SEARCH, WEB-PLUGIN,
    WEB-TOOL-CHOOSER, WEB-VOICE, LANE-TUI-LAND, LANE-CI-TOKEN, LANE-DEFAULTTUI-FIX, LANE-SHAPES.

## Remaining RED / implementation / integration / verifier lanes

- Not-started plan tasks (40): COORD-001..008 (8, chain: COORD-001 first, deps AUD-017 done),
  DISC-101 (deps AUD-001+COORD-003), DISC-118 (deps AUD-016 blocked -> device-gated),
  DISC-119 (deps AUD-017+COORD-001), NET-001..015 (15, chain from NET-001),
  MOB-001..006 (6, chain into MOB-006 device-gated), SHIP-001..008 (8, release chain).
- In-progress (must finish or diagnose): APP-012, ROUTE-010, GUARD-SHARE-001, APP-012-TOOL-RED
  (RED-ready sha 945236c4...), plus 20 wave-5 LANE-* lanes on origin (AGENTS-LOAD, AUTH-SETUP,
  BOOTSTRAP, CANVAS-RENDER, CLI-ONCE, CMD-LOAD, DCLIENT-DOCS, MCP-CONFIG, RULES-INJECT, SES-IMPORT,
  STREAM-FIX, THEME-RENDER, TUI-BEARER2, TUI-PAINT, ULTRA-LIVE, VALIDATE, VERSION, WEB-ACTIONS,
  WEB-BEARER, WF-LIVE; scratchpads `worklog/W5-*.md` absent locally = this branch behind origin).
- Integration lanes required (serialized, one owned file each): APP-012 read-path landing (done
  locally 40d56d5/2ff1e7b), DISC-108/109/111/112/113/114 mod prewires (orchestrator-done, must verify
  on merged tree), frozen-contract-review corrections (installed_default_entrypoint 5x todo!,
  native_artifact_manifest 2 assertions; controller authority only), DISC-003 78-row retire
  (controller only), Windows ps1 native-closure RED author (needs Windows runner).
- Verifier lanes required: independent rerun of every frozen suite on exact integrated revision;
  convergence_gate + validate_repository must go GREEN (controller-owned fixes); SHIP-004 full-scope
  rerun; SHIP-006 soak; SHIP-003 live canaries (budgeted, credentials-gated).

## Conflicts / owned-file collisions

- `crates/cli/tests/installed_default_entrypoint.rs`: INSTALLED-DEFAULT-CONTRACT vs APP-012 parent RED.
- `crates/server/src/lib.rs`: FIX-STREAM turn path vs integrator wiring patches.
- `crates/tools/src/lib.rs`, `crates/server/src/lib.rs` mod lines: prewire centrally, never in lanes.
- `crates/cli/src/main.rs, app_start.rs, tui_entry.rs`: APP-001 repair vs LANE-TUI-LAND vs LANE-DEFAULTTUI-FIX.
- `crates/tools/src/mcp_config.rs` + frozen `tests/mcp_config.rs`: impl complete, test-owner fix pending.
- `tasks/completion/claims.json`: only via `tools/completion_claims.py`; orchestrator owns release/reclaim.
- `.github/`, `ralph.json`, verifier config: integration authority + CODEOWNER only.

## Parallelism (which can run together)

- Parallel-safe now: COORD-001 RED-author + NET-001 RED-author + DISC-101 RED-author (distinct files,
  distinct deps) + any missing-artifact rebuild lane with a defined owned file + verifier reruns
  (serial heavy validation: one at a time, CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, 8GiB budget).
- Blocked behind controller: DISC-003 retire, frozen-test corrections, SESS-008 retire/decompose,
  PROV-023 catalog entry, INT-010 ownership assignment, COORD-002..008 (need COORD-001).
- Blocked on external: SHIP-001 signing (macOS notarization + Windows signing identities),
  SHIP-003 canaries (credentials/budget), SHIP-005/MOB-006/DISC-118 (devices), MSVC (out of scope:
  Windows ABI is x86_64-pc-windows-gnu only per 2026-09-23 controller decision).
- Blocked on platform runners: Windows GNU release closure + ps1 RED (Windows runner), Linux PTY
  matrix (Linux runner); macOS-only evidence is not cross-platform proof.

## Effort ranges (per lane, rough)

- RED-author lane (defined contract): S-M (1-3 sessions).
- Implementation lane (owned file, wired): M (2-5 sessions).
- Repair-child wiring (approval-resume channel, executor read/grep/write/edit, persistent keyring
  setup via keyring 3.6.3 MSRV-compatible): M-L each (3-8 sessions).
- Rebuild lane (missing artifact): S-M. Frozen-contract review + refreeze: S + controller decision.
- Integration + verifier rerun per wave: S-M, serialized.
- SHIP chain (001..008): L-XL; SHIP-005/006 device/soak longest; external blockers unbounded.

## Critical path to unsigned Phase 1 release

1. Controller rulings (DISC-003 retire, frozen corrections, SESS-008 decompose, PROV-023 child,
   INT-010 ownership) -> gates unblock.
2. APP-001 repair + APP-005 secure-red + APP-012 tool/approval/restart REDs GREEN (macOS source).
3. TUI-011 vendor/copy lanes + ps1 closure RED on Windows runner + Linux PTY matrix.
4. APP-012 golden journey GREEN on packaged artifact (macOS first) -> SHIP-001 unsigned artifacts
   (signing steps recorded blocked, artifacts reproducible + checksums/SBOM).
5. SHIP-002 clean-machine matrix (macOS/Linux/Windows-GNU; MSVC absent by decision, signing blocked
   external) -> COORD chain + SHIP-004 full-scope rerun + SHIP-006 soak + SHIP-007 docs.
6. SHIP-008 decision: remains BLOCKED while any mandatory parent/evidence missing; signing +
   devices + live canaries stay explicit external blockers, never fabricated.

## Wave design (20-worker harness: 4 integration-spine + 2 verifier reserved, <=14 breadth)

- Spine (4): S1 APP-012 parent RED/integration; S2 TUI-011 packaging/vendor integration;
  S3 turn-path wiring (executor tools + approval-resume); S4 setup/keyring persistence wiring.
- Verifier (2): V1 frozen-rerun + gate tracker (convergence/validate, serialized heavy runs);
  V2 platform-evidence auditor (source vs packaged vs clean-machine matrix ledger).
- Breadth (<=14, pick ready): COORD-001, NET-001, DISC-101 RED-authors; PROV-023 child (once catalog
  entry approved); missing-artifact rebuilds with defined files (AUD-008-LEDGER, TUI-010-CAPS,
  WEB-ARTIFACT-EXEC, WEB-ATTACH-TRANSMIT); FIX-MCP-STUBS test-owner proposal; wave-5 LANE
  completions; SHIP-007 docs draft (no acceptance). No wave of leaves-only: each wave must land
  >=1 integration/wiring improvement.

## Platform / proof ledger

- macOS: strongest (builder matrix arm64+x86_64 GREEN, PTY journeys GREEN source-built).
- Linux: builder matrix GREEN (gnu x86_64+aarch64 libs), no PTY/installed proof here.
- Windows-GNU: DLL+import-lib pair proven constructible (shas in worklog/TUI-011.md); no on-target
  proof, ps1 closure missing, no release workflow, no pwsh/gh on this host.
- Windows-MSVC: explicitly out of scope (no pinned target; fail-closed).
- Source GREEN != packaged proof != clean-machine proof; SHIP-002/004/006/008 require the latter two.
- Signing/notarization: external blocker, only unsigned CI runners available.

## Decisions / unknowns

- No product code written (lane authority forbids). Branch is behind origin/main (missing W5
  scratchpads + 8a91a7b wave commit); rebase plan/phase1-roadmap before integration use.
- Ledger row stays `in-progress`; no acceptance claimed. Verifier must confirm FIX-STREAM env_lock
  claim and wave-5 LANE outcomes independently.

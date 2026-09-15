# Integration proposal: pending native lanes

Status: PROPOSAL ONLY. This file does not change controller state, acceptance
flags, frozen tests, verifier configuration, accounting, or release criteria.

## Current revision evidence

- Repository source was inspected at `3db7402` as reported by the prior triage
  session. The working tree is not clean; `git status --short` shows modified
  shared files and numerous untracked lane artifacts.
- `PLAN.md:113-120` assigns acceptance and verifier ownership to a trusted
  controller, not the implementer.
- `PLAN.md:149-155` requires serialized integration for shared registries,
  manifests, schemas, lockfiles, and central `lib.rs` wiring.
- `docs/TDD.md:32-41` requires a compiling RED, frozen test hash and command
  manifest before implementation, followed by integrated verification.

## Blocked reconciliation

`python3 tools/validate_repository.py` currently reports 122 errors. The
failures include missing non-accepted stories (`PROV-015` through `PROV-024`,
`TOOL-016` through `TOOL-020`, and other newly materialized cards), stale
`FEATURES.md` statuses, stale ownership-gap classifications, and controller
stories appearing in the exhaustion ledger. These are protected reconciliation
surfaces and must be handled by an authorized integration lane.

No changes are proposed to `ralph.json`, `FEATURES.md`,
`requirements/user-requirements.json`, `sources/backlog-exhaustion.json`,
`.github/CODEOWNERS`, `.github/protection-policy.json`, frozen tests, or
verifier configuration.

## Canonicalization decisions required

The integrator should resolve these collisions before accepting additional
implementation lanes:

1. `OPS-006`: choose between `crates/foundation/src/repo_ref.rs`,
   `ops_repo_ref.rs`, and the uncommitted `repo_ref_ext.rs` candidate.
2. `SESS-006` and `SESS-007`: compare their requested state/archive surfaces
   with existing `state.rs`, `lifecycle.rs`, and `archive.rs`; do not overwrite
   existing ownership without a transfer decision.
3. `PROV-008`: compare the pending card with existing accepted
   `crates/providers/src/metrics.rs`.
4. `TOOL-011`: compare the pending card with existing accepted
   `crates/tools/src/diff_tool.rs`.
5. `PROV-023` and `PROV-024`: validate the untracked catalog and fixture tree,
   freeze their test/manifest receipts, then reconcile their status.

## Candidate implementation lanes held

The following paths are absent and appear uniquely owned by their cards, but
they are not ready for implementation in this worktree because their RED tests
are not present or frozen and server `lib.rs` wiring is integrator-owned:

- `crates/server/src/acp_bridge.rs` (`ACP-001`)
- `crates/server/src/acp_files.rs` (`ACP-002`)
- `crates/server/src/sync_log.rs` (`SYNC-001`)
- `crates/server/src/workspace_proxy.rs` (`WSX-001`)
- `crates/server/src/sdk_client.rs` (`SDK-001`)
- `crates/server/src/sdk_spawns.rs` (`SDK-002`)
- `crates/sessions/src/part_events.rs` (`SYNC-002`)
- `crates/sessions/src/runner.rs` (`RUN-001`)

The card contracts are useful design evidence, but they are not a substitute
for the trusted RED/freeze receipt required by `docs/TDD.md:43-66`.

## Proposed safe order

1. Authorized controller lane reconciles story/status/accounting conflicts and
   records canonical ownership decisions.
2. Trusted test author creates and runs one compiling RED target for exactly one
   absent module, then freezes its hash and command manifest.
3. One implementation worker owns only that module file and reports evidence.
4. Integrator wires the module through the shared `lib.rs` and runs the focused
   target on the exact integrated revision.
5. Repeat serially for independent lanes, then run the mandatory harness lanes
   and repository gates under the documented memory limits.

## Verification blockers preserved

- Mandatory harness lanes remain `UNRUN`: `test:integration_v2`,
  `test:import_v2`, `test:quota_v2`, and `test:perf_modules_v2`.
- The full tools suite still has the timing-sensitive
  `executor::tests::execute_success` failure (`duration_ms > 0`), while its
  focused test passes. No test or unrelated executor code should be weakened.
- `cargo fmt --all -- --check` still has unrelated pre-existing differences.
- No acceptance, commit, push, merge, deployment, or controller reconciliation
  has occurred.

# DISC-003 - Extract all public behavior surfaces

Status: IN PROGRESS. Kind: enabler-or-assurance. Runtime optional: False.
Mandatory for full declared release: yes. Seed source audit: unresolved.

## User-observable outcome
Catalog routes CLI commands config keys events DB schemas tools hooks SDK exports UI actions and feature flags

## Prerequisites and exclusive ownership
Dependencies: DISC-002.
Ownership locks: feature:DISC-003.
Suggested module: `crates/discovery/src/features/disc_003.rs`.
Acceptance fixtures: `tests/acceptance/DISC-003/`.
Contracts/schema/build-manifest edits go through the integration lane, not competing agents.

## Source reconnaissance before coding
Read the locked commit, source evidence OC-TREE, OC-CONTEXT, OC-RUNNER, NR-ARCH, and `sources/ownership-candidates.json`.
The selectors are hints, not proof that a file exists or that V2 implements this feature.
Record exact source file and line range, callers, callees, schemas, error variants, side effects,
configuration precedence, CLI/API/UI entrypoints, and upstream test IDs in `source-map.json`.
If more behavior is found, propose a required child slice. Never silently shrink scope.

## Acceptance criteria
- Catalog routes CLI commands config keys events DB schemas tools hooks SDK exports UI actions and feature flags
- Treat dynamic registration and regex extraction gaps as unresolved findings requiring source review
- Chunk large files and retain hashes rather than duplicated source copies
- Every applicable public entrypoint reaches the same policy and persistence contract.
- All named tests have independently captured execution evidence; stubs and skipped mandatory tests do not pass.

## Test-first execution
1. Scout establishes behavior and explicit deviations from reference.
2. Independent test author creates black-box tests and captures the baseline.
3. Trusted verifier runs tests: compilation succeeds, intended behavior fails (RED).
4. Controller freezes test and acceptance hashes. Implementer changes only the owned code.
5. Trusted verifier executes the frozen tests and required suites (GREEN).
6. Independent reviewer checks hidden interactions and attempts to falsify the result.
7. Integrator merges serially and verifies the integrated tree again before acceptance.

Documentation and inventory enablers need checkable evidence; any accompanying code still uses RED/GREEN.

## Named test obligations
- **DISC-003-T01 (behavior):** Catalog routes CLI commands config keys events DB schemas tools hooks SDK exports UI actions and feature flags
- **DISC-003-T02 (negative):** Treat dynamic registration and regex extraction gaps as unresolved findings requiring source review
- **DISC-003-T03 (resource):** Chunk large files and retain hashes rather than duplicated source copies
- **DISC-003-T04 (lifecycle):** Exercise cancellation, restart or reload at each relevant state transition; observe durable state and absence of leaked resources. Mark non-applicable only with verifier-reviewed rationale.
- **DISC-003-T05 (reference):** For parity: replay pinned reference fixtures and compare declared observations. For deliberate additions: assert the recorded security or product invariant and retain a reviewed expected deviation.

## Scratchpad contract
Keep `source-map.json`, `progress.md`, `open-questions.json`, `decisions.md`, and evidence references
inside this task's workspace. Include facts with source locations, not unsupported summaries.
Persist work before a context reset. Do not embed raw credentials or private user source in receipts.

## Completion and failure
The implementer cannot edit `passes`, the trusted test set, global scope, policies, or budgets.
Return a candidate with exact source tree hash and evidence locations. Only the controller accepts it.
Missing authority/credentials/platform support or repeated failures become BLOCKED with a typed reason.
Never retry by using an unsandboxed command or accepting an untested compatibility shortcut.


## Current DISC-003 slice

The reconciliation remains in progress and is not accepted. It preserves all 32 DISC-002 candidate families, now with all 32 represented by pinned reviewed `partial` records and zero queued path-derived candidates. The latest bounded slices attach pinned evidence for the remaining `opencode.app-client`, `opencode.clients-ui`, and `opencode.desktop-client` families in addition to the previously reviewed OpenCode and 9router surfaces. The three client families are evidence-only: browser/router/SDK/terminal behavior, TUI renderer/process/plugin/runtime behavior, and Electron/sidecar/WSL/IPC behavior are concrete upstream contracts, but their mapped UI/WEB stories remain unowned at a one-to-one task boundary and the target Rust workspace has no approved client/TUI/desktop architecture or dependency set. BASE-005 remains accepted and is not re-owned through the desktop family. Dependency-constrained UI/TUI/desktop product work remains explicitly excluded from the current lane. Zero queued candidate families is **not** DISC-003 completion: every record remains `partial`, explicit unresolved findings remain, full-tree dynamic/generated/platform traversal is still incomplete, and verifier/controller acceptance remains external. The reconciliation status therefore stays `in-progress-not-release-evidence`; no product task, `ralph.json` acceptance field, or verifier/controller state is changed by this source inventory. The validator continues to reject scope shrinkage, wrong-repository/unpinned evidence, unjustified implementation status, and falsely reconciled records that lack required evidence. See `sources/disc-003-reconciliation.json`, its hash-bound manifest, and `workspaces/DISC-003/`.

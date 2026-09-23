# GUARD-EXT-009 reconciliation audit

Claim: EXT-009, session subagent-guard-EXT-009, scratchpad worklog/GUARD-EXT-009.md.
Worktree: guard-EXT-009 (branch lane/GUARD-EXT-009-20260923, HEAD 06ed486). Audit only; no canonical/product/test/validator edits.

## Source evidence (guard worktree paths)

- ralph.json EXT-009 (lines 970-984): status accepted, requirementIds [REQ-005], userStory "TBD - see source audit", dependencyIds [], testObligations EXT-009-T01..T05.
- tasks/EXT-009.md: EXISTS (9.6K, product card, REQ-005, namespacing-only contract, MAX_NAMES=512/MAX_NAME_LEN=64, EXT-009-T01..T05, suggested module `crates/ext/src/namespacing.rs` + `crates/ext/tests/plugin_namespacing.rs`, verification references `cargo test -p opencode-rk-ext`).
- worklog/EXT-009.md: EXISTS (76 lines, prior lane GREEN 5/5, frozen sha b87649f0, impl pre-complete at HEAD 248f519).
- sources/backlog-exhaustion.json EXT-009 row (lines 326-340): category unresolved-decomposition, controllerStatus in-progress, reasonKey extensibility-family-not-decomposed, requirementIds [REQ-005], surfaceIds [opencode.extensibility], taskCard null, worklog null, implementationCommits [], reopenPolicy source-grounded-task-decomposition-required.
- sources/extensibility-remaining-ownership-gap.json EXT-009 entries (lines 11-63): ownershipDecision EXT-009 null, taskBindingState EXT-009 {controllerStatus in-progress, ralphStory "TBD - see source audit", requirementIds [REQ-005], taskCard null, worklog null}, REQ-005 topology conclusion: "EXT-009 and EXT-012 remain requirement-identical. Requirement subtraction does not define which plugin lifecycle or UI-adjacent behavior belongs to either row."
- FEATURES.md EXT-009 rows (lines 52, 137, 214, 622): accepted placeholder, userStory "TBD - see source audit", obligations T01..T05.
- ralph.completion.json contract: legacyAcceptedIsReleaseEvidence false (line 26).

## Existing implementation/test evidence (distinguished from ownership/acceptance)

Product implementation that pre-exists this lease (not introduced by this lane):
- crates/tools/src/plugin_namespace.rs EXISTS (124 lines, real code, `#![forbid(unsafe_code)]` via crate-level, `pub mod plugin_namespace` wired at crates/tools/src/lib.rs:45).
- crates/tools/src/ext_namespacing_lane.rs EXISTS (124 lines, byte-identical twin of plugin_namespace.rs per EXT-DEDUP.md:4 line 13 + EXT-TWINS-DISPOSITION.md:19).
- crates/tools/tests/plugin_namespace.rs EXISTS (214 lines, frozen EXT-009-T01..T05, uses `#[path = "../src/plugin_namespace.rs"]`).
- crates/tools/tests/ext_namespacing_lane.rs EXISTS (198 lines, lane companion test, 5/5 GREEN per EXT-VERIFY-3.md:12).
- crates/ext/ DOES NOT EXIST. `cargo test -p opencode-rk-ext` in tasks/EXT-009.md verification section does not resolve. Implementation lives in opencode-rk-tools, not opencode-rk-ext.

Caller/wiring: no product code outside crates/tools references plugin_namespace or ext_namespacing_lane (EXT-TWINS-DISPOSITION.md:10 grep = 0 hits outside src|tests). Namespace is a pure in-memory struct with no registered caller in lib.rs or server.

RED-VALIDITY evidence from prior lanes (read-only, not reproduced here):
- RED-VALIDITY-EXT2B.md:40: EXT-009 stub `Namespace::register` returns `Err(Overflow)` first => 0 pass / 5 fail (T01-T05). Restore byte-identical (cmp exit 0, sha pre==post).
- RED-VALIDITY-EXT2C.md:40: same EXT-009 stub => 0 pass / 5 fail; GREEN 5/5; `cmp` identical, sha pre==post.
- EXT-VERIFY-3.md:12: 5/5 canonical + 5/5 lane, 0/4 hash match vs post-edit dedupe shims (shims reverted/unlanded).

## Observed validator output (read-only, guard worktree)

- No `python3 tools/lane_gate.py` entry for EXT-009 (lane_gate.py LANES list covers storage crates only; EXT-009 not present).
- No `python3 tools/validate_repository.py` EXT-009-specific error (that validator is repo-wide; per INTEGRATION-19:215 EXT-009 shows "yD 5/5 + ACC-d; RED xD 0/5" in convergence table).
- No `python3 tools/validate_backlog_exhaustion.py` run on this worktree (no heavy commands per task constraint). Gap status from sources JSON: EXT-009 row records taskCard null / worklog null / category unresolved-decomposition, consistent with acceptance-not-release-evidence contract.

## Attributable guard findings for EXT-009

1. Source-card drift: tasks/EXT-009.md:94-98 suggests `crates/ext/src/namespacing.rs` + `crates/ext/tests/plugin_namespacing.rs` + `cargo test -p opencode-rk-ext`, but no `crates/ext` crate exists; code is wired in `crates/tools` (`opencode-rk-tools`). The task card verification command fails to resolve. This is a card/authoritative-spec drift, not a code gap: implementation + tests exist and are GREEN in the correct crate.
2. Ownership gap: sources/extensibility-remaining-ownership-gap.json records ownershipDecision EXT-009 null and taskBindingState taskCard null / worklog null. The on-disk task card + worklog prove existence but not task-specific ownership: EXT-009 and EXT-012 share identical surface signature [opencode.extensibility] and REQ-005; closure criteria forbid ownership by surface arithmetic or task order. The prior EXT-009 worklog explicitly notes "Verifier acceptance external, not claimed here."
3. Acceptance gap: ralph.json status accepted + FEATURES.md accepted rows are explicitly not release evidence per ralph.completion.json contract (legacyAcceptedIsReleaseEvidence false). Controller status per sources is in-progress / not-started (backlog-exhaustion.json records taskStatus null). No acceptance certificate attaches EXT-009.
4. Twin duplication: ext_namespacing_lane.rs is byte-identical to plugin_namespace.rs (EXT-TWINS-DISPOSITION.md:19). Both compile and pass frozen tests independently via `#[path]` includes. The dedupe shim conversion (EXT-009-DEDUP.md) was reverted/unlanded; integrator owns twin cleanup. This is a maintenance gap, not a correctness gap: 10/10 tests GREEN (5 canonical + 5 lane).
5. Caller wiring gap: Namespace/NameDecl/EntryKind are defined and testable but no product code registers or calls Namespace through a real entrypoint. The contract is purely in-memory with no persistence or service binding. INT-001 (ext_commands compat) and skill_commands/skill_gate/skill_defs reference skill/command tables but do not import plugin_namespace. This is a wiring gap: the unit is implemented and tested in isolation but unwired into any caller path.

## Disposition: BLOCKED (source-grounded)

EXT-009 has real implementation (plugin_namespace.rs), real frozen tests (5/5 GREEN), and a passing RED-VALIDITY stub-bite, but the guard audit surfaces persistent source-level gaps that are outside this lane's authority to fix:

- Ownership not established: sources record ownershipDecision null + taskCard null + worklog null for EXT-009. The on-disk card/worklog came from a prior lane, not source-grounded controller decomposition distinguishing EXT-009 from the requirement-identical EXT-012. Closure criteria (sources/extensibility-remaining-ownership-gap.json:61-67) require task-specific distinguishing evidence (inputs/outputs/failure-state/ownership-lifetime/authority bounds) that does not exist.
- Task-card spec drift: tasks/EXT-009.md verification references `crates/ext` and `cargo test -p opencode-rk-ext`, neither of which exists; the real crate is `opencode-rk-tools`. Correcting the card is controller/authoritative scope.
- Acceptance not release evidence: ralph.json/FREATURES.md accepted rows are explicitly non-authoritative per completion contract; no verifier acceptance attaches.
- Twin duplication: ext_namespacing_lane.rs byte-duplicate of plugin_namespace.rs; dedupe shim not landed; integrator-owned.
- No caller wiring: Namespace is implemented-test-isolated but has no registered product caller or service binding.

No authority patch exists inside guard bounds (audit-only lane, no canonical/product/test/validator edits per task constraint). Lawful change requires controller-authored source-grounded decomposition (distinguish EXT-009 from EXT-012 per closure criteria), task-card spec correction (crates/ext -> crates/tools path), integration of twin dedupe, caller wiring of Namespace into a real entrypoint, and deliberate reconciliation of sources/extensibility-remaining-ownership-gap.json + sources/backlog-exhaustion.json + ralph.json under integration authority.

## Authority patch fields

- None (audit only, blocked, no product/test/validator edits permitted in this lane).
- Controller-only follow-ups:
  1. Distinguish EXT-009 vs EXT-012 per sources/extensibility-remaining-ownership-gap.json closureCriteria (task-specific binding evidence); reconcile taskCard/worklog null + controllerStatus in-progress -> accepted-with-evidence.
  2. Correct tasks/EXT-009.md verification section: `crates/ext` -> `crates/tools`, `cargo test -p opencode-rk-ext` -> `cargo test -p opencode-rk-tools --test plugin_namespace --test ext_namespacing_lane`.
  3. Land EXT-009-DEDUP shim conversion for ext_namespacing_lane.rs (byte-identical twin -> re-export of plugin_namespace) or delete lane twin + migrate test, per EXT-TWINS-DISPOSITION.md recommendation.
  4. Wire Namespace into a real caller (e.g. skill_commands.rs or plugin_lifecycle.rs registration path) and add a caller-visible test obligation, closing the unwired isolation gap.

## Verification (read-only, no Cargo/network/heavy)

- git status/branch/log: on lane/GUARD-EXT-009-20260923, HEAD 06ed486, claims.json modified only (EXT-009 row added).
- File existence checks: tasks/EXT-009.md (exists), worklog/EXT-009.md (exists), crates/tools/src/plugin_namespace.rs (exists), crates/tools/src/ext_namespacing_lane.rs (exists), crates/tools/tests/plugin_namespace.rs (exists), crates/tools/tests/ext_namespacing_lane.rs (exists), crates/ext/ (absent).
- lib.rs grep: plugin_namespace wired at crates/tools/src/lib.rs:45.
- Prior GREEN evidence: EXT-VERIFY-3.md:12 (5/5 canonical + 5/5 lane = 10/10), RED-VALIDITY-EXT2B.md:40 and RED-VALIDITY-EXT2C.md:40 (0/5 RED stub-bite, 5/5 GREEN restore).
- No heavy commands run; no Cargo build/test executed in this lane.

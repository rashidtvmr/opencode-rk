# GUARD-INT-009

## Audit scope

- Audit-only. Owned files: this scratchpad plus the `INT-009` ledger row.
- No canonical, product, test, validator, Ralph, FEATURES, or source-record edits.
- Audit tree: `b3e0d012822da9fe48e43126c9085e9782c2b082`.
- Pinned upstream: OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41`.

## Exact state reconciliation

| Surface | Evidence | Observed state | Guard result |
|---|---|---|---|
| Ralph | `ralph.json:1143-1154` | `INT-009` is `accepted`; generic DISC-002 story; no requirements | Conflicts with card and source ledger |
| FEATURES summary | `FEATURES.md:56-63` | `INT-009` accepted, `I18 s7 r20`, quiesced-tree caveat | Acceptance mirror is not independent acceptance |
| FEATURES family | `FEATURES.md:627-640` | `INT-009` accepted, generic story, no requirements | Same stale broad story as sibling INT rows |
| Task card | `tasks/INT-009.md:1-7` | `Status: NOT STARTED`; five frozen obligations | Conflicts with Ralph/FEATURES |
| Task card boundary | `tasks/INT-009.md:9-20,72-111` | Pure caller-supplied resolution only; no FS/git/DB/Location runtime | Narrow slice, not full integration behavior |
| Source ownership gap | `sources/integrations-ownership-gap.json:8-46` | Six residual INT rows share `opencode.integrations`; all `ownershipDecision` values null; INT-009 controller status `not-started`, task/worklog null | Task-specific ownership remains unresolved |
| Backlog ledger | `sources/backlog-exhaustion.json:657-679` | `INT-009` is `unresolved-decomposition`; no implementation, task card, or worklog recorded | Stale after local lane artifacts appeared |
| Completion ledger | `tasks/completion/claims.json:400-404` | This guard row is `in-progress` under this session | Must end `blocked`, not completed |
| Completion receipt | `ralph.completion.json` | No `INT-009` match | No completion receipt found |
| Existing worklog | `worklog/INT-009.md:58-92` | Claims frozen hash, GREEN 5/5, pure implementation; acceptance explicitly external; unresolved discovery/persistence gaps remain | Candidate evidence, not controller acceptance |

## Guard reproductions

1. `python3 tools/validate_backlog_exhaustion.py` -> exit `1`, `51 error(s)`.
   INT-009-specific output: `INT-009: integrations ownership-gap is stale after Ralph semantics changed`.
   Same run reports controller-accepted stories in the exhaustion ledger, summary drift, and deliberate-review drift for sibling ownership gaps.
2. `python3 tools/validate_repository.py` -> exit `1`; ruleset/readback/protection pass, then backlog exhaustion fails with the same 51 errors. Validator entrypoint is `tools/validate_repository.py:21-28`; integration checks are `tools/validate_backlog_exhaustion.py:2021-2063`.
3. `python3 tools/convergence_gate.py` -> exit `1`, `CONVERGENCE BLOCKED`, `total=80`. It reports off-plan completed ledger rows and self-disqualifying completion notes (`AUD-017`, `AUD-020` no acceptance). This is repository-wide, not attributable solely to INT-009.

The INT validator contract hard-codes `INT-009` expected status `not-started` while siblings are `in-progress` (`tools/validate_backlog_exhaustion.py:2039-2051`), requires unresolved ownership (`sources/integrations-ownership-gap.json:32-46`), and rejects any task/worklog appearance (`tools/validate_backlog_exhaustion.py:2061-2063`). Current Ralph/FEATURES/card/worklog/artifacts cannot all satisfy that contract without an authorized controller reconciliation.

## Source and ownership findings

### Duplicate lane implementations

- Canonical candidate: `crates/providers/src/location_ctx.rs`, exported by `crates/providers/src/lib.rs:37`.
- Duplicate fallback candidate: `crates/providers/src/int_location_lane.rs`; its header explicitly says `fallback lane` and `Distinct fallback file`.
- Both expose the same `resolve_request` and `resolve_session` functions (`location_ctx.rs:152-205`; `int_location_lane.rs:153-207`) and duplicate all public types/validation/identity rules.
- Duplicate frozen test suites: `crates/providers/tests/location_ctx.rs:6-7` and `crates/providers/tests/int_location_lane.rs:7-8`; both contain T01..T05.
- Hashes at audit time: `location_ctx.rs` `d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b`; `int_location_lane.rs` `f29d89753ef8da4bf91637097c67e2c05c434bbb00594ebfa10e147997812df0`; tests `638f265fb6b3515b1aabbcfa313629880b13396661c8560ba5b184662959e1d9` and `b7e260ee44fc3d53a39ffb2e4712e966f0139ae94d7493578ddf6cc3fb30aa44`.
- `int_location_lane` is not exported by `crates/providers/src/lib.rs`; both implementations are test-reachable, while only `location_ctx` is library-exported. This is duplicate lane work, not proof of two product owners.

### Caller and protocol blockers

- `grep` finds no production caller of either resolver; matches are limited to the two module definitions and their tests. `pub mod location_ctx` is not a caller path.
- Upstream source partition records request callers in `packages/server/src/location.ts:11-60` and `packages/server/src/middleware/session-location.ts:15-67`, with protocol evidence in `specs/v2/api.html:424-516` (`sources/integrations-ownership-gap.json:161-180`). No native Rust server/session caller is established by this slice.
- The source record states all six residual IDs have the same surface/signature and no task-specific binding (`sources/integrations-ownership-gap.json:8-46`); generic client/server contract, generated clients, PTY transport, project/location, VS Code bridge, and auth lifecycle cannot be assigned to INT-009 by row number.
- `sources/integrations-ownership-gap.json:343-349` explicitly leaves project-location candidate ownership false. The task card narrows this to pure resolution, but canonical source ownership still requires controller review.

### Enterprise/spec, auth, and resource blockers

- No genuine upstream identity-algorithm spec is recorded; the task card classifies the identity precedence as partial upstream plus safer pure deviation (`tasks/INT-009.md:62-65`). The enterprise remote missing-spec surface is separately excluded as `INT-010` (`sources/integrations-ownership-gap.json:61-65`), not transferable to INT-009.
- Full upstream behavior requires filesystem/git discovery, remote/history reads, and session DB state (`sources/integrations-ownership-gap.json:175-180`). The native slice intentionally does none (`tasks/INT-009.md:108-111`), so it cannot claim caller authority, persistence, or Location runtime integration.
- Credential/OAuth/provider callback, environment, persistence, event, and network authority is separately mapped to accepted PROV/SEC plus local EXT-007/INT-004 exclusions (`sources/integrations-ownership-gap.json:199-215,299-311`). INT-009 must not absorb those auth/secret lifetimes through broad integration overlap.
- Resource limits in the pure slice are bounded strings and one synchronous result (`tasks/INT-009.md:129-141`), but the unresolved caller paths own FS/git/DB/network lifetimes. No end-to-end cancellation, timeout, persistence, or caller resource receipt exists for INT-009.

## Verdict

**BLOCKED.** Candidate pure resolver has local 5/5 evidence, but acceptance is prohibited by stale/conflicting controller records, unresolved six-row integration ownership, missing native caller wiring, duplicate fallback implementation/tests, absent full protocol/FS/git/DB authority, and separately owned enterprise/auth/resource surfaces. Controller/integrator must reconcile or retire/award the lane, select one implementation, wire a real caller, and rerun the frozen suite on the exact integrated revision. No product or guard-policy change made here.

## Verification

- `python3 tools/validate_backlog_exhaustion.py` -> exit `1`, 51 errors.
- `python3 tools/validate_repository.py` -> exit `1`, backlog exhaustion failure after protection checks.
- `python3 tools/convergence_gate.py` -> exit `1`, `CONVERGENCE BLOCKED`, total 80.
- `grep` production resolver callers -> only duplicate module definitions and test call sites.
- `git diff --name-only` before this audit -> only `tasks/completion/claims.json` (claim mutation); no canonical/product/test edits.

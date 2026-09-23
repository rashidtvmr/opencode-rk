# GUARD-SHARE-004 reconciliation audit

Claim: SHARE-004, session `ses_f32482b15ffeZP2080YbbYYvRo`, scratchpad `worklog/GUARD-SHARE-004.md`.
Worktree: `guard-SHARE-004`; branch `lane/GUARD-SHARE-004-20260923`.
Scope: scratchpad plus this task's ledger row only. No canonical, product, test, controller, or acceptance edits.

## Exact reconciliation

- `tasks/SHARE-004.md:1-7`: product task, mandatory, REQ-007, no dependencies, T01-T05.
- `tasks/SHARE-004.md:9-12`: local metadata lifecycle; secret call-local; no network, secret rows, or secret logging.
- `tasks/SHARE-004.md:27-34`: contract names `ShareMeta`, `ShareStore`, `ShareSecret`; suggested path `crates/share/src/store.rs` / crate `opencode-rk-share`.
- `tasks/SHARE-004.md:44-49`: bounds `MAX_SHARES=1024`, URL 2048 bytes, single-owner in-memory map.
- `tasks/SHARE-004.md:51-57`: frozen T01-T05 obligations, including duplicate, cascade, invalid URL, regression, cap, secret redaction, determinism, fixture-only writes.
- `ralph.json:2234-2247`: `SHARE-004` is `accepted`, user story remains `TBD - see source audit`, dependencies empty, T01-T05 attached.
- `FEATURES.md:79`: accepted; note says `unwired lane (by design) + quiesced re-run`. `FEATURES.md:230,757`: accepted/TBD source-audit placeholder.
- `sources/completion/legacy-evidence.json:1802-1808`: accepted, `statusIsReleaseEvidence=false`, `tbd=true`, repair lane `PAR-003`. Accepted is not acceptance/release proof.
- `sources/backlog-exhaustion.json:1548-1562`: unresolved-decomposition; `implementationCommits=[]`, `localEvidencePaths=[]`, `reasonKey=sharing-family-not-decomposed`, `taskCard=null`, `worklog=null`, controllerStatus `not-started`.
- `sources/sharing-ownership-gap.json:33-38`: `ownershipDecision.SHARE-004=null`.
- `sources/sharing-ownership-gap.json:120-167`: `share-metadata-persistence` has no owner; upstream input includes session/share ID/secret/public URL; output is a `session_share` row; secret-bearing user DB persistence is explicitly outside the authorized no-user-DB lane.
- `sources/sharing-ownership-gap.json:210-215`: merge/storage/secret service ownership remains unassigned.
- `sources/sharing-ownership-gap.json:257-290`: remote HTTP/share deletion/auth is a separate unresolved partition, not proof of local lifecycle ownership.
- `sources/completion/audits/AUD-015.json` (`assignedLegacy`, `legacySample`, `repairChildren`, verdict): SHARE-004 was audit-covered as accepted/TBD; audit verdict explicitly says no acceptance claimed and requires publish/revoke, pairing/tunnel, and authZ/cancel/bounds repair children.

## Existing implementation and wiring

- Task-card candidate `crates/share/src/store.rs` is absent; `crates/share/` and `opencode-rk-share` do not exist.
- Lane implementation exists at `crates/sessions/src/share_store.rs:1-248`; it is an in-memory secret-free `ShareStore` with `ShareSecret`, `ShareMeta`, `StoreError`, URL validation, `MAX_SHARES`, monotonic sync, remove, and session cascade.
- `crates/sessions/src/share_store.rs:8-10` explicitly says it is `#[path]`-included by tests and integrator wiring is deferred.
- `crates/sessions/src/lib.rs:4-42` exports `share`, `share_audit`, `share_count`, `share_expiry`, `share_invite`, `share_list`, `share_merge`, `share_policy`, `share_queue`, `share_revoke`, `share_scope`, `share_token`; it does not export `share_store`.
- Frozen test files exist at `crates/sessions/tests/share_store.rs` and `share_store_lane.rs`; both include the lane source with `#[path]`, therefore tests prove isolated module behavior only, not public crate wiring or a real caller journey.
- `worklog/SHARE-004.md:3-4,13-29`: prior implementation lane recorded the lane-variant ownership, frozen 5+5 GREEN evidence, and explicitly left `lib.rs` unwired; acceptance mapping remained unknown.
- `worklog/AUDIT-NONACCEPTED.md:19,78`: task-card crate paths are systematically absent; SHARE-004 is listed as in-progress with candidate path MISSING, lane variant present, and sessions wiring noted. This is a path/ownership reconciliation gap, not evidence of release completion.
- `worklog/INTEGRATION-19.md:239-243`: SHARE-004 row is in-progress versus ralph accepted, with `xI 3/2 + aA + yI/bA/DK` and the same unwired caveat.
- `worklog/SHARE-DEDUP2.md:40-44` and `SHARE-DEDUP3.md:37-41`: isolated suites reported 5/5 each for share_store and lane variants; these are not public wiring or acceptance evidence.

## Exact failures and blocker

1. Status split: ralph/FEATURES/legacy evidence say accepted, while the backlog ledger says controllerStatus not-started, taskCard/worklog null, implementationCommits empty, and ownershipDecision is null.
2. Path split: card requires `crates/share/src/store.rs` / `opencode-rk-share`; repository has no such crate. Existing code is a sessions lane variant.
3. Wiring gap: `crates/sessions/src/share_store.rs:8-10` and `crates/sessions/src/lib.rs:4-42` confirm the implementation is not exported. Frozen `#[path]` tests cannot prove an application caller.
4. Ownership gap: source review explicitly leaves secret-bearing persistence unowned. Any controller patch must separate/assign local secret-free metadata ownership from remote secret persistence, deletion, retry, HTTP, and auth partitions.
5. Acceptance gap: isolated GREEN evidence does not resolve controller ownership, path mapping, public wiring, or the audit repair children. No acceptance claim is made here.

## Controller patch fields/checks

Controller/integration authority must, outside this lane:

- bind `SHARE-004` to an explicit owner and canonical path, or retire/re-decompose the card into local metadata, secret persistence, remote HTTP, deletion/retry, and auth partitions;
- reconcile `sources/sharing-ownership-gap.json`: set `ownershipDecision.SHARE-004` only after source-grounded scope and secret lifetime are approved;
- reconcile `sources/backlog-exhaustion.json`: fill `taskCard`, `worklog`, `implementationCommits`, controller status, and evidence paths; clear `sharing-family-not-decomposed` only after all mandatory partitions have owners;
- reconcile `ralph.json`/`FEATURES.md` status and story text; do not treat legacy `accepted` as release proof;
- if the sessions lane variant is retained, explicitly register/export the real caller and map frozen tests to the canonical module; otherwise create the approved crate/path through a product lane;
- require checks for: secret absent from durable state and Debug/log output; create rollback on remote failure; delete/session cascade; bounded URL/record counts; authorization denial with no side effects; cancellation/restart; and no mutation of the user's existing DB.

## Disposition

BLOCKED. No lawful controller/product/test change exists within this guard lane. Blocker is the unresolved source-grounded ownership/path/wiring/acceptance reconciliation above. This lane records no acceptance and performs no heavy verification.

## Verification

- Read-only path/status checks only; no Cargo, workspace build, test, migration, network, or database commands.
- `git status --short` before artifact: pre-existing only `tasks/completion/claims.json` after claim; no product/test files changed.

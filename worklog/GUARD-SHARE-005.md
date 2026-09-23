# GUARD-SHARE-005 - reconciliation audit

## Claim and boundary

- Task: `SHARE-005`; session: `ses_f32482b14ffeqkB2k9JP0ZrLe4`.
- Candidate: `c8f6dcb39d98302f27f04a25897d3880e09e2c1b`.
- Scope: this audit file plus the `SHARE-005` ledger row only.
- No canonical plan, product source, test, validator, feature index, or source-gap edit.
- No commit or push. No acceptance claimed.

## Exact disposition

**BLOCKED. `SHARE-005` remains not accepted and cannot authorize release or parent
completion.** Applied authority patch: ledger row only, `in-progress` to `blocked`,
with this bounded blocker:

> Controller reconciliation required. Ralph/FEATURES say `accepted`, while the
> task card says `NOT STARTED`, the source gap keeps ownership `null`, and the
> backlog ledger classifies `SHARE-005` as unresolved decomposition with no task,
> worklog, or implementation receipt. Existing lane policy tests are isolated
> `#[path]` suites; transport twins are not wired into `crates/sessions/src/lib.rs`
> and no real share caller is proven. Preserve enterprise/share HTTP,
> support-admin, credential, secret-lifetime, auth, deletion, retry, and bounded
> queue blockers. Do not flip acceptance or infer ownership from lane variants.

The ledger patch is the only authority change made by this audit. The required
controller-only follow-up is an explicit reconciliation patch, not an
implementation patch:

1. Reconcile `ralph.json`/`FEATURES.md`, `sources/sharing-ownership-gap.json`,
   and `sources/backlog-exhaustion.json` under the canonical repository guard.
   They currently disagree; this lane cannot choose which protected authority
   wins.
2. Keep the unresolved decomposition and `SHARE-003` enterprise-remote missing
   specification unless a separately authorized source audit assigns exact
   ownership. Do not convert lane test receipts into product acceptance.
3. Before any future promotion, require a real caller/wiring path, direct
   legacy-versus-org auth and failure tests, explicit secret/user-DB lifetime,
   deletion confirmation, bounded queue/backpressure/retry semantics, and
   preserved support-admin human authority. Otherwise retain blocked status.

## Trace

### Plan, Ralph, card, index

- `PLAN.md:19-25`: sharing is mandatory full-release scope; safety remains
  capability-based. `PLAN.md:113-120` assigns acceptance to the trusted
  controller/verifier, not a worker.
- `ralph.json:2250-2263`: canonical Ralph currently reports `SHARE-005` as
  `accepted`, with `REQ-007` and five obligations.
- `tasks/SHARE-005.md:1-11`: card reports `NOT STARTED`, mandatory, and defines
  deterministic sync transport behavior. `:33` proposes nonexistent
  `crates/share/src/policy.rs` / `opencode-rk-share`.
- `FEATURES.md:3-5`: index says statuses are generated from canonical plan and
  every story is mandatory. `FEATURES.md:76-80`: `SHARE-005` is `accepted`, with
  caveat `unwired lane (by design) + quiesced re-run`.
- `prd.json:1795-1803`: flat export still reports `SHARE-005` as
  `not-started`, confirming accounting drift rather than acceptance evidence.

### Existing lane and worklog

- `worklog/SHARE-005.md:3-5`: lane owns
  `crates/sessions/src/share_policy_lane.rs` and `share_policy2_lane.rs`, not
  the card's nonexistent crate; it explicitly says no `lib.rs` edit.
- `worklog/SHARE-005.md:13-20`: both twins are pure policy candidates; tests
  include source by `#[path]`; no caller, I/O, DB, or real transport wiring.
- `crates/sessions/src/lib.rs:23-25`: only `share_merge`, unrelated
  visibility `share_policy`, and `share_queue` are wired. The two SHARE-005
  transport twins are absent. `worklog/SHARE-TWINS-DISPOSITION.md:33-49`
  confirms the visibility-module name collision and recommends no merge into
  `share_policy`.
- `crates/sessions/tests/share_policy_lane.rs:1-7` and
  `share_policy2_lane.rs:1-7`: frozen suites directly include lane files;
  passing these suites does not prove crate or application wiring.
- `worklog/SHARE-005.md:22-30` and `worklog/CONFIRM-P12.md:7-18`: prior receipts
  report both lane suites 5/5 and the ten share suites 50/50. These are
  candidate verification receipts only; no tests were run in this audit.

### Source gap and authority blockers

- `sources/sharing-ownership-gap.json:33-80`: all five sharing ownership
  decisions are `null`; `SHARE-005` has no task-card or worklog binding in the
  machine-checked source audit.
- `sources/sharing-ownership-gap.json:257-293`: enterprise share HTTP and the
  separate support-admin route require network, credentials, support authority,
  remote lifetime, and explicit ownership. Preserve this blocker.
- `sources/sharing-ownership-gap.json:296-324`: event subscription/coalescing
  has `boundEstablished: false`; upstream failure removes a batch before
  transport and has no retry/requeue contract. Do not advertise the pure policy
  as the queue implementation.
- `sources/sharing-ownership-gap.json:357-375`: unresolved partitions include
  local secret persistence, merge/snapshot bounds, migration, legacy/org auth
  and failure policy, queue backpressure/retry lifetime, deletion/secret
  lifetime, support-admin authority, and hosted sync/deployment.
- `worklog/DISC-003.md:443-451`: sharing decomposition remains unresolved;
  source/caller/test evidence is heterogeneous and no implementation is
  justified without task ownership and explicit bounds/authority.
- `sources/completion/audits/AUD-015.json:34-52,100-105`: shard audit found
  share/publish/revoke and account/device pairing absent; verdict is audit
  complete with no acceptance claim.

### Backlog, validator, convergence

- `sources/backlog-exhaustion.json:1565-1590`: `SHARE-005` is
  `unresolved-decomposition`, controller status `not-started`, reason
  `sharing-family-not-decomposed`, with no task card, worklog, local evidence,
  or implementation commit.
- `tools/validate_backlog_exhaustion.py:1650-1666` requires sharing-gap rows
  to remain `not-started` and rejects task/worklog appearance; this is why the
  current lane artifacts cannot be silently treated as ownership.
- `tools/validate_backlog_exhaustion.py:1698-1708` requires the corresponding
  backlog row to remain unresolved with no local task ownership.
- `rtk python3 tools/validate_repository.py`: exit 1. Protection checks passed;
  backlog exhaustion reported 51 errors, including accepted stories in the
  exhaustion ledger, summary drift, `SHARE-005` gap staleness after Ralph
  semantics changed, and task/worklog appearance requiring deliberate review.
- `rtk python3 tools/convergence_gate.py`: exit 1, `total=80`; 78 off-plan
  completed aliases plus `AUD-017`/`AUD-020` bad-note rows. This is an
  independent controller/integrator blocker, not evidence for SHARE-005.
- `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:13-69`: the exact authority
  algorithm for the 80-finding convergence ledger is controller/integrator-only;
  no worker may apply it here.

## Verification

- Claim succeeded through `tools/completion_claims.py` before this file was
  created.
- Existing receipts only: SHARE-005 primary 5/5, fallback 5/5; no heavy test
  command run in this audit.
- Validator and convergence commands above reproduced the blockers.
- Pending after ledger update: `python3 -m json.tool tasks/completion/claims.json`
  and `git diff --check` only. No product or test verification is claimed.

## Remaining unknowns

- Controller must choose and record one canonical reconciliation of the
  `ralph.json` accepted state versus the source-gap/backlog not-started model.
- No authority exists in this lane to select a canonical policy twin, wire it,
  assign the missing crate, or authorize enterprise/support/auth side effects.
- Acceptance, release readiness, and parent completion remain external
  controller/verifier decisions.

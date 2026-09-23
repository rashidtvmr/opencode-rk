# GUARD-SHARE-001: Guard reconciliation audit of the SHARE story family

Owned file (scratchpad only). Gate total=80. No canonical/product/test/verifier/config edits.

## Claim

- Task id: `GUARD-SHARE-001`
- Session: `ses_guard_share001`
- Scratchpad: `worklog/GUARD-SHARE-001.md`
- Branch: `lane/GUARD-SHARE-001-20260923`
- Ledger row: `tasks/completion/claims.json` -> `GUARD-SHARE-001`
- Revision: `c8f6dcb39d98302f27f04a25897d3880e09e2c1b` (HEAD at branch start)
- Final status: `blocked` (audit-only; no implementation/acceptance per task scope)

Prior report not reflected on disk (confirmed: working tree clean, no GUARD-SHARE-001 claim existed in `claims.json` before this claim). This scratchpad is the single owned artifact.

## Source evidence (exact path/line)

ralph.card (controller):
- `ralph.completion.json:3-4` status `implementation-required`; `includes` list (line 7); `contract` block (lines 23-36).
- `ralph.completion.json:37-57` `auditShards`: AUD-015 `Sharing, remote auth and identity gaps`, `legacyPrefixes:["SHARE"]`, `local:["crates/server/src"]`, `upstream:["packages/core/src/account.ts","packages/core/src/control-plane","packages/opencode/src/share"]`.
- `ralph.completion.json:59-64` `auditTests` (enumerated every assigned legacy story including accepted ones; trace installed entrypoint; run existing tests; exercise errors/authorization/cancellation/bounded resources; classify native/legacy/planned/new + repair children).

ralph.json (plan loader forces story statuses to `not-started`; ledger is progress record):
- `ralph.json:2190-2264` SHARE-001..005 userStories.
- `ralph.json:2190-2204` SHARE-001: requirementIds `["REQ-007"]`, status `accepted`, userStory `TBD - see source audit`, testObligations SHARE-001-T01..T05.
- `ralph.json:2206-2217` SHARE-002: requirementIds `[]`, status `accepted`, userStory `Discovered during DISC-002 surface extraction`.
- `ralph.json:2220-2231` SHARE-003: requirementIds `[]`, status `accepted`, userStory `Discovered during DISC-002 surface extraction`.
- `ralph.json:2234-2247` SHARE-004: requirementIds `["REQ-007"]`, status `accepted`.
- `ralph.json:2250-2263` SHARE-005: requirementIds `["REQ-007"]`, status `accepted`.

FEATURES.md:
- `FEATURES.md:139` REQ-007 `Session sharing` -> `SHARE-001, SHARE-004, SHARE-005`.
- `FEATURES.md:76-80` sync rows 34-38: SHARE-001..005 all `accepted` at `I18 s7 r34-r38, gate e`; rows 36-38 flagged `unwired lane (by design) + quiesced re-run`.
- `FEATURES.md:750-758` SHARE family story table (5 stories).

sharing-ownership-gap source:
- `sources/sharing-ownership-gap.json:1-14` `storyIds` SHARE-001..005; `status: source-reviewed-no-exact-task-owner`; `treeSha`, `commit` pins.
- `sources/sharing-ownership-gap.json:15-39` `storySurfaceSignatures` + `taskBindingState`: all five `controllerStatus: not-started`, `taskCard: null`, `worklog: null`.
- `sources/sharing-ownership-gap.json:40-93` `requirementTopology` REQ-007: tasks `SHARE-001,SHARE-004,SHARE-005`; conclusion: "requirement groups three sharing stories but gives no semantic partition... SHARE-002 and SHARE-003 are DISC-generated rows without requirement bindings."
- `sources/sharing-ownership-gap.json:94-103` `surfaceRulePatterns`: `opencode.sharing` -> `packages/core/src/share/**, packages/enterprise/**`; `opencode.enterprise-remote` -> `packages/enterprise/**, packages/function/**`.
- `sources/sharing-ownership-gap.json:104-118` `enterpriseRemoteGap`: path `sources/enterprise-remote-spec-gap.json`, status `searched-no-qualifying-in-surface-spec`, storyId `SHARE-003`, missingKinds `["spec"]`.
- `sources/sharing-ownership-gap.json:120-325` `reviewedPartitions` (5):
  - `share-metadata-persistence` (lines 121-168): evidence `packages/core/src/share/sql.ts:1-13`, caller `packages/opencode/src/share/share-next.ts:224-359`, test `packages/opencode/test/share/share-next.test.ts:138-224`, spec `specs/storage/remove-opencode-db.md:1-239`. ownershipBlocker: "No SHARE task is bound to the persistence row; native implementation would mutate the user database and persist a remote secret, which is outside the authorized no-user-DB lane without explicit ownership."
  - `deterministic-share-merge-and-secret-validation` (lines 169-216): evidence `packages/enterprise/src/core/share.ts:10-172`, test `packages/enterprise/test/core/share.test.ts:25-263`, caller `packages/enterprise/src/routes/api/[...path].ts:29-140`, spec same. ownershipBlocker: "mechanically portable but private inside a broader storage/secret service. REQ-007 maps SHARE-001/004/005 together and does not distinguish merge ownership. SHARE-002 generic, SHARE-003 cross-surface."
  - `legacy-share-snapshot-migration` (lines 217-255): evidence `packages/enterprise/src/core/share.ts:78-115`, test `:205-225`, spec same. ownershipBlocker: "migration/storage behavior, not a task-bound generic session-share contract; would invent persistent remote compatibility semantics."
  - `enterprise-share-http-and-support-admin` (lines 256-294): evidence `packages/enterprise/src/routes/api/[...path].ts:29-155`, caller `packages/opencode/src/share/share-next.ts:206-359`, test `:84-224`. ownershipBlocker: "Network, credentials, support-admin authority and hosted service lifetime are not authorized by a generic SHARE story. SHARE-003 additionally blocked by enterprise-remote missing-spec."
  - `share-event-subscription-and-coalescing-queue` (lines 295-325): evidence `packages/opencode/src/share/share-next.ts:112-204`, test `:227-324`. ownershipBlocker: "coalescing but not explicitly bounded; flush HTTP transport + credentials + user-DB state required; native subscriber/network work needs explicit task owner + target bound/backpressure contract."
- `sources/sharing-ownership-gap.json:326-356` `candidateFragments` (2): `deterministic-share-merge` + `share-event-coalescing`, both `ownershipEstablished: false` with disqualifiers.
- `sources/sharing-ownership-gap.json:357-366` `unresolvedPartitions` (8).
- `sources/sharing-ownership-gap.json:367-370` `historyReview`: locked-checkout-grafted-at-pinned-commit; `limitation`: pinned OpenCode checkout used for current share contracts only; unavailable historical intent not used to assign SHARE-001..005.
- `sources/sharing-ownership-gap.json:371-376` `closureCriteria` (4).

Validator:
- `tools/validate_backlog_exhaustion.py:27` `ENTERPRISE_REMOTE_GAP_STORIES = ("INT-010", "SHARE-003", "WEB-004")`.
- `tools/validate_backlog_exhaustion.py:90` `SHARING_GAP_STORIES = ("SHARE-001", "SHARE-002", "SHARE-003", "SHARE-004", "SHARE-005")`.
- `tools/validate_backlog_exhaustion.py:91-100` `SHARING_UNRESOLVED_PARTITIONS` (8 strings).
- `tools/validate_backlog_exhaustion.py:196` backlog list includes SHARE-001..005 + WEB-004.
- `tools/validate_backlog_exhaustion.py:253` `"SHARE": "sharing-family-not-decomposed"`.
- `tools/validate_backlog_exhaustion.py:584` residual per-surface evidence gaps must remain exactly INT-010/SHARE-003/WEB-004.
- `tools/validate_backlog_exhaustion.py:953` `OC-ENTERPRISE-SHARE`, `OC-FUNCTION-REMOTE` evidence keys.
- `tools/validate_backlog_exhaustion.py:1616` `req007.get("tasks") != ["SHARE-001","SHARE-004","SHARE-005"]` check.
- `tools/validate_backlog_exhaustion.py:1630-1634` storyId-to-requirement topology for SHARE-001..005.
- `tools/validate_backlog_exhaustion.py:1638` storyId default for generic-disc stories.
- `tools/validate_backlog_exhaustion.py:1709-1724` SHARE-003 enterprise-remote spec gap + ownership-gap residual linkage assertions.

Native code already present in candidate (already-implemented native modules; NOT introduced by this lane):
- `crates/sessions/src/share_merge.rs:1` `//! SHARE-001 deterministic share-snapshot merge + secret validation.`; `#![forbid(unsafe_code)]` line 5; caps lines 11-13; `ct_eq` lines 133-143; `validate` lines 146-152; `merge_share_records` lines 168-207; `apply_sync` lines 211-228.
- `crates/sessions/src/share_store.rs:1` `//! Local secret-free share-metadata lifecycle (SHARE-004).`; `#![forbid(unsafe_code)]` line 11; caps `MAX_SHARES=1024` line 20, `MAX_URL_BYTES=2048` line 22; `ShareId` Debug redacts suffix lines 40-48; `ShareSecret` borrow-only + Drop zeroize lines 50-77; `ShareMeta` no secret field line 87-93; `ShareStore::create` borrows `_secret: &ShareSecret` line 165 (never stored/cloned/logged); session-deletion cascade `remove_session` lines 213-218; URL validation lines 220-235; `redact_url` lines 238-248.
- `crates/sessions/src/share_queue.rs:1` `//! SHARE-002 bounded local share-event coalescing queue.`; `#![forbid(unsafe_code)]` line 10; `QueueCaps` default 4096 items / 8_388_608 bytes / 256 sessions lines 51-58; `CoalescingQueue` latest-value-per-key with eviction lines 82-306; `push`/`drain`/`requeue`/`finalize` lines 221-275; count-only `QueueStats`/Debug lines 71-107.
- `crates/sessions/src/share_enterprise.rs:1` `//! Enterprise-remote boundary (SHARE-003).`; `EnterpriseOp` enum lines 19-36; `partition` const fn lines 60-68; `refusal_reason` const fn lines 74-82; `EnterpriseBoundary::authorize` always `Err` lines 93-100.

## Observed scenario

ralph.card declares SHARE-001..005 accepted (FEATURES.md rows 34-38), but SHARE-003/004/005 are flagged "unwired lane (by design) + quiesced re-run" and SHARE-003 carries an explicit blocked note. `sources/sharing-ownership-gap.json` contradicts `accepted` with `controllerStatus: not-started` for all five and null ownership decisions; the canonical reconciliation source is the gap file + validator (`validate_backlog_exhaustion.py`), which still enumerates the SHARE family as a live gap group (`SHARING_GAP_STORIES`, `SHARING_UNRESOLVED_PARTITIONS`, closure-criteria assertions 1616/1709-1724).

Gap: `ralph.completion.json` acceptance rows vs `sharing-ownership-gap.json` not-started + ownership null. This is a controller/reconciliation artifact, not an implementation gap to close here: the SHARE stories are not task-owned in the candidate (no test cards, no owned impl files), and the native `share_*` modules present in `crates/sessions/src` are pre-existing lane wiring (per their headers "Lane-owned module: included by the test via #[path]; the integrator wires pub mod share_store; into lib.rs later") with no SHARE-00x test files authored in this candidate tree.

No SHARE-00x test files found under `tests/`, `tasks/completion/*`, or `crates/sessions/src/` test directories. Native modules compile but are not wired into `sessions/lib.rs` and carry no frozen RED/GREEN test obligations for SHARE-001..T05. Therefore no lane completion test command exists; the guard lane is evidence/audit only.

## Target boundary

- Own only `worklog/GUARD-SHARE-001.md` + ledger row `GUARD-SHARE-001` in `tasks/completion/claims.json`.
- No edits to: `ralph.json`, `ralph.completion.json`, `FEATURES.md`, `crates/**`, `tests/**`, `tools/**` (except the mandated `completion_claims.py` claim/update/release API), `docs/**`, `.github/**`.
- No commit/push of canonical/product/test/verifier artifacts. Lightweight commit/push of scratchpad + ledger row only on `lane/GUARD-SHARE-001-20260923`.
- No implementation, no acceptance claim, no GREEN. Status set `blocked` with audit-only note.

## Authority disposition / patch fields (audit only; nothing applied)

Preserving secret/persistence blockers (per `sharing-ownership-gap.json` closureCriteria and `share_store.rs`/`share_enterprise.rs` design):
1. `local-share-metadata-and-secret-persistence`: `ShareSecret` borrow-only, Drop-zeroized in `share_store.rs:50-77`; persistence row (`session_share` keyed by session id) is outside the authorized no-user-DB lane. No patch field proposed.
2. `deterministic-data-keying-merge-and-snapshot-size-policy`: `share_merge.rs` caps at MAX_BATCHES=16/MAX_RECORDS_PER_BATCH=10_000/MAX_RECORD_BYTES=1MiB but declares "no upstream target size cap" (line 334); no snapshot byte cap exists. Disposition: bounded call-local merge is native-faithful; snapshot byte cap is a follow-up child task, not authored here.
3. `legacy-event-compaction-to-snapshot-migration`: migration/storage behavior inventive risk; remains blocked. No patch field.
4. `legacy-versus-org-share-http-auth-and-failure-policy`: HTTP transport + credentials not authorized. No patch field.
5. `event-subscription-coalescing-backpressure-and-retry-lifetime`: `share_queue.rs` has item/byte/session bounds (lines 51-58) but no retry/requeue-after-failure contract for HTTP flush (`requeue` is caller-retains-batch, line 247-265; flush failures removed before transport per gap). Disposition: queue bounds native-faithful; flush retry/loss semantics are a child task. No patch field.
6. `share-create-remove-deletion-and-secret-lifetime`: creation/delete lifecycle exists in `share_store.rs` (create/mark_synced/remove/remove_session); secret lifetime is caller-owned. Disposition: bound present, external transport blocked.
7. `support-admin-removal-authority`: enterprise-remote missing-spec; `share_enterprise.rs` refuses all ops. No patch field.
8. `enterprise-function-hosted-sync-and-deployment-boundary`: SHARE-003 blocked pending `sources/enterprise-remote-spec-gap.json` resolution. No patch field.

## Reconciliation result

- ralph.card `accepted` (FEATURES.md:76-80 rows 34-38) vs `sharing-ownership-gap.json` `not-started`/ownership null is a controller reconciliation
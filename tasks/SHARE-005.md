# SHARE-005

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007 (Session sharing).
Dependencies: none.
Test obligations: SHARE-005-T01, SHARE-005-T02, SHARE-005-T03, SHARE-005-T04, SHARE-005-T05.

## User-observable outcome

Deterministic share-sync transport policy: a drained coalesced batch is sent at most once per sequence number, HTTP 4xx/5xx maps to a typed retry/abort decision, and deletion is confirmed only on explicit success. No silent batch loss, no secret logging, no unbounded retry.

## Source evidence

- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `packages/opencode/src/share/share-next.ts:206-359` (blob `60112e10d964b3d9693727501e1e40dac6b1fdf1`): upstream create/sync/delete transport (create/delete require success; sync only logs HTTP >=400 and drops the batch; this slice replaces drop-on-failure with retain-and-retry policy).
- `packages/opencode/test/share/share-next.test.ts:84-224` (blob `fe036be8c959b889516a894266bf4bb390ce1827`): direct transport test matrix mirrored below.
- `packages/enterprise/src/routes/api/[...path].ts:29-155` (blob `4677d68d33c48c6a030ae66100d59823fc241aa9`): remote route surface this slice treats as transport peer only.
- `sources/sharing-ownership-gap.json`: partition `enterprise-share-http-and-support-admin`; unresolved `legacy-versus-org-share-http-auth-and-failure-policy`, `share-create-remove-deletion-and-secret-lifetime`.
- `sources/disc-003-reconciliation.json` finding `opencode.sharing` (partial): legacy `/api/share` vs org `/api/shares` semantics and remote failure/retry policy must be reconciled before claiming parity.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-016.md: task-card model (Status/Kind/contract/test obligations).
- Classification: partial upstream (transport shape exists; failure/retry policy is new), deliberate resource-bounded deviation (bounded retries, at-most-once per seq, explicit delete confirmation).

## Observable contract

- `SyncBatch { share_id: ShareId, base_seq: u64, records: Vec<ShareRecord> }`; `SyncOutcome = Sent | Retry { backoff_ms: u64 } | Abort { reason: String }`; `DeleteOutcome = Deleted | NotFound | Retry { backoff_ms: u64 }`.
- `SyncPolicy { max_retries: u32 = 3, base_backoff_ms: u64 = 500 }`; `classify(status: u16) -> SyncOutcome`: 2xx => `Sent`; 429/5xx => `Retry` (bounded, `backoff_ms = base * 2^attempt` capped at 30_000); other 4xx => `Abort`.
- `decide_delete(status: u16) -> DeleteOutcome`: 2xx/404-on-delete => `Deleted`/`NotFound` (terminal); 429/5xx => `Retry`; other 4xx => `Abort`-equivalent terminal refusal (no delete claimed).
- Endpoint selection is explicit: `Endpoint = Legacy | Org`; caller passes it in; policy never silently defaults (missing => `Abort { reason: "endpoint-unspecified" }`).
- At-most-once per seq: `should_send(last_acked_seq, batch.base_seq) -> bool` is true only when `base_seq > last_acked_seq`.
- Suggested module boundary: `crates/share/src/policy.rs` (crate `opencode-rk-share`); worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- Failed flush: batch retained for retry (never dropped); retry count bounded by `max_retries`, then `Abort`.
- Non-success on create: nothing persisted locally (caller must roll back the SHARE-004 record); policy returns `Abort`.
- Non-success on delete: record retained unless `Deleted`; `NotFound` is terminal success-equivalent (idempotent delete).
- Unspecified endpoint: `Abort`; no request constructed, zero bytes sent.
- Secret safety: policy handles status codes and seq numbers only; share secrets never enter the policy; logs carry status/seq/endpoint only. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory fixtures only.

## Resource bounds

- Retry state: two `u64` seq markers + one `u32` attempt counter per share; no per-record retention inside the policy (batch owned by caller).
- Backoff computed arithmetically (no timer/thread/sleep inside policy; caller schedules).
- Byte bound: policy inspects no payload bytes; batch size enforced by SHARE-001/SHARE-002 caps upstream of this policy.
- Zero hidden cost when unused: pure functions, no global state, no I/O.

## Test obligations (frozen)

- SHARE-005-T01 (happy path): 2xx sync => `assert_eq!(outcome, Sent)` and `should_send` false for same seq after ack; 2xx delete => `assert_eq!(outcome, Deleted)`.
- SHARE-005-T02 (retry mapping): 429/500/503 => `assert!(matches!(outcome, Retry { .. }))` with `backoff_ms` doubling per attempt and capped at 30_000; attempts beyond `max_retries` => `Abort`.
- SHARE-005-T03 (abort + endpoint): 400/401/403/404-on-sync => `Abort`; missing endpoint => `Abort { reason: "endpoint-unspecified" }`; legacy vs org endpoints produce distinct request targets (no silent default).
- SHARE-005-T04 (delete + create semantics): 404-on-delete => `NotFound` terminal; non-success create => caller-visible `Abort` with local record unpersisted (assert store empty); failed sync batch retained (assert batch bytes intact for retry).
- SHARE-005-T05 (safety + determinism): same status/seq inputs => identical outcomes; no test writes outside disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain zero secret bytes and zero record-payload bytes.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, gap files + disc-003 `opencode.sharing` finding (done, see evidence).
2. Contract: defined above.
3. Author tests SHARE-005-T01..T05; establish compiling RED (fail: no policy module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust sync/delete policy.
6. GREEN, refactor, rerun; negative tests (retry exhaustion, abort codes, missing endpoint, seq replay).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SHARE-005.md
cargo test -p opencode-rk-share policy
cargo check --workspace
```

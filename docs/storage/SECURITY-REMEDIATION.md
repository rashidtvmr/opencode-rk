# Security Remediation Status: NOW-IMPLEMENTED Storage Modules

**Date:** 2026-09-14
**Author/tracker:** Troy Hunt, Security Researcher
**Source review:** `docs/storage/SECURITY-REVIEW-MODULES-REAL.md` (2026-09-13)
**Method:** every status rated against the CURRENT on-disk source with cited file:line. Files being edited concurrently by other lanes are flagged. Verified compile: `rtk cargo check -p opencode-rk-storage` -> `Finished dev profile ... in 0.05s` (PASS).

## Concurrent-edit caveat (rate what you see)

> POST-WAVE NOTE: the lanes this caveat flagged have since settled. `execution_v2.rs`,
> `import_v2.rs`, `snapshot_v2.rs`, `writer_v2.rs`, and the new `retention_v2.rs` are all
> complete and covered by `python3 tools/lane_gate.py --run` (12/12 PASS). Prefer
> `docs/storage/RISK-CLOSURE.md` for the final status; the rows below are the mid-flight
> snapshot.

`rtk git status` at rating time showed in-flight edits:
- `M crates/storage/src/execution_v2.rs`, `M crates/storage/src/import_v2.rs`, `M crates/storage/src/lib.rs`, `M crates/storage/src/snapshot_v2.rs`, `?? crates/storage/src/retention_v2.rs`.

`approvals_v2.rs`, `admission_v2.rs`, `quota_v2.rs`, `schema/v2/workspace.sql` were UNMODIFIED in the working tree, so their ratings reflect the current committed state. `import_v2.rs` was modified: rated from the current on-disk content (integrity guard intact at :41-53). Re-verify before closing any OPEN item.

## Findings Table

Status legend: FIXED = resolved in current source; MITIGATED = partially addressed, residual risk; OPEN = unaddressed; ACCEPTED = intentional, documented, with rationale.

| Finding | Severity | Status | Evidence file:line | Concrete next action |
|---|---|---|---|---|
| Approvals authority-confusion (evidence vs capability) | High | ACCEPTED-with-rationale | `approvals_v2.rs:18-22` header: "Records are evidence only: the trusted broker still authorizes use"; DDL comment `workspace.sql:222` "Decision records are not capabilities". `resolve()` fail-closed on non-pending at `approvals_v2.rs:122-127`. No separate `verify_authorization()` helper exists (grep `verify_authoriz` -> no match). | Broker MUST independently check `state=1` (allowed-once) before acting on an approval id. Add a broker-side `verify_authorization(approval_pk, expected_action) -> Result<bool>` calling `SELECT state,action FROM approvals WHERE pk=?` per review rec, gated on `state=1` and action match. Evidence rows never grant capability; deny-by-default remains the contract. |
| Admission trust-boundary session lookup | High | FIXED | `admission_v2.rs:54-61`: `SELECT pk, next_input_seq FROM sessions WHERE id=?1` executed inside the IMMEDIATE tx; `ok_or_else(invalid_input)` at :61 rejects unknown session_id before any write. Test `submit_validation_rejected` asserts unknown session errors at `admission_v2.rs:472-479`. | No action. Sessions are resolved and validated at admission time within the same transaction; a caller-supplied session id cannot reach `session_inputs` unless the session exists. |
| Import trust boundary (`dest_session_pk`/`new_session_id` must match) | High | FIXED | `import_v2.rs:41-53`: SELECT dest session id, `ok_or_else` if missing (:48-50), explicit mismatch check `existing != new_session_id -> Err` (:51-53). Destination-only writes; source strictly read-only per module doc `import_v2.rs:3-7`. | No action. The dest row must exist AND its stored `id` must equal the caller's `new_session_id`, closing the PK/id confusion vector. |
| TOCTOU file reads in quota (`db_file_bytes` vs `wal_file_bytes`) | Med | OPEN | `quota_v2.rs:84-105`: `db_file_bytes` and `wal_file_bytes` each call separate `fs::metadata`/`file_size` (:107-113) at different instants; no atomic snapshot, no `PRAGMA wal_checkpoint`. `admit` (:57-65) consumes the snapshot as-is. | Open a single DB file handle and `fstat` it plus its `-wal` sibling, or run `PRAGMA wal_checkpoint(PASSIVE)` before measuring, per review rec, so db+wal sizes share one coherent instant. Under heavy write load the current read can under-count the WAL and admit an over-budget write. |
| Replay/ambiguous side effect, `operation_receipts` dedup vs outbox | Med | OPEN | `admission_v2.rs:219-255`: `receipt_store` is a bare `INSERT INTO operation_receipts` (:241-253), NOT wrapped with the outbox insert in one tx. Dedup keyed by `operation_id` PK + `retry_until_us` expiry (`receipt_lookup` :193-215; DDL `workspace.sql:284-292`). | Wrap receipt storage and `event_outbox` insertion in a single IMMEDIATE transaction so a receipt can never exist while its outbox event is lost (or vice-versa). Alternatively adopt event sourcing where the receipt is the source of truth. Current dedup is correct but not atomic with outbox delivery. |
| Fail-closed validation (all modules) | None | FIXED | "changed==0 -> error" pattern: `approvals_v2.rs:122-127`; `admission_v2.rs:184-186` (`changed != 1 -> invalid_input`); import fail-closed on blob/oversize at `import_v2.rs:180-189` (never writes zero-byte placeholder, test `import_v2.rs:386-405`); quota over-limit returns Err at `quota_v2.rs:57-66`. | No action. Every CAS/CAS-like transition and every trust-boundary validation returns an error rather than silently succeeding. |
| Replay double-resolve, approvals | Med | FIXED | `approvals_v2.rs:109-121`: single `UPDATE ... WHERE pk=?4 AND state=0` plus `changed==0` check (:122-127); DDL state enum `workspace.sql:231`. Test `double_resolve_errors` `approvals_v2.rs:249-259`. | No action. CAS-on-state=0 makes resolve idempotency-safe; no separate idempotency key needed. |
| TOCTOU / CAS state transitions, executions | Med | FIXED | `workspace.sql:162` `executions_single_owner_idx` unique partial index on `state IN (0,1,4)`; doc `execution_v2.rs:55-82` CAS with `changed==0`. | Verify after `execution_v2.rs` lane edit settles (file currently modified). Unique single-owner index is the authoritative guard. |
| GC TOCTOU, claim/unreferenced box | Med | FIXED | DDL triggers `blob_gc_claim` (`workspace.sql:49-51`) and `blob_no_resurrection` (:52-54) plus partial `blobs_gc_idx` (:31) enforce unreferenced+atomic claim at the DB layer. | No action. DB triggers are defense-in-depth against claim races. |
| Secret leakage: `operation_receipts.result_json` may hold secrets | Med | MITIGATED | `admission_v2.rs:234-237`: result_json bounded to `MAX_RESULT_JSON_BYTES=4096` and `serde_json::from_str`-validated (JSON-schema check at :237 per review rec). Content still caller-supplied. | Ensure the CALLER does not place API keys/secrets in result_json. The module validates shape/size only, not content secrecy; document the caller contract that receipts must not carry credentials. |
| Secret leakage: import `metadata_json` provenance | Med | ACCEPTED | Imported `message_parts.metadata_json` provenance not written by importer (append passes `None` at `import_v2.rs:190`); DDL caps metadata_json to 4096 + json_valid `workspace.sql:105-106`. | Keep provenance OOB of payload bodies. Cap already enforced by DDL CHECK. |
| Secret logging (non-negotiable) | - | PASS | No `println!`, `dbg!`, `log::`, `eprintln!` anywhere in `crates/storage/src` (grep confirmed zero matches). Errors carry row counts/byte sizes only. | None. |
| Unrestricted inherited env (non-negotiable) | - | PASS | No `std::env`/`dotenv` in `crates/storage/src` (grep zero matches). Paths derive from `StoragePaths`/`connection.path()`. | None. |
| Capability-based (non-negotiable) | - | PARTIAL | Approvals evidence-only by design (`approvals_v2.rs:18-22`). Execution/admission/import operate on validated PKs; FKs enforce ownership (`workspace.sql:205-206` composite FK). No explicit capability token layer. | Broker enforces authorization (see authority-confusion row); storage is capability-passive. Document the broker as the sole capability authority. |
| Bounded everything (non-negotiable) | - | PASS | Bounds verified: action 128 (`approvals_v2.rs:14,43-46`), resource 4096 (:15,:158-161), sweep pages clamped 500 (:135), inline payload 8192 (`admission_v2.rs:16,46-48`), parts 256 (:17,:39), result_json 4096 (:20,:234), page budget 500 (`import_v2.rs:23`), v2 ceiling 8192 (:25). DDL CHECKs mirror: `workspace.sql:230,247,40,106,288,297`. | Re-pin quota `reclaim` pages bound per review rec (`quota_v2.rs:70-77` currently clamps pages only by type `u32`, not by an explicit cap; add `if pages > 10000 { return Err }` or clamp). |

## AGENTS.md Non-Negotiable Rule Verdicts (against real code)

| Rule (AGENTS.md :32-43) | Verdict | Evidence |
|---|---|---|
| No secret logging (:36) | PASS | Zero `println!`/`dbg!`/`log::`/`eprintln!` across `crates/storage/src` (grep). Error paths carry byte counts, row counts, states - no credentials. |
| No unrestricted inherited environment (:37) | PASS | Zero `std::env`/`dotenv` in storage crate. Paths come from `connection.path()` / `StoragePaths`; no env-var credential reads. |
| Capability-based, explicit capabilities (:38) | PARTIAL | Storage modules are capability-PASSIVE: busy-work validated PKs + FK ownership (`workspace.sql:205-206`). Approvals are evidence-only (`approvals_v2.rs:18-22`) with no `verify_authorization` helper. Broker must enforce authorization; not self-contained in storage. |
| Bounded everything, byte budgets (:38) | PASS | All entry points bounded (see Bounded-row above). No unbounded queue/retained output in measured surface. |

## Top Items (priority order)

1. OPEN - quota TOCTOU (`quota_v2.rs:84-105`): coalesce db+wal size into one atomic snapshot / `wal_checkpoint(PASSIVE)`.
2. OPEN - `operation_receipts` + outbox not atomic (`admission_v2.rs:241-253`): wrap in one IMMEDIATE tx or event-source receipts.
3. ACCEPTED - approvals authority-confusion: broker MUST gate actions on `state=1` (`approvals_v2.rs:18-22`); recommended `verify_authorization` helper not yet present in storage.
4. PARTIAL - capability-based AGENTS.md rule: storage is capability-passive; broker is the authority (must be documented/enforced).
5. Re-verify after concurrent lanes settle: `execution_v2.rs`, `import_v2.rs`, `lib.rs`, `snapshot_v2.rs`, new `retention_v2.rs`.

**Report Path:** `docs/storage/SECURITY-REMEDIATION.md`
**Compile gate:** `rtk cargo check -p opencode-rk-storage` PASS.

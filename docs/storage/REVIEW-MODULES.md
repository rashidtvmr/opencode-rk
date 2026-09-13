# Adversarial Review: Storage V2 Modules

> SUPERSEDED (2026-09-13, later same day): written while the seven modules were still
> 3-line stubs. All seven are now implemented (see `docs/STORAGE.md` "Implemented modules"
> and `crates/storage/src/*_v2.rs`). Treat the stub verdicts below as a historical
> baseline only. A fresh adversarial review is tracked in `worklog/STORAGE-V2-IMPL.md`.

Reviewer: Michael Feathers mode. Evidence only, file:line cited. Tree state: HEAD, 2026-09-13.

Reviewed modules: `gc_v2`, `admission_v2`, `execution_v2`, `approvals_v2`, `snapshot_v2`, `import_v2`, `quota_v2`.

## Headline verdict

ALL SEVEN modules are unimplemented stubs. Each file is 3 lines: a doc comment
(`stub; filled by a dedicated lane`), `#![allow(dead_code)]`, and one empty
marker struct with zero methods:

- `crates/storage/src/gc_v2.rs:1-3` - `pub struct GcV2;`
- `crates/storage/src/admission_v2.rs:1-3` - `pub struct AdmissionV2;`
- `crates/storage/src/execution_v2.rs:1-3` - `pub struct ExecV2;`
- `crates/storage/src/approvals_v2.rs:1-3` - `pub struct ApprovalsV2;`
- `crates/storage/src/snapshot_v2.rs:1-3` - `pub struct SnapshotV2;`
- `crates/storage/src/import_v2.rs:1-3` - `pub struct ImportV2;`
- `crates/storage/src/quota_v2.rs:1-3` - `pub struct QuotaV2;`

All seven are untracked (`git status`: `?? crates/storage/src/{gc,admission,execution,approvals,snapshot,import,quota}_v2.rs`);
only `lib.rs` is modified to wire them (`crates/storage/src/lib.rs:7-13` `pub mod`, `:17-23` `pub use`).
Worklog corroborates: `worklog/STORAGE-V2-IMPL.md:336-341` lists these lanes MISSING; `:394-396` states they "never landed".

What DOES exist: the format-2 SQL schema (`crates/storage/schema/v2/workspace.sql`, 313 lines),
three implemented Rust modules (`schema_v2.rs` 197 lines, `writer_v2.rs` 221 lines, `catalog_v2.rs` 330 lines,
plus out-of-scope `fork_v2.rs`), and 62 Python contract tests over the SQL invariants
(`tests/bootstrap/test_storage_schema_v2.py`). Therefore per-module review rates the
SQL-layer invariants separately from the Rust-layer behavior.

Legend: CONFIRMED = verified in code/tests at cited lines. PARTIAL = invariant exists but with gaps. MISSING = no code anywhere.

---

## gc_v2 - STUB

Rust sweep code: MISSING. Empty struct (`gc_v2.rs:1-3`). No method ever transitions `blobs.state`, deletes orphan payloads, or advances retention.

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth (blob/payload orphans) | CONFIRMED RISK | `payload_roots` view exists (`workspace.sql:302-313`) but no Rust query consumes it; no sweep, no cron, no manual path. `blobs` rows and blob files accumulate forever once blob-writing lands. |
| Ownership/lifetime of `state=1` (deleting) | PARTIAL | State machine defined: 0=ready,1=deleting,2=unavailable (`workspace.sql:24`); triggers block collecting referenced blobs (`workspace.sql:49-51`) and resurrection (`workspace.sql:52-54`); `blobs_gc_idx` (`workspace.sql:31`). But nothing in Rust ever claims or finishes a deletion - no owner for rows stuck in state 1. |
| Panic paths | CONFIRMED NONE | Stub has no code; crate is `#![forbid(unsafe_code)]` (`lib.rs:2`). |
| Silent data loss | MISSING (cannot occur yet) | No writer creates blob-referencing payloads: `writer_v2.rs:57-63` rejects both oversized inline and ALL `PayloadRef::Blob` variants with `InlinePayloadTooLarge`. Blob GC is moot until a blob writer exists. |
| Unsafe state transitions | PARTIAL | SQL-side CONFIRMED: tombstone reattachment, referenced-claim, identity immutability all trigger-enforced and Python-tested (`test_storage_schema_v2.py:93` `test_blob_gc_rejects_reattachment_to_tombstone`, `:99` `test_blob_gc_cannot_claim_referenced_blob`, `:105` `test_unavailable_blob_blocks_new_payloads`, `:187` `test_backup_pin_prevents_collection`, `:193` `test_every_payload_fk_is_in_gc_root_view`). Rust-side MISSING. |
| Tests | MISSING (Rust) / CONFIRMED (SQL) | Zero Rust tests for gc_v2; 5+ Python tests cover SQL triggers above. |
| Retention pins | CONFIRMED (SQL only) | `retained_payloads` purpose pin table (`workspace.sql:276-283`) blocks collection of artifact/share/backup payloads - but no Rust inserts or expires pins. |

## admission_v2 - STUB

Rust promotion/cancellation code: MISSING (`admission_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth (pending inputs) | CONFIRMED RISK | `session_inputs` has no count cap and `inputs_pending_idx` (`workspace.sql:129`) covers `state=0` rows; no Rust code ever promotes (`state=1`) or cancels (`state=2`) them, so every admitted input stays pending forever. |
| Ownership (promotion binding) | PARTIAL | SQL enforces same-session promotion via composite FK (`workspace.sql:124-125`) and state/promoted-at coherence (`:126-127`); identity immutable (`:131-132`). No Rust performs promotion. |
| Panic paths | CONFIRMED NONE | Stub; crate forbids unsafe. |
| Silent data loss | MISSING | No deletion paths exist at all, so none can lose data yet. Cascade delete of inputs with session (`workspace.sql:115`) is tested (`test_storage_schema_v2.py:122` `test_admission_promotion_shares_payload_and_deletes_cleanly`). |
| Unsafe state transitions | PARTIAL | SQL CONFIRMED: cross-session promotion rejected and identity frozen (`test_storage_schema_v2.py:136` `test_admission_rejects_cross_session_message`, `:140` `test_admission_request_identity_is_immutable`). Rust MISSING. |
| Tests | MISSING (Rust) / CONFIRMED (SQL) | No Rust tests; 4 Python tests cover the table contract. |

## execution_v2 - STUB

Rust execution lifecycle code: MISSING (`execution_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth | PARTIAL | `executions`/`provider_attempts`/`tool_calls` are bounded by session lifetime via `ON DELETE CASCADE` (`workspace.sql:147,173,205-206`), but sessions themselves have no quota (see quota_v2 below). Completed rows never archived. |
| Ownership (single runner, generation) | PARTIAL | SQL CONFIRMED single active owner per session: partial unique index `executions_single_owner_idx ... WHERE state IN (0,1,4)` (`workspace.sql:162`); `owner_generation` recorded (`:151`) with recovery index (`:165`); parent immutability trigger (`:167-169`). Rust MISSING: nothing claims, releases, or generation-bumps an execution. |
| Panic paths | CONFIRMED NONE | Stub. Adjacent implemented code is clean: `writer_v2.rs:77,197` use `checked_add` for sequence overflow. |
| Silent data loss | MISSING (yet) | `ON DELETE SET NULL` for parent (`workspace.sql:148`) preserves child rows - safe. No Rust writes exist. |
| Unsafe state transitions | PARTIAL - the strongest gap | SQL blocks a queued run while another is active/uncertain (`test_storage_schema_v2.py:145` `test_uncertain_execution_blocks_new_run`), and tool binding/ownership triggers are tested (`:156` `test_tool_requires_same_session_execution_and_assistant`, `:164`, `:168`, `:173`, `:178`). But NO code resolves `uncertain` (state 4/5): `attempts_recovery_idx` and `tools_recovery_idx` (`workspace.sql:185,212`) exist precisely for a recovery sweep that does not exist. An uncertain execution wedges its session's single-owner slot indefinitely. |
| Tests | MISSING (Rust) / CONFIRMED (SQL) | 6+ Python tests; zero Rust. |

## approvals_v2 - STUB

Rust approval resolution code: MISSING (`approvals_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth (pending approvals) | CONFIRMED RISK | `approvals_pending_idx` (`workspace.sql:242`) grows without bound; `expires_at_us` exists with CHECK (`:238`) but NO code transitions pending to expired/consumed - expiry is dead metadata, not a sweep. |
| Ownership (human-only grants) | PARTIAL | SQL enforces mandatory-human client identity at allowed-once/consumed (`workspace.sql:239`, tested at `test_storage_schema_v2.py:178` `test_mandatory_human_approval_needs_client_identity`); `approval_resources` binds scope (`:244-249`). Rust MISSING: nothing records or consumes an approval, so `Permission *` bypass concerns in AGENTS.md have no storage-side enforcement path yet beyond schema. |
| Panic paths | CONFIRMED NONE | Stub. |
| Silent data loss | MISSING | No transition code exists to lose data. Cascade with tool call/session (`workspace.sql:237`). |
| Unsafe state transitions | PARTIAL | State enum 0-4 (`:231`) unconstrained between non-terminal states in SQL (only the human-identity CHECK at `:239`); relies wholly on unwritten Rust. |
| Tests | MISSING (Rust) / PARTIAL (SQL) | Human-identity + digest-binding immutability tested (`test_storage_schema_v2.py:173,178`); expiry transition untested anywhere. |

## snapshot_v2 - STUB

Rust epoch/compaction code: MISSING (`snapshot_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth | PARTIAL | One open epoch per session enforced (`context_open_idx`, `workspace.sql:260`, tested `test_storage_schema_v2.py:182`); closed epochs and `compaction_checkpoints` rows accumulate with no cap and no Rust rollover. |
| Ownership/lifetime | PARTIAL | `closed_at_us` NULL/nonnull is the only epoch-close marker (`workspace.sql:257`); no Rust opens/closes epochs, so lifetime is undefined. |
| Panic paths | CONFIRMED NONE | Stub. |
| Silent data loss | MISSING (yet) | Baseline/snapshot/summary payload FKs are `ON DELETE RESTRICT` (`workspace.sql:254-255,267-268`) - snapshot content cannot be GC'd while referenced. Good design, unexercised. |
| Unsafe state transitions | PARTIAL | `UNIQUE(session_pk, epoch)` (`:258`) prevents epoch reuse; checkpoint boundary FK to a real message is DEFERRABLE (`:271-272`). No transition code to review. |
| Tests | MISSING (Rust) / PARTIAL (SQL) | Only-one-open-epoch tested; compaction/rollover untested. |

## import_v2 - STUB

Rust import code: MISSING (`import_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth | MISSING | No import tables exist in `workspace.sql`; `operation_receipts` (`workspace.sql:284-292`) is the nearest surface (idempotent replay receipts) and has no writer. Nothing to grow yet. |
| Ownership | MISSING | No code assigns importer ownership; nothing exists. |
| Panic paths | CONFIRMED NONE | Stub. AGENTS.md rule "No changes to the user's existing OpenCode database in the implementation loop" (`AGENTS.md`, root) is currently satisfied vacuously - there is no importer to violate it. |
| Silent data loss | MISSING | No code. Highest-risk future module: bulk import + `INSERT OR REPLACE` patterns would bypass the immutability triggers (`workspace.sql:47-48,110-111,131-132,219-220`). Review again when it lands. |
| Unsafe state transitions | MISSING | No code. |
| Tests | MISSING | Nothing. |

## quota_v2 - STUB

Rust quota code: MISSING (`quota_v2.rs:1-3`).

| Concern | Rating | Evidence |
|---|---|---|
| Unbounded growth | CONFIRMED RISK (worst) | No quota tables, no counters, no caps anywhere in `workspace.sql`. Sessions, messages (seq counter `workspace.sql:70`), executions, events, approvals, inputs - all unbounded in count and aggregate bytes. v1 had a per-session event cap (`lib.rs:29` `DEFAULT_MAX_EVENTS_PER_SESSION=10_000`, applied `lib.rs:44`); format 2 has NO equivalent. AGENTS.md: "No unbounded queue, unbounded retained output" - violated by omission across every v2 table once writers land. |
| Ownership | MISSING | No code. |
| Panic paths | CONFIRMED NONE | Stub. |
| Silent data loss | MISSING | No code. |
| Unsafe state transitions | MISSING | No code. |
| Tests | MISSING | Nothing. |

---

## Cross-cutting findings (verified in implemented code)

1. `event_outbox` retention is comment-only. `workspace.sql:301` says "Retention deletes only a sequence prefix. Do not punch holes" and `event_floor_seq` exists (`workspace.sql:17` with coherence CHECK `:19`), but no Rust advances the floor or deletes the prefix. `writer_v2.rs:110-138` only appends. CONFIRMED unbounded.
2. `operation_receipts` never purged. `receipts_expiry_idx` (`workspace.sql:292`) exists for a sweeper that does not exist. CONFIRMED unbounded.
3. `writer_v2.rs:60-62` rejects ALL `PayloadRef::Blob` message bodies. Format 2 cannot currently store any message over 8192 bytes inline (`writer_v2.rs:9`) - loud error, not data loss, but the blob store, `payload_ready` trigger (`workspace.sql:43-46`) and entire `blobs` table are dead code paths awaiting gc_v2 + writer lanes.
4. Implemented modules hold the line where they exist: crate is `#![forbid(unsafe_code)]` (`lib.rs:2`); schema init is fail-closed against non-empty files (`schema_v2.rs:32-39`, tested `:168-179`) with checksum gate on reopen (`schema_v2.rs:107-124`, tested `:181-196`); sequence allocation is overflow-checked (`writer_v2.rs:197`).

## Top-5 risks with owners

| # | Risk | Evidence | Owner |
|---|---|---|---|
| 1 | Whole-v2 unbounded growth: no quota module, no caps on sessions/messages/events/approvals/inputs; `event_outbox` and `operation_receipts` have indexes for cleanup but zero purge code. | `quota_v2.rs:1-3` stub; `workspace.sql:301` comment vs no code; `writer_v2.rs:110-138` append-only; `workspace.sql:292` | Writer (prefix-trim event_outbox + purge receipts inside the same txn that writes); SQL cannot own time/count retention. Operator: disk ceiling + alerting until lane lands. |
| 2 | Uncertain executions wedge sessions forever: single-owner slot blocked, no recovery sweep to resolve state 4/5 executions/attempts/tool_calls. | `workspace.sql:162` partial unique index; recovery indexes `:185,212` unused; `execution_v2.rs:1-3` stub | Writer (execution_v2 lane must own a generation-checked recovery pass); SQL constraint correctly blocks, cannot self-heal. Operator: restart/verify procedure in the interim. |
| 3 | No GC: blob/payload orphans unreclaimable; `payload_roots` view consumed by nothing; `blobs.state` machine has no driver; retained pins never expire. | `gc_v2.rs:1-3` stub; `workspace.sql:302-313` view unused; `:24,31,49-54` machine undriven | Writer (gc_v2 lane owns sweep + finish-deletion); SQL already guards the transitions; operator for retained-pin expiry policy. |
| 4 | Pending approvals and session inputs never leave state 0: expiry metadata exists but no transition code, so pending sets grow monotonically per session. | `approvals_v2.rs:1-3` and `admission_v2.rs:1-3` stubs; `workspace.sql:238` expires_at_us unused; `:129` inputs_pending_idx | Writer (approvals_v2/admission_v2 lanes own promote/cancel/expire transitions); SQL CHECKs already prove coherent terminal states; operator: none possible - storage must do it. |
| 5 | Import lane will be the dangerous one: bulk writes against a trigger-heavy immutable schema; `operation_receipts` idempotency surface exists with no writer, inviting future `INSERT OR REPLACE` shortcuts that bypass immutability triggers. | `import_v2.rs:1-3` stub; `workspace.sql:47-48,110-111,131-132,219-220` triggers; `:284-292` receipts | Writer (import_v2 lane must go through receipts + plain INSERT, never OR REPLACE); SQL: consider a trigger rejecting `ON CONFLICT DO UPDATE` patterns on immutable tables if feasible; operator: backup before first import run per AGENTS.md "never run host-destructive test commands". |

## Verdict per module (one line each)

- gc_v2: STUB. SQL state machine CONFIRMED solid (triggers + 5 Python tests); Rust collector MISSING.
- admission_v2: STUB. SQL promotion contract CONFIRMED; Rust promoter MISSING; pending inputs unbounded.
- execution_v2: STUB. SQL single-owner + recovery indexes CONFIRMED; Rust lifecycle/recovery MISSING; uncertain-wedge is top functional gap.
- approvals_v2: STUB. SQL human-identity gate CONFIRMED; Rust resolver MISSING; expiry dead metadata.
- snapshot_v2: STUB. SQL one-open-epoch CONFIRMED; Rust epoch/compaction MISSING.
- import_v2: STUB. Nothing exists; nothing reviewable; highest future risk.
- quota_v2: STUB. Nothing exists; its absence is the single largest systemic gap (risk 1).

Bottom line: the SQL layer is well-designed and independently tested (62 Python
contract tests). Every behavioral guarantee above the SQL layer is currently
vacuous. Until the seven lanes land, format 2 can create sessions and small
inline messages (`writer_v2.rs:32-108`) and nothing else - which also means the
triggers protecting blobs, executions, approvals, and inputs guard tables no
Rust code will ever populate.

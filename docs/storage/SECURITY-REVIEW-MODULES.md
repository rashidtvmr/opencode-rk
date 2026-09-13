# Security Review: Storage v2 Modules

> SUPERSEDED (2026-09-13, later same day): written while the scoped modules were still
> 3-line stubs. All are now implemented (see `docs/STORAGE.md` "Implemented modules" and
> `crates/storage/src/*_v2.rs`). The findings about the *live weak v1 path*, the missing
> capability broker, and replay handling remain relevant and should be re-checked against
> the real implementations.

Reviewer persona: Troy Hunt (threat modeling, evidence-based findings).
Scope: `approvals_v2`, `admission_v2`, `execution_v2`, `import_v2`, `gc_v2`.
Date: 2026-09-13. Repo: `opencode-rk`. Review against:
`crates/storage/src/*.rs`, `crates/storage/schema/v2/workspace.sql`,
root `AGENTS.md` non-negotiable rules.

## Module status (top line)

All five reviewed modules are **stubs**. Each file contains only a doc comment
and a unit struct, no logic:

- `crates/storage/src/approvals_v2.rs:1-3`  (105 bytes)
- `crates/storage/src/admission_v2.rs:1-3`  (105 bytes)
- `crates/storage/src/execution_v2.rs:1-3`  (100 bytes)
- `crates/storage/src/import_v2.rs:1-3`     ( 99 bytes)
- `crates/storage/src/gc_v2.rs:1-3`          ( 91 bytes)

Per the brief, each stub is marked **MISSING** in its table below. Because the
security-critical logic for these modules does not exist, the review covers the
*implemented* sibling v2 code (`lib.rs`, `catalog_v2.rs`, `schema_v2.rs`,
`fork_v2.rs`, `writer_v2.rs`) and the v2 schema (`workspace.sql`) that these
modules are supposed to wrap, since those are the surfaces that would own the
same trust boundaries.

## Per-module findings

### approvals_v2  ->  MISSING
- No Rust API. Schema defines `approvals` / `approvals_session_idx` etc.
  (`workspace.sql:223-249`). Authority boundary (who may set `state`,
  `mandatory_human`, `human_client_id`, `expires_at_us`) is unimplemented.
- Design intent at `workspace.sql:222`: "Decision records are not capabilities:
  the trusted broker still authorizes use." The trusted broker is absent.

### admission_v2  ->  MISSING
- No Rust API. Schema defines `session_inputs` / `session_input_parts`
  (`workspace.sql:112-142`) with immutable-identity trigger
  (`workspace.sql:131-132`). The admission validation gateway (validate
  `request_hash`, steer vs queue, promote) does not exist in code.

### execution_v2  ->  MISSING
- No Rust API. Schema defines `executions`, `provider_attempts`, `tool_calls`
  (`workspace.sql:144-220`) with owner-generation and single-owner indexes
  (`workspace.sql:162`) and immutable-parent trigger (`workspace.sql:167-169`).
  The execution lifecycle (queued/running/uncertain/cancelled) and the
  uncertain-side-effect handling are unimplemented.

### import_v2  ->  MISSING
- No Rust API. No bulk-import trust boundary. Ingesting external data into
  `payloads`/`sessions`/`messages` would currently have no validation layer.

### gc_v2  ->  MISSING
- No Rust API. Schema defines the blob lifecycle (`blobs.state` ready/deleting/
  unavailable, `workspace.sql:24`), GC guards (`blob_gc_claim`,
  `blob_no_resurrection`, `workspace.sql:49-54`), and the `payload_roots` view
  (`workspace.sql:302-313`). No reclaim logic exists, so unreferenced blobs
  accumulate unbounded (see F4).

## Consolidated findings (>=8, with severity + fix)

| # | Module | Dimension | Severity | Evidence (file:line) | Concrete fix |
|---|--------|-----------|----------|----------------------|--------------|
| F1 | all 5 | Authority confusion / capability | High | `approvals_v2.rs:1-3`, `admission_v2.rs:1-3`, `execution_v2.rs:1-3`, `import_v2.rs:1-3`, `gc_v2.rs:1-3` | Implement the modules before any untrusted input reaches the DB. Treat SQL `CHECK`/triggers as defense-in-depth only; the authority boundary must be enforced in Rust. |
| F2 | approvals_v2 / admission_v2 / execution_v2 | Authority confusion (evidence vs capability) | High | `workspace.sql:222` ("trusted broker still authorizes use"); stubs at `approvals_v2.rs`, `admission_v2.rs`, `execution_v2.rs` | Add a capability/broker layer: validate `intent_hash`/request_hash against a consumed `approvals` row before inserting `executions`/`tool_calls`. Deny direct INSERT by capability check. |
| F3 | execution_v2 / gc_v2 | Replay / uncertain-side-effect handling | Med | `workspace.sql:284-291` (`operation_receipts` request_hash + retry_until_us); no consumer code | In the execution write path, upsert `operation_receipts` and reject when `request_hash` already present within `retry_until_us`. Prevents double-execution of ambiguous side effects. |
| F4 | gc_v2 | Unbounded input / unbounded retained output | Med | `workspace.sql:21-30` (`blobs` lifecycle); `workspace.sql:302-313` (`payload_roots`); stub `gc_v2.rs` | Implement GC that walks `payload_roots`, claims unreferenced blobs via the `state` transition (guarded by `blob_gc_claim`/`blob_no_resurrection`), and updates `verified_at_us`. Bounds the retained set. |
| F5 | import_v2 | Missing validation at trust boundary / unbounded input | Med | `import_v2.rs:1-3` stub; `workspace.sql:32-41` (`payloads`, inline cap 8192) | Route import through `V2Writer` (`writer_v2.rs`) and the same bounded validators; never build SQL from raw imported strings; enforce `MAX_INLINE_PAYLOAD_BYTES`. |
| F6 | lib.rs (legacy v1 path) | SQL built from strings (injection hygiene) | Low | `lib.rs:45` `connection.execute_batch(&format!("PRAGMA incremental_vacuum({pages});"))` | `pages: u32` is numeric so not exploitable, but stop building SQL via `format!`. Keep it a validated constant or bounds-checked numeric; never interpolate caller-controlled strings. |
| F7 | catalog_v2 / schema_v2 | TOCTOU on file/DB open | Low | `catalog_v2.rs:46-55`, `schema_v2.rs:32-40` (stat `path.exists()` then `Connection::open`) | Open-then-verify or `O_NOFOLLOW`; fail if the path exists *at all* rather than only when non-empty, closing the symlink-swap race. |
| F8 | catalog_v2 / schema_v2 | Spec-compliance / integrity digest | Low | `catalog_v2.rs:4-7`, `schema_v2.rs:4-7` (uses blake3-256; docs/STORAGE.md specifies SHA-256; acknowledged `ponytail`) | Add `sha2` dep and switch the migration checksum to SHA-256 once engine qualification lands (per the stated upgrade path), so external verifiers agree on the digest. |
| F9 | lib.rs (BlobStore) | TOCTOU / predictable temp name | Low | `lib.rs:49` `format!(".{hash}.tmp-{}", std::process::id())` | Use `create_new(true)` (O_EXCL) in the same owned dir, or a `tempfile` handle; prevents a local pre-create of the predictable temp path. |
| F10 | all (architecture) | Dual schema: live path is weaker v1 | High | `lib.rs:47` migrate() builds non-STRICT tables with minimal CHECKs and no approvals/executions/tool_calls; v2 hardening (`workspace.sql` STRICT + triggers) reachable only via stubs | Route `Storage` through `SchemaV2`/`CatalogV2` once v2 modules exist, or port equivalent STRICT tables + triggers + capability tables into the live schema. Today the enforced trust boundary is the weak v1 schema. |
| F11 | fork_v2 (proxy for admission) | Missing authority check on operation | Med | `fork_v2.rs:28-168` validates `title`/`through_seq` but performs no caller-authorization; `agent_name` hardcoded `'default'` (`fork_v2.rs:68`); provider/model copied without re-validation (relies on DB CHECK) | Add an admission/capability check at the fork entry point; re-validate copied `provider_id`/`model_id` lengths in Rust, not only via SQL CHECK, so a bypass-inserted parent cannot poison the copy. |
| F12 | all (positive) | AGENTS.md non-negotiable rules | Pass | grep of `crates/storage/src` for `env`/`secret`/`log::`/`tracing`/`println!` found nothing in product code (only `eprintln!` in `tests/perf_v2.rs`, a perf harness) | No secret logging and no unrestricted inherited environment present. Capability-based rule satisfied only at schema level (see F2); not yet in code. Keep the src tree free of `std::env` access and secret emission. |

## AGENTS.md rule verification

Non-negotiable rules (`AGENTS.md:34-43`):

- "No secret logging" -> **PASS** for `crates/storage/src`. No emission of
  secrets, no `log::`/`tracing` of payloads, no `println!` of DB content.
- "No direct secret file access or unrestricted inherited environment" ->
  **PASS** for src. No `std::env`, no env reading, no `.env` access in the
  reviewed tree.
- "Use explicit capabilities, scoped cancellation, byte budgets and lazy
  services" -> **PARTIAL**. Schema expresses capability intent
  (`workspace.sql:222` decision-records-are-not-capabilities) and byte budgets
  exist (inline 8192, event 4096, title 1024, etc.), but the *capability
  enforcement code* (admission/approvals/execution modules) is MISSING. The
  rule is satisfied in design and schema, not in executable code.

## Summary of top findings

1. High (F1/F10): the five target modules are stubs while the live `Storage`
   path (`lib.rs`) runs the weaker v1 schema. The hardened v2 trust boundary is
   vaporware until the modules are implemented.
2. High (F2): no capability/broker enforcement; `approvals` rows are decision
   records with no code that authorizes their use.
3. Med (F3/F4): replay receipts and GC are schema-only; ambiguous side effects
   and unbounded blob retention are unhandled in code.

Recommendation: block acceptance of any untrusted input against this crate
until F1, F2, F3, F4, F5, F10 are closed. The schema-level hardening
(STRICT, triggers, CHECKs, indexes) is strong and should be preserved; it must
be paired with the missing Rust modules.

## Verification performed

- Read all 12 `crates/storage/src/*.rs` files (full).
- Read `crates/storage/schema/v2/workspace.sql` (full, 313 lines).
- Read root `AGENTS.md` non-negotiable rules (lines 32-43).
- grep `crates/storage` for `env|secret|log::|tracing|println|eprintln` to
  verify the no-secret-logging / no-env rules.
- No source files were modified. No commit made.

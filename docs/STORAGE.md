# Storage format 2: exact schema and operating contract

Status: reviewed design with executable SQL contracts. This wave does NOT switch
`Storage::open`, upgrade format 1, or implement filesystem durability/actor code.
The existing `crates/storage/src/lib.rs` remains the bootstrap implementation.
No DB story, source-parity claim or independent verification gate is marked done.

## Authoritative files

- `crates/storage/schema/v2/workspace.sql`: complete 19-table workspace DDL.
- `crates/storage/schema/v2/catalog.sql`: complete 6-table installation DDL.
- `docs/storage/CRASH_CONSISTENCY.md`: publication, deletion, recovery and backup.
- `docs/storage/REVIEW.md`: evaluator findings and outstanding activation gates.
- `tests/bootstrap/test_storage_schema_v2.py`: executable local SQLite contracts.
- `crates/storage/tests/schema_v2.rs`: Rust tests using the SAME SQL files.

Both scripts create NEW EMPTY files inside one explicit transaction. Never apply
these over format 1 or a user's OpenCode database. Sharing-service state, saved
permissions, usage accounting, import mappings and credential-refresh orchestration
need their own feature migrations; this is not a claim of full product coverage.

## Layout and resource model

```
data/catalog.db                          workspace registry/settings/account metadata
     credentials/                        broker/OS-managed; inaccessible to agents
     workspaces/<UUID>/state.db            canonical relational records
                       blobs/.staging/    private quota-reserved spooling
                       blobs/ab/<hash>.blob
                       search/           optional/rebuildable
                       cache/            disposable
```

One daemon holds OS ownership locks; clients use authenticated IPC. One blocking
writer per active workspace owns SQLite writes/checkpoints, not a Tokio scheduler
thread. Start with one query-only reader. Budget connections, threads, SQLite
caches and queues globally; initially allow at most four active workspace handles
(subject to benchmarking). Evict only quiescent handles. A busy session must not
be cancelled merely because its cache is least recently used. Local WAL storage
requires appropriate same-host filesystem semantics [1].

New public IDs are UUID BLOB(16); joins use integer rowids. Rowids need not be
monotonic after deletion and are never external identifiers. Time is signed Unix
microseconds, not ordering authority: clocks can move backward. Explicit session
sequences order history. An importer maps non-UUID upstream IDs rather than losing
them or assuming they parse as UUIDs.

An immutable `payloads` row contains either <=8192 inline bytes or a reference to
a READY blob with matching raw length. The 8 KiB boundary is an initial policy,
NOT a measured optimum. Shared payload references prevent admission promotion
and forks copying bodies. Promotion creates the visible message, copies only part
references, removes inbox part references and marks admission promoted atomically.
Forks copy bounded message metadata through their boundary and share payloads;
parent deletion cannot cascade into the fork's content. Parent UUID is provenance.

Every free text/JSON field has a byte ceiling; parts/resources have bounded
ordinals. These are NOT total memory/disk limits. Admission must additionally cap
pending inputs, per-command bytes, total canonical bytes, staged bytes, retained
outbox bytes, backup pins and query result bytes. Count DB/index pages, WAL, CAS,
staging and cache separately. Moving bytes out of SQLite does not remove their
disk cost. Do not silently delete precious transcripts at quota; reject/pause new
admission while reserving headroom for settlement/recovery/cleanup. No storage
operation requires collecting a complete large object in memory.

## Exact tables and constraints

| Boundary | Tables | Meaning |
| --- | --- | --- |
| Identity | schema_migrations, workspace_state | Version/checksum, owner generation, cursor epoch/head/floor |
| Content | blobs, payloads | Immutable logical content; ready/deleting/unavailable lifecycle |
| History | sessions, messages, message_parts | Explicit ordering, provider provenance, independent fork metadata |
| Admission | session_inputs, session_input_parts | Request digest, steer/queue and promotion receipt |
| Execution | executions, provider_attempts, tool_calls | Parent/child work, per-run config/model, explicit uncertain effects |
| Authority | approvals, approval_resources | Intent hash, policy generation, human identity, expiration |
| Context | context_epochs, compaction_checkpoints | One open epoch and completed compaction boundary |
| Retention | retained_payloads | Artifact/share/backup roots |
| Delivery | operation_receipts, event_outbox | Bounded retries and noncanonical invalidations |

Enum values are documented at their SQL columns. Named custom agents remain
names, not forced UUIDs. Execution configuration preserves the resolved agent,
permissions digest, effort and model options WITHOUT credentials. Root and child
runs may use different providers. An uncertain run keeps the session ownership
gate until explicitly resolved; completion state lives in tables, not the outbox.

Composite FKs bind tools to an execution and assistant message in the SAME
session. Tool input/name/intent and admission identity are immutable after final
validation. The promotion FK is deferred NO ACTION, not SET NULL contradicting
the promoted-state CHECK. Whole-session deletion removes dependent rows together.
Deleting an individual promoted message requires explicitly reconciling its receipt.

An approval row is evidence, not authority: the broker checks the actual command,
canonical resource, expiry, consumption, policy generation and authenticated human.
An agent cannot mutate this DB or mint approvals. This DDL is not an OS sandbox.
The writer must enforce allowed state transitions, terminal-state immutability,
no explicit-PK cycle injection, and command generation checks; row constraints do
not replace that typed API. Mutable open message parts finalize through writer
transactions; forks only reuse their immutable payloads, never mutable rows.

## Indexes mapped to queries

No duplicate descending index over an equivalent composite UNIQUE key: reverse
scans serve context/checkpoint retrieval. FK-support indexes count toward cost [2].

| Access | Index |
| --- | --- |
| State-filtered recent session page | sessions_state_updated_idx |
| All sessions recent page | sessions_updated_idx |
| Ordered message history | UNIQUE messages(session_pk,seq) |
| Ordered message/input parts | composite PRIMARY KEYs |
| Pending steer/queue FIFO | inputs_pending_idx |
| Same-session promotion checks | UNIQUE messages(pk,session_pk), inputs_message_idx |
| One queued/running/uncertain execution | executions_single_owner_idx (partial UNIQUE) |
| Execution children/history/recovery | executions_parent_idx, executions_session_idx, executions_recovery_idx |
| Provider attempts / tools recovery | attempts_recovery_idx, tools_recovery_idx |
| Provider-local call IDs | tool_provider_call_idx (assistant scoped) |
| Approvals and FK deletion checks | approvals_pending_idx, approvals_session_idx, approvals_tool_idx |
| Current context and compaction | context_open_idx plus existing composite UNIQUEs |
| Reachability / GC | payload_roots, payload FK indexes, payloads_blob_idx, blobs_gc_idx |
| Replay | outbox INTEGER PRIMARY KEY, event_session_idx |
| Receipt expiry | receipts_expiry_idx |
| Account selection/cooldown | accounts_route_idx, account lock composite PK |

Representative bound queries (also cap LIMIT and response bytes):

```sql
SELECT pk,id,title,updated_at_us FROM sessions
 WHERE state=? AND (updated_at_us,pk)<(?,?)
 ORDER BY updated_at_us DESC,pk DESC LIMIT ?;
SELECT pk,id,seq,role,status FROM messages
 WHERE session_pk=? AND seq>? ORDER BY seq LIMIT ?;
SELECT pk,id FROM session_inputs
 WHERE session_pk=? AND delivery=? AND state=0 AND seq>?
 ORDER BY seq LIMIT ?;
SELECT seq,kind,payload_json FROM event_outbox
 WHERE session_id=? AND seq>? AND seq<=? ORDER BY seq LIMIT ?;
```

No huge OFFSET, whole-session hydration or client-held read transaction. Local
EXPLAIN tests verify those four access paths; this is not a workload benchmark.

## Initialization and connection policy

Before creating schema or entering WAL: page_size=4096, auto_vacuum=INCREMENTAL.
Verify application identity and format BEFORE any write to an existing file.
Workspace application_id=0x4F525732, catalog=0x4F524332; user_version=2. Checksum
is SHA-256 of exact migration SQL bytes; checksum drift/unknown versions fail
closed, never overwrite the schema version to appear current.

Apply new-file DDL, singleton identity, migration record and user_version in one
IMMEDIATE transaction. Sync new filesystem entries before catalog visibility.
Every writer: foreign_keys=ON, synchronous=FULL, busy_timeout=5000,
trusted_schema=OFF, cache_size=-8192, temp_store=FILE, mmap_size=0 initially.
Verify journal_mode=WAL actually returns `wal`. Keep wal_autocheckpoint=1000 until
explicit scheduling/admission tests pass. The writer serializes checkpoints too.
Readers: foreign_keys=ON, trusted_schema=OFF, query_only=ON, a small globally
budgeted cache and bounded transactions. These settings are proposals to measure.

FULL is the canonical durability default [3]. A NORMAL option is not enabled in
this wave, especially at deletion/dispatch barriers. max_page_count does not cap
WAL, blobs or temporary files. Checkpoint starvation must trigger admission
backpressure and bounded reader cancellation/retry, not WAL deletion [1]. Reclaim
free pages in small incremental-vacuum batches during maintenance, not per event.

Production engine gate: bundle SQLite >=3.51.3 or an explicitly audited fixed
backport, recording sqlite_source_id. SQLite documents a rare WAL-reset race fixed
in 3.51.3 and backported to 3.44.6/3.50.7 [1]. Local Python uses 3.46.1; DDL tests
on that engine are not production qualification. No dependency is silently changed
here; the Rust bundling/upgrade must be compiled and separately verified.

## Retention, retries and cursor semantics

Canonical messages/results/context/pinned artifacts are not removed with outbox
rows. The outbox is a short-lived invalidation stream, NOT durable job delivery,
security audit, usage accounting or an eternal event-sourced transcript. Version
any incompatible replay API; provide an adapter/snapshot contract for upstream
consumers. Do not label this invisible OpenCode event-history parity.

Admission UUID+digest binds workspace, session, delivery and exact request. Exact
retry returns the existing receipt; changed content conflicts. Generic receipts
have a retry horizon: validate a trusted authenticated issuance/expiry token
BEFORE lookup, so pruning cannot turn an old retry into a fresh operation. The
receipt commits with mutation/outbox. A token is not authorization on its own.

Cursor=(workspace UUID,cursor_epoch,seq). Head is separate from retained rows;
never use MAX(retained seq) to allocate. Prune a bounded CONTIGUOUS prefix through
K and advance floor in the same transaction. Age/count/byte policy chooses a
prefix, not arbitrary holes. Cursor below floor, above head or old epoch -> RESYNC.
Filtered queries return the scanned high watermark even without matching events.

Subscribe to a bounded advisory wake signal BEFORE snapshot/page read; read
floor/head/page together, then requery durable rows when notified. Coalesce wakes;
slow consumers resync rather than pin old outbox rows forever. Resync pages return
their read watermark and subsequent invalidations refresh affected resources.
Independent pages are not a consistent whole-workspace snapshot; consumers needing
one must use a bounded-lifetime snapshot/export protocol with deletion exclusion.

## Activation gates

This design does not modify the user's 90 GB DB. Import is explicit, read-only
source to NEW destination, paged and resumable, with count/hash verification before
catalog switch. Preserve unknown IDs/provider metadata instead of silently dropping.

Next implementation: new-file schema initializer and bounded writer; streaming CAS;
GC/backup/recovery lifecycle; format-1 importer only after source verification.
Required before activation: compiled Rust tests, actual fsync/no-clobber/OS-lock
fault tests, crash and race stress, disk-full behavior, backup/restore validation,
and measured CPU/RAM/latency/TOTAL-disk/write-amplification workloads. No performance
savings, full parity or production durability are claimed from SQL tests alone.

References checked 2026-09-13:
[1] https://sqlite.org/wal.html
[2] https://sqlite.org/foreignkeys.html
[3] https://sqlite.org/pragma.html

# Embedded-database evaluator / optimizer review

One-author self-review, not separately authorized subagents or independent
acceptance evidence. Scope: exact new-file SQLite schema, index contracts and
crash-consistency protocol. Existing runtime and user data are not switched.

## Review role prompt

Act as a senior embedded-database and storage-reliability engineer. Optimize total
RAM, CPU, disk, latency and write amplification while preserving promised data.
For every invariant name its SQL constraint or runtime owner, crash boundary,
adversarial interleaving, recovery outcome and test. Distinguish process crash
from power loss, atomic visibility from durable publication, proposed tuning
from measurements, and canonical history from disposable caches. Reject claims
stronger than evidence; never delete user data to make a benchmark look smaller.

## Findings and optimizations

| Earlier weakness | Revised decision |
| --- | --- |
| Delete blob row then unlink races republishing | DELETING tombstone until physical unlink/sync finishes |
| Mark/sweep grace misses live readers and publishers | Retention leases plus fresh transaction-time reachability checks |
| NORMAL can resurrect references after durable deletion | Canonical FULL; weaker profile deferred |
| Promoted FK SET NULL contradicts state CHECK | Deferred same-session NO ACTION FK |
| Inbox promotion duplicates prompt bytes | Shared immutable payloads; metadata-only promotion |
| Tools can refer across sessions | Composite ownership FKs, assistant-role and immutable-binding guards |
| UUID string overhead everywhere | Binary external IDs and compact internal integer keys |
| Duplicate descending indexes | Reuse equivalent UNIQUE keys; query-to-index map |
| Generic idempotency outlives deleted receipt | Authenticated retry horizon; reject expired retries |
| Pruning resets MAX(sequence) or punches TTL holes | Separate head/floor/epoch and contiguous-prefix retention |
| Uncertain side effect treated as failure/retry | Durable uncertain state keeps execution gate closed |
| SQL cell caps mistaken for whole-disk bounds | Independent DB/WAL/CAS/staging/cache/backup quotas |
| Unlimited temp memory and disabled checkpoints | FILE temp policy, checkpoint safety net, bounded readers |
| Exists() treated as valid dedup content | Validate immutable header/digest/length before adoption |
| Backup uses live roots after snapshot | Snapshot-specific root copy/pins while GC excluded |
| Forgotten newer SQLite WAL defect | Fixed engine qualification gate, not blind dependency bump |

The SQL contract tests validate constraints, ownership, immutable data, collection
barriers, promotion/fork retention, query plans, rollback and process-exit recovery.
An intentional removal of the readiness trigger must cause a negative test to
fail. This demonstrates that test detects that fault, not all storage faults.

## Not certified by this wave

No independently frozen TDD acceptance, full-repository regression run, Rust
compilation, production resource benchmark, real filesystem durability, OS
confinement or power-loss proof. SQL constraints do not implement writer state
machines, authority checks, filesystem exclusion, queues or retry-token issuance.
The exact existing OpenCode 90 GB schema/content was not inspected or modified.
Saved grants, usage accounting, sharing/import protocols and compatibility
adapters still require feature work. These gaps remain explicit release gates.

Next implementation boundary: schema initialization for NEW files and a bounded
Rust writer, using these same DDL files. Only then activate streaming CAS, GC and
recovery after platform fault tests. Leave existing format-1 databases untouched.

# Storage format 2: crash-consistency protocol

Normative DESIGN, not a claim that the existing BlobStore implements it. SQL tests
cover constraints and transaction recovery; filesystem/power-loss tests remain.

## Guarantees and assumptions

Same-host local storage, working atomic installation and durable sync, a suitable
SQLite VFS, and exclusive trusted-daemon ownership are required. Process death,
power loss, partial writes, disk full and uncertain external effects are distinct.
Media corruption, a hostile administrator or hardware lying about flushes cannot
be repaired by an ordering rule. Detect/quarantine rather than fabricate content.

Canonical SQLite uses WAL+FULL [1]. File fsync alone does not durably publish the
containing directory entry on Linux; directory fsync is also needed [2]. Windows
and macOS need tested platform implementations. `std::fs::rename` is not a portable
no-replace install or a universal durability guarantee.

After the platform/runtime gates pass: acknowledge only durably committed state;
commit blob references only after durable publication; prevent GC deleting live
references/readers/backups; commit mutation/receipt/outbox atomically; never
silently replay ambiguous external effects. No exactly-once arbitrary shell/API
promise and no transaction spanning SQLite, catalog and filesystem [3].

## Ownership, exclusion and lock order

Acquire installation/workspace OS-held ownership locks before opening writable
stores or scanning staging. A lock filename or stale PID is not ownership. Use
private canonical paths; agents/plugins never open DB/blob directories. Increment
owner_generation under FULL after lock acquisition, and reject writer commands
from earlier generations. This is same-host fencing, not distributed ownership.

Initial implementation uses a workspace retention RW gate. Publishers hold a
shared lease from installation through canonical commit; readers acquire a shared
lease BEFORE querying payload references and retain it until streaming closes;
backups hold a shared lease through snapshot-specific root pinning/copy. GC,
unlinks, tombstone recovery and orphan sweeps take the exclusive side. This closes
the old-read-snapshot-to-file-open gap. Ordinary readers/publishers can coexist;
a slow stream may delay GC, so leases have cancellation/budget policy. Do not
expire a lease and unlink while its reader is still executing. Narrower per-hash
GC exclusion is an optimization requiring separate race tests.

Per-hash locks serialize duplicate publishers. Acquire multiple hashes in byte
order. Global order: OS ownership -> retention gate -> sorted hash locks -> DB
command. No SQL transaction may wait for locks, compression, files, network,
subprocesses or async awaits. The retention gate is not held during initial staging
spool. GC processes small batches; it never holds exclusion while waiting for an
unbounded client. Bound/evict idle hash-lock map entries.

## DB-only mutation and idempotency

Validate authority, schema, generation, budgets and retry token first. In one
BEGIN IMMEDIATE transaction: check the operation digest/receipt; allocate explicit
session/input sequence; mutate canonical state; record bounded result receipt;
increment event_head_seq; insert corresponding outbox row. COMMIT FULL before
acknowledgement and advisory notification. On rollback no partial pieces remain.

A lost reply is resolved by receipt, not blind reexecution. A failed COMMIT may
leave outcome unclear: recover/lookup operation ID and return its result or an
uncertain outcome. Do not assume every I/O error proves rollback. Group commits
only within documented batch semantics; isolate unrelated errors with savepoints
or separate transactions. One failed command never receives another's success.

Admission promotion is one such transaction: create visible message; move/copy
part references without copying payload bytes; set receipt/promoted message;
remove input part references; commit outbox. Pending input is not history until
promotion. Forks copy immutable payload references into independent metadata.

## Blob container

Logical identity=BLAKE3(`opencode-rk/blob/v2\0` + exact raw bytes), workspace-local.
Do not normalize text, JSON or provider signatures silently. Codec is independent
of identity; already-compressed media may use raw. Enforce independent raw/stored
byte and decompressor-window ceilings; streaming does not mean unlimited disk.

52-byte header, integers little-endian:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 8 | ASCII ORKBLOB followed by NUL |
| 8 | 1 | format version 2 |
| 9 | 1 | codec 0=raw, 1=zstd |
| 10 | 2 | flags=0; unknown flags rejected |
| 12 | 8 | raw length |
| 20 | 32 | logical digest including domain prefix |
| 52 | remaining | encoded payload |

Check magic/version/flags, codec, decoded length, digest and no unexpected trailing
bytes. A fresh or unverified stream cannot be treated as verified before complete
validation. Previously verified immutable objects may serve bounded reads; actual
media integrity still requires access validation/scrubbing. Do not store secrets
or unsolicited full provider recordings in CAS.

## Publication: durable bytes before references

1. Reserve memory, staging and retained-disk capacity. Exclusively create a random
   private staging file on the SAME filesystem as final blobs. Stream/hash/encode
   with bounded buffers and previews. Enforce raw AND stored ceilings while writing.
   Discarded bytes mean explicitly partial output, never a claim of full retention.
2. Finish encoder, finalize header, flush and sync file. On any error create no
   canonical reference and return no success. Cleanup may leave a staging orphan.
3. Acquire shared retention lease then hash lock. A DELETING tombstone is not
   adoptable. Release leases and request deletion completion/retry; do not deadlock
   waiting for GC while retaining a shared lease that excludes GC.
4. Install at `blobs/ab/<digest>.blob` with tested atomic NO-REPLACE semantics.
   Existing file requires complete validation/known immutable identity, not simply
   `exists()`. Corrupt live files are unavailable/quarantined, never overwritten as
   a convenient dedup fix. Validate header, digest, lengths and codec metadata.
5. Sync final directory and newly created ancestors in required order. Also sync
   staging directory when rename/removal persistence matters. Any failed durability
   barrier prevents reference commit. Sync newly created DB/directory entries too.
6. Holding leases, execute a short DB transaction: insert-or-find READY blob;
   validate stored metadata; insert immutable payload and canonical root;
   receipt/outbox; COMMIT FULL. After an INSERT conflict, explicitly select the
   existing key rather than trusting last_insert_rowid. No files/compression here.
7. Release leases, acknowledge. Crash before step 6 may leave harmless orphans;
   crash after commit leaves a root whose bytes were installed durably first.

Staged/published bytes alone do not constitute a committed user message.

## GC: retain tombstone through physical removal

Delete-registry-then-unlink is unsafe: a publisher can recreate the hash in between
and lose a new live reference. Format 2 retains a UNIQUE hash tombstone instead.

1. Acquire exclusive retention gate (wait for/cancel and JOIN active leases under
   policy). Compute/recheck payload_roots, including inputs, messages, execution
   config, tools, epochs/checkpoints and artifact/share/backup pins. Delete unrooted
   payload rows in bounded DB transactions. FKs remain enforced. Every new payload
   owner must extend the root view and its coverage test.
2. Lock selected hash. In a short IMMEDIATE transaction recheck READY and no
   payload reference. Set DELETING; COMMIT FULL. A stale mark snapshot is not proof.
   SQL rejects reference creation to DELETING/UNAVAILABLE and collecting referenced
   blobs. Do not resurrect DELETING; complete removal then republish if needed.
3. Still holding exclusion/hash lock, unlink physical file and sync its directory.
   Missing file is idempotent success for DELETING, but corruption for a READY
   reference. I/O failure retains tombstone for a later retry.
4. Delete tombstone in a second FULL transaction, then release locks. Only then
   can a new publisher use that hash. Crash after unlink retains a resumable intent.

Orphan files lacking registry rows use the same exclusive gate/hash lock and a
fresh lookup before unlink. Grace age reduces churn, not races. Staging cleanup
also requires ownership and explicit exclusion from active spool jobs; never delete
an in-flight spool merely because its modification time is old. Traverse bounded
private directories without following symlinks. Corrupt READY objects become
UNAVAILABLE with references preserved for diagnostics/recovery, not empty messages.

Canonical FULL barriers matter: NORMAL could lose a committed reference deletion
after power loss even though an unlink became durable. Any future weaker mode
must separately prove destructive and dispatch barriers; not a trivial toggle.

## Startup, execution uncertainty and shutdown

Under OS ownership, let SQLite recover WAL; never manually delete -wal/-shm.
Validate format/checksum/application identity. Read previous clean marker, persist
clean_shutdown=0 and advance owner generation under FULL BEFORE accepting work.
Dirty stores receive bounded integrity checks; clean marker is not proof against
media corruption. Finish DELETING intents. Reconcile staging/orphans under locks.
Validate referenced files on access plus bounded background scrubs, not a full
blob scan on every launch. Wall-clock rollback must not invalidate active leases.

Before provider/tool side effects, commit prepared intent and final permission
binding, then mark DISPATCHED under FULL BEFORE the actual call. After result,
publish payload and terminal state/receipt/outbox atomically. A crash between
marking dispatch and making the call is conservatively UNCERTAIN. Persisting
intent after side effect would invite duplicate execution.

Unsettled dispatched attempts/tools and old running executions become UNCERTAIN,
not fabricated failures eligible for silent retry. This blocks replacement runs.
Pending unpromoted inputs stay pending; promoted inputs are not re-admitted.
Interrupted streams retain only durably checkpointed parts; recent ephemeral
deltas may be lost and must not appear committed. Parent completion delivery is
reconciled from durable execution rows, not from retained outbox notifications.

Retry/resume/abandon requires an explicit authorized policy decision. Query known
provider status/idempotency contracts where supported; arbitrary shell commands
have no universal exactly-once contract. Retry uses a new attempt identity.
OAuth remote rotation can invalidate credentials before local persistence; failure
may need reauthentication, not a fictional local transaction over the provider.

Shutdown: stop admission, drain/cancel producers, resolve/record ambiguous state,
join readers, finish writer work, optionally checkpoint, set clean marker only
when owned activity is resolved, close connections, release OS locks. A failed
checkpoint does not authorize deleting WAL. Checkpoint work is writer-serialized;
blocked readers cause bounded backpressure, not unlimited WAL growth.

## Backups, restore and cross-store boundaries

A DB snapshot must be paired with the blobs reachable from THAT snapshot, not a
later live scan. Acquire a shared retention lease before snapshot creation so GC
cannot unlink. Use SQLite backup API [4], enumerate copied DB roots, copy/verify
all required immutable blobs, sync files/manifest/directories, atomically publish
backup, then release lease. Writes may continue through backup API semantics.
Timeout/restart bounded backups rather than keeping exclusion forever. Large or
remote backups need durable snapshot-specific pins before releasing the lease;
never hold a client-speed SQL transaction. Pins expire only after owner-controlled
complete/abandon reconciliation, not a timer that can race an active backup.

Restore into NEW private storage. Verify manifest, schema, FK integrity and every
rooted blob; rotate cursor_epoch, mark external effects uncertain, then switch
catalog visibility. Outbox cursors from the old epoch require resync.

Workspace creation is a saga: catalog PROVISIONING -> create matching workspace
UUID/schema and sync entries -> catalog READY. Hide provisional entries and
reconcile interrupted states. Cached counts are advisory. User-approved deletion:
catalog DELETING -> stop/close all workspace activity -> remove only derived
private app storage -> sync parent -> retire catalog row. Never delete the user's
project tree. A move/import is copy+verify then pointer switch, not cross-file ACID.

New secrets are durably created in broker before switching secret_ref generation
in catalog; old material retires only after leases/reconciliation. Failure should
leave an unused secret, not a reference to never-persisted material. No raw secret
values in SQLite, test logs, model-visible data or shared backups.

## Crash/failure matrix

| Boundary | Expected recovery |
| --- | --- |
| Incomplete spool/header/codec | Private partial file, no root; bounded cleanup |
| File synced, installation not durable | No allowed reference commit; orphan possible |
| Blob durable, canonical commit absent | Orphan; later sweep after lease ends |
| Canonical transaction rolled back | No partial state/receipt/outbox |
| Commit succeeds, reply lost | Same operation returns committed receipt |
| Commit I/O outcome uncertain | Recover/read receipt before deciding |
| GC tombstone committed, unlink not run | New references blocked; resume deletion |
| File unlinked, tombstone remains | Idempotent sync/removal completion |
| Publisher races sweep | Retention/hash gates and fresh state check serialize |
| Fork parent deleted | Independent metadata keeps shared payload rooted |
| Process lost around external call | UNCERTAIN, no automatic side-effect replay |
| Backup fails | Partial backup hidden; live roots/pins preserved |
| Disk-full/fsync failure | No false acknowledgement; keep recovery headroom |

Required runtime tests: failpoint at every boundary; real process kill/reopen;
separate power-loss/torn-write/VFS tests; publish/GC/reader/backup race stress;
corrupt header/digest/length/decompression bounds; filesystem no-replace and sync
on each supported OS; stale generations; disk-full settlement; replay-floor/restore
cursor and expired receipt tests. Local SQL/process tests are not substitutes.

References checked 2026-09-13:
[1] https://sqlite.org/pragma.html
[2] https://man7.org/linux/man-pages/man2/fsync.2.html
[3] https://sqlite.org/wal.html
[4] https://sqlite.org/backup.html

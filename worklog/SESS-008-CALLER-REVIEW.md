# SESS-008 caller/lifetime review

## Claim and scope

- Task: `SESS-008`.
- Session: `ses_f32bf295effe9f16wtSPaJNIxT`.
- Candidate: `da34ba8fc1331aa90643952c91b040a36928d256`.
- Scope: integration-boundary audit only. No product, test, manifest, Cargo, or
  controller edits.

## Source evidence

- `crates/sessions/src/persist.rs:21-30,38-121` defines an owned `Vec<SessionSummary>`
  plus `HashSet<SessionId>`. `save` replaces and marks dirty; `load` clones;
  `flush` only clears dirty; `compact` removes dirty entries; no `Storage`, I/O,
  task, scheduler, or caller exists. `flush_interval_ms` is advisory metadata.
- `crates/sessions/src/lib.rs:331-367` defines `SessionService` over one
  `Arc<Storage>`. There is no `mod persist` declaration at lines 4-51 and no
  source reference to `persist::PersistentSessionStore`.
- `crates/sessions/src/lib.rs:394-475` routes `create`, `get`, `list`, `rename`,
  `archive`, and `append_text` directly to `Storage` through
  `spawn_blocking`. `append_assistant_with_reasoning` continues the same path at
  lines 476-509. Fork calls use the separate `SessionManager` path selected by
  `fork_manager_for` at lines 854-867.
- `crates/sessions/src/store.rs:26-125` already owns a different,
  SQLite-backed `PersistentSessionStore`; the same name is not a shared trait or
  implementation seam.
- `crates/storage/src/lib.rs:100-196,198-240` owns SQLite session/message
  writes and reads. `Storage::open` configures/migrates the database at
  lines 105-116; mutations commit through SQLite, including transactional
  message writes at lines 222-239.
- `crates/server/src/lib.rs:137-200,371-440` exposes the public HTTP session
  boundary and calls `SessionService`, not `persist.rs`.
- `crates/server/src/runtime_wiring.rs:322-340` and
  `crates/server/src/app_runtime.rs:609-637` make one daemon-owned
  `SessionService` the shared durable handle; `store` is only a clone of that
  same service, not a second cache authority.
- `crates/storage/tests/restart_v2.rs:92-107,110-159,162-191` proves committed
  SQLite rows survive close/reopen and uncommitted rows do not. This is the
  Phase 1 durability owner, not `persist.rs`.
- `crates/sessions/src/lib.rs:1052-1069` exercises public `SessionService`
  create, append, reconstruct, get, and message reads. It is already GREEN and
  uses `Storage`; it does not call the in-memory cache.

## Caller and lifetime decision

No legitimate Phase 1 caller exists. `persist.rs` is state-only/dead under the
current module graph. Wiring it into `SessionService` would duplicate the
SQLite source of truth:

1. `create` would write SQLite and separately populate a cache.
2. `rename`, `archive`, message append, reasoning append, artifact paths, and
   fork paths would each need cache updates or invalidation.
3. A second daemon/client or a direct `Storage`/`SessionManager` writer could
   change the row without notifying the cache. `get`/`list` would then return a
   stale `SessionSummary`.
4. `flush()` currently clears a set in memory and performs no write. Treating it
   as a durability boundary would falsely promise persistence; process exit
   loses all cache contents while committed SQLite data remains.

The cache has no current user-visible benefit. A defensible future benefit would
be only a bounded read-through reduction in repeated summary queries, with exact
results equal to SQLite and a measured query/latency improvement. No such
public contract, instrumentation, or acceptance target exists here.

## Required contract if a cache is later authorized

- **Owner/lifetime:** one cache owned by the daemon's `SessionService`/`Storage`
  authority, shared by service clones; never a server-route-local cache.
- **Invalidation:** atomically update or evict after every session-summary
  mutation (`create`, `rename`, `archive`, message timestamp updates, all fork
  writes), and detect/reject writes from other authorities. Current APIs provide
  no cross-authority version/notification mechanism.
- **Crash semantics:** cache is disposable; dirty entries may be lost. Only a
  committed `Storage` transaction survives reopen. `flush` must not be named or
  documented as durable unless it actually awaits a bounded SQLite commit and
  reports failures.
- **Bounds:** explicit maximum entries, total retained bytes, title/summary
  byte accounting, and eviction/backpressure. Current `Vec`/`HashSet` grow with
  every save and have no quota or eviction policy.
- **One-file sequence:** not applicable. No single `pub mod persist` change can
  supply a caller, invalidation protocol, durability semantics, or bounds.

## SESS-011 disposition

`tasks/SESS-011.md:3-30` describes async flush to a `Storage` backend and maps to
forking. That is a distinct durable-cache/delegation contract, while
`tasks/SESS-008.md:22-36` defines an in-memory write-through model. Do not make
SESS-011 reuse this module by inference. Controller authority must retire
SESS-008 as redundant for Phase 1 or decompose SESS-011 into a separately owned,
Storage-backed task with a real caller, invalidation, commit/error, and resource
contract. No task-state or controller edit is authorized here.

## RED assessment

No valid behavioral RED exists at the current public `SessionService` boundary
for SESS-008. Existing public operations already compile and pass against
SQLite. A cache-hit test needs hidden query instrumentation or a new public API;
neither exists before implementation. A test requiring `persist::flush()` to
survive restart would contradict its current in-memory contract and would make
`flush()` falsely imply durability. A valid RED can be authored only after the
controller approves a new public cache contract, such as bounded cache metrics
or an explicit durable flush API. Do not invent that contract in this lane.

## Verification and blocker

- `python3 tools/convergence_gate.py`: blocked, 53 pre-existing findings; no
  product change made.
- SESS-008 corrected harness evidence remains `5 passed; 0 failed` per
  `worklog/SESS-008-TEST-REPLACEMENT.md:55-75` and
  `worklog/SESS-008-SAVE-TEST-CORRECTION.md:52-77`; those tests do not establish
  a real caller or durability.
- Decision: `SESS-008` blocked. Retire/decompose through controller authority;
  do not wire the cache into the Phase 1 SQLite `SessionService`.

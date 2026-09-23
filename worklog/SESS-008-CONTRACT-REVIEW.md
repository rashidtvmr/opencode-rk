# SESS-008 contract review (authority resolution)

## Claim
- Task: SESS-008. Session: ses_f3c4de578ffelQv59xDXmOs03B (reclaimed from prior stopped owner).
- Ledger: claimed `in-progress` via tools/completion_claims.py (row verified).
- Scope: audit/contract proposal only. NO Cargo, NO product/test edits in this wave.

## Source evidence (commit 9ded2b901b2376a4a5ee1815a41c7a1234ece157)

### Task cards
- tasks/SESS-008.md:1-55 (Status IN PROGRESS, product, mandatory for full release)
  - Observable contract (lines 22-36): `compact()` removes **orphaned (still-dirty) sessions** and retains flushed; `flush()` clears dirty set retaining sessions. Fields: sessions, dirty, flush_interval_ms. Methods: save/load/flush/compact/stats/flush_interval_ms.
  - Test obligations: SESS-008-T01..T05 (lines 38-47).
  - Verification (lines 49-55): `cargo test -p opencode-rk-sessions && cargo check --workspace`.
  - Dependencies: `crates/contracts provides SessionSummary, SessionId, Timestamp, SessionState`.
  - NOTE: SESS-008.md explicitly governs this lane (lines 14, 15).

- tasks/SESS-011.md:1-37 (Summary: PersistentSessionStore in-memory cache + async flush to Storage)
  - Design Notes (lines 26-30): "flush/compact are stubbed for now (clear dirty set / no-op removal)" and "compact removes sessions that are no longer referenced (simplified: removes archived sessions from the in-memory list as 'orphans')."
  - SESS-011-T01..T05 (line 24).
  - Owned File: crates/sessions/src/persist.rs (line 33).

### Backlog (ralph.json)
- SESS-008 (line 2046): status "accepted", requirementIds [], dependencyIds [], testObligations SESS-008-T01..T05.
- SESS-011 (line 2088): status "accepted", requirementIds ["REQ-008"], dependencyIds [], testObligations SESS-011-T01..T05.
- REQ-008 (requirements/user-requirements.json:82-90): "Session forking", tasks SESS-011/UI-004/SESS-018/SESS-019, mandatory true.
- No declared dependency between SESS-008 and SESS-011; both accepted.

### Product code
- crates/sessions/src/persist.rs:1-203 (HEAD 9ded2b9, sha256 3b6faddb801e5dd9ea8099ecaf5bbbbafecd1a3685009ee1788ec8454075e56a)
  - Struct: sessions: Vec<SessionSummary>, dirty: HashSet<SessionId>, flush_interval_ms: u64 (lines 26-30).
  - save() (62-70): insert-or-replace + mark dirty.
  - load() (73-75): find+clone or None.
  - flush() (81-84): dirty.clear() -> Ok (write-through in-memory, no I/O).
  - compact() (91-98): **retain iff id NOT in dirty; drop removed from dirty** (card-faithful to SESS-008).
  - flush_interval_ms() (113-115): accessor added.
  - is_dirty() (119-121): accessor added.
  - Error enum (14-19): FlushError(String), LoadError(String).
  - Tests (124-203): save_and_load, flush_clears_dirty, compact_removes_orphans (SESS-011 archived semantics at 162-176), load_missing, stats_track.

- crates/sessions/src/lib.rs:4-51 — `pub mod` list has NO `persist` declaration. persist.rs is **dead code** (never compiled). Confirmed by worklog/SESS-008.md:26-28: `cargo test -p opencode-rk-sessions --lib save_and_load` runs 0 tests.

- crates/sessions/src/store.rs:27-125 — **separate** type `PersistentSessionStore { conn: Connection }` (SQLite-backed, sync CRUD). Name collision only with persist.rs in-memory struct. No shared code (no shared trait). Doc comment at store.rs:1-3 describes it as "Persistent session store wrapping an SQLite connection."

- crates/storage/src/lib.rs:100-638 — `Storage` struct with SQLite WAL persistence, `create_session`/`get_session`/`list_sessions`/`archive_session`/`append_message` etc. This is the **actual durable persistence** layer. `incremental_vacuum` (634-637) is the SQLite-level compaction; `retention_v2.rs` does bounded row sweeps. No in-memory dirty-tracking cache here.

- crates/sessions/src/lib.rs:332-869 — `SessionService` (the format-2 lifecycle caller). `create` (394-408) -> `Storage::create_session`. `get` (409-416) -> `Storage::get_session`. `archive` (443-450) -> `Storage::archive_session`. `append_text` (451-475) -> `Storage::append_message`. SessionService wraps `Arc<Storage>` (333); no reference to persist.rs `PersistentSessionStore`.

- crates/storage/tests/restart_v2.rs:1-9 (header doc) — "Process-restart durability contracts... proves committed data survives connection close plus reopen (process-equivalent) on the SAME host." This is the Phase 1 close/reopen persistence contract.

- crates/storage/src/snapshot_v2.rs:372-381 — `close_then_reopen_increments_epoch`: epoch counter survives reopen.

## The semantic conflict

| Aspect | SESS-008 (authoritative for persist.rs) | SESS-011 (Session Forking / REQ-008) |
|---|---|---|
| compact semantics | remove still-dirty (unflushed) sessions; retain flushed | remove archived sessions (simplified orphan = archived) |
| flush semantics | clear dirty set (write-through, in-memory only) | "async flush to Storage backend" (future durable) |
| current state | compact impl at persist.rs:91-98 follows SESS-008; test at 162-176 follows SESS-011 | — |
| error return | Result<(), PersistentSessionStoreError> | Result<(), PersistentSessionStoreError> |

The in-file test `compact_removes_orphans` (persist.rs:162-176) saves one Active + one Archived session, does NOT flush either, then calls `compact()` and asserts exactly 1 remains (the Active one). Under SESS-008 semantics: both are dirty (neither flushed) -> both should be removed -> 0 remain. The current test will FAIL against the SESS-008-compliant implementation once `mod persist` is wired.

## Authority resolution

1. SESS-008 owns the file `crates/sessions/src/persist.rs` and its card says "governs this lane" (SESS-008.md:14). Its observable contract is internally consistent and deterministic (in-memory, no I/O, no threads, no clock).

2. SESS-011 maps to REQ-008 (Session Forking) — a higher-level product requirement about fork branch workspaces, not about the in-memory dirty-tracking cache. SESS-011's `PersistentSessionStore` description ("async flush to a Storage backend", "stubbed for now") describes a **future durable store** that would sit on top of `Storage` (storage/src/lib.rs), not the in-memory cache SESS-008 defines. SESS-011 is a separate concern: cache coherence vs. durable fork persistence.

3. ralph.json: SESS-008 has no requirementIds (standalone leaf); SESS-011 has REQ-008. No dependency declared between them. Both accepted.

4. PLAN.md ADR-003 (behavior parity) and AGENTS.md step 2 demand distinguishing native V2 / planned upstream / new requirement / deviations. SESS-008's compact is a **new requirement** (in-memory cache contract). SESS-011's "archived removal" was likely copied from a V1-era notion and is **partial/planned upstream** for the durable store.

**Conclusion: SESS-008's contract supersedes for the file it owns.** SESS-011 must not redefine the semantics of persist.rs. They describe different layers:
- persist.rs = in-memory dirty-tracking cache (SESS-008).
- A future durable session store wrapping `Storage` = SESS-011's async flush domain (not yet implemented; would be a separate module/method or a `Storage`-backed `PersistentSessionStore` variant).

The conflict is **resolved by separation**, not by one overriding the other. The in-file `compact_removes_orphans` test (persist.rs:162-176) is the artifact of the old SESS-011 description and must be rewritten to SESS-008's contract (the test obligation SESS-008-T03 defines: "only un-flushed (dirty) sessions are removed; flushed sessions remain").

## Phase 1 persistence / crash / restart contract

Source evidence: docs/CONVERGENCE.md:8-24 (hard local boundary step 7: "persist the session/tool result, exit, restart, and resume the same history"); PLAN.md:127-141 (M1 = create+reopen session); crates/storage/tests/restart_v2.rs:1-9 (WAL survival across reopen); crates/storage/src/snapshot_v2.rs:372 (epoch increments on reopen).

Observable contract required by Phase 1 create/reopen journey:
- `SessionService::create` (lib.rs:394) writes via `Storage::create_session` (storage/src/lib.rs:134) to SQLite in WAL mode (lib.rs:641).
- `SessionService::get` (lib.rs:409) reads via `Storage::get_session` (lib.rs:142).
- Restart: re-instantiate `Storage` with same DB path -> WAL survives reopen (restart_v2.rs:5-8 documented claim).
- Epoch: `SnapshotV2::close_then_reopen_increments_epoch` (snapshot_v2.rs:372) proves metadata version advances on reopen.

persist.rs `PersistentSessionStore` is NOT in this durable path. It is an in-memory cache with no path to `Storage`. Its flush/compact are cache-coherence operations, not write-to-disk. Therefore:
- Durable crash/restart: owned by `Storage` (rusqlite SQLite WAL) + `SessionService`.
- In-memory cache flush/compact: owned by persist.rs (SESS-008).
- The two layers are orthogonal. SESS-011's "async flush to Storage backend" belongs to a not-yet-built durable cache layer above `Storage`, NOT to persist.rs as currently specified.

## Failure states and resource bounds (per persist.rs)
- flush(): always Ok(()) in-memory. Returns Err(FlushError) only if a future caller wires an I/O backend that fails.
- compact(): always Ok(()) in-memory. Returns Err(FlushError) only if future async/Storage backend fails.
- load(): always returns Option (infallible in-memory). Returns Err(LoadError) only if future backend fails; current signature returns Option (no error path) per SESS-008 card.
- Resource bounds: Vec + HashSet grow with saves; compact reclaims only dirty (flushed) entries. No quota per card. Process exit loses all (non-durable). No background threads, no clock, no globals.

## Migration / wiring impact
- lib.rs has NO `mod persist` (lib.rs:4-51 grep clean). Wiring is an integrator lane (worklog/SESS-008.md:71). This subagent must NOT touch lib.rs.
- store.rs `PersistentSessionStore` (27-125) is a separate SQLite type in the same crate. Name collision is pre-existing; if persist.rs is wired via `pub mod persist`, consumers must qualify: `persist::PersistentSessionStore` vs `store::PersistentSessionStore` (or `crate::PersistentSessionStore` via the existing `pub type SessionRecord` alias at lib.rs:64). A naming collision resolution should be proposed to the integrator, not authored here.
- The in-file test `compact_removes_orphans` at persist.rs:162-176 must be rewritten to assert dirty-removal semantics (SESS-008-T03). This is a RED test author action by the independent test-author role before freeze, per TDD.md:3-4 and AGENTS.md convergence boundary. This subagent does NOT edit tests in this audit wave.

## Recommendation (exact)

1. **Accept SESS-008 as authoritative** for `crates/sessions/src/persist.rs`. Its compact/flush/dirty-tracking contract is the operative spec for this file.

2. **Decompose SESS-011**: it describes a *durable* async-flush session cache over `Storage`, which is a distinct layer. Either:
   (a) SESS-011 should own a separate file (e.g. `crates/sessions/src/persist_durable.rs`) backed by `Storage`, with async `flush`/`compact` calling `Storage` writes; OR
   (b) SESS-008's in-memory `PersistentSessionStore` is renamed (e.g. `SessionCache`) and SESS-011 gets `PersistentSessionStore` as the durable variant. Recommended: separate methods/variants in separate files to avoid name collision with store.rs:27.

3. **RED test to author next** (independent test-author, pre-freeze): rewrite `compact_removes_orphans` in persist.rs to assert SESS-008-T03 semantics:
   - save Active + Archived (both dirty, neither flushed);
   - call compact();
   - assert sessions.len() == 0 (both dirty -> both removed);
   - alternatively: save, flush one, re-save the other (so one flushed, one dirty); compact removes only the dirty one.
   - This is a frozen-test-blocking issue: the existing test at 162-176 must be replaced
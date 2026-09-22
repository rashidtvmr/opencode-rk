# SESS-008 scratchpad

## Claim
- Task: SESS-008. Session: ses_f3840883fffeumWydX7cbtnZ04.
- Ledger: in-progress via tools/completion_claims.py claim (row verified).
- Owned file: crates/sessions/src/persist.rs only. No edits to store.rs/lib.rs/tests/Cargo/schema/controller/verifier/policy.

## Source evidence (HEAD f49777a0f821c5fbd5727be90676b83373aa9e5a)
- tasks/SESS-008.md:1-55 governs. Observable contract: save insert/replace+dirty;
  load clone/None; flush clears dirty, retains sessions; compact removes ONLY
  dirty/unflushed, retains flushed; stats + interval accessors deterministic.
  Tests SESS-008-T01..T05. Verify: cargo test -p opencode-rk-sessions && cargo check --workspace.
- tasks/SESS-011.md:1-37 conflicting history: compact described as stub removing
  archived sessions; flush/compact called stubs. SESS-008 governs this lane.
- Baseline crates/sessions/src/persist.rs (sha256
  0b0ef0c6efb2cec5469e02937aa12763ffb265bae67e6f406ff9d72ec42503af,
  copy at /tmp sess008/persist_baseline.rs): struct fields
  sessions:26-30/dirty/flush_interval_ms. save:62-70 correct. load:73-75
  correct. flush:81-84 correct. compact:90-94 WRONG per SESS-008 (retained
  state != Archived, ignored dirty set). No flush_interval_ms() accessor
  (field write-only). Error enum:13-19 kept.
- In-file tests:106-183 names match T01..T05; compact_removes_orphans:143-156
  encodes SESS-011 archived semantics (both dirty, expects active retained).
- crates/sessions/src/store.rs:27-29 SEPARATE SQLite PersistentSessionStore::new(conn).
  Name collision only; no shared code. persist.rs store is in-memory only.
- crates/sessions/src/lib.rs: NO `mod persist` (grep clean). persist.rs dead
  code: never compiled; `cargo test -p opencode-rk-sessions --lib save_and_load`
  and compact_removes_orphans each run 0 tests (42 filtered out).
- crates/contracts/src/lib.rs:59 SessionId Copy/Hash/Eq; :146-154
  SessionSummary fields; :155-160 SessionState; :124-144 Timestamp.
- tools/lane_gate.py:31-44 no SESS-008 lane; gate not applicable.

## Observable contract / failure / resource semantics
- Failure: flush/compact return Result<(), PersistentSessionStoreError>;
  in-memory path always Ok. No I/O, threads, clock, globals.
- Caller owns store lifetime. Vec + HashSet grow with saves; compact reclaims
  only dirty entries; no quota per card. No durability: process exit loses all.
- Durable SQLite crash recovery is OUT OF SCOPE (card authorizes in-memory).

## Tests
- Frozen: in-file mod tests (5 names = T01..T05). Baseline hash above.
- Full cargo test on baseline impossible (module unwired, 0 tests execute).
- Compiling RED (scratch crate sess008check, disposable, outside repo):
  `red_compact_retains_flushed_removes_dirty` vs baseline prod code:
  FAILED as required (panicked src/lib.rs:32 `dirty must be removed by compact`).
  Accessor probe could not compile against baseline (no method; done as
  compile observation, not a test failure).
- GREEN (same scratch crate vs fixed prod code): 3 passed, 0 failed:
  red_compact_retains_flushed_removes_dirty, green_flush_interval_accessor,
  green_stats_and_resave.
- Repo: CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-sessions --tests: pass,
  no persist errors (pre-existing warnings only: unused import/var elsewhere).
  CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-sessions:
  all suites ok (lib 42 passed; 49x5-passed integration files; zero failures).
  cargo check --workspace NOT run (other lanes building; budget rule).
- Memory: vm_stat head-8 recorded pre-commit; host stable.

## Decisions
- compact: retain iff id NOT in dirty; drop removed ids from dirty
  (retain flushed, remove orphans, keep sets consistent).
- Added flush_interval_ms() and is_dirty() accessors (card: accessor helpers).
  Kept Result signatures (frozen tests call .unwrap()). Real code, no stubs.
- Did NOT edit lib.rs wiring (integrator lane) or frozen tests.
- Did NOT claim durability or release acceptance.

## Remaining unknowns / blockers
- B1 (contract review, NOT implementation edit): in-file
  compact_removes_orphans (persist.rs:162-175 new numbering) still encodes
  archived semantics; card-faithful compact fails it when wired. Disputed
  frozen test = blocked contract review per AGENTS.md convergence boundary.
- B2: `pub mod persist` wiring in lib.rs needs integrator (out of authority).
- Follow-up seam: durable SQLite crash recovery needs a Storage/SQLite caller
  change (store.rs SessionManager/Storage own durability); this lane covers
  only the in-memory dirty/flush/compact contract. If verifier requires
  durability here, mark that portion BLOCKED, do not fabricate it.

## Landing
- New file hash: 3b6faddb801e5dd9ea8099ecaf5bbbbafecd1a3685009ee1788ec8454075e56a.
- Commit+push: pending (ledger status to be set honestly; see completion msg).

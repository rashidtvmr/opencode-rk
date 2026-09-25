# OWN-LOCK-RED scratchpad

Claim: OWN-LOCK-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `758618d643a72c826f80147d6333e97525181a3f` on
`red/APP012-WORKSPACE-OWNER-LOCK`; dependency `OWN-GENERATION-RED` is present
with frozen hash `0c528325a57d2d8eeb48e2f917f3181fd70f476a9e17a3948310da6b516e56a3`.

Source evidence:

- Current production caller, `crates/storage/src/facade.rs:19-62`,
  `StorageFacade::open`, opens or initializes a writable workspace and returns
  without acquiring or retaining an ownership guard.
- Current lifetime boundary, `crates/storage/src/facade.rs:135-143`,
  `StorageFacade::close`, checkpoints and marks clean shutdown but releases no
  explicit workspace ownership lock.
- Current low-level seam, `crates/storage/src/schema_v2.rs:98-150`,
  `SchemaV2::open_existing`, permits another writable SQLite connection; this is
  shared database concurrency, not daemon ownership.
- Planned contract, `docs/storage/CRASH_CONSISTENCY.md:25-31`, requires an
  OS-held installation/workspace lock before opening writable storage and ties
  generation advancement to that ownership.
- Current status, `docs/STORAGE.md:262`, says daemon ownership locks are not
  implemented.
- Synthesis stage `OWN-LOCK-RED` assigns this test file, while the later
  `OWN-LOCK-IMPL` currently owns only `crates/storage/src/workspace_lock.rs`.

Observable contract: while one production `StorageFacade` owns a disposable
workspace, a second writable facade for the same path fails closed. An orderly
close releases ownership so a successor can open. The owner object defines lock
lifetime; no PID-file existence claim, wall clock, network, user database, or
unbounded resource is involved.

Target boundary: RED only. No product, manifest, library, schema, or frozen-test
changes. The test uses the real production facade instead of inventing a future
`WorkspaceLock` API.

Known planning gap: satisfying this real-caller RED will require wiring the
future guard into `StorageFacade` and retaining it in the struct. The synthesis
currently assigns `OWN-LOCK-IMPL` only `workspace_lock.rs`, with no facade/lib
prewire or integration stage for this caller. The independent freeze/authority
review must expand or prewire ownership before implementation; a standalone new
module would not satisfy this test.

## RED receipt

- Focused command, run twice after correcting the public import before freeze:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_workspace_owner_lock_red --
  --test-threads=1`.
- Both valid runs compiled and produced the same behavioral result: 1 passed, 1
  failed, 0 ignored. Orderly close/reopen passed; a simultaneous second writable
  production facade was accepted instead of denied.
- Frozen SHA-256:
  `ca49a6f43a2a5e5bbf49e103c1609a5f68b819f77f81a62bed0018e742f23d6a`.
- After freezing: hash unchanged, `git diff --check` passed, and no Cargo,
  rustc, or focused-test process survived.

Remaining: `V1-FREEZE-RED` must record this hash and authority must correct the
implementation ownership/wiring gap before GREEN. No implementation or
acceptance is authorized.

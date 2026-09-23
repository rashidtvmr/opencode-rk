# SESS-008 save.clone() test correction

## Claim

- Task: SESS-008. Session: `ses_f32d3cd57ffeudkCVZIl43vEwO`.
- Ledger: claimed `in-progress` via `tools/completion_claims.py` (row verified).
- Owned paths: `crates/sessions/src/persist.rs` test section only, this scratchpad, own ledger row.
- No implementation, `lib.rs`, manifest, verifier, controller, or other test changes.

## Authorization

Authorized by task prompt: change pre-existing test call `store.save(session)` to `store.save(session.clone())` in the `save_and_load` test because the test later reads `session` (`session.id`, `session.title`) and otherwise cannot compile when the module is wired into `lib.rs`. `SessionSummary` derives `Clone` but not `Copy`; `save()` takes ownership by value.

## Source evidence

Repo: `/Users/mymac/Projects/opencode-rk-sess008` at `26d1bfb557167ac702b35b6b44d85ef9e7e5a002` (HEAD on `lane/SESS-008-contract`).

- `crates/sessions/src/persist.rs:141-150` — `save_and_load` test calls `store.save(session)` at line 144, then reads `session.id` (line 145) and `session.title` (line 149). Move-after-save compile error when module is wired.
- `crates/contracts/src/lib.rs:162` — `SessionSummary` derives `Clone` (not `Copy`).
- `crates/contracts/src/lib.rs:34-36` — `SessionId` derives `Copy`.
- `crates/sessions/src/persist.rs:62-70` — `save(&mut self, session: SessionSummary)` takes ownership.
- `crates/sessions/src/persist.rs:73-75` — `load(&self, id: SessionId)` returns `Option<SessionSummary>` (clone internally).
- `crates/sessions/src/lib.rs:4-51` — NO `pub mod persist` declaration. Module is dead code; `cargo test -p opencode-rk-sessions --lib save_and_load` runs 0 tests when unwired.

## Authorized correction applied

**File:** `crates/sessions/src/persist.rs`, test section, `save_and_load` test, line 144.

Before (original bytes at 26d1bfb):
```rust
        store.save(session);
```

After (one-line correction):
```rust
        store.save(session.clone());
```

### Hash evidence

| Block | sha256 |
|---|---|
| `save_and_load` block (pre-correction, 26d1bfb) | `0d2bf6f3127a38c823173a3a062605a6e3a90baa2d43ed62e0cfb4de8435ff18` |
| `save_and_load` block (post-correction) | `1d0dff1dacac20b07c6a491ec99eb668c35adaff59ac11ada5586a92197051d9` |
| Full `persist.rs` (26d1bfb, pre-correction) | `634bc9286b772f9ab64e0307dee8b786a0b4007fced541b414cfd6070f8b899f` |
| Full `persist.rs` (current working tree) | `1399460be4a50d1d8d998e5e2c211d2afaf91725ee4de2fe1cbcd00f0e7e769f` |
| `compact_removes_orphans` block (26d1bfb) | `9539acf7a4777d0aed87cb3096dc0454e07ae6a7b221624c55574c8b0c64bb17` |
| `compact_removes_orphans` block (current working tree) | `9539acf7a4777d0aed87cb3096dc0454e07ae6a7b221624c55574c8b0c64bb17` |

**Prior compact correction unchanged by hash/block:** `compact_removes_orphans` block sha256 is identical between 26d1bfb and the current working tree (`9539acf7...`). The only diff from 26d1bfb is the single `save(session)` -> `save(session.clone())` change at line 144.

## Verification

Package does not declare `mod persist`; in-crate tests cannot execute this file directly. A disposable standalone Cargo harness was built:

- Harness path: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/sess008-save-correction/`
- `persist.rs` copied VERBATIM to `src/lib.rs` (crate root) so `//!` module doc comments are valid as crate-level inner doc comments. No source/test byte normalization.
- Copied file hash: `1399460be4a50d1d8d998e5e2c211d2afaf91725ee4de2fe1cbcd00f0e769f` (matches repository).
- `Cargo.toml` path-depends on actual `opencode-rk-contracts` at `/Users/mymac/Projects/opencode-rk-sess008/crates/contracts`.

Command:
```text
cd /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/sess008-save-correction && \
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --manifest-path /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/sess008-save-correction/Cargo.toml --lib -- --test-threads=1
```

Result: `5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. All 5 tests nonzero GREEN, jobs=1/threads=1.

```text
running 5 tests
test tests::compact_removes_orphans ... ok
test tests::flush_clears_dirty ... ok
test tests::load_missing ... ok
test tests::save_and_load ... ok
test tests::stats_track ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## TDD deviation

No RED was fabricated. This is an authorized frozen-contract compile correction: the `save_and_load` test could not compile once `mod persist` is wired because `save()` takes ownership of `SessionSummary` (not `Copy`). Using `.clone()` preserves the test's original assertion logic exactly — identical assertions, identical expected values. The implementation in persist.rs is unchanged (pre-complete at 26d1bfb, already SESS-008-compliant).

## Blockers

- `SESS-008` remains BLOCKED pending separate `crates/sessions/src/lib.rs` wiring (`pub mod persist`) and real caller proof. `SessionService` in lib.rs:332-869 does not reference `persist::PersistentSessionStore`; it uses `Arc<Storage>` for durability. No caller exists.
- Durable restart is SQLite-owned by `Storage`/`SessionService`; not implemented or claimed here.

# SESS-008 test replacement

## Claim and scope

- Task: SESS-008. Session: `ses_f32f85a86ffeg1aDu4Jj7PCqRO`.
- Claimed through `tools/completion_claims.py`; ledger row verified `in-progress`.
- Candidate base: `7cd184fe110d9432e4725802abf6c29f5515169a` on `lane/SESS-008-contract`.
- Owned paths: `crates/sessions/src/persist.rs` test section only, this scratchpad, own ledger row.
- No implementation, `lib.rs`, manifest, verifier, controller, or other test changes.

## Source evidence

- `tasks/SESS-008.md:22-36,38-47`: `flush` clears dirty state; `compact` removes still-dirty sessions and retains flushed sessions; T03 requires dirty-only compaction.
- `tasks/SESS-011.md:24-30`: contradictory archived-state compact wording; separate card, not authoritative for SESS-008's owned contract.
- `worklog/SESS-008-CONTRACT-REVIEW.md:53-78,80-104`: authority resolution; `persist.rs` is the in-memory dirty-tracking layer; durable restart belongs to SQLite `Storage`/`SessionService`; old inline test must be replaced.
- `crates/sessions/src/persist.rs:86-98`: implementation already retains IDs absent from `dirty`, removes IDs present in `dirty`, and keeps sets consistent.
- `crates/sessions/src/persist.rs:124-203`: inline test module; only `compact_removes_orphans` changed.

## Authorized contract correction

Explicit user authorization in the task prompt permits replacing the contradictory inline compact expectation. The old test saved Active and Archived sessions, flushed neither, then expected the Active session to survive solely because its state was not Archived. Both IDs were dirty, so that expectation contradicted SESS-008 dirty-only semantics.

Original `compact_removes_orphans` block at candidate base:

- SHA-256: `92187e3f86ad55d2f57971bf726b6ba9064b78c62975477a68c5488cbd895e4e`
- 523 bytes.
- Content:

```rust
    #[test]
    fn compact_removes_orphans() {
        let mut store = PersistentSessionStore::new();
        let active = make_session("active-session");
        let mut archived = make_session("archived-session");
        archived.state = SessionState::Archived;

        store.save(active.clone());
        store.save(archived.clone());
        assert_eq!(store.sessions.len(), 2);

        store.compact().unwrap();
        assert_eq!(store.sessions.len(), 1);
        assert!(store.sessions[0].id == active.id);
    }
```

Replacement SHA-256: `cf0eac1d2ce655d9d4a980a9004fb26cfefbaf1cc8068ef5974fbe96497b9a73` (691 bytes).

Replacement scenario: save retained session, flush all, save new dirty session, compact; assert retained clean session survives, dirty session disappears, dirty count is zero. Archived state remains incidental, not the removal criterion. All other tests remain byte-preserved.

## TDD deviation

No RED was fabricated. This is an authorized frozen-contract correction, baseline GREEN: the implementation at `persist.rs:91-97` already matches the replacement semantics. The original contradiction could not be treated as a product failure. Separate `lib.rs` wiring and caller proof remain outside this lane.

## Verification

The package does not declare `mod persist`; direct package tests cannot execute this file. A disposable standalone Cargo harness is used, with a nonzero test count, importing the actual `persist.rs` source and actual `opencode-rk-contracts` dependency. The unchanged pre-existing `save_and_load` test has a move-after-save compile defect (`persist.rs:145`), so the disposable copy applies only `session.clone()` at that call to permit execution; repository source remains unchanged outside the authorized compact test replacement. This harness-only normalization is not a source edit or test assertion change.

Executed command:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --manifest-path /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/sess008-test-harness/Cargo.toml --lib
```

Result: `5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
The final run used `-- --test-threads=1` as shown in the command receipt:

```text
cargo test --manifest-path /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/sess008-test-harness/Cargo.toml --lib -- --test-threads=1
```

The harness imported the actual repository source copy and ran all five inline
tests. Harness-only `session.clone()` normalization was required because the
unchanged existing `save_and_load` test otherwise fails to compile after moving
`SessionSummary`; repository source was not changed for that pre-existing issue.

## Blockers

- `SESS-008` remains blocked pending separate `crates/sessions/src/lib.rs` wiring and real caller proof.
- Durable restart is SQLite-owned by `Storage`/`SessionService`; not implemented or claimed here.

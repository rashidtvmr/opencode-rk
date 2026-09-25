# BRIDGE-PAR-119 scratchpad

Claim: BRIDGE-PAR-119, session ses_par119, ledger in-progress.

Source evidence:
- TS truth `packages/tui/src/context/sync.tsx:41-52` `search(items,target,key)` binary search, miss returns lower-bound `left`.
- Peer pattern `crates/opentui-bridge/src/sync_store.rs:1` `#![forbid(unsafe_code)]`, `MAX` cap const.

Observed: sorted Vec<String> index port; std `binary_search_by` gives same Ok/Err(index) semantics as TS found/index.

Target boundary: ONE new file `crates/opentui-bridge/src/sync_search.rs`. No lib.rs/Cargo.toml/sync_store.rs edits. No cargo, no commit.

Tests: in-file #[cfg(test)] 6 tests: empty insert 0, sorted order, find some, find none, dup stable, cap refuse.

Decisions: `sorted_insert` delegates to `upsert_sorted`; cap refuse returns `(len,false)` / `len` without growing; dup returns stable index, no insert.

Unknowns: none.

Verification: `rustfmt --check crates/opentui-bridge/src/sync_search.rs` PASS, 98 lines.

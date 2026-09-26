# BRIDGE-PAR-226 scratchpad

Claim: pinned scrollback core over ScrollbackSharedFull.
Evidence: crates/opentui-bridge/src/scrollback_shared_full.rs:13-48 (store lines/offset, push re-pins offset=0, visible(h) window).
Boundary: ScrollbackCore {store, pinned}; push/unpin/repin/view only. No scroll logic, no lib.rs edit.
Tests: 6 tests in-file (new/push/unpin/repin/view/repin-after-push). rustfmt --check PASS.
Decisions: Default pinned=true; repin zeroes store.offset; view clones visible(h).
Unknowns: none.

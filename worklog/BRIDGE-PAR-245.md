# BRIDGE-PAR-245 scratchpad

- claim: BRIDGE-PAR-245 via cc.claim, session ses_par245. ok.
- source evidence: crates/opentui-bridge/src/fork_route.rs:17-20 ForkRoute{source_id,target}; 25-43 plan/complete gate; MAX_ID 64.
- target boundary: ONE new file crates/opentui-bridge/src/fork_route_full.rs only. No lib.rs/Cargo.toml/fork_route.rs edits. No cargo/commit.
- tests: 4 unit tests in-file (add_ok, blank_rejected, cap+truncate, select_active).
- decisions: Default active=0; add rejects blank/full, truncates 128 chars; select bounds-checked; active_of via get(active).
- unknowns: none. rustfmt --check pending.

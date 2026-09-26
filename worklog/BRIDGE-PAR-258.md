# BRIDGE-PAR-258 scratchpad

claim: BRIDGE-PAR-258 in-progress ses ses_par258.
source evidence:
- crates/opentui-bridge/src/page_adapter.rs:10-25 Page enum + page_label
- crates/opentui-bridge/src/page_router.rs:9-11,19-43 PageRouter new/show/current/label
- style ref crates/opentui-bridge/src/debounce_full2.rs:1 forbid + ponytail comment
target boundary: ONE new file crates/opentui-bridge/src/page_adapter_full.rs only. No lib.rs/Cargo.toml/page_adapter/page_router edits. No cargo, no commit.
tests: 4 in-file (new_zero, show_bumps, accumulate, saturate_max).
decisions: PageFlow {router: PageRouter pub, visits: u64 pub} Copy per router semantics; show saturating_add; label delegates; visits getter; std-only forbid(unsafe_code).
remaining: integrator prewires `pub mod page_adapter_full;` (file standalone, uses crate:: paths).

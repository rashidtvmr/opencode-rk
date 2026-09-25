# BRIDGE-PAR-201: bounded dirty-flag render queue

claim: BRIDGE-PAR-201 / ses_par201 via completion_claims.claim OK.
source: new infra, no TS truth. std-only, forbid(unsafe_code).
target: crates/opentui-bridge/src/render_queue.rs (owned, single file).
tests: 5 unit tests (new_is_clean, mark_dirty_reset, cap8, truncate256, take_resets).
decisions: truncate by chars (not bytes) to cap 256; push_note false on full (drop newest); take resets both.
unknowns: none.

# BRIDGE-PAR-200 scratchpad

claim: BRIDGE-PAR-200 in-progress ses_par200.
source: crates/cli/src/tui_entry.rs:646-658 - `poll_tick: u32 = 0`, `wrapping_add(1)`, `if poll_tick % 20 == 0` re-poll snapshot.
target: ONE file crates/opentui-bridge/src/poll_ticker.rs, std-only, forbid(unsafe_code), <100 lines.
tests: >=4 in-file.
decisions: wrapping_add mirrors caller overflow behavior; bump true on tick % every == 0; set_every rejects zero (keeps old); default every=20 matches TS truth.

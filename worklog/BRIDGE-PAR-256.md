# BRIDGE-PAR-256

claim: ledger BRIDGE-PAR-256 ses_par256 scratchpad worklog/BRIDGE-PAR-256.md
source: crates/opentui-bridge/src/event_wire.rs:1-71 (pump/pump_into over EventBus, forbid unsafe, std-only VecDeque)
source: crates/opentui-bridge/src/event_bus_full.rs:12-56 (EventFanout {subs,log}, emit->usize + always logs, forbid unsafe)
pattern: crates/opentui-bridge/src/footer_menu_full2.rs:6-42 (pub inner + private counter + accessor)
target: ONE new file crates/opentui-bridge/src/event_wire_full.rs, no lib.rs/Cargo.toml/event_wire.rs/event_bus_full.rs edits
tests: 5 in-file (new_zero, hit, miss_still_counts, accumulates, log_passthrough)
decisions: pub fan + private sent with sent() accessor; send() always bumps sent even on 0-match (mirrors emit always-logs); saturating_add; Default derive
unknowns: none

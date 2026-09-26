# BRIDGE-PAR-253 scratchpad

claim: BRIDGE-PAR-253 ses_par253 owned file crates/opentui-bridge/src/input_full2.rs
source evidence:
- crates/opentui-bridge/src/input_adapter.rs:13 feed_bytes(buf,bytes)->usize caps at MAX_TEXT head bytes
- crates/opentui-bridge/src/input_adapter.rs:44 drain_step(buf)->Vec<String> decode+drain, invalid fail-closed
- crates/opentui-bridge/src/native_input.rs:21 MAX_TEXT=4096
- style ref crates/opentui-bridge/src/footer_menu_full2.rs:1 forbid(unsafe_code), wrapper+in-file tests
observed: no input_full2.rs yet; feed_bytes returns total kept len; drain_step drains consumed
target boundary: ONE new file input_full2.rs only. No lib.rs/Cargo.toml/input_adapter/native_input edits. No cargo, no commit.
tests: 4 in-file (submit roundtrip, 4KiB cap, empty drain, ctrl-c quit)
decisions: feed->bool = full acceptance (kept-before==len); pending=len; drain=drain_step; derive Default; ponytail: no partial-take count, add when caller needs it
unknowns: none
verification: rustfmt --check only per scope

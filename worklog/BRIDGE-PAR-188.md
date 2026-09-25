# BRIDGE-PAR-188 subagent_wire.rs
claim: ledger in-progress ses_par188
source: crates/opentui-bridge/src/subagent_data_full.rs:22 SubagentDataFull (id+task+progress, label); crates/opentui-bridge/src/subagent_footer.rs:46 SubagentFooter (push_line, summary); style ref turn_wire.rs (thin adapter, pub fields, STATUS cap)
target: crates/opentui-bridge/src/subagent_wire.rs, std-only, forbid(unsafe_code), <130 lines
tests: 6 in-file (new ids, push buffers+empty-false, progress bounds, status join, status cap 512, status reflects progress)
decisions: pub data/footer fields like TurnWire.rt; status = "label | summary" char-capped 512
verify: rustfmt --check FMT_OK, 101 lines; no cargo per scope

# BRIDGE-PAR-269 scratchpad

Claim: lane owned via cc.claim BRIDGE-PAR-269 ses_par269 worklog/BRIDGE-PAR-269.md. In-progress confirmed.

Source evidence:
- crates/opentui-bridge/src/subagent_footer.rs:46 SubagentFooter {agent_id, lines, status}; :89 summary() "agent {id} {status} {N} lines"; :67 push_line(&str)->bool false-on-empty, evicts oldest at 100
- crates/opentui-bridge/src/subagent_wire.rs:18 SubagentWire {data, footer}; :44 status() "label | summary" capped STATUS_CAP 512

Target boundary: ONE new file subagent_footer_full.rs. No lib.rs/Cargo.toml/subagent_footer.rs/subagent_wire.rs edits. No cargo/commit.

Tests: 4 in-file (push buffers+empty, status delegates, summary short passthrough, summary cap 256). Written with impl, unrun per scope.

Decisions: summary_capped caps footer.summary() at 256 chars via chars().take, matching ctx_bundle.rs SUMMARY_CAP=256 pattern; ponytail: skipped progress/finish passthrough, add when callers need.

Unknowns: none. Integrator must prewire `pub mod subagent_footer_full;` + `pub use`.

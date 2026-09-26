# BRIDGE-PAR-124 scratchpad
claim: ses_par124 owned file crates/opentui-bridge/src/subagent_footer.rs
source: packages/tui/src/routes/session/subagent-footer.tsx:1-132 (label+sibling+usage/cost+Parent/Prev/Next), dialog-subagent.tsx:1-26
target: SubagentFooter{agent_id cap64,lines cap100x512,status}+push_line+finish+summary std-only forbid-unsafe <160L 5 tests
tests: push_cap,line_trunc,finish_ok_failed,summary_format,trunc_id_empty

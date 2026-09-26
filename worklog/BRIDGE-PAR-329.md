# BRIDGE-PAR-329 dialog_console_org_full

Claim: attempted via completion_claims.claim (session ses_par329) -> ClaimError
"claims must be a bounded mapping": ledger has 501 rows > MAX_ROWS 500.
Pre-existing repo-wide blocker, not lane-caused. No ledger edit (out of scope).

Source: packages/tui/src/component/dialog-console-org.tsx:24 DialogConsoleOrg
org select (options memo + active default). Pattern ref:
crates/opentui-bridge/src/fork_dialog_full.rs:1-59 (cap/truncate/select idiom).

Target: crates/opentui-bridge/src/dialog_console_org_full.rs, std-only,
forbid(unsafe_code), under 100 lines.
ConsoleOrg { orgs: Vec<String> cap 16 each 128, cursor: usize }
+ push(&mut self,&str)->bool + move_cursor(&mut self,delta:isize)
+ selected(&self)->Option<&str>. 6 in-file tests.

Decisions: push trims-blank check + cap + char-truncate (matches fork_dialog_full
idiom); move_cursor clamps, empty resets to 0; selected via get(cursor).

Unknowns: ledger overflow prevents claim/update; orchestrator must reclaim/prune
then lane can flip status.

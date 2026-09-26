# BRIDGE-PAR-160
claim: ses_par160 owns directory_ctx.rs
TS: packages/tui/src/context/directory.ts:7-17 useDirectory memo (project dir + abbreviateHome + branch suffix).
impl: DirectoryCtx cwd cap1024 entries cap512 each256 set_cwd/add_entry/list/count std-only forbid_unsafe 125L.
tests: 6 in-file (empty false, add/list, cap, trunc cwd+entry).
verify: rustfmt --check PASS FMT_OK.

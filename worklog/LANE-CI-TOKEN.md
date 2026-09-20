# LANE-CI-TOKEN scratchpad

Claim: LANE-CI-TOKEN, session ses_f423cd0e3ffe1siD409EuLQ1Sr.
Source evidence:
- crates/cli/src/ci_run.rs:153-177 spawn_daemon(addr) sets no env, probes /health with None.
- crates/cli/src/ci_run.rs:179-190 ensure_daemon(bearer) drops bearer on spawn path.
- crates/cli/src/ci_run.rs:53-59 daemon_bearer returns "Bearer {hex}" from OPENCODE_RK_DAEMON_TOKEN.
- crates/cli/src/main.rs:673 serve() always DaemonAuth::mint (no env honor; other lane's scope).
- crates/server/src/daemon_auth.rs:150-176 /health public, /api/* gated.
Observed: child mints fresh token; parent bearer mismatches -> /api 401/403 -> UsageError 64.
Target: spawn_daemon(addr, bearer) sets OPENCODE_RK_DAEMON_TOKEN env (raw hex), probes with bearer.
Tests: frozen ci_run tests untouched; verify via cargo check bins.
Decisions: minimal diff in ci_run.rs only; serve-side env honor belongs to serve lane.
Unknowns: none for this lane.

Status: blocked.
- Diff in crates/cli/src/ci_run.rs only: spawn_daemon(addr,bearer) sets OPENCODE_RK_DAEMON_TOKEN=hex on child, probes with bearer. ensure_daemon passes bearer.
- Zero ci_run errors (grep bins short-output for ci_run = 0 lines on first pass).
- bins check blocked by pre-existing crates/tools/src/shell_tool.rs errors (PermissionBroker/Decision/ShellError::Denied) from another lane; workspace dirty with 11 files, commit unsafe.
- serve() still mints fresh token (main.rs:673); full e2e needs serve-side env honor lane.
- No commit, no push. No test edits.

Update (2nd pass): tree churned under lane (other lanes dirtied main.rs/daemon.rs/tui_entry.rs); ci_run.rs edit had been lost, re-applied identical diff. rustfmt --check on ci_run.rs: only pre-existing line-330 format nit, not from this diff. bins check: 0 ci_run error lines; blocked by other-lane shell_tool errors. No commit/push (dirty tree, not owned). Zero test edits.

Verify tail (bins check, JOBS=1 THREADS=1, timeout 100):
- 3 errors, all tui_entry.rs:534/548 (E0061 + stdin_tty E0425 x2), other lane's breakage.
- grep ci_run.rs error count = 0. Lane-owned file clean.
- No commit/push: tree dirty with other lanes; committing would sweep foreign files. No force-push.

# LANE-DESC-STALE scratchpad

Claim: LANE-DESC-STALE, session ses_f423cd0adffe1mKflQIPCPcvT5, owned file crates/server/src/daemon.rs.
Base rev: 6b19524.

Source evidence:
- crates/server/src/daemon.rs:137-165 `read_backend_descriptor`: single OR-gate mapped
  schema/pid/origin/empty-token all to Ok(None); non-empty malformed token fell through to Some.
- crates/server/src/daemon_auth.rs:54-63 `from_published` rejects empty/non-64-hex (TOKEN_HEX_LEN=64).
- Callers trusting Ok(Some): crates/cli/src/main.rs:617 web(), :645 serve-AlreadyRunning print
  origin with no token check; chat.rs:446 reuse_credential re-checks via is_wellformed_token.
- Frozen RC-02 unit tests (daemon.rs:436-538) use tokenless JSON + expect Ok(None) for spoofed
  origin (line 436) and foreign loopback IP (line 501): gate order must keep those Ok(None).

Observed scenario: live daemon + legacy empty-token descriptor -> Ok(None) ("absent") lets
callers spawn second daemon / report none while live one runs. Live + malformed token ->
Some -> attach, then 403 at /api/*.

Target boundary: split gate. Schema/pid/origin fail -> Ok(None) (unchanged). Live+valid
endpoint: empty token -> Err(Descriptor legacy); malformed (not 64-hex) -> Err(Descriptor
malformed); valid 64-hex -> Some. New helper is_wellformed_token mirrors from_published via
crate::daemon_auth::TOKEN_HEX_LEN. No lib.rs / frozen-test / other-lane touches.

Tests (authored in owned file, frozen suite untouched):
- desc_stale_empty_token_errors, desc_stale_malformed_token_errors, desc_stale_valid_token_attaches.

Decisions: token check placed AFTER Ok(None) gate so RC-02 spoof/occupied cases keep Ok(None).
Minimal diff: gate split + helper + 3 tests + field-doc fix.

Remaining unknowns: none in scope. Integration tests discovery_auth_red.rs (todo! RED) and
web_singleton_lock.rs (publishes empty token, expects Some) not run by lane verify filter;
flagged to orchestrator as follow-up owned by other lanes.

# NET-005-conn — outbound connector (remote_connector.rs)

Claim: `crates/server/src/remote_connector.rs` implements NET-005 outbound
connector types; `rustc --test` suite passes 7/7.

Source evidence (HEAD 5af7884, file untracked `??` at session start):
- `crates/server/src/remote_connector.rs:13` — `#![forbid(unsafe_code)]`, std only
  (`VecDeque`, `fmt`, `time::Duration`).
- `GatewayEndpoint::parse/verify_gateway` (~line 147/184): host lowercased +
  validated, pin must be `CERT_PIN_LEN` (32); verify = case-insensitive host
  match AND constant-time pin compare, fails closed → `ForgedGateway`.
- `DeviceCredentials` (~line 221): `Debug` redacts token; `Drop` zeroizes;
  `with_token` scoped access; `token_matches` constant-time.
- `ReconnectPolicy::delay_for` (~line 320): `min(base*2^n, cap)` with
  splitmix64 jitter into `[delay/2, delay]`; deterministic, no RNG/clock.
  Consts: base 1000ms, cap 30000ms, max attempts 10.
- `OutboundConnector::handle_request` (~line 493): gateway verify → shape/size
  → device binding → constant-time token; typed errors, no secret payloads.
- `push` (~line 528): `TooLarge` over 262144 B; `QueueFull` on 128 items or
  1048576 B total.
- `heartbeat_timed_out` (~line 568): caller-clock silence ≥ 45000ms.
- `disable(mut self)` (~line 586): zeroes queue, `joined:true`,
  `cancelled_tasks=listeners`, `listeners_remaining:0`; consumes self so no
  residual handle.

Observed scenario:
- First read showed 4 STUB-RED sites (`verify_gateway`, `delay_for`,
  `handle_request`, `push`, `heartbeat_timed_out`, `disable`). RED run:
  1 passed, 6 failed (missing-behavior failures, e.g. `attempt 0: 300s
  exceeds cap`, accept-everything `Ok(ControlResponse...)`).
- Before my patch applied, a concurrent worker completed the same file:
  my `edit` anchor missed and my idempotent python patch aborted on
  `assert count==1` (heartbeat stub already gone). I made zero edits.
- GREEN run after: 7 passed, 0 failed.

Target boundary: caller owns all I/O (sockets, TLS, timers, listeners).
Module owns identity, redacted creds, reconnect policy, caps, queue, clock.
No threads/sockets/wall-clock; time via `advance(delta_ms)`.

Tests (`rustc --edition 2021 --test ... -o /tmp/opencode/rc && /tmp/opencode/rc`):
authorized_control_request_accepted, forged_gateway_rejected,
mismatched_device_credentials_rejected,
reconnect_sleep_bounded_jittered_and_offline_truthful,
disable_cancels_joins_and_leaks_no_secret,
backpressure_heartbeat_and_size_caps_enforced,
oversized_control_payload_rejected — 7/7 pass.

Decisions:
- No edits from this lane; file already met spec. Left concurrent worker's
  implementation untouched rather than re-churning it.
- `verify_gateway` lacks explicit `valid_host(presented)` check; fails closed
  anyway (empty/evil host ≠ expected host → ForgedGateway). Noted, not changed
  (owned-file race + tests pass).

Remaining unknowns: none in-file. `lib.rs` wiring NOT checked (out of scope;
another lane owns it). Verifier decides integration/acceptance.

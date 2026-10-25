# Discovery proof security review

**Verdict: NEEDS_FIX**

Reviewed working-tree candidate atop `fa845813c55a579407d322431f2fb6a5034ad676`; no build/tests run.

## Finding: P1 - `web` reuse opens browser without ownership proof

`crates/cli/src/main.rs:719-724` reads a live-PID/loopback descriptor then calls `open_web_browser` with its bearer, without `verify_discovery_proof`. A stale/forged descriptor plus a foreign listener therefore receives the token in the browser URL fragment; foreign page script can read it. The default-entrypoint attach and `serve`-already-running browser path do verify proof, but explicit `web` does not. Verify before `open_web_browser` on this existing-descriptor path; keep `--no-open` token-free.

## Finding: P2 - malformed UTF-8 nonce can panic public route

`crates/server/src/lib.rs:644-645` checks byte length then slices `&str` at fixed byte offsets. A 64-byte query containing a multibyte UTF-8 character at an odd boundary panics on this unauthenticated route. Iterate raw bytes or reject non-ASCII hex before slicing; malformed public requests must return 400.

Proof review: server MAC binds nonce, configured listener `local_addr()` origin, schema, PID; client uses OS nonce, verifies echoed fields and MAC without bearer; proof response omits token. DaemonAuth Debug redacts token. Constant-time compare scans the full public input length; response cap bounds it. Existing tests cover nonce/origin/key rejection, not altered PID/schema or replay. No tests run, no PASS claimed.

## Reviewed file SHA-256

- `crates/cli/src/chat.rs`: `5a41453da1054300775cba71ee0bd9092c7ce37bcf031b15edc995ef32c8e56b`
- `crates/cli/src/main.rs`: `b6aaa7146585c1b9c4d37e9b8193b1b254e88b3c5cd340ea2d6c40b0709a8b4a`
- `crates/server/src/lib.rs`: `e1b6dd265d446b921ee1ac481a5ae423fd4fe0411251576426b01e22592beade`
- `crates/server/src/daemon_auth.rs`: `ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a`
- `crates/server/src/discovery_proof.rs`: `9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c`

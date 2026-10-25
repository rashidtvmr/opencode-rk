# Discovery-proof functional review

**Verdict: NEEDS_FIX**  
**Reviewed HEAD:** `fa845813c55a579407d322431f2fb6a5034ad676`

## Scope

Blind source review of the five requested paths. Exact SHA-256 artifacts:

```text
e8b6d546c4f89e2866e36844eeeeaf3e35ac7fd5bfca65085e060b5ed4a7ef7f  crates/cli/src/chat.rs
b6aaa7146585c1b9c4d37e9b8193b1b254e88b3c5cd340ea2d6c40b0709a8b4a  crates/cli/src/main.rs
e1b6dd265d446b921ee1ac481a5ae423fd4fe0411251576426b01e22592beade  crates/server/src/lib.rs
ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a  crates/server/src/daemon_auth.rs
9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c  crates/server/src/discovery_proof.rs
```

## Finding

`crates/server/src/lib.rs:644`, `hex_decode_nonce` slices `&str` by byte offsets after checking only `len() == 64`. A 64-byte UTF-8 nonce containing a 3-byte character (for example `€` plus ASCII padding) can make `s[i*2..i*2+2]` split a code point and panic instead of returning `400`. Decode `s.as_bytes()` in bounded two-byte chunks (or reject non-ASCII first) before indexing.

## Other checks

Genuine proof binds token, nonce, origin, schema, and PID; malformed lengths fail closed; ephemeral `:0` publication is reused; explicit targets are matched; stale/foreign descriptors do not receive a bearer. `/health` remains intentionally public. `AlreadyRunning` proof is required on browser-open, while `--no-open` only reports the validated descriptor and sends no credential. No other source-level blocker found.

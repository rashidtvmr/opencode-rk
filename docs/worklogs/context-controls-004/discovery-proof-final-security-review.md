# Discovery proof final security review

**Verdict: APPROVE**

Reviewed current source, regression tests, and dependency manifest at working-tree candidate atop `fa845813c55a579407d322431f2fb6a5034ad676`. Prior NEEDS_FIX findings are resolved:

- Explicit `web` reuse and `serve` AlreadyRunning verify fresh ownership proof before browser receives bearer. `--no-open` remains token-free.
- Nonce parser consumes byte pairs and rejects non-ASCII; public route avoids UTF-8 slicing panic.
- Proof uses fresh OS nonce, MAC binds configured bound-listener origin/schema/PID; client checks echoed identity and MAC before bearer use. Response cap is 1024 bytes. Proof endpoint does not derive origin from Host or echo client Origin.
- New HTTP tests cover malformed Unicode, differing Host versus configured origin, valid proof, and unconfigured origin. CLI tests cover foreign listener/no bearer/no browser and genuine reuse with fake browser opener.

No Cargo/build/test/installed journey run by reviewer. This is source review approval only, not installed acceptance.

Post-approval change reviewed: only `crates/cli/tests/discovery_web_identity.rs` changed. Marker checks now wait up to eight seconds for complete newline-terminated entries; removed unused health counter/Mutex. Foreign no-bearer/no-open/fail-closed assertions and genuine-origin/token fake-opener assertions remain intact. All other nine reviewed paths match prior approved hashes.

Final test correction reviewed: only `crates/server/tests/discovery_proof_http.rs` changed. The malformed nonce decodes to 64 bytes with ASCII followed by multibyte `é`, forcing the former byte-offset string slice at index 2 onto a non-boundary. Fixture `TempDir` stays owned through request completion. Valid configured-origin-versus-Host proof and unconfigured-origin rejection assertions remain intact. Other nine paths unchanged.

## Reviewed file SHA-256

- `crates/cli/src/chat.rs`: `5a41453da1054300775cba71ee0bd9092c7ce37bcf031b15edc995ef32c8e56b`
- `crates/cli/src/main.rs`: `3b79dcdb233f8f1f06418ecfd1347dde61a71a87b57ad4d57263bbad07a1b8de`
- `crates/server/src/lib.rs`: `3a932b9e11bd6341682a9b9789d17d5fc3096bc44a2584fe33c4127f052b02cf`
- `crates/server/src/daemon_auth.rs`: `ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a`
- `crates/server/src/discovery_proof.rs`: `9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c`
- `crates/server/tests/discovery_proof_http.rs`: `377ec2e344ecffd0013f150c4ddd5188f4879dc229579b0a9e5cef5b79ebc45c`
- `crates/cli/tests/discovery_identity_boundary.rs`: `f70095349c929e62093e51254c129c2e1657db0152c25f29d100addafbcee5f6`
- `crates/cli/tests/discovery_web_identity.rs`: `3fb5d6336197d79638d93314a2d37887e6f27bda6910bafe2d2bd0198d2190a3`
- `crates/cli/Cargo.toml`: `44082a8ff168c8fd7ddb12f6d98a1a9aef0717f7d93a13f6b8764f3ecc26a560`
- `Cargo.lock`: `0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f`

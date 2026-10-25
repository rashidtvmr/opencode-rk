# Discovery-proof final functional review

**Verdict: APPROVE (source review only; no Cargo/build/test execution).**

Reviewed the exact requested ten paths against base `fa845813`. The nonce fix now validates ASCII and decodes bytewise, preventing UTF-8 slicing panics and preserving malformed-input 400 behavior. Discovery MAC binds token, nonce, configured origin, schema, and PID; unconfigured identity fails closed. Native reuse still requires public `/health` plus proof, while web reuse/open paths now require proof before browser launch; `--no-open` does not bypass proof because it does not open. Ephemeral-port publication/reuse and explicit-target matching remain coherent. `/health` stays public. The HTTP test covers malformed Unicode, configured-origin binding, and unconfigured failure. The frozen native foreign-listener test remains unchanged and its expected health/bearer assertions are preserved. New web tests cover foreign denial/no open/no bearer and genuine same-origin token reuse. No source-level blocker found.

## Hash receipt

```json
{
  "reviewed_base": "fa845813c55a579407d322431f2fb6a5034ad676",
  "working_tree_head": "fa845813c55a579407d322431f2fb6a5034ad676",
  "file_count": 10,
  "unique_sha256_count": 10,
  "files": [
    {
      "path": "crates/cli/src/chat.rs",
      "sha256": "5a41453da1054300775cba71ee0bd9092c7ce37bcf031b15edc995ef32c8e56b"
    },
    {
      "path": "crates/cli/src/main.rs",
      "sha256": "3b79dcdb233f8f1f06418ecfd1347dde61a71a87b57ad4d57263bbad07a1b8de"
    },
    {
      "path": "crates/cli/Cargo.toml",
      "sha256": "44082a8ff168c8fd7ddb12f6d98a1a9aef0717f7d93a13f6b8764f3ecc26a560"
    },
    {
      "path": "Cargo.lock",
      "sha256": "0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f"
    },
    {
      "path": "crates/server/src/lib.rs",
      "sha256": "3a932b9e11bd6341682a9b9789d17d5fc3096bc44a2584fe33c4127f052b02cf"
    },
    {
      "path": "crates/server/src/daemon_auth.rs",
      "sha256": "ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a"
    },
    {
      "path": "crates/server/src/discovery_proof.rs",
      "sha256": "9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c"
    },
    {
      "path": "crates/server/tests/discovery_proof_http.rs",
      "sha256": "1b5517dd40e57aaceed14c329b243ee2e84564e81f5259ba6ebc5c9346c9c23c"
    },
    {
      "path": "crates/cli/tests/discovery_identity_boundary.rs",
      "sha256": "f70095349c929e62093e51254c129c2e1657db0152c25f29d100addafbcee5f6"
    },
    {
      "path": "crates/cli/tests/discovery_web_identity.rs",
      "sha256": "c49b6ef83bd249a09db4701b70a4005876194a3efe5002b0cf5bf8692c9fcfb2"
    }
  ]
}
```

## Final execution receipt

**Verdict: APPROVE — final candidate PASS.** The post-approval HTTP test correction
constructs the intentionally misaligned UTF-8 nonce (`aé` plus ASCII padding),
checks its 64-byte encoding, and retains the fixture `TempDir` instead of leaking
it. This directly covers the former string-slice panic. The final reported gates
passed: server 253 tests (including corrected HTTP 3, observer 3, and lib 247),
installed debug CLI 3 tests (frozen foreign 1 and web reuse 2), and final native
CLI build. No source-level blocker remains.

### Python-generated refreshed 10-file SHA-256 manifest

```json
{
  "reviewed_base": "fa845813c55a579407d322431f2fb6a5034ad676",
  "working_tree_head": "fa845813c55a579407d322431f2fb6a5034ad676",
  "file_count": 10,
  "unique_sha256_count": 10,
  "files": [
    {"path":"crates/cli/src/chat.rs","sha256":"5a41453da1054300775cba71ee0bd9092c7ce37bcf031b15edc995ef32c8e56b"},
    {"path":"crates/cli/src/main.rs","sha256":"3b79dcdb233f8f1f06418ecfd1347dde61a71a87b57ad4d57263bbad07a1b8de"},
    {"path":"crates/cli/Cargo.toml","sha256":"44082a8ff168c8fd7ddb12f6d98a1a9aef0717f7d93a13f6b8764f3ecc26a560"},
    {"path":"Cargo.lock","sha256":"0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f"},
    {"path":"crates/server/src/lib.rs","sha256":"3a932b9e11bd6341682a9b9789d17d5fc3096bc44a2584fe33c4127f052b02cf"},
    {"path":"crates/server/src/daemon_auth.rs","sha256":"ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a"},
    {"path":"crates/server/src/discovery_proof.rs","sha256":"9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c"},
    {"path":"crates/server/tests/discovery_proof_http.rs","sha256":"377ec2e344ecffd0013f150c4ddd5188f4879dc229579b0a9e5cef5b79ebc45c"},
    {"path":"crates/cli/tests/discovery_identity_boundary.rs","sha256":"f70095349c929e62093e51254c129c2e1657db0152c25f29d100addafbcee5f6"},
    {"path":"crates/cli/tests/discovery_web_identity.rs","sha256":"3fb5d6336197d79638d93314a2d37887e6f27bda6910bafe2d2bd0198d2190a3"}
  ]
}
```


## Refreshed final source-only review

**Verdict: APPROVE.** The only changed scoped file is the web identity test, whose marker polling now waits for complete newline-terminated lines before asserting the initial and reuse browser invocations. The unused synchronization/health-counter state is removed. Foreign denial still asserts proof attempted, no Authorization, no browser open, and non-success; genuine reuse still asserts exact same-origin/token URL. Production five paths and other tests are unchanged. No source-level blocker found. No Cargo/build/test execution performed.

### Python-generated 10-file SHA-256 manifest

```json
{
  "reviewed_base": "fa845813c55a579407d322431f2fb6a5034ad676",
  "working_tree_head": "fa845813c55a579407d322431f2fb6a5034ad676",
  "file_count": 10,
  "unique_sha256_count": 10,
  "files": [
    {
      "path": "crates/cli/src/chat.rs",
      "sha256": "5a41453da1054300775cba71ee0bd9092c7ce37bcf031b15edc995ef32c8e56b"
    },
    {
      "path": "crates/cli/src/main.rs",
      "sha256": "3b79dcdb233f8f1f06418ecfd1347dde61a71a87b57ad4d57263bbad07a1b8de"
    },
    {
      "path": "crates/cli/Cargo.toml",
      "sha256": "44082a8ff168c8fd7ddb12f6d98a1a9aef0717f7d93a13f6b8764f3ecc26a560"
    },
    {
      "path": "Cargo.lock",
      "sha256": "0dcd8464bf873ddb288dd7f30fbcb59785df7a8df18c8889cf19b1fc922f6d9f"
    },
    {
      "path": "crates/server/src/lib.rs",
      "sha256": "3a932b9e11bd6341682a9b9789d17d5fc3096bc44a2584fe33c4127f052b02cf"
    },
    {
      "path": "crates/server/src/daemon_auth.rs",
      "sha256": "ea94cc32c8ef89dfc32ae3ea4835383376fee09a2fd285b241067f495a6cfa4a"
    },
    {
      "path": "crates/server/src/discovery_proof.rs",
      "sha256": "9bdde71a9c756aee4795ed02e93828bca1669f7d994dcb0501c1ecc090512f6c"
    },
    {
      "path": "crates/server/tests/discovery_proof_http.rs",
      "sha256": "1b5517dd40e57aaceed14c329b243ee2e84564e81f5259ba6ebc5c9346c9c23c"
    },
    {
      "path": "crates/cli/tests/discovery_identity_boundary.rs",
      "sha256": "f70095349c929e62093e51254c129c2e1657db0152c25f29d100addafbcee5f6"
    },
    {
      "path": "crates/cli/tests/discovery_web_identity.rs",
      "sha256": "3fb5d6336197d79638d93314a2d37887e6f27bda6910bafe2d2bd0198d2190a3"
    }
  ]
}
```

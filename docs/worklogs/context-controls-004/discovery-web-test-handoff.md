# Discovery web identity regression handoff

Added `crates/cli/tests/discovery_web_identity.rs` (macOS-only installed tests; no production edits). The explicit `web` existing-descriptor path is required to contact fresh `/discovery-proof`; the first test runs the actual `oc2 web` binary against a live foreign TCP listener whose descriptor PID is valid, confirms proof contact, no Authorization header, process failure, and no private fake `open` invocation. The foreign server can answer plausible `/health`, but this test deliberately does not require health contact: the frozen default-entrypoint regression separately covers health plus proof. The second starts the actual CLI daemon on disposable `127.0.0.1:0` data, then re-runs `web` against the same fixture and verifies the fake `open` receives exactly the descriptor origin plus `#oc2-token=<token>`; all child environments are cleared and bounded.

Source SHA-256: `3fb5d6336197d79638d93314a2d37887e6f27bda6910bafe2d2bd0198d2190a3`

Verification intentionally not run per parent instruction; parent must build/run with `OC2_E2E_BIN` or Cargo’s `CARGO_BIN_EXE_oc2` fallback.

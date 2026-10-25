# Discovery identity repair handoff

## Package

Observed P1: public `/health` plus PID liveness could cause a foreign TCP
listener to receive the daemon bearer. Implemented nonce/MAC discovery proof.

## Changed paths

- `crates/server/src/discovery_proof.rs`
- `crates/server/src/daemon_auth.rs`
- `crates/server/src/lib.rs`
- `crates/cli/src/daemon_client.rs`
- `crates/cli/src/chat.rs`
- `crates/cli/src/main.rs`

The daemon records its actual bound `TcpListener::local_addr()` origin before
publishing the descriptor and MACs a fresh 32-byte nonce, canonical origin,
wire schema, and daemon PID with the bearer-derived keyed BLAKE3 key. The
client sends no bearer during proof, checks every echoed identity field and
constant-time compares the MAC. Missing server origin fails closed.

## Evidence and verification

Requirement evidence: current explicit repair request (fa845813); frozen RED
observation f70095349c929e62093e51254c129c2e1657db0152c25f29d100addafbcee5f6.
Pinned upstream authority remains `sources/upstream.lock.json` commit
`95daf90670b7c039c436c85537da5fbfe2205b41`; this is an RK security extension.

Follow-up compile repair: proof I/O and verification now live in `chat.rs`,
leaving `daemon_client.rs` standalone and dependency-free. Discovery responses
use a 1024-byte read cap. Existing-daemon browser opening also verifies proof.
Focused server positive/negative MAC tests are in `discovery_proof.rs`.
The HTTP regression suite is `crates/server/tests/discovery_proof_http.rs`;
the nonce decoder now parses ASCII bytes without UTF-8 slicing or panic.
The malformed-input regression uses one UTF-8 code point beginning at byte 1
of a 64-byte decoded nonce, which would split the old byte slice; test fixtures
retain and clean up their `TempDir` rather than leaking it.
All reused-descriptor `open_web_browser` paths now require the discovery proof;
the newly-started daemon path remains a local freshly-bound listener path.

Not run per worker instruction: Cargo, builds, tests, Git/ref operations,
installed journeys. Parent should compute final changed-path hashes and run
focused proof tests plus the frozen installed discovery journey on the exact
integrated SHA.

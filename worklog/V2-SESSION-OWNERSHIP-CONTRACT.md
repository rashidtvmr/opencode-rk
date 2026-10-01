# V2 session execution ownership contract

* Package: G4-SESSION-EXECUTION-SERIALIZATION
* Role: independent test owner; source-only candidate
* Base: `af3c8ca1989868110f2de4db0cc924fc712406ae`
* Allowed new paths: `crates/server/tests/session_execution_ownership.rs`, this worklog
* Product paths, manifests, lockfiles, existing tests and dependencies were not changed.

## Frozen invariant

For the live authenticated daemon, provider execution is mutually exclusive per
session ID. This contract intentionally does **not** decide busy-versus-queue
responses, endpoint-specific interrupt behavior, or full upstream wake/coalescing
parity. Different session IDs may execute concurrently up to the existing global
capacity of two.

The suite has five live-network cases: stream/stream, non-stream/stream,
stream/non-stream, two different sessions, and an unauthenticated second client.
The provider fixture is a bounded loopback HTTP listener that accepts concurrent
connections, validates the exact model and bearer, holds requests behind an
explicit release condition, records maximum active requests, and bounds wire
input at 128 KiB. Native clients use independent TCP connections and the actual
`router_with_auth`/`DaemonAuth` middleware.

## Source evidence

* Pinned upstream `95daf90670b7c039c436c85537da5fbfe2205b41`,
  `packages/core/src/session/run-coordinator.ts`: `Coordinator.run` joins an
  active key and the coordinator map is keyed by session ID; distinct keys can
  run concurrently. `wake` coalesces, and `interrupt` is owner-scoped.
* Pinned upstream `packages/core/src/session/execution/local.ts`: local execution
  constructs one coordinator and passes session IDs to `SessionRunner`.
* Current Rust `crates/server/src/lib.rs`: `router_with_auth` is the live auth
  route; `create_turn` and `create_turn_stream` both use process-wide
  `TURN_PERMITS` (capacity two), with user-message append occurring in each
  endpoint. The test freezes the missing per-session ownership boundary without
  inventing an HTTP interrupt or busy status.

## Bounds and verification manifest

The fixture bounds provider request bytes to 128 KiB, accepted provider requests
to two, active output to finite fixture responses, and all client/provider/server
waits with deadlines. It uses one Tokio multi-thread runtime per test, owned
`spawn_blocking` TCP clients, joined provider workers, and awaited aborted Axum
tasks on every normal assertion path. No build, runtime, network, or RED command
was run by this source-only owner.

Proposed frozen gate (parent/verifier runs it after reviewing this candidate):

```text
cargo test --offline --locked -p opencode-rk-server --test session_execution_ownership -- --test-threads=1 --nocapture
```

Status: CANDIDATE / pending parent compilation, actual RED run, hash freeze, and
independent verification. No GREEN or ACCEPTED claim is made.

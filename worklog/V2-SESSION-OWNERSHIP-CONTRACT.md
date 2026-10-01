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

## Controller review before any freeze

Source-only attempts `17f1627` and `593852d` remain preserved. Review found
fixture defects: qualified/unqualified model confusion, an ineffective inline
auth override name, last-header bearer parsing, a worker cap checked after
spawn, blocking synchronization inside async workers, incomplete-response
success, and insufficient negative-side-effect assertions. No compiling RED
or test freeze had occurred.

The controller repaired these fixture issues and retains the five-case scope.
Requests use qualified `openai/gpt-5.6` at the daemon and actual unqualified
`gpt-5.6` at the provider. Provider workers are capped at two before spawning;
HTTP lengths are checked before allocation, reads/writes have deadlines, and
responses must reach EOF and decode complete JSON/NDJSON. Stream client A must
observe an actual assistant delta before client B is launched. Shared durable
history is fetched over authenticated TCP and unauthorized or rejected requests
must not append a user message. Actual responses must complete successfully or
use the existing typed capacity rejection without side effects.

All client results are collected, the owned listener is aborted and awaited,
and provider workers joined before final behavior assertions. Environment
restoration occurs before releasing the async environment lock. Full prepared
test SHA-256: `9fdb5a6c01eb7d663421956fda1cc6c5bbf3ffcd43f7873f44bfb8e1f1cbb70c`.
This still requires independent compilation, actual RED execution and freeze.

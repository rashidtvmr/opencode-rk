# S1 provider sequential streams diagnostic

Package: S1-PROVIDER-SEQUENTIAL-STREAMS. This is a diagnostic candidate only;
it does not modify the provider implementation or server lane.

The test uses a bounded loopback TCP fixture, `Connection: close` on both SSE
responses, a reusable `OpenAiResponsesClient`, and disposable environment
configuration under a process-global environment lock. It records both accepted
requests and checks that round two contains the exact typed `function_call` /
`function_call_output` pair.

The parent session must record the RED/GREEN result, request acceptance timeline,
candidate SHA, and frozen file hash here after running the focused test. No
network, real credential, or user database is permitted.

## Candidate evidence

- Base ref: `f5f440d8f4ea7085409a06f10343494945fef6c`.
- `rustc 1.98.1`, `cargo 1.98.1`; no active cargo/rustc process was present.
- Focused command (twice, both GREEN):
  `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test responses_sequential_streams --offline --locked -- --nocapture`
- Both variants completed in about 0.01s. Reusable-client request acceptance was
  observed at fixture-relative instants `351524.690601041` and
  `351524.693714750` (about 3.1ms apart); the retained-completed-stream variant
  also passed. Round two asserted both typed items and their exact `call_s1` ID.
- This is provider-client exoneration for the reproduced `connection: close`
  sequence. S1 should remain server-side unless a separate protected-provider
  repair is demonstrated by a different fixture.

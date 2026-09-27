# Wave 1 - Reconcile scope, unblock builds and freeze contracts

Status: DRAFT. Entry: approved scope and preflight.

## Demonstration that closes this wave
Approved scope and source matrix; runnable platform/build/test harness; real OpenTUI render/load/restoration smoke and authenticated shared-daemon contract. Not yet a complete coding app.

## Work packages
- **P1-W1-01 - Reconcile all task records and freeze Phase 1 scope**. One approved, revisioned local-release scope, a disposition for every old ID, and no automatic inheritance of accepted/completed flags.
- **P1-W1-02 - Review and safely salvage the existing native TUI draft PR**. Existing partial work is reused only after review; newly added labels or call counters never constitute product completion.
- **P1-W1-03 - Build pinned OpenTUI libraries and resolve native loader paths**. An installed native executable resolves its real OpenTUI dependency on every declared platform, without Node/Bun/Zig/Cargo at runtime.
- **P1-W1-04 - Make the Rust/OpenTUI FFI boundary memory-safe and behaviorally correct**. Every safe bridge operation has a correct native contract and a real rendered-frame/lifecycle test; it cannot dereference a null required pointer or paint a non-presented buffer.
- **P1-W1-05 - Resolve broken frozen tests and nondeterministic fixtures through the test authority**. A compiling, deterministic behavioral test baseline that preserves test-authority rules and distinguishes test defects from product defects.
- **P1-W1-06 - Restore trustworthy repository and CI checks**. CI must execute the code and required journeys; a runner-allocation failure, static contract pass or green commit message is not test evidence.
- **P1-W1-07 - Unify authenticated daemon startup and client lifetime**. Fresh and concurrent client launches converge to one authenticated daemon per data directory; closing a UI detaches, explicit service stop owns termination.
- **P1-W1-08 - Freeze the shared engine, UI event and capability contracts**. All clients submit the same operation to the same daemon-owned runtime, and subscribe to identical normalized results.
- **P1-W1-09 - Freeze secure provider configuration and live-test prerequisites**. Use the supplied local 9router as a configured OpenAI-compatible service, not by pretending every model is an OpenAI Responses model.
- **P1-W1-10 - Create the real installed-app test harness and source-surface manifest**. The harness can prove a missing behavior RED and a working installed journey GREEN, with exact sources and nonzero test counts.

## Gate criteria
- Scope/target/naming exceptions are explicitly approved, not guessed.
- Invalid frozen-test contracts have an independent resolution; canonical plan checks and required builds actually run.
- Platform/native/CI/service blockers are resolved or explicitly change the declared scope before fan-out.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.

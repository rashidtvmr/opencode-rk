# Phase 1 acceptance contract

Phase 1 is the production-ready **local native macOS application spine**. It is intentionally smaller than the full G1-G8 release roadmap so Recovery Mode can converge instead of reopening every platform/client simultaneously.

## Required user journey

On one exact release-built `main-v2` SHA, using a fresh disposable HOME/workspace:

1. `oc2`/`opencode2` with no subcommand enters the real native interactive path.
2. The client discovers or starts exactly one authenticated daemon and attaches without manual `serve`.
3. If provider configuration is missing, the product can configure/persist it; persisted configuration is used by the actual outbound provider request.
4. The real native OpenTUI renderer starts, accepts raw input and renders streaming output.
5. A prompt travels through the daemon-owned session engine.
6. A provider tool call reaches the real permission broker, denial is side-effect free, approval executes the real tool, typed call/result metadata is persisted and the provider turn continues.
7. A second user turn succeeds after the tool continuation.
8. Exit and restart resume identical typed history without replaying ambiguous side effects.
9. A second local client attachment does not duplicate in-flight work.
10. Normal exit, denial, interruption, daemon restart and failure restore terminal state exactly.
11. Focused security smoke checks cover protected paths, symlink/hardlink escape attempts, stale/foreign daemon identity and secret-safe output.

## Acceptance rules

- Every requirement above must have executable evidence.
- Candidate/isolated GREEN is insufficient; the check must pass on the exact integrated SHA.
- Tests may use disposable fixtures and local provider stubs, but the actual CLI, daemon, renderer, permission broker, storage and tool wiring must run.
- No worker may weaken a semantic assertion to manufacture GREEN.
- `cargo check` passing is necessary but not acceptance.
- Structural/source-presence checks are necessary but not acceptance.
- Phase 1 is accepted only when all entries in `recovery/phase1-gates.json` are `ACCEPTED` and `recovery/failures.json` contains no unresolved Phase-1 item.

## Explicitly deferred after Phase 1

These remain required by the wider roadmap but do not block today's Phase-1 local-native release unless a regression in them breaks the local core:

- full real-browser G6 acceptance;
- Ubuntu archive/install/golden journey G7;
- cross-platform G8 release qualification;
- remote gateway/control;
- infinite delegation canvas;
- iOS/Android clients;
- notification product;
- broader client parity/extensions.

Deferred does not mean deleted. Existing branches/tests remain preserved as evidence for the next phase.

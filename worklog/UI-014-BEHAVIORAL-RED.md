# UI-014 behavioral RED no-seam blocker

## Claim and boundary

- Branch: `lane/UI-014-behavioral-red-20260923`; base `bb66a2a`.
- Independent research session: `ses_f317f4526ffeMRKFxC65ekhYAl`.
- Integrator receipt session: `ses_f3c4de578ffelQv59xDXmOs03B`.
- Scope: discovery receipt only. No product or test file was written or edited.

## Observable caller contract sought

A real native caller RED must prove that the first submission has a scoped owner and remains cancellable while input continues; a second submission queues while busy; completion starts the FIFO successor exactly once; interrupt cancels and joins the request while preserving draft/queue state; input and result channels reject overflow; and an ambiguous POST is never automatically replayed.

Pure `native_composer` state tests cannot prove this caller wiring.

## Source evidence and no-seam finding

- `crates/cli/src/tui_entry.rs:693-719` keeps `submit_native_text` behind `cfg(feature = "native")` and calls synchronous `execute_submit` before reporting `TurnFinished`.
- `crates/cli/src/tui_entry.rs:722-907` keeps `native_interactive_loop` private and native-gated. Its blocking one-byte `stdin.read` at line 784 and synchronous submit call at line 823 provide no owned request task, cancellation token, join handle, or bounded result channel.
- `crates/cli/src/native_composer.rs:438-616` has bounded draft/queue state, but no request owner and no transport cancellation. Testing it alone would falsely advertise application behavior.
- `crates/cli/src/terminal_host.rs:319-404` supplies a bounded terminal event channel but it is not wired to native turn execution.
- `crates/cli/Cargo.toml:9-18` exposes binaries only; there is no library target through which an integration test can call crate-private native functions.
- Existing binary tests exercise the non-native line loop, not `native_interactive_loop` or `submit_native_text`.
- `crates/opentui-bridge/build.rs:50-63` deliberately panics when the target artifact is absent. This arm64 host has no valid `native/lib/aarch64-apple-darwin/libopentui.a` or `.dylib`.
- The preserved external arm64 fixture is a broken symlink to the absent `opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib`; it was not copied, edited, or committed.

Therefore no public binary/PTY or callable non-native seam can establish a compiling behavioral RED for the actual native caller. Adding a test-only export, duplicating the caller in a test, source scanning, or treating pure composer tests as caller evidence would violate the frozen-test and no-stub contracts.

## Required next implementation boundary

Before an honest behavioral RED can be authored, integration authority must approve a production-owned, renderer-independent turn-worker seam with scoped cancellation/join and bounded input/result channels, wired into the real native loop. The native arm64 artifact must separately be supplied with provenance. The RED must then exercise that real seam or packaged binary, not a test-only clone.

UI-014 remains blocked. No acceptance or implementation completion is claimed.

# V2 native streaming/tool contract

## Scope

Source-only test-owner preparation from canonical base `8cb80815855702ccfc658f63504d6f90e8e6f4e5`.
No product files or frozen tests are modified. Product implementation remains locked until the exact integrated G2 revision is accepted.

The new fixture is `tests/e2e/native_stream_tool.py`. It is deliberately separate from the existing G2 provider fixture and imports only the reusable `TerminalScreen`, bounded PTY reader, artifact attestation, descriptor readiness, and owned-daemon cleanup helpers.

## Observable contract

1. An installed, SHA-attested native binary/library starts an authenticated owned daemon and reaches model readiness.
2. The first native prompt reaches the real `/api/sessions/{id}/turns/stream` path and the upstream loopback provider receives `stream: true`.
3. The provider emits `STREAM_PARTIAL_7d91`, then waits. The fixture only releases the final text/completion after the PTY screen exposes that exact partial marker. Seeing the final marker before release is a failure.
4. The first provider round requests the currently accepted native turn tool `write` (not an invented `write_file` identifier). Its exact current arguments are `{path,content,append}` as implemented by `crates/server/src/lib.rs:866-895`; `OPENCODE_RK_TURN_TOOLS=write` is the allowlist used by the current server.
5. The real broker writes exact marker bytes into the generated project. The continuation request must contain exactly one ordered typed `function_call` and `function_call_output` pair before the second provider response.
6. A second user turn is submitted without restarting. The daemon is restarted through the owned installed CLI, and a resumed third user turn must retain the same typed pair and prior assistant/user history.
7. Provider request count is exactly four: initial tool request, tool continuation, second user turn, resumed post-restart turn.

## Upstream/current evidence

- Pinned upstream `packages/protocol/src/groups/session.ts:307-342` defines finite durable history and live event subscription semantics; `:345-349` defines interrupt.
- Pinned upstream `packages/core/src/session/runner/publish-llm-event.ts:255-263` publishes text deltas before completion; `:313-374` publishes typed tool call and result lifecycle.
- Current server route is `crates/server/src/lib.rs:168-170`, `/api/sessions/{id}/turns/stream`.
- Current stream event names are `assistant_delta` (`:1066-1075`), `tool_call` (`:1086-1111`), `tool_output` (`:1318-1330`), and `assistant_message` (`:1139-1158`).
- Current persisted history API is `/api/sessions/{id}/messages` and `/api/sessions/{id}/history` (`crates/server/src/lib.rs:131-134`, `:425-463`); no current `/tool_history` route was found, so the fixture verifies typed history through the real subsequent provider inputs rather than inventing an endpoint.

## Bounds/security

Provider request 128 KiB, provider response 256 KiB, PTY capture 256 KiB via imported helper, descriptor validation 8 KiB intent, Unix socket path 100 bytes, provider/fixture sockets five seconds, fixture barriers ten seconds. Keys are generated only inside the disposable fixture, auth remains mode 0600 when created by the product, and evidence redacts the key. Cleanup signals only descriptor-validated owned process groups and joins the fixture thread with a five-second bound.

## Verification status

Not run by design: no Cargo, PTY, Docker, node, or live-product execution was performed in this source-only preparation. The actual command after G2 exact-SHA integration is:

```sh
TMPDIR=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp \
python3 tests/e2e/native_stream_tool.py \
  --binary /absolute/installed/oc2 \
  --native-library /absolute/installed/libopentui.dylib \
  --build-json /absolute/installed/build.json \
  --artifact-dir /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-stream-tool-evidence
```

This handoff is a source-contract candidate only: neither PREVERIFIED nor ACCEPTED.

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
- Current server next-round handling clears `assistant_text` after a tool round at `crates/server/src/lib.rs:1339-1350`/the corresponding continuation path. That means the present Rust stream implementation can lose the pre-tool `PARTIAL` from the durable assistant message even though the provider emitted it before the tool lifecycle. This contract intentionally requires the settled assistant history to preserve `PARTIAL + " " + FINAL`; it does not silently relax the assertion to accept the loss. The eventual coherent implementation grant must include the server stream state/persistence path as well as native rendering.

## Bounds/security

Provider request 128 KiB, provider response 256 KiB, PTY capture 256 KiB via imported helper, descriptor validation 8 KiB intent, Unix socket path 100 bytes, provider/fixture sockets five seconds, fixture barriers ten seconds. Keys are generated only inside the disposable fixture, auth remains mode 0600 when created by the product, and evidence redacts the key. Cleanup signals only descriptor-validated owned process groups and joins the fixture thread with a five-second bound.

## Verification status

Source-only helper validation was run; no Cargo, product PTY, Docker, browser, or Node execution was performed. The bounded self-check uses the actual stdlib HTTP handler and an actual `http.client` stream reader. It validates absent/false JSON responses, early partial SSE bytes before completion, barrier release, auth/model/schema rejection, exact four-request continuation/second/restart synthetic history, final-marker separation, output-before-call rejection, and wrong-history rejection:

```text
python3 tests/e2e/native_stream_tool.py --self-check
self-check: JSON defaults, actual early SSE frame, gated completion, auth, invalid stream, and history rejection passed

python3 AST parse: OK
git diff --check: OK
```

The actual installed-product command after G2 exact-SHA integration is:

```sh
TMPDIR=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp \
python3 tests/e2e/native_stream_tool.py \
  --binary /absolute/installed/oc2 \
  --native-library /absolute/installed/libopentui.dylib \
  --build-json /absolute/installed/build.json \
  --artifact-dir /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-stream-tool-evidence
```

This handoff is a source-contract candidate only: neither PREVERIFIED nor ACCEPTED. The installed-product gate is still expected to RED only when the native client fails to expose the gated partial delta before provider completion; setup, auth, model readiness, typed history, restart, and fixture protocol failures are independently diagnosed.

## Controller fixture maintenance and genuine baseline

The installed, accepted `ac635fac5298e9151c6575aa6a089f53c5aa4309` reached healthy
authenticated model readiness and one real provider request, omitted `stream`,
and displayed `assistant: NONSTREAM_COMPLETED`. It failed to display the required
early partial marker in 11.41s. This is product RED, not fixture startup failure:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-native-stream-red-ac635fa-v8ptt_7w`.
The source-only self-check proves actual HTTP framing/gated early bytes; the
remaining four rounds are state-level codec checks, not four live product turns.
The controller corrected a self-check barrier ordering delay, made typed-pair
negative cases reach their intended validation instead of the request-count cap,
and ensured actual assertion failures and whole-capture key echo are recorded.
No product file or existing frozen semantic test changed. The exact corrected
contract must be rerun and hashed before a product implementation lease starts.

## Frozen integrated contract and implementation unlock

The final test was integrated as `771607929cc6f2026384a820de68d7f6d8c606d6`.
Its frozen SHA-256 is
`f7d8fbe0cdc3f7be94e60a18bdfc2845f992038c6081bcba9c40ebbbe0c5f921`.
Earlier source proposals share the same base rather than a linear ancestry;
their immutable archive refs are `refs/archive/v2-native-stream-contract/<SHA>`
for `39190de6`, `7fb01a7e`, `29f598a8` and `bb8b0980`. None were discarded.

The controller reran the exact integrated fixture against the accepted installed
`ac635fa` release. Self-check passed in 0.71s; actual PTY journey failed in 11.37s
with healthy auth/catalog and one real **non-stream** provider request. The
current screen shows the completed-only response and no early partial. The
actual assertion error is captured, and owned CLI/daemon cleanup completed.
The frozen command manifest, test/source/log hashes and sanitized reproduction
are retained at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-stream-frozen-red-d2wrmua_`.

Dependency `G2-native-provider` is ACCEPTED on exact integrated `ac635fa`.
One coherent native stream/tool/resume implementation package is now unlocked;
its writer cannot modify this contract or existing semantic tests. The scoped
Rust final-message grouping requires retaining the pre-tool partial in the final
durable assistant text; complete upstream event replay remains a separate gate.

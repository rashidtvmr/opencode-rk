# Server observer contract corrections

Failure source: `compiled logserver-remaining-failures-baseline.log` reports obsolete `/api/capabilities` expectations and a stale-PID fixture rejected before its liveness assertion. Decision: update tests only; do not alter product metadata/dispatch.

## Authority and evidence

- Pinned upstream: `sources/upstream.lock.json` pins OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41`; the full local checkout is `/Users/mymac/Projects/tmp/opencode`, whose working HEAD differs. The pinned files were inspected with explicit `git show <pin>:<path>`.
- Upstream was independently checked with `git -C /Users/mymac/Projects/tmp/opencode show 95daf90670b7c039c436c85537da5fbfe2205b41:packages/opencode/src/tool/registry.ts` (not the current checkout HEAD, which is `70a...`). Its registry imports `WebFetchTool` and `WebSearchTool`, constructs both, and includes both in the builtin tool map. The corresponding upstream executable implementations are `packages/opencode/src/tool/webfetch.ts:24-47` (permission-gated fetch) and `packages/opencode/src/tool/websearch.ts:99-140` (permission-gated provider-backed search); neither is an unavailable/placeholder tool.
- Current source: `crates/tools/src/registry.rs:83-161` registers eight core tools plus real `websearch` and `webfetch`; `crates/tools/src/native_web.rs:12-41,97-129` validates and executes these adapters; `crates/tools/src/executor.rs:319,378-382` routes both through broker authorization and execution.
- Current API: `crates/server/src/lib.rs:655-688` emits actual `ToolRegistry::new()` contents sorted, setting `available_for_web_turn` from `turn_tool_config()`. `crates/server/src/lib.rs:2072-2089` defaults to bash, shell, read, write, edit, grep, glob, websearch, webfetch, task and todowrite, filtering only unsupported entries. `/api/capabilities` has ten registered entries: nine dispatched registry tools plus legacy `file`, which is excluded from the default allowlist. `crates/server/tests/agent_loop_turns.rs:536-551` independently pins real provider schemas containing both web tools.
- The web-specific search capability remains honestly unavailable: `crates/server/src/web_turn_adapter.rs:145-153,383-392` distinguishes web-search adapter from native websearch tool (which searches public web through Exa); it is not evidence that the native `websearch` tool is unavailable.
- Descriptor authority: `crates/server/src/daemon.rs:322-362` validates schema, liveness and loopback origin before validating bearer format. Dead PID is `Ok(None)`; only a live descriptor must supply a valid token. `crates/server/src/daemon.rs:783-819` separately preserves live empty/malformed-token error tests.

## Corrections

- `crates/server/tests/web_capabilities_api.rs`: replace stale eight-entry / seven-dispatchable contract with frozen explicit ten-entry sorted inventory and nine dispatchable IDs. Preserve default, empty allowlist, narrowed allowlist, unsupported `file` non-dispatch, unavailable adapter, reason and sorted-order assertions.
- `crates/server/tests/web_capabilities_full.rs`: update explicit expected inventory to ten and real dispatchable tool count to nine. Preserve exact sorted IDs, allowlist honesty, unavailable sections, nonempty reasons and repeat-response determinism.
- `crates/server/tests/web_capabilities_reason.rs`: update dispatchable IDs to nine, freeze all ten sorted registry IDs, preserve honest tool flags, nonempty turn-adapter reasons, nonempty unavailable reasons, and unavailable web capability sections.
- `crates/server/tests/web_singleton_lock.rs`: stale dead-PID descriptor is published through `publish_backend_descriptor_with_auth` with a valid 64-character hex dummy token. Existing assertion still requires stale descriptor omission (`None`); separate live empty/malformed-token tests remain intact.

## Candidate manifest (not accepted)

- Base HEAD at handoff: `d514e8bcc028fd2bbc1f115af9e5723d5206a70c`.
- Python SHA-256 evidence for the four owned test sources after the correction (computed before handoff):
  - `crates/server/tests/web_capabilities_api.rs` — `50fdb76c751472bec9e631a804058132aa11b5c8a3d2e9cbc190c61b7ce279e1`
  - `crates/server/tests/web_capabilities_full.rs` — `410fbcbf104a8303ed8874e6797146cfcad2cadbd73841a1ca2cbc9fd899d153`
  - `crates/server/tests/web_capabilities_reason.rs` — `f0921714ee512be82bb9a490330330e52b9848506f93460212171ca5d83402d4`
  - `crates/server/tests/web_singleton_lock.rs` — `ef2c6f5205d6bb20bb6e91f4be4194f3af3e659af9794d0d2fb61ded3a1dff55`
- This is a candidate diff only: no Cargo/build/test command was run, no commit was created, and the candidate is **NOT ACCEPTED**. Parent integration owner must independently review and run the four owned targets on the exact resulting SHA.

Superseded contract: old eight-tool/seven-dispatchable API inventory claimed `webfetch` and `websearch` were absent/unavailable. Current registry and executable adapter contradict that claim; changing tests to advertise those two honestly follows the actual product. This is not a change to Phase-1 goldens or production behavior.

Verification deferred to parent integration owner: no Cargo/build command run here per task boundary.

## Independent parent source review

APPROVE the four-file correction as a test-owner candidate. Parent read the
actual diff, the full registry at the exact upstream pin using `git show`, and
current `crates/tools/src/registry.rs`, `crates/tools/src/executor.rs`, and
`web_capabilities` in `crates/server/src/lib.rs`. Both web tools are registered
and routed through the real broker before execution. Explicit expected IDs
remain pinned constants; they are not computed from the response under test.
Legacy `file` remains unavailable for turns, and unavailable capability reasons,
allowlist denial, ordering, and deterministic response assertions remain.
The descriptor fixture now supplies valid auth metadata before changing only
the PID, preserving both the live `Some` and stale `None` assertions.
No production behavior or installed Phase-1 golden was changed. Execution on
the integrated SHA remains required and is not claimed by this source review.

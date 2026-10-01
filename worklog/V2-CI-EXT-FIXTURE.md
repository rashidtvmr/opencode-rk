# CI-EXT-AUTH-READINESS fixture maintenance

Package: `CI-EXT-AUTH-READINESS` (independent test/fixture owner)

Original source-only base SHA: `89aeaa81be72e4af95dc6fb39c28f54c07c92ae1`.
Prepared current canonical base: `bbfa1f20b4afc9ff93bc6afa992d9e9aa27fe91e`.

Correction pass is based on normal fixture commit `4bf7dea`; candidate commit is
reported with the final handoff.

## Observed RED

The frozen `crates/cli/tests/ci_ext.rs` test `ci_ext_t03_max_steps_one_completes_like_baseline`
failed with exit `64` before a turn on exact `a79ae96`; the other 11 target
tests passed. The old
`spawn_openai_fixture` reserved an address but never started either a provider or a
daemon and never supplied `OPENCODE_RK_DAEMON_TOKEN`.  The current `ci_run::run_ci`
fail-closed bearer gate therefore correctly returned UsageError before `max_steps`.

Independent evaluator classification: stale mechanical fixture/readiness problem;
the authenticated API contract is correct.  This repair does not alter `ci_run`,
authentication, product code, manifests, dependencies, or the six frozen test bodies.

## Repair and evidence

`crates/cli/tests/ci_ext.rs` helper region now owns a disposable real `serve` child
and a bounded loopback OpenAI-compatible `/v1/responses` + `/v1/models` fixture.
The helper waits for `runtime/backend.json`, bounds and validates its JSON, checks the
published PID, loopback origin, and 64-hex bearer, then passes the authenticated
origin/token and exact fixture model to the CI child.  Provider and daemon ownership
is held by `TestHome`; cleanup has bounded provider-thread shutdown and child reap.
All child environments are cleared and use disposable HOME/XDG paths.

Frozen test-body SHA-256 (from the first `#[test]` through EOF):

`8b106a38df1a6eb81726d418011e4ddf24eef0934073e1a991281251d17fb6e3`

The before and after hashes are identical.  No test/build/runtime gate was run by
this fixture owner, per package scope and parent heavy-slot policy.  This is a
source-only candidate, not a GREEN or ACCEPTED claim.

Correction details: the fixture uses qualified `openai/gpt-5.6` while sending
`gpt-5.6` upstream, supplies an absolute disposable `models.json` through
`serve --models-file`, validates exact provider routes/auth/model/prompt, and
owns resources immediately after each spawn. No runtime/build gate was run.

## Controller current-base preparation

Original worker commits `4bf7dea269a5fdad28ad9e36b2730c9ba2f392c8` and
`640d1110fe8cef19192a38d3275967babffa57ff` remain preserved. The controller
selectively prepared their helper-only changes against current canonical
`bbfa1f2`, then corrected inspected mechanical fixture defects:

- CI uses the non-streaming `/api/sessions/{id}/turns` path. Its actual
  `OpenAiResponsesClient::create_with_items` requests `stream: false` and reads
  JSON (`crates/providers/src/responses.rs:508–536`). The provider fixture now
  asserts that request flag and returns actual Responses JSON instead of SSE.
- CI model resolution preserves the full qualified `openai/gpt-5.6` hint;
  server `create_turn` splits provider/model before sending `gpt-5.6` upstream.
- Bounded HTTP request parsing, exact request-line handling, readiness reads,
  and capped output drains replace nominal timeouts on blocking line readers.
- Owned CI/daemon guards reap children on failure; provider shutdown joins its
  thread and turns protocol/request-count violations into fixture failures.
  Earlier source-only worklog claims of a failing `t04` are corrected above:
  the captured workspace failure is `t03`, not both cases.

All six frozen test function bodies and their assertions remain byte-identical.
This is mechanical fixture maintenance, not a product/authentication change.
Runtime preverification remains required before integration and acceptance.

Independent preverification of `c59fa7e` stopped at formatting: Cargo's workspace
edition orders the atomic import before `Arc`. The controller applied that exact
two-line mechanical correction. No compiler or runtime test had run. Receipt
`v2-ci-ext-preverify-c59fa7e-g4n41l15` remains preserved under the approved
artifact parent; the full unchanged gate is required again.

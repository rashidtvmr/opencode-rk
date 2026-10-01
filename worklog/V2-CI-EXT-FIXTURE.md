# CI-EXT-AUTH-READINESS fixture maintenance

Package: `CI-EXT-AUTH-READINESS` (independent test/fixture owner)

Base SHA: `89aeaa8` (actual resolve used by the evaluator: `a79`)

## Observed RED

The frozen `crates/cli/tests/ci_ext.rs` tests `ci_ext_t03_max_steps_one_completes_like_baseline`
and `ci_ext_t04_timeout_large_value_completes` exited `64` before a turn.  The old
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

# V2-CI-MODE-FIXTURE

Package: `CI-MODE-AUTH-READINESS` (independent test owner, source-only)

- Base SHA: `364c92dfa919c43b89331ffbaa5a759afa271fbe`
- Candidate SHA: to be recorded by the parent after the normal commit
- Allowed source path: `crates/cli/tests/ci_mode.rs`
- Worklog path: `worklog/V2-CI-MODE-FIXTURE.md`
- Original frozen test suffix (from first `#[test]` marker through EOF):
  - bytes: `5396`
  - SHA-256 before: `0cebc7013745fd4caefc011064ae3f2b81c864c47d84b543acaa27efc54337cc`
  - SHA-256 after rustfmt: `0cebc7013745fd4caefc011064ae3f2b81c864c47d84b543acaa27efc54337cc`

## Authority and source evidence

The fresh exact-364 workspace receipt supplied by the controller reports the
attested installed binary completing 808 tests, then timing out after 300
seconds at `ci_mode::ci_t01_jsonl_output_parseable_and_exits_zero`; this is a
fixture readiness/authentication timeout, not a semantic RED. `crates/cli/src/ci_run.rs`
requires a validated daemon bearer before CI work begins. The prior fixture gave
`ci_command` no bearer, while its unbounded provider accept left the original
test blocked in `provider_task.join()`.

`crates/cli/tests/ci_ext.rs` is the accepted source pattern for disposable
daemon/provider ownership: real `serve --listen 127.0.0.1:0`, models fixture,
descriptor PID/origin/token validation, authenticated `/api/models` readiness,
isolated HOME/XDG variables, and bounded provider HTTP I/O. This file reuses
those patterns independently and does not modify `ci_ext.rs`.

## Change summary

Only helpers before the first frozen test were replaced. The helper fixture now
starts a real disposable authenticated daemon, discovers its published dynamic
origin/token, probes authenticated readiness, and supplies the descriptor token
to each CI child. The provider fixture validates one bounded non-streaming
`gpt-5.6` Responses request and returns one finite JSON response. Child stdout,
provider join, process cleanup, and fixture deadlines are owned and bounded;
cleanup attempts to reap the CI child before joining the stdout reader.

No `ci_run` bypass, usage-code relaxation, join assertion weakening, test-body
change, Cargo/build/runtime/network command, or verification claim was made.

## Verification status

Only `rustfmt --edition 2021 crates/cli/tests/ci_mode.rs` was run, followed by
the source-only suffix hash check above. No Cargo/build/runtime/network command
was run as required. This is **CANDIDATE/source-prepared only**, not PREVERIFIED
or ACCEPTED. Parent must run the focused nine-test gate on the exact candidate,
then repeat it after integration.

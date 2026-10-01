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

## Controller source review

Worker preparation is preserved as normal commit
`6eb99f1c8036b851f86cc95283e161227f70b415`. Review found that its helper changed
the provider function signature while frozen callers still supply no home,
duplicated `run --ci`, checked the qualified model on the provider wire, omitted
descriptor PID equality, and could silently ignore a provider-thread panic.
No runtime or compiler verification was claimed for that source-only attempt.

The controller repaired only the helper region: the no-argument provider owns
its thread immediately; `ci_command` starts and owns the authenticated daemon
using the original caller's fake provider settings. The daemon descriptor is
bounded before allocation, checked against the spawned PID and live child, and
probed with its exact minted token. Child completion and complete stdout drain
are bounded; output overflow/read failure and provider panic cannot pass through
the original ignored join result. The provider checks actual unqualified
`gpt-5.6` and the final user message. Both subprocess environments include all
disposable HOME/XDG paths. The three frozen test bodies remain byte-identical.

Prepared current-base source is based on
`358690524cb82c5f9d49ce3043868195686d23b6`. Original worker and controller
commits remain preserved in `v2/ci-mode-fixture-current`; normal cherry-picks
`314d833` and `beeaf3a` carry only this helper and worklog to the current base.
Full prepared fixture SHA-256:
`12adcc77ad0f245c548e04e9917a8f370f32ecdc1d73737a5a121f0367abfa37`.
Independent runtime preverification is still pending.

## Exact integrated acceptance

Independent verifier `ses_f0a624a71ffeCRLPv0cbD4wDZ8` PREVERIFIED exact
`3abcc879dd68f5cf7df413d3118d03785f86fca2`. After fast-forward integration,
the controller repeated the same bounded isolated manifest on that exact SHA:

```text
cargo fmt --all -- --check
  exit 0
cargo test --offline --locked -p opencode-rk-cli --test ci_mode -- --test-threads=1 --nocapture
  9 passed; 0 failed; 0 ignored; exit 0
```

The actual successful turn and both deterministic runs reach the real
authenticated daemon and exactly one provider request each. Provider and output
guards report no protocol, overflow, read, timeout or cleanup failure. The three
original test function bodies remain frozen at the suffix hash above. Product
code and Cargo.lock are unchanged by this package.

Receipts are retained under the approved artifact parent at
`v2-ci-mode-preverify-3abcc87-504tj1l4` and
`v2-ci-mode-integrated-3abcc87-pg4t_2n_`. State: **ACCEPTED for mechanical
CI-MODE-AUTH-READINESS on exact integrated `3abcc87`**. The original workspace
timeout and source-only worker preparation remain preserved.

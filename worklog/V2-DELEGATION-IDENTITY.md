# V2 Delegation Identity — Source-Only Candidate

## Package and status

- Package: `CHILD-SPAWN-IDENTITY`
- Role: implementation, source-only
- Status: `CANDIDATE` (not PREVERIFIED or ACCEPTED)
- Base SHA: `f6e2d3b05c21f97c7224a333f177c60dfe210c0d`
- Branch: `v2/delegation-identity`
- Candidate SHA: `48702f3139458378bc9a9273ccc285a66c72e259`

## Failure and frozen contract

The canonical workspace runtime RED was `t09_distinct_session_ids`: two
back-to-back `build_spawn_plan` calls produced the same
`child-parent-distinct-1790811103783515000`. The frozen test source is
`crates/agents/tests/delegation_live.rs`, SHA256
`98fa9cca9e8a37d903c5c0ce88a74fbad5ddde31663a6a5515afd06ed433d8ba`.
The captured command and log are preserved at:

`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-workspace-gates-f6e2d3b-20jfnwb5/commands.json`

and `tests.log` in the same directory.

The defect was timestamp-only identity generation in
`crates/agents/src/delegation_live.rs` lines 300–307. Clock resolution does
not guarantee distinct values for adjacent calls.

## Change

`fresh_session_id` retains the existing externally observed
`child-{parent}-{suffix}` shape and replaces the timestamp suffix with
`uuid::Uuid::new_v4()`. `crates/agents/Cargo.toml` already declares
`uuid.workspace = true`; no manifest, lockfile, test, or registration change
was made. The UUID is generated locally, with no global counter, sleep,
retry-loop, or unbounded state.

## Upstream/current-requirement evidence

The pinned upstream checkout is
`/Users/mymac/Projects/opencode-upstream-reference` at
`95daf90670b7c039c436c85537da5fbfe2205b41`.

- `packages/core/src/session.ts:207–210`, `V2Session.create`, selects
  `input.id ?? SessionSchema.ID.create()` for a new session.
- `packages/schema/src/session-id.ts:5–12`, `SessionID.create`, is the
  upstream fresh-ID generator used by session creation.
- Local UUID-backed identity precedent is `crates/contracts/src/lib.rs:12`
  and `:43`, where `Uuid::new_v4()` constructs a fresh contract identity.

The repair is scoped to the child-spawn identity composition contract. This
historical self-contained module is imported directly by integration tests and
is not exported from `crates/agents/src/lib.rs`; this candidate therefore does
not claim production delegation wiring, child-history persistence, background
task-engine behavior, or full product completion.

## Allowed paths and verification

Changed paths are exactly:

- `crates/agents/src/delegation_live.rs`
- `worklog/V2-DELEGATION-IDENTITY.md`

Only source reads, Git diff/status checks, and rustfmt on the owned Rust source
are permitted in this lane. No heavy Cargo validation was run; the parent
verifier owns the focused test gate and regression follow-up:

```text
cargo test -p opencode-rk-agents --test delegation_live
```

The parent must independently verify the frozen test hash, run the focused
gate and agents regressions, integrate this candidate, and rerun the required
gate on the exact integrated SHA. No source-only pass or acceptance claim is
made here.

## Exact integrated contract acceptance

Final candidate `2f06dc6fb3f5162e6aa9cde6a6267a4f11ec00d2` was independently
PREVERIFIED by `ses_f0b4ae7dbffeEfFB4wLruGJkYg`. The unchanged frozen
`delegation_live` target passes all 11 tests, and all agents targets pass
**126 tests, 0 failed, 0 ignored**. Candidate receipts are retained at
`v2-verify-child-2f06dc6` under the approved artifact parent.

The controller integrated the net identity repair as `ce913a7`, then reran the
same focused and all-target commands on exact integrated
**`7ffdc6e13385027a1ecabff66d570ad88b963502`**:

```text
cargo test --offline --locked -p opencode-rk-agents --test delegation_live -- --test-threads=1
  11 passed, exit 0
cargo test --offline --locked -p opencode-rk-agents --all-targets -- --test-threads=1
  126 passed, 0 failed, 0 ignored, exit 0
```

Workspace formatting also passes. Actual command/environment/source/log hashes
and test counts are at `v2-identity-provider-integrated-7ffdc6e-alf5eet2` under
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`.
State: **ACCEPTED for the child-spawn identity composition contract on exact
`7ffdc6e`**. The earlier production-wiring limitation remains the scope boundary.

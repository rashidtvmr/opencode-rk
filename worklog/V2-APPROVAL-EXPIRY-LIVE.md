# V2 approval expiry live candidate

The controller moved this owned candidate handoff into its granted `worklog/`
path before integration. Product and frozen-test bytes remain identical to
independently verified candidate `c8864330eb018b0e8c8cf02e0ead94a1647414de`.

Package: approval-expiry-live
Lifecycle: CANDIDATE (source-only; not independently verified or accepted)

## Base and scope

- Base SHA: `c84b4f741b28b57308ef75a248fb292a4f8e3ffa`
- Branch: `v2/approval-expiry-live`
- Worktree: `/Users/mymac/Projects/opencode-rk-v2-approval-expiry-live`
- Historical evidence `dc3331ee257dbb21197f5310b8455d32a38b34ed` was selectively
  used as evidence only; no blind merge was performed.

## Observed RED

The frozen approval-lifetime contracts fail on the canonical source because an
approval at its expiry boundary is still accepted and expired approvals have no
resolution timestamp. The independent controller captured these runtime failures
in the retained fixture outputs; the frozen test sources and controller-owned
configuration were not modified by this candidate.

## Product changes

- `Grant::covers` now treats `now == expires_at` as expired, matching the
  inclusive expiry boundary used by `ApprovalsV2::expire_sweep`.
- `ApprovalsV2::expire_sweep` records `resolved_at_us = now_us` while moving
  bounded pending rows to terminal expired state 3. The pending guard, ordering,
  limit clamp, and existing expiry predicate remain unchanged.
- Approval retention backlog and deletion sweep include terminal expired state 3
  alongside states 1, 2, and 4. Existing non-null timestamp, cutoff, ordering,
  limit clamp, and cascade behavior remain unchanged.

No schema, migration, manifest, lockfile, test, verifier configuration, policy
default, sandbox claim, or authority document was changed.

## Verification status

No Cargo, test, build, PTY, Docker, or browser command was run, per the heavy
validation-slot restriction. This is a source-only candidate pending the parent
independent verifier. Candidate commit evidence and the exact changed-path list
are reported in the handoff; this file does not claim GREEN, PREVERIFIED, or
ACCEPTED.

## Exact integrated acceptance

Independent verifier `ses_f0c3bda7effeqXZjDfJqgOWKVF` PREVERIFIED product
candidate `c8864330eb018b0e8c8cf02e0ead94a1647414de`, base
`c84b4f741b28b57308ef75a248fb292a4f8e3ffa`. The controller moved the owned
handoff into its granted worklog path in preserved commit `6ab4dfb` and integrated
the package as **`dddb8d4ab1de0d4501353451651bec979058f84f`**.

Only three product paths change: `crates/security/src/app_policy.rs`,
`crates/storage/src/approvals_v2.rs`, `crates/storage/src/retention_v2.rs`.
The two frozen contract hashes match the compiling RED exactly. Existing
in-module test bodies, schema, Cargo lock and policy defaults are preserved.

The controller reran all five required commands, serially with two Cargo jobs
and one test thread, `--offline --locked`, on the exact integrated SHA:

```text
cargo test -p opencode-rk-security --lib --test app012_approval_expiry_red
  148 library + 1 contract passed
cargo test -p opencode-rk-storage --test app012_approval_retention_red
  3 passed
cargo test -p opencode-rk-storage --lib approvals_v2
  8 passed
cargo test -p opencode-rk-storage --lib retention_v2
  6 passed
cargo clippy -p opencode-rk-security --lib -- -D warnings
  exit 0
```

Candidate logs are retained at `v2-approval-expiry-independent-c886`, and exact
integrated commands, source/test/log hashes at
`v2-approval-lifetime-integrated-l1ymyfck`, both under
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`.
State: **ACCEPTED for approval expiry/terminal retention on integrated
`dddb8d4`**. This closes the selected historical `dc3331ee` product/test salvage
with an additional real expiry timestamp repair; historical claims and proposal
refs remain preserved evidence. Full interactive approval journeys remain open.

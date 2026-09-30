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

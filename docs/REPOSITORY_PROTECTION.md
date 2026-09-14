# Repository protection contract

This repository keeps its desired `main` protection state in source so accidental
workflow, validator, ownership, or discovery-authority drift is visible in normal
CI. Source-controlled policy is **not** proof that the hosting platform has enabled
the corresponding branch or ruleset settings.

## Source-controlled guarantees

`tools/validate_protection_policy.py` is part of the canonical repository gate. It
requires:

- `.github/CODEOWNERS` to retain a catch-all owner plus exact protected-path
  ownership records for `@rashidtvmr`, including ownership of `.github/` itself;
- `.github/protection-policy.json` to retain the exact repository, `main` target,
  `planning` required-check intent, review/bypass desired state, and protected
  path inventory;
- every protected path to exist;
- `.github/workflows/ci.yml` to retain the `planning` job and canonical repository
  validation/bootstrap commands through `tools/validate_repository.py`; the
  planning checkout is pinned to the signed `actions/checkout` v4.4.0 commit and
  does not persist checkout credentials;
- the canonical validator chain to retain backlog exhaustion, checked-in DISC-003
  reconciliation-manifest validation, and plan validation.

Bootstrap mutation tests exercise missing/renamed protected paths, CODEOWNERS owner
or pattern drift, required-status drift, weakened review/bypass settings, and
removal of protection validation from the canonical repository gate.

These checks make accidental weakening fail locally and in the existing
`planning` CI job. They cannot make a deleted or bypassed GitHub workflow execute
itself.

## Desired hosting-platform ruleset

`.github/protection-policy.json` records the desired external state for branch
`main`. Configure the hosting platform to require:

1. pull requests before merge;
2. the `planning` status check, with the branch required to be up to date;
3. one approving review and code-owner review;
4. dismissal of stale approvals and approval of the most recent reviewable push;
5. conversation resolution;
6. linear history;
7. no force pushes, branch deletion, or bypass actor.

The CODEOWNERS file names only the source-grounded repository owner currently
visible from the canonical Git remote: `@rashidtvmr`. If that owner authors a
protected-path pull request, a required independent approval/code-owner policy may
need another trusted write-capable owner before the external ruleset can be used
without deadlocking self-authored changes. Do not invent that reviewer in source;
add the real user/team and update the policy validator in the same reviewed change.

## What source cannot attest

The checked-in policy intentionally keeps `platformState.verified` false. Local
validation cannot prove that GitHub currently requires `planning`, requires code
owner review, blocks administrators/bypass actors, or blocks force pushes/deletion.
Those are repository-host settings controlled outside the Git tree. A platform
administrator must configure them and separately verify them in GitHub. Do not
change the checked-in field to `true` based only on local tests.

DISC-003 and backlog exhaustion semantics are unchanged by this policy. They
remain review/blocker evidence, not release evidence or controller acceptance.

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
- `.github/rulesets/main.disabled.json` to remain the exact GitHub-importable,
  **disabled** projection of that desired state; `tools/render_ruleset_import.py
  --check` fails if the recipe or its `planning` context drifts, while `--write`
  is the explicit regeneration path after an intentional policy edit;
- `tools/verify_ruleset_readback.py` plus its checked fixtures to remain a
  read-only local verifier for administrator-supplied GitHub ruleset JSON. The
  normal repository gate runs only the fixture self-test; CI never acquires
  credentials, calls GitHub, activates rules, or treats fixture success as
  platform evidence;
- every protected path to exist;
- `.github/workflows/ci.yml` to retain the `planning` job and canonical repository
  validation/bootstrap commands through `tools/validate_repository.py`; the
  planning checkout is pinned to the signed `actions/checkout` v4.4.0 commit and
  does not persist checkout credentials;
- the canonical validator chain to retain backlog exhaustion, checked-in DISC-003
  reconciliation-manifest validation, and plan validation.

Bootstrap mutation tests exercise missing/renamed protected paths, CODEOWNERS owner
or pattern drift, required-status drift, weakened review/bypass settings, and
removal of protection/import validation from the canonical repository gate. They
also reject an import artifact that changes `enforcement` to `active` in source.

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

The import handoff is `.github/rulesets/main.disabled.json`. Its JSON shape follows
GitHub's repository ruleset import/export format, but its checked-in
`"enforcement": "disabled"` is intentional. Source control can prepare an exact
recipe; it must not switch a hosting-platform control on by implication.

### Administrator import, activation, and verification

1. On the exact commit intended for import, run
   `/usr/bin/python3 tools/render_ruleset_import.py --check` and
   `/usr/bin/python3 tools/validate_repository.py`. Do not import a recipe that
   fails either command.
2. In GitHub, open `rashidtvmr/opencode-rk` -> **Settings** -> **Rules** ->
   **Rulesets** -> **New ruleset** -> **Import a ruleset**, and select
   `.github/rulesets/main.disabled.json`.
3. Before creating it, confirm the preview remains **Disabled**, targets only
   `refs/heads/main`, has no bypass actors, requires pull requests, one approval,
   CODEOWNERS review, stale-review dismissal, last-push approval and resolved
   conversations, requires strict `planning`, requires linear history, and blocks
   deletion/non-fast-forward updates. If GitHub changes or rejects the imported
   shape, stop and update the source recipe/validator first rather than hand-editing
   an untracked platform variant.
4. Create/save the imported ruleset while it is still **Disabled**. This records
   the candidate policy without enforcing it. Read the saved disabled ruleset back
   in the UI or with `GET /repos/rashidtvmr/opencode-rk/rulesets/RULESET_ID` and
   compare its target, bypass list, and rules to the checked-in recipe before
   changing enforcement. A disabled readback is a pre-activation check only; it is
   not evidence that the desired active platform state exists.
5. If this repository/plan exposes **Evaluate** mode, it may be used temporarily
   to inspect Rule Insights before activation. Evaluate availability is a GitHub
   platform capability, not something this repository can assert or require.
6. Resolve reviewer feasibility before activation. Because the only currently
   source-grounded CODEOWNER is `@rashidtvmr`, a self-authored protected-path PR
   may require a second real write-capable owner/reviewer. Add that real identity
   to source and validators first; never invent one to make activation possible.
7. Confirm the `planning` job has run successfully on a pull request. GitHub uses
   a workflow **job name** as the required status-check context, so the expected
   context is exactly `planning`.
8. Only an administrator should then edit the imported ruleset in GitHub and set
   enforcement to **Active**. This is the external activation step; no repository
   commit may flip `platformState.verified` or the import recipe to `active` as a
   substitute.
9. Verify the configured ruleset independently. In the GitHub UI, confirm the
   ruleset is Active and targets `main`. With an authenticated administrator or
   other caller whose ruleset read includes `bypass_actors`, save the **single
   configured ruleset** response to a local file, then run the offline verifier:

   ```sh
   gh api -H 'X-GitHub-Api-Version: 2026-03-10' \
     repos/rashidtvmr/opencode-rk/rulesets/RULESET_ID \
     > /tmp/opencode-rk-main-ruleset.json
   /usr/bin/python3 tools/verify_ruleset_readback.py \
     --input /tmp/opencode-rk-main-ruleset.json
   ```

   The verifier does not use credentials, perform network calls, write repository
   files, or mutate GitHub. Exit `0` with `status: verified-matching` means only
   that the supplied point-in-time ruleset object matches the checked-in desired
   **active** state: `Repository` source type, `refs/heads/main`, exact empty bypass
   list, pull-request/review requirements, strict `planning`, linear history,
   deletion protection and non-fast-forward protection. Exit `1` is an explicit
   mismatch. Exit `2` is **unverified/incomplete** evidence, including no input or
   a ruleset-history export that lacks `bypass_actors`.
10. GitHub ruleset-history JSON downloads are useful configuration snapshots but
    are insufficient for this no-bypass verification because GitHub omits the
    bypass list from those exports. Likewise, the REST endpoint can omit
    `bypass_actors` when the caller cannot read that protected field. Do not fill
    the missing list with `[]` by assumption; obtain an administrator-capable
    single-ruleset readback instead.
11. The verifier tolerates only the documented top-level server metadata `id`,
    `node_id`, `source`, `_links`, `created_at`, and `updated_at`; when `source` is
    present it must identify `rashidtvmr/opencode-rk`. Unknown rule fields remain
    semantic drift. A non-null status-check `integration_id` or an
    `allowed_merge_methods` field yields **unverified/policy-underspecified**, not a
    guessed match or guessed mismatch, because this source policy owns neither a
    GitHub App identity nor a merge-method policy. A serialized null
    `integration_id` is treated only as the absence of an app-specific constraint.
12. Separately inspect effective rules for `main`, for example with
    `GET /repos/rashidtvmr/opencode-rk/rules/branches/main`, because matching one
    configured repository ruleset does not prove the absence or interaction of
    organization/enterprise rules. Then use an ordinary non-destructive PR to
    confirm `planning` appears as a required check and protected-path review follows
    CODEOWNERS. Platform/API observations belong in an external administrator
    record; they are never written into Ralph/DISC acceptance or used to set the
    checked-in `platformState.verified` field.

The CODEOWNERS file names only the source-grounded repository owner currently
visible from the canonical Git remote: `@rashidtvmr`. If that owner authors a
protected-path pull request, a required independent approval/code-owner policy may
need another trusted write-capable owner before the external ruleset can be used
without deadlocking self-authored changes. Do not invent that reviewer in source;
add the real user/team and update the policy validator in the same reviewed change.

## What source cannot attest

The checked-in policy intentionally keeps `platformState.verified` false. Local
validation cannot prove that this repository/plan currently supports ruleset import
or Evaluate mode; that a ruleset was imported; that it is Active; what organization
or enterprise rules are layered onto `main`; which GitHub App/integration produced
the observed `planning` check; that `@rashidtvmr` is currently an eligible
write-capable reviewer; or that an administrator has not changed platform settings
after a prior verification. It likewise cannot prove that GitHub currently requires
`planning` or code-owner review, blocks bypass actors, or blocks force
pushes/deletion. Those are repository-host settings controlled outside the Git tree.
A platform administrator must configure and separately verify them in GitHub. Do
not invent an `integration_id`, and do not change the checked-in field to `true`
based only on local tests, checked fixtures, or a point-in-time platform
observation. Even `verified-matching` proves only that the administrator-supplied
JSON matched the desired configured repository ruleset at that moment; it does not
prove who supplied the file, that the platform has not changed since, effective
layered rules, actual PR behavior, or controller/verifier acceptance.

DISC-003 and backlog exhaustion semantics are unchanged by this policy. They
remain review/blocker evidence, not release evidence or controller acceptance.

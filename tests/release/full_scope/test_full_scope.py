#!/usr/bin/env python3
"""SHIP-004 RED: full legacy + source-parity release proof (test-author only).

Executable failing fixture for task SHIP-004 (tasks/completion/delivery.json:15).
Asserts the releasable end-state on the integrated tree:

  T01 every legacy ID + original requirement represented in evidence set.
  T02 all required source surfaces have test mappings + verified behavior.
  T03 accepted-but-unwired code, TBD descriptions, missing evidence block cert.
  T04 a release evidence commit may follow a tested candidate, but the tested
      candidate must be a valid ancestor of HEAD and no product/source change
      may land after it (only release evidence/test-attestation paths under
      tests/release/full_scope/ are exempt, so the attestation commit itself
      does not self-invalidate); stale product changes fail.
  T05 no optional-runtime flag removes a mandatory feature from accounting.

Stdlib unittest only. Read-only: no network, no wall-clock in verdict, no DB
mutation, no secret access. HEAD resolved via bounded `git rev-parse HEAD`;
ancestry/diff vetted via bounded read-only git plumbing
(`cat-file -e`, `merge-base --is-ancestor`, `diff --name-only`).

Controller pre-freeze rejection (prior owner stopped; orchestrator reclaimed
to not-started): old T04 required committed evidence files to contain the
exact HEAD hash of the commit containing themselves (pins == HEAD), which is
unsatisfiable by construction (a blob cannot contain its own commit hash).
This revision re-authors T04 to the satisfiable ancestor + no-post-candidate-
product-change contract above.

RED expectation: all 5 FAIL now for real missing release evidence:
no release ledger, certified=False, 32/32 surfaces without executable test
or entrypoint trace, 82 TBD rows, 0/258 legacy IDs in any release evidence
set, no tested candidate.
"""
from __future__ import annotations

import json
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
LEGACY_LEDGER = ROOT / "sources" / "completion" / "legacy-evidence.json"
SURFACE_LEDGER = ROOT / "sources" / "completion" / "surface-evidence.json"
REQUIREMENTS = ROOT / "requirements" / "user-requirements.json"
RELEASE_LEDGER = ROOT / "tests" / "release" / "full_scope" / "release-ledger.json"
# Paths exempt from T04 post-candidate product-change detection: writing the
# release evidence / test attestation itself must not self-invalidate proof.
# Everything else (product code, source evidence ledgers, requirements,
# task ledgers, worklogs, tooling) invalidates stale proof.
EVIDENCE_EXEMPT_PREFIXES = ("tests/release/full_scope/",)


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def git_run(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git"] + args,
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        timeout=15,
    )


def head_rev() -> str:
    proc = git_run(["rev-parse", "HEAD"])
    if proc.returncode != 0:
        raise AssertionError("cannot resolve HEAD revision")
    return proc.stdout.strip().split()[0]


def commit_exists(rev: str) -> bool:
    if not rev or len(rev) < 7:
        return False
    return git_run(["cat-file", "-e", f"{rev}^{{commit}}"]).returncode == 0


def is_ancestor(candidate: str, tip: str) -> bool:
    return git_run(["merge-base", "--is-ancestor", candidate, tip]).returncode == 0


def post_candidate_product_changes(candidate: str, tip: str) -> list[str]:
    """Non-exempt paths changed in (candidate, tip].

    Exempts only the release evidence / test-attestation paths needed to
    avoid self-reference. Any other change (product code, source evidence
    ledgers, requirements, tooling) after the tested candidate fails T04.
    """
    proc = git_run(["diff", "--name-only", f"{candidate}..{tip}"])
    if proc.returncode != 0:
        raise AssertionError("cannot diff tested candidate against HEAD")
    changed = [ln.strip() for ln in proc.stdout.splitlines() if ln.strip()]

    def exempt(path: str) -> bool:
        return any(path.startswith(p) for p in EVIDENCE_EXEMPT_PREFIXES)

    return sorted(p for p in changed if not exempt(p))


class FullScopeReleaseProofTests(unittest.TestCase):
    def test_t01_every_legacy_id_and_requirement_represented(self):
        """T01: every legacy ID + every original requirement in evidence set."""
        legacy = load_json(LEGACY_LEDGER)
        tasks = legacy.get("tasks", [])
        legacy_ids = [t.get("id") for t in tasks if t.get("id")]
        self.assertGreater(len(legacy_ids), 0, "legacy ledger has no task IDs")
        if RELEASE_LEDGER.is_file():
            evidenced = set(load_json(RELEASE_LEDGER).get("legacyIds", []))
        else:
            evidenced = set()
        missing = sorted(set(legacy_ids) - evidenced)
        reqs = load_json(REQUIREMENTS).get("requirements", [])
        req_ids = [r.get("id") for r in reqs if r.get("id")]
        if RELEASE_LEDGER.is_file():
            evidenced_reqs = set(load_json(RELEASE_LEDGER).get("requirementIds", []))
        else:
            evidenced_reqs = set()
        missing_reqs = sorted(set(req_ids) - evidenced_reqs)
        self.assertEqual(
            (len(evidenced), len(missing), len(missing_reqs)),
            (len(legacy_ids), 0, 0),
            f"T01 legacy IDs represented: {len(evidenced)}/{len(legacy_ids)} "
            f"in evidence set; missing e.g. {missing[:5]}; "
            f"requirements missing e.g. {missing_reqs[:5]}",
        )

    def test_t02_all_surfaces_have_test_mappings_and_verified_behavior(self):
        """T02: every required surface maps to a test + verified behavior."""
        surface = load_json(SURFACE_LEDGER)
        surfaces = surface.get("surfaces", [])
        self.assertGreater(len(surfaces), 0, "surface ledger has no surfaces")
        mapped = [
            s
            for s in surfaces
            if s.get("executableTest") not in (None, "none", "")
            and s.get("entrypointTrace") not in (None, "none", "")
            and s.get("disposition") == "implemented"
        ]
        self.assertEqual(
            (len(mapped), bool(surface.get("certified"))),
            (len(surfaces), True),
            f"T02 surfaces with executable test + entrypoint trace: "
            f"{len(mapped)}/{len(surfaces)}; certified={surface.get('certified')}",
        )

    def test_t03_unwired_tbd_missing_evidence_prevents_certification(self):
        """T03: accepted-but-unwired, TBD, missing evidence block cert."""
        legacy = load_json(LEGACY_LEDGER)
        surface = load_json(SURFACE_LEDGER)
        tbd_rows = [t for t in legacy.get("tasks", []) if t.get("tbd")]
        blockers = surface.get("certificationBlockers", [])
        bad_dispositions = [
            s.get("id")
            for s in surface.get("surfaces", [])
            if s.get("disposition") != "implemented"
        ]
        record_only = [
            t.get("id")
            for t in legacy.get("tasks", [])
            if t.get("status") == "accepted"
            and t.get("statusIsReleaseEvidence") is False
        ]
        self.assertEqual(
            (len(tbd_rows), len(bad_dispositions), len(blockers), len(record_only)),
            (0, 0, 0, 0),
            f"T03 accepted-but-unwired/TBD/missing evidence blocks certification: "
            f"{len(record_only)} accepted-as-record-only, {len(tbd_rows)} TBD rows, "
            f"{len(bad_dispositions)} non-implemented surfaces, "
            f"{len(blockers)} certification blockers",
        )

    def test_t04_post_candidate_changes_invalidate_stale_proof(self):
        """T04: tested candidate is a valid ancestor; no product change after it.

        Satisfiable revision-specific proof contract (re-authored after
        controller pre-freeze rejection of the pins==HEAD self-reference):
        the release evidence commit may follow the tested candidate, but the
        tested candidate must exist, must be an ancestor of HEAD, and the
        diff (candidate..HEAD] must touch no non-exempt path. Stale product
        changes fail; evidence-only follow-ups pass the freshness clause
        (T01-T03 still gate certification independently).
        """
        head = head_rev()
        if not RELEASE_LEDGER.is_file():
            self.fail(
                f"T04 no release ledger; no tested candidate binds HEAD {head[:7]}"
            )
            return
        release = load_json(RELEASE_LEDGER)
        tested = release.get("testedCommit", "") or ""
        self.assertTrue(
            commit_exists(tested),
            f"T04 tested candidate {tested[:7] if tested else 'absent'} "
            f"is not a resolvable commit (HEAD {head[:7]})",
        )
        self.assertTrue(
            is_ancestor(tested, head),
            f"T04 tested candidate {tested[:7]} is not an ancestor "
            f"of HEAD {head[:7]} (rebased/orphaned proof)",
        )
        stale = post_candidate_product_changes(tested, head)
        self.assertEqual(
            stale,
            [],
            f"T04 stale product/source change after tested candidate "
            f"{tested[:7]} (HEAD {head[:7]}): e.g. {stale[:5]}",
        )

    def test_t05_no_optional_flag_removes_mandatory_feature(self):
        """T05: mandatory features stay accounted regardless of runtime flags."""
        reqs = load_json(REQUIREMENTS).get("requirements", [])
        mandatory = [r.get("id") for r in reqs if r.get("mandatory") is True]
        self.assertGreater(len(mandatory), 0, "no mandatory requirements found")
        if RELEASE_LEDGER.is_file():
            release = load_json(RELEASE_LEDGER)
            covered = set(release.get("requirementIds", []))
            excluded = set(release.get("optionalExcluded", []))
        else:
            covered, excluded = set(), set()
        dropped = sorted(set(mandatory) - covered)
        leaked = sorted(set(mandatory) & excluded)
        self.assertEqual(
            (len(dropped), len(leaked)),
            (0, 0),
            f"T05 no release ledger covers {len(mandatory)} mandatory "
            f"requirements; dropped e.g. {dropped[:5]}; "
            f"optional-excluded mandatory e.g. {leaked[:5]}",
        )


if __name__ == "__main__":
    unittest.main()

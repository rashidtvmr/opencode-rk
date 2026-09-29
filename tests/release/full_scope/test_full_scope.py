#!/usr/bin/env python3
"""SHIP-004 RED: full legacy + source-parity release proof (test-author only).

Executable failing fixture for task SHIP-004 (tasks/completion/delivery.json:15).
Asserts the releasable end-state on the integrated tree:

  T01 every legacy ID + original requirement represented in evidence set.
  T02 all required source surfaces have test mappings + verified behavior.
  T03 accepted-but-unwired code, TBD descriptions, missing evidence block cert.
  T04 post-candidate changes invalidate stale revision-specific proof.
  T05 no optional-runtime flag removes a mandatory feature from accounting.

Stdlib unittest only. Read-only: no network, no wall-clock in verdict, no DB
mutation, no secret access. HEAD resolved via bounded `git rev-parse HEAD`.

RED expectation: all 5 FAIL now for real missing release evidence:
stale pins (legacy 5af7884 / surface 1f7640a vs HEAD), certified=False,
32/32 surfaces without executable test or entrypoint trace, 82 TBD rows,
0/258 legacy IDs in any release evidence set, no release ledger.
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


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def head_rev() -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        timeout=15,
    )
    if proc.returncode != 0:
        raise AssertionError("cannot resolve HEAD revision")
    return proc.stdout.strip().split()[0]


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
        """T04: stale revision-specific proof invalid after tree changes."""
        head = head_rev()
        legacy = load_json(LEGACY_LEDGER)
        surface = load_json(SURFACE_LEDGER)
        pins = {
            "legacy": legacy.get("inspectedCommit", ""),
            "surface": surface.get("inspectedCommit", ""),
        }
        if RELEASE_LEDGER.is_file():
            release = load_json(RELEASE_LEDGER)
            tested = release.get("testedCommit", "")
        else:
            tested = ""
        self.assertEqual(
            (pins["legacy"], pins["surface"], tested),
            (head, head, head),
            f"T04 stale pins (legacy {pins['legacy'][:7] if pins['legacy'] else '?'}"
            f", surface {pins['surface'][:7] if pins['surface'] else '?'}"
            f", release {tested[:7] if tested else 'absent'}) vs HEAD {head[:7]}",
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

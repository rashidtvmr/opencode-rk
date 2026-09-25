"""SHIP-004 full-scope release proof.

These are fail-closed release assertions over checked-in evidence. They do not
turn task status, source presence, or audit prose into product acceptance.
"""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
LEGACY_EVIDENCE = ROOT / "sources/completion/legacy-evidence.json"
SURFACE_EVIDENCE = ROOT / "sources/completion/surface-evidence.json"


def _json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _git_head() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=2,
    )
    return result.stdout.strip()


def _canonical_story_ids() -> set[str]:
    ids = {
        story["id"]
        for story in _json(ROOT / "ralph.json")["userStories"]
        if isinstance(story, dict) and isinstance(story.get("id"), str)
    }
    for path in sorted((ROOT / "tasks/completion").glob("*.json")):
        document = _json(path)
        ids.update(
            story["id"]
            for story in document.get("stories", [])
            if isinstance(story, dict) and isinstance(story.get("id"), str)
        )
    return ids


class FullScopeReleaseProofTests(unittest.TestCase):
    def test_ship004_t01_legacy_ids_and_original_requirements_are_mapped(self):
        canonical = _json(ROOT / "ralph.json")["userStories"]
        canonical_by_id = {story["id"]: story for story in canonical}
        evidence = _json(LEGACY_EVIDENCE)["tasks"]
        evidence_by_id = {row["id"]: row for row in evidence}

        self.assertEqual(
            set(canonical_by_id),
            set(evidence_by_id),
            "legacy ID set differs between canonical plan and evidence",
        )

        missing_requirement_mapping = []
        missing_test_mapping = []
        for task_id, story in canonical_by_id.items():
            row = evidence_by_id[task_id]
            required = set(story.get("requirementIds", []))
            mapped = set(row.get("requirementIds", []))
            if not required.issubset(mapped):
                missing_requirement_mapping.append(task_id)
            obligations = story.get("testObligations", [])
            mapped_obligations = set(row.get("testObligations", []))
            if not set(obligations).issubset(mapped_obligations):
                missing_test_mapping.append(task_id)

        mandatory_tasks = {
            task_id
            for requirement in _json(ROOT / "requirements/user-requirements.json")[
                "requirements"
            ]
            if requirement.get("mandatory") is True
            for task_id in requirement.get("tasks", [])
        }
        absent_mandatory = sorted(mandatory_tasks - set(evidence_by_id))
        self.assertEqual(
            missing_requirement_mapping,
            [],
            "legacy evidence lacks original requirement mappings",
        )
        self.assertEqual(
            missing_test_mapping,
            [],
            "legacy evidence lacks original test-obligation mappings",
        )
        self.assertEqual(absent_mandatory, [], "mandatory original tasks absent from evidence")

    def test_ship004_t02_every_surface_has_test_trace_and_verified_behavior(self):
        document = _json(SURFACE_EVIDENCE)
        surfaces = document["surfaces"]
        invalid = []
        for surface in surfaces:
            if (
                surface.get("disposition") != "implemented"
                or not surface.get("entrypointTrace")
                or surface.get("entrypointTrace") == "none"
                or not surface.get("executableTest")
                or surface.get("executableTest") == "none"
            ):
                invalid.append(surface.get("id", "<missing-id>"))
        self.assertEqual(invalid, [], "source surfaces lack executable verified traces")
        self.assertTrue(surfaces, "source surface evidence is empty")
        self.assertTrue(document.get("certified"), "surface evidence is not certified")

    def test_ship004_t03_gaps_and_tbd_evidence_block_certification(self):
        legacy = _json(LEGACY_EVIDENCE)
        surfaces = _json(SURFACE_EVIDENCE)
        audits = sorted((ROOT / "sources/completion/audits").glob("AUD-*.json"))

        gaps: dict[str, object] = {}
        non_evidence_acceptance = [
            row["id"]
            for row in legacy["tasks"]
            if row.get("status") == "accepted" and row.get("statusIsReleaseEvidence") is not True
        ]
        if non_evidence_acceptance:
            gaps["accepted_without_release_evidence"] = non_evidence_acceptance

        tbd = [row["id"] for row in legacy["tasks"] if row.get("tbd") is True]
        if tbd:
            gaps["tbd"] = tbd

        unresolved_surfaces = [
            surface["id"]
            for surface in surfaces["surfaces"]
            if surface.get("disposition") != "implemented"
            or surface.get("entrypointTrace") == "none"
            or surface.get("executableTest") == "none"
        ]
        if unresolved_surfaces:
            gaps["unresolved_surfaces"] = unresolved_surfaces

        if surfaces.get("certificationBlockers"):
            gaps["certification_blockers"] = surfaces["certificationBlockers"]

        audit_gaps = [
            path.name
            for path in audits
            if _json(path).get("verdict") != "accepted"
            or _json(path).get("repairChildren")
        ]
        if audit_gaps:
            gaps["audit_gaps"] = audit_gaps

        self.assertEqual(gaps, {}, "mandatory release gaps still block certification")
        self.assertTrue(surfaces.get("certified"), "certification flag is false")

    def test_ship004_t04_revision_bound_evidence_matches_current_head(self):
        head = _git_head()
        evidence_paths = [LEGACY_EVIDENCE, SURFACE_EVIDENCE]
        evidence_paths.extend(sorted((ROOT / "sources/completion/audits").glob("AUD-*.json")))
        stale = []
        for path in evidence_paths:
            document = _json(path)
            inspected = document.get("inspectedCommit")
            if inspected != head:
                stale.append(f"{path.relative_to(ROOT)}:{inspected!r}")
        self.assertEqual(
            stale,
            [],
            f"revision-bound release evidence is stale for candidate {head}",
        )

    def test_ship004_t05_runtime_optional_does_not_remove_mandatory_tasks(self):
        mandatory_cards: dict[str, bool] = {}
        for path in sorted((ROOT / "tasks").glob("*.md")):
            text = path.read_text(encoding="utf-8")
            identifier = re.search(r"^#\s+(\S+)", text, re.MULTILINE)
            mandatory = re.search(
                r"^Mandatory for full declared release:\s*(yes|no)\.?$",
                text,
                re.MULTILINE | re.IGNORECASE,
            )
            optional = re.search(
                r"^Runtime optional:\s*(True|False)\.?$",
                text,
                re.MULTILINE | re.IGNORECASE,
            )
            if identifier and mandatory and mandatory.group(1).lower() == "yes":
                mandatory_cards[identifier.group(1)] = bool(
                    optional and optional.group(1).lower() == "true"
                )

        plan_ids = _canonical_story_ids()
        missing = sorted(set(mandatory_cards) - plan_ids)
        optional_missing = sorted(
            task_id for task_id, is_optional in mandatory_cards.items() if is_optional and task_id not in plan_ids
        )
        self.assertEqual(missing, [], "mandatory task cards are absent from canonical accounting")
        self.assertEqual(
            optional_missing,
            [],
            "runtime-optional flags removed mandatory features from accounting",
        )


if __name__ == "__main__":
    unittest.main()

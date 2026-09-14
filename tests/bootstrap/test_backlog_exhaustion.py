import copy
import json
import pathlib
import unittest
from types import SimpleNamespace
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]

from tools.validate_backlog_exhaustion import (  # noqa: E402
    CATEGORY_IDS,
    build_expected_ledger,
    disc_status_errors,
    enterprise_remote_gap_errors,
    extensibility_remaining_gap_errors,
    integrations_ownership_gap_errors,
    operations_ownership_gap_errors,
    req017_extensibility_gap_errors,
    release_assurance_gap_errors,
    residual_evidence_kind_errors,
    routing_ownership_gap_errors,
    sharing_ownership_gap_errors,
    surface_evidence_gaps,
    unresolved_gap_coverage_errors,
    validate_ledger,
)


def load_ledger():
    return json.loads((ROOT / "sources/backlog-exhaustion.json").read_text(encoding="utf-8"))


class BacklogExhaustionTests(unittest.TestCase):
    def test_current_ledger_is_valid_and_exact(self):
        document = load_ledger()
        self.assertEqual(validate_ledger(document, ROOT), [])
        self.assertEqual(document["summary"]["storyCount"], 219)
        self.assertEqual(document["summary"]["controllerAccepted"], 133)
        self.assertEqual(document["summary"]["nonAccepted"], 86)
        self.assertEqual(
            document["summary"]["classificationCounts"],
            {
                "dependency-constrained": 22,
                "explicit-blocker": 9,
                "local-implemented-stale": 21,
                "unresolved-decomposition": 34,
            },
        )

    def test_missing_story_is_rejected(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"].pop()
        self.assertTrue(any("missing non-accepted stories" in error for error in validate_ledger(bad, ROOT)))

    def test_duplicate_story_is_rejected(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"].append(copy.deepcopy(bad["stories"][0]))
        self.assertTrue(any("duplicate classifications" in error for error in validate_ledger(bad, ROOT)))

    def test_summary_count_drift_is_rejected(self):
        bad = copy.deepcopy(load_ledger())
        bad["summary"]["controllerAccepted"] -= 1
        bad["summary"]["classificationCounts"]["unresolved-decomposition"] += 1
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("summary drifted" in error for error in errors))

    def test_accepted_story_cannot_enter_residual_ledger(self):
        bad = copy.deepcopy(load_ledger())
        bad["stories"][0]["id"] = "AGENT-001"
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("accepted/unknown stories" in error for error in errors))

    def test_explicit_blocker_category_is_locked(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "OPS-009")
        row["category"] = "unresolved-decomposition"
        self.assertTrue(any("category drift for explicit-blocker" in error for error in validate_ledger(bad, ROOT)))
        self.assertEqual(len(CATEGORY_IDS["explicit-blocker"]), 9)

    def test_dependency_constrained_category_is_locked(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "UI-014")
        row["category"] = "unresolved-decomposition"
        self.assertTrue(any("category drift for dependency-constrained" in error for error in validate_ledger(bad, ROOT)))
        self.assertEqual(len(CATEGORY_IDS["dependency-constrained"]), 22)

    def test_surface_and_evidence_projection_cannot_silently_drift(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "SHARE-003")
        row["surfaceIds"] = []
        row["evidenceIds"] = []
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("surfaceIds drifted" in error for error in errors))
        self.assertTrue(any("evidenceIds drifted" in error for error in errors))

    def test_surface_backed_rows_retain_source_caller_test_and_spec_evidence(self):
        ledger = load_ledger()
        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        self.assertEqual(residual_evidence_kind_errors(ledger["stories"], reconciliation), [])

        bad = copy.deepcopy(reconciliation)
        target = next(row for row in bad["reviewedSurfaces"] if row["id"] == "opencode.extensibility")
        target["evidence"]["caller"] = []
        story = next(row for row in ledger["stories"] if row["id"] == "EXT-001")
        errors = residual_evidence_kind_errors([story], bad)
        self.assertTrue(any("lost classes: caller" in error for error in errors))

    def test_per_surface_evidence_gaps_are_explicit_and_cannot_be_masked(self):
        ledger = load_ledger()
        for story_id in ("INT-010", "SHARE-003", "WEB-004"):
            row = next(item for item in ledger["stories"] if item["id"] == story_id)
            self.assertEqual(
                row["surfaceEvidenceGaps"],
                [{"surfaceId": "opencode.enterprise-remote", "missingKinds": ["spec"]}],
            )

        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        reviewed = {row["id"]: row for row in reconciliation["reviewedSurfaces"]}
        weakened = copy.deepcopy(reviewed)
        weakened["opencode.enterprise-remote"]["evidence"]["caller"] = []
        self.assertEqual(
            surface_evidence_gaps(["opencode.enterprise-remote"], weakened),
            [{"surfaceId": "opencode.enterprise-remote", "missingKinds": ["caller", "spec"]}],
        )

    def test_enterprise_remote_negative_spec_result_is_exact_and_fail_closed(self):
        ledger = load_ledger()
        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        self.assertEqual(enterprise_remote_gap_errors(ledger["stories"], reconciliation, ROOT), [])

        with self.subTest("unrelated spec cannot silently fill the gap"):
            changed = copy.deepcopy(reconciliation)
            surface = next(row for row in changed["reviewedSurfaces"] if row["id"] == "opencode.enterprise-remote")
            surface["evidence"]["spec"] = [{"evidenceId": "OC-CLIENT-SPEC"}]
            errors = enterprise_remote_gap_errors(ledger["stories"], changed, ROOT)
            self.assertTrue(any("spec evidence must remain empty" in error for error in errors))

        with self.subTest("residual gap set cannot silently shrink"):
            changed_rows = copy.deepcopy(ledger["stories"])
            row = next(item for item in changed_rows if item["id"] == "WEB-004")
            row["surfaceEvidenceGaps"] = []
            errors = enterprise_remote_gap_errors(changed_rows, reconciliation, ROOT)
            self.assertTrue(any("must remain exactly" in error for error in errors))

    def test_routing_and_adjacent_singleton_ownership_gap_is_exact_and_fail_closed(self):
        ledger = load_ledger()
        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        self.assertEqual(routing_ownership_gap_errors(ledger["stories"], reconciliation, ROOT), [])

        gap_path = ROOT / "sources/routing-ownership-gap.json"
        original_gap = json.loads(gap_path.read_text(encoding="utf-8"))

        def errors_for(changed_gap):
            from tools import validate_backlog_exhaustion as module

            original_load = module._load

            def fake_load(path):
                if pathlib.Path(path) == gap_path:
                    return changed_gap
                return original_load(path)

            with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                return routing_ownership_gap_errors(ledger["stories"], reconciliation, ROOT)

        with self.subTest("routing singleton cannot become ownership proof"):
            changed = copy.deepcopy(original_gap)
            changed["ownershipDecision"]["ROUTE-010"] = "9router.routing"
            self.assertTrue(any("ownership must remain unresolved" in error for error in errors_for(changed)))

        with self.subTest("identical ROUTE-008/009 surface signatures cannot silently diverge"):
            changed = copy.deepcopy(original_gap)
            changed["storySurfaceSignatures"]["ROUTE-009"] = ["9router.account-storage"]
            self.assertTrue(any("ROUTE-009: routing ownership-gap surface signature drifted" in error for error in errors_for(changed)))

        with self.subTest("safe-looking pure fragment remains unowned without task evidence"):
            changed = copy.deepcopy(original_gap)
            changed["candidateFragments"][0]["ownershipEstablished"] = True
            self.assertTrue(any("cannot become owned from singleton arithmetic" in error for error in errors_for(changed)))

        with self.subTest("EXT-005 adjacent singleton cannot be promoted by subtraction"):
            changed = copy.deepcopy(original_gap)
            changed["adjacentSingletonChecks"][0]["ownershipEstablished"] = True
            self.assertTrue(any("EXT-005: adjacent singleton ownership must remain unresolved" in error for error in errors_for(changed)))

    def test_operations_and_release_ownership_gaps_are_exact_and_fail_closed(self):
        ledger = load_ledger()
        self.assertEqual(operations_ownership_gap_errors(ledger["stories"], ROOT), [])
        self.assertEqual(release_assurance_gap_errors(ledger["stories"], ROOT), [])

        operations_path = ROOT / "sources/operations-ownership-gap.json"
        release_path = ROOT / "sources/release-assurance-gap.json"
        original_operations = json.loads(operations_path.read_text(encoding="utf-8"))
        original_release = json.loads(release_path.read_text(encoding="utf-8"))

        def operation_errors_for(changed_gap):
            from tools import validate_backlog_exhaustion as module

            original_load = module._load

            def fake_load(path):
                if pathlib.Path(path) == operations_path:
                    return changed_gap
                return original_load(path)

            with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                return operations_ownership_gap_errors(ledger["stories"], ROOT)

        def release_errors_for(changed_gap):
            from tools import validate_backlog_exhaustion as module

            original_load = module._load

            def fake_load(path):
                if pathlib.Path(path) == release_path:
                    return changed_gap
                return original_load(path)

            with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                return release_assurance_gap_errors(ledger["stories"], ROOT)

        with self.subTest("operations fragment cannot be assigned by residual arithmetic"):
            changed = copy.deepcopy(original_operations)
            changed["candidateFragments"][0]["ownershipEstablished"] = True
            self.assertTrue(any("cannot become owned from residual arithmetic" in error for error in operation_errors_for(changed)))

        with self.subTest("surface-equivalent operations stories cannot silently diverge"):
            changed = copy.deepcopy(original_operations)
            changed["storySurfaceSignatures"]["OPS-005"] = []
            self.assertTrue(any("OPS-005: operations ownership-gap surface signature drifted" in error for error in operation_errors_for(changed)))

        with self.subTest("release assurance cannot become a native product owner"):
            changed = copy.deepcopy(original_release)
            changed["nativeProductOwner"] = True
            self.assertTrue(any("must remain declarative with no native product owner" in error for error in release_errors_for(changed)))

        with self.subTest("release validator cannot be assigned by requirement arithmetic"):
            changed = copy.deepcopy(original_release)
            changed["candidateValidatorContracts"][0]["ownershipEstablished"] = True
            self.assertTrue(any("cannot become owned from requirement arithmetic" in error for error in release_errors_for(changed)))

    def test_req017_extensibility_gap_rejects_skill_command_task_inference(self):
        ledger = load_ledger()
        self.assertEqual(req017_extensibility_gap_errors(ledger["stories"], ROOT), [])

        gap_path = ROOT / "sources/req017-extensibility-ownership-gap.json"
        original_gap = json.loads(gap_path.read_text(encoding="utf-8"))

        def errors_for(changed_gap):
            from tools import validate_backlog_exhaustion as module

            original_load = module._load

            def fake_load(path):
                if pathlib.Path(path) == gap_path:
                    return changed_gap
                return original_load(path)

            with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                return req017_extensibility_gap_errors(ledger["stories"], ROOT)

        with self.subTest("task order cannot assign SkillV2 to EXT-001"):
            changed = copy.deepcopy(original_gap)
            changed["reviewedPartitions"][0]["candidateTaskOwner"] = "EXT-001"
            self.assertTrue(any("cannot gain task owner by task order/arithmetic" in error for error in errors_for(changed)))

        with self.subTest("identical EXT-001/002 surface signatures cannot silently diverge"):
            changed = copy.deepcopy(original_gap)
            changed["storySurfaceSignatures"]["EXT-002"] = []
            self.assertTrue(any("EXT-002: REQ-017 extensibility surface signature drifted" in error for error in errors_for(changed)))

        with self.subTest("command registry cannot become custom-slash ownership by subtraction"):
            changed = copy.deepcopy(original_gap)
            changed["candidateFragments"][1]["ownershipEstablished"] = True
            self.assertTrue(any("cannot become owned from residual requirement arithmetic" in error for error in errors_for(changed)))

    def test_sharing_gap_rejects_task_inference_and_unbounded_queue_promotion(self):
        ledger = load_ledger()
        self.assertEqual(sharing_ownership_gap_errors(ledger["stories"], ROOT), [])

        gap_path = ROOT / "sources/sharing-ownership-gap.json"
        original_gap = json.loads(gap_path.read_text(encoding="utf-8"))

        def errors_for(changed_gap):
            from tools import validate_backlog_exhaustion as module

            original_load = module._load

            def fake_load(path):
                if pathlib.Path(path) == gap_path:
                    return changed_gap
                return original_load(path)

            with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                return sharing_ownership_gap_errors(ledger["stories"], ROOT)

        with self.subTest("pure merge cannot be assigned from REQ-007 membership"):
            changed = copy.deepcopy(original_gap)
            changed["candidateFragments"][0]["ownershipEstablished"] = True
            self.assertTrue(any("cannot become owned from residual arithmetic" in error for error in errors_for(changed)))

        with self.subTest("SHARE-003 cannot become enterprise owner by overlap"):
            changed = copy.deepcopy(original_gap)
            changed["ownershipDecision"]["SHARE-003"] = "enterprise-share-http"
            self.assertTrue(any("ownership must remain unresolved" in error for error in errors_for(changed)))

        with self.subTest("coalescing queue cannot silently become bounded"):
            changed = copy.deepcopy(original_gap)
            queue = next(item for item in changed["reviewedPartitions"] if item["id"] == "share-event-subscription-and-coalescing-queue")
            queue["boundEstablished"] = True
            self.assertTrue(any("queue bound must remain explicitly unresolved" in error for error in errors_for(changed)))

        with self.subTest("SHARE-004 signature cannot silently diverge"):
            changed = copy.deepcopy(original_gap)
            changed["storySurfaceSignatures"]["SHARE-004"] = []
            self.assertTrue(any("SHARE-004: sharing ownership-gap surface signature drifted" in error for error in errors_for(changed)))


    def test_remaining_extensibility_and_integrations_gaps_reject_arithmetic_ownership(self):
        ledger = load_ledger()
        self.assertEqual(extensibility_remaining_gap_errors(ledger["stories"], ROOT), [])
        self.assertEqual(integrations_ownership_gap_errors(ledger["stories"], ROOT), [])

        cases = [
            (
                ROOT / "sources/extensibility-remaining-ownership-gap.json",
                extensibility_remaining_gap_errors,
                lambda gap: gap["ownershipDecision"].__setitem__("EXT-010", "plugin-v2-lifecycle"),
                "ownership must stay unresolved",
            ),
            (
                ROOT / "sources/extensibility-remaining-ownership-gap.json",
                extensibility_remaining_gap_errors,
                lambda gap: gap["storySurfaceSignatures"].__setitem__("EXT-006", []),
                "EXT-006: remaining extensibility surface signature drifted",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["candidateFragments"][0].__setitem__("ownershipEstablished", True),
                "cannot become owned from controller-status/task arithmetic",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["storySurfaceSignatures"].__setitem__("INT-009", []),
                "INT-009: integrations surface signature drifted",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["ptyBoundaryConstraints"].__setitem__("websocketOutboxBoundEstablished", True),
                "PTY websocket outbox bound must remain explicitly unresolved",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["adjacentAcceptedOwnership"].__setitem__(
                    "featureIds", ["SEC-001", "SEC-002", "SEC-008", "TOOL-005"]
                ),
                "PTY accepted process-terminal ownership guard drifted",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["unresolvedRulePartitions"].__setitem__(2, "packages/core/src/pty/**"),
                "integrations unresolved rule remainder drifted",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["generatedClientConstraints"].__setitem__("promiseAutomaticReconnect", True),
                "generated Promise client cannot silently gain automatic reconnect semantics",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["candidateFragments"][1].__setitem__("ownershipEstablished", True),
                "cannot become owned from controller-status/task arithmetic: generated-promise-effect-client-emission",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["projectLocationConstraints"].__setitem__("resolveWritesProjectCache", True),
                "ProjectV2 resolve must remain read-only with respect to the project cache",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["projectLocationConstraints"].__setitem__(
                    "migrationAndPersistenceOwnedByLegacyService", False
                ),
                "ProjectV2 cannot silently absorb legacy migration/persistence ownership",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["projectLocationConstraints"].__setitem__(
                    "projectIdentityAlgorithmSpecEstablished", True
                ),
                "ProjectV2 identity algorithm must remain explicitly missing a genuine spec",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["candidateFragments"][2].__setitem__("ownershipEstablished", True),
                "cannot become owned from controller-status/task arithmetic: project-location-context-resolution",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["vscodeBridgeConstraints"].__setitem__("directAutomatedTestEstablished", True),
                "VS Code bridge must remain explicitly missing direct automated behavior coverage",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["vscodeBridgeConstraints"].__setitem__("fetchTimeoutEstablished", True),
                "VS Code localhost fetch timeout must remain explicitly unresolved",
            ),
            (
                ROOT / "sources/integrations-ownership-gap.json",
                integrations_ownership_gap_errors,
                lambda gap: gap["candidateFragments"][3].__setitem__("ownershipEstablished", True),
                "cannot become owned from controller-status/task arithmetic: vscode-cli-terminal-bridge",
            ),
        ]

        for gap_path, validator, mutate, expected in cases:
            with self.subTest(path=gap_path.name, expected=expected):
                original_gap = json.loads(gap_path.read_text(encoding="utf-8"))
                changed = copy.deepcopy(original_gap)
                mutate(changed)
                from tools import validate_backlog_exhaustion as module
                original_load = module._load

                def fake_load(path):
                    if pathlib.Path(path) == gap_path:
                        return changed
                    return original_load(path)

                with mock.patch("tools.validate_backlog_exhaustion._load", side_effect=fake_load):
                    errors = validator(ledger["stories"], ROOT)
                self.assertTrue(any(expected in error for error in errors), errors)


    def test_every_unresolved_row_has_machine_checked_gap_coverage(self):
        ledger = load_ledger()
        self.assertEqual(unresolved_gap_coverage_errors(ledger["stories"], ROOT), [])

        synthetic = copy.deepcopy(ledger["stories"])
        synthetic.append({"id": "SYNTH-999", "category": "unresolved-decomposition"})
        errors = unresolved_gap_coverage_errors(synthetic, ROOT)
        self.assertTrue(any("lack machine-checkable decomposition coverage" in error for error in errors), errors)

        downgraded = copy.deepcopy(ledger["stories"])
        row = next(item for item in downgraded if item["id"] == "EXT-004")
        row["category"] = "explicit-blocker"
        errors = unresolved_gap_coverage_errors(downgraded, ROOT)
        self.assertTrue(any("cover rows no longer unresolved-decomposition" in error for error in errors), errors)

    def test_stale_local_implementation_receipts_cannot_disappear(self):
        bad = copy.deepcopy(load_ledger())
        row = next(item for item in bad["stories"] if item["id"] == "INT-008")
        row["worklog"] = None
        row["implementationCommits"] = []
        errors = validate_ledger(bad, ROOT)
        self.assertTrue(any("worklog drifted" in error for error in errors))
        self.assertTrue(any("implementationCommits drifted" in error for error in errors))

        with mock.patch(
            "tools.validate_backlog_exhaustion.subprocess.run",
            return_value=SimpleNamespace(returncode=1),
        ):
            history_errors = validate_ledger(load_ledger(), ROOT)
        self.assertTrue(any("implementation commit is missing from history" in error for error in history_errors))

    def test_accidental_disc_acceptance_is_rejected_by_expected_projection(self):
        reconciliation = json.loads((ROOT / "sources/disc-003-reconciliation.json").read_text(encoding="utf-8"))
        manifest = json.loads((ROOT / "sources/disc-003-reconciliation.manifest.json").read_text(encoding="utf-8"))
        source_map = json.loads((ROOT / "workspaces/DISC-003/source-map.json").read_text(encoding="utf-8"))
        task_text = (ROOT / "tasks/DISC-003.md").read_text(encoding="utf-8")
        progress_text = (ROOT / "workspaces/DISC-003/progress.md").read_text(encoding="utf-8")
        self.assertEqual(disc_status_errors(reconciliation, manifest, source_map, task_text, progress_text), [])

        bad_reconciliation = copy.deepcopy(reconciliation)
        bad_reconciliation["status"] = "accepted"
        self.assertTrue(any("reconciliation status changed" in error for error in disc_status_errors(
            bad_reconciliation, manifest, source_map, task_text, progress_text
        )))

        bad_reconciliation = copy.deepcopy(reconciliation)
        bad_reconciliation["reviewedSurfaces"][0]["reviewState"] = "reconciled"
        self.assertTrue(any("all reviewed surfaces to remain partial" in error for error in disc_status_errors(
            bad_reconciliation, manifest, source_map, task_text, progress_text
        )))

        bad_manifest = copy.deepcopy(manifest)
        bad_manifest["status"] = "release-evidence"
        self.assertTrue(any("manifest status changed" in error for error in disc_status_errors(
            reconciliation, bad_manifest, source_map, task_text, progress_text
        )))

        bad_source_map = copy.deepcopy(source_map)
        bad_source_map["status"] = "accepted"
        self.assertTrue(any("source-map status changed" in error for error in disc_status_errors(
            reconciliation, manifest, bad_source_map, task_text, progress_text
        )))

        expected = build_expected_ledger(ROOT)
        self.assertEqual(expected["status"], "in-progress-not-release-evidence")


if __name__ == "__main__":
    unittest.main()

import json
import pathlib
import tempfile
import unittest
from unittest import mock

from tools.inventory import (
    candidate_groups,
    candidate_surfaces,
    file_role,
    object_type,
    sha256_file,
    validate_surface_rules,
)
from tools.surface_candidates import aggregate, build_candidate_manifest, write_jsonl_atomic

ROOT = pathlib.Path(__file__).resolve().parents[2]


class Disc002InventoryTests(unittest.TestCase):
    def test_object_kinds_explicitly_account_for_symlink_executable_and_submodule(self):
        self.assertEqual(object_type("120000", "blob"), "symlink")
        self.assertEqual(object_type("100755", "blob"), "executable")
        self.assertEqual(object_type("160000", "commit"), "submodule")
        self.assertEqual(object_type("100644", "blob"), "blob")

    def test_roles_explicitly_account_for_tests_docs_infra_generated_binary_and_submodules(self):
        cases = {
            "packages/core/test/session.test.ts": "test",
            "specs/v2/session.md": "documentation",
            ".github/workflows/test.yml": "infrastructure",
            "packages/sdk/generated/client.ts": "generated",
            "packages/app/public/logo.png": "binary",
            "vendor/dependency": "submodule",
            "packages/core/src/session.ts": "source",
            "packages/core/migrations/001.sql": "migration",
            "package.json": "manifest",
            "bun.lock": "lockfile",
            "LICENSE": "license",
        }
        for path, wanted in cases.items():
            with self.subTest(path=path):
                if wanted == "submodule":
                    self.assertEqual(file_role(path, "160000", "commit"), wanted)
                else:
                    self.assertEqual(file_role(path, "100644", "blob"), wanted)

    def test_generic_release_and_automation_globs_are_not_candidate_owners(self):
        selectors = {
            "discovery": ["**"],
            "release": ["**"],
            "automation": ["**"],
            "sessions": ["packages/core/src/session/**"],
        }
        self.assertEqual(candidate_groups("README.md", selectors), [])
        self.assertEqual(candidate_groups("packages/core/src/session/runner/llm.ts", selectors), ["sessions"])

    def test_surface_rules_cover_requested_core_and_9router_paths(self):
        rules = json.loads((ROOT / "sources/behavior-surface-rules.json").read_text())
        session = candidate_surfaces("opencode", "packages/core/src/session/runner/llm.ts", rules)
        self.assertTrue(any(item["id"] == "opencode.session-runtime" for item in session))
        self.assertTrue(any(item["id"] == "opencode.agent-delegation" for item in session))
        router = candidate_surfaces("9router", "src/sse/handlers/chat.js", rules)
        self.assertTrue(any(item["id"] == "9router.routing" for item in router))
        self.assertTrue(any(item["id"] == "9router.translation-proxy" for item in router))

    def test_all_surface_rule_features_exist_in_canonical_plan(self):
        rules = json.loads((ROOT / "sources/behavior-surface-rules.json").read_text())
        plan = json.loads((ROOT / "ralph.json").read_text())
        ids = {story["id"] for story in plan["userStories"]}
        for rule in rules["rules"]:
            self.assertTrue(rule["featureIds"], rule["id"])
            self.assertLessEqual(set(rule["featureIds"]), ids, rule["id"])

    def test_candidate_aggregation_remains_unreviewed_and_deduplicates_sources(self):
        task_ids = {"SESS-001"}
        surface = {"id": "opencode.session-runtime", "kind": "session-lifecycle", "featureIds": ["SESS-001"]}
        entries = [
            {"key": "oc:a:one", "candidateSurfaces": [surface]},
            {"key": "oc:a:one", "candidateSurfaces": [surface]},
            {"key": "oc:a:two", "candidateSurfaces": [surface]},
        ]
        result = aggregate(entries, task_ids)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["status"], "candidate-unreviewed")
        self.assertEqual(result[0]["sourceCount"], 2)
        self.assertNotIn("reviewReceipt", result[0])

    def test_candidate_aggregation_rejects_unknown_feature(self):
        entries = [{
            "key": "oc:a:one",
            "candidateSurfaces": [{"id": "x", "kind": "x", "featureIds": ["NOPE-001"]}],
        }]
        with self.assertRaises(ValueError):
            aggregate(entries, {"SESS-001"})

    def test_inventory_hashing_is_streamed(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "inventory.jsonl"
            path.write_bytes(b"x" * (3 * 1024 * 1024 + 17))
            real_open = pathlib.Path.open
            reads = []

            class Reader:
                def __init__(self, handle):
                    self.handle = handle
                def __enter__(self):
                    return self
                def __exit__(self, *args):
                    self.handle.close()
                def read(self, size=-1):
                    reads.append(size)
                    return self.handle.read(size)

            def patched_open(self, *args, **kwargs):
                return Reader(real_open(self, *args, **kwargs))

            with mock.patch("pathlib.Path.open", patched_open):
                digest = sha256_file(path, chunk_size=1024 * 1024)
            self.assertEqual(len(digest), 64)
            self.assertTrue(reads)
            self.assertNotIn(-1, reads)
            self.assertTrue(all(size <= 1024 * 1024 for size in reads))

    def test_observed_structure_is_pinned_and_marked_partial(self):
        observed = json.loads((ROOT / "sources/disc-002-observed-structure.json").read_text())
        self.assertEqual(observed["schemaVersion"], 1)
        self.assertEqual(observed["status"], "partial-metadata-observation")
        self.assertFalse(observed["exhaustive"])
        self.assertEqual(observed["repositories"]["opencode"]["commit"], "95daf90670b7c039c436c85537da5fbfe2205b41")
        self.assertEqual(observed["repositories"]["9router"]["commit"], "17c4cc76877bd1755030a8414f8d0083f48dcccf")
        self.assertTrue(observed["repositories"]["opencode"]["behaviorEvidence"])
        self.assertIn("auth.js", observed["repositories"]["9router"]["observed"]["sseServices"])

    def test_runtime_configuration_and_process_paths_have_candidate_surfaces(self):
        rules = json.loads((ROOT / "sources/behavior-surface-rules.json").read_text())
        config = candidate_surfaces("opencode", "packages/core/src/config.ts", rules)
        self.assertTrue(any(item["id"] == "opencode.configuration-runtime" for item in config))
        process = candidate_surfaces("opencode", "packages/core/src/cross-spawn-spawner.ts", rules)
        self.assertTrue(any(item["id"] == "opencode.process-terminal" for item in process))
        ops = candidate_surfaces("opencode", "packages/core/src/repository-cache.ts", rules)
        self.assertTrue(any(item["id"] == "opencode.repository-operations" for item in ops))

    def test_disc002_source_map_is_pinned_candidate_evidence_only(self):
        source_map = json.loads((ROOT / "workspaces/DISC-002/source-map.json").read_text())
        plan = json.loads((ROOT / "ralph.json").read_text())
        task_ids = {story["id"] for story in plan["userStories"]}
        self.assertEqual(source_map["status"], "partial-candidate-evidence")
        self.assertIn("exact locked local checkouts", source_map["completionBlocker"].lower())
        self.assertGreaterEqual(len(source_map["observations"]), 6)
        for observation in source_map["observations"]:
            self.assertEqual(observation["reviewState"], "candidate-unreviewed")
            self.assertRegex(observation["blobSha"], r"^[0-9a-f]{40}$")
            self.assertLessEqual(set(observation["candidateFeatureIds"]), task_ids)

    def test_surface_rules_cover_remaining_runtime_and_client_families(self):
        rules = json.loads((ROOT / "sources/behavior-surface-rules.json").read_text())
        cases = {
            ("opencode", "packages/core/src/permission.ts"): "opencode.permission-runtime",
            ("opencode", "packages/core/src/integration.ts"): "opencode.integration-auth",
            ("opencode", "packages/codemode/src/index.ts"): "opencode.code-mode",
            ("opencode", "packages/app/src/app.tsx"): "opencode.app-client",
            ("opencode", "packages/desktop/src/main.ts"): "opencode.desktop-client",
            ("opencode", "packages/http-recorder/src/index.ts"): "opencode.provider-recording",
            ("9router", "src/sse/services/tokenRefresh.js"): "9router.token-refresh",
            ("9router", "src/store/providerConnections.js"): "9router.account-storage",
            ("9router", "src/mitm/proxy.js"): "9router.network-proxy",
        }
        for (repository, path), expected in cases.items():
            with self.subTest(repository=repository, path=path):
                ids = {item["id"] for item in candidate_surfaces(repository, path, rules)}
                self.assertIn(expected, ids)

    def test_surface_rule_validation_rejects_duplicate_and_unknown_features(self):
        plan = json.loads((ROOT / "ralph.json").read_text())
        task_ids = {story["id"] for story in plan["userStories"]}
        rules = json.loads((ROOT / "sources/behavior-surface-rules.json").read_text())
        validate_surface_rules(rules, task_ids, {"opencode", "9router"})
        duplicate = {"schemaVersion": 1, "rules": [rules["rules"][0], rules["rules"][0]]}
        with self.assertRaises(ValueError):
            validate_surface_rules(duplicate, task_ids, {"opencode", "9router"})
        bad = json.loads(json.dumps(rules))
        bad["rules"][0]["featureIds"] = ["NOPE-001"]
        with self.assertRaises(ValueError):
            validate_surface_rules(bad, task_ids, {"opencode", "9router"})

    def test_candidate_manifest_binds_queue_to_inventory_and_rules_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            inventory = root / "inventory"
            inventory.mkdir()
            (inventory / "manifest.json").write_text('{"schemaVersion":3,"repositories":[]}\n')
            output = root / "surface-candidates.jsonl"
            candidates = [{"id": "x", "sourceCount": 2}]
            write_jsonl_atomic(output, candidates)
            manifest = build_candidate_manifest(inventory, output, candidates)
            self.assertEqual(manifest["status"], "candidate-unreviewed")
            self.assertEqual(manifest["candidateSurfaces"], 1)
            self.assertEqual(manifest["candidateSources"], 2)
            self.assertRegex(manifest["outputSha256"], r"^[0-9a-f]{64}$")
            self.assertRegex(manifest["inventoryManifestSha256"], r"^[0-9a-f]{64}$")
            self.assertRegex(manifest["surfaceRuleSha256"], r"^[0-9a-f]{64}$")

    def test_candidate_aggregation_records_repository_and_file_role_counts(self):
        task_ids = {"SESS-001"}
        surface = {"id": "s", "kind": "session", "featureIds": ["SESS-001"]}
        entries = [
            {"key": "oc:a:one", "repository": "opencode", "fileRole": "source", "candidateSurfaces": [surface]},
            {"key": "oc:a:two", "repository": "opencode", "fileRole": "test", "candidateSurfaces": [surface]},
        ]
        result = aggregate(entries, task_ids)[0]
        self.assertEqual(result["repositoryCounts"], {"opencode": 2})
        self.assertEqual(result["fileRoleCounts"], {"source": 1, "test": 1})


if __name__ == "__main__":
    unittest.main()

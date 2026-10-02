#!/usr/bin/env python3
"""R1 unit tests: receipt/artifact validation and stale cross-SHA rejection.

Stdlib-only, disposable tmp fixtures, no heavy platform gates, no network,
no user DB, no credentials. Also smoke-checks the real repo contract via
run_release_gates.py --check-contract against the base-pinned manifest.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "release"))
import receipts  # noqa: E402
import run_release_gates  # noqa: E402

SHA = "f5f440d8f4ea7085409a06f10343494945fef6c0"
OTHER_SHA = "a79ae96b93f9a0ab94ceafadcd9425db08f28027"
MANIFEST = ROOT / "release" / "release-gates.manifest.template.json"
DRIVER = ROOT / "release" / "run_release_gates.py"


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def base_manifest() -> dict:
    return {
        "source_sha": SHA,
        "profile": "release",
        "native": True,
        "binary_sha256": "b" * 64,
        "native_library_sha256": "c" * 64,
        "archive_sha256": "a" * 64,
        "runner_sha256": "d" * 64,
        "frozen_tests": {"tests/example_gate.py": "e" * 64},
    }


class ManifestSchema(unittest.TestCase):
    def test_missing_key_rejected(self):
        manifest = base_manifest()
        del manifest["binary_sha256"]
        with self.assertRaises(receipts.ReceiptError):
            receipts.validate_manifest(manifest)

    def test_bad_sha_shapes_rejected(self):
        for key, bad in (("source_sha", "deadbeef"), ("binary_sha256", "XYZ")):
            manifest = base_manifest()
            manifest[key] = bad
            with self.assertRaises(receipts.ReceiptError, msg=key):
                receipts.validate_manifest(manifest)

    def test_non_release_or_non_native_rejected(self):
        for mutate in ({"profile": "dev"}, {"native": False}):
            manifest = {**base_manifest(), **mutate}
            with self.assertRaises(receipts.ReceiptError, msg=str(mutate)):
                receipts.validate_manifest(manifest)

    def test_real_template_contract_validates(self):
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
        self.assertEqual(data["source_sha"], receipts.TEMPLATE_SHA)
        self.assertEqual(data["artifact_contract"]["source_sha"], receipts.TEMPLATE_SHA)

    def test_template_materializes_exact_sha(self):
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as td:
            out = pathlib.Path(td) / "concrete.json"
            result = receipts.materialize_template(data, SHA, out)
            self.assertEqual(result["source_sha"], SHA)
            self.assertEqual(result["artifact_contract"]["source_sha"], SHA)
            receipts.validate_manifest(result["artifact_contract"])
            self.assertEqual(json.loads(out.read_text())["source_sha"], SHA)

    def test_template_materialization_rejects_mixed_and_stale(self):
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
        data["artifact_contract"]["source_sha"] = SHA
        with tempfile.TemporaryDirectory() as td:
            with self.assertRaises(receipts.ReceiptError):
                receipts.materialize_template(data, SHA, pathlib.Path(td) / "x.json")
            stale = json.loads(MANIFEST.read_text())
            stale["source_sha"] = OTHER_SHA
            with self.assertRaises(receipts.ReceiptError):
                receipts.materialize_template(stale, OTHER_SHA, pathlib.Path(td) / "y.json")

    def test_template_materialization_refuses_existing_output(self):
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as td:
            out = pathlib.Path(td) / "x.json"
            out.write_text("{}")
            with self.assertRaises(receipts.ReceiptError):
                receipts.materialize_template(data, SHA, out)


class ArtifactHashBinding(unittest.TestCase):
    def _artifact_dir(self, root: pathlib.Path, manifest: dict) -> pathlib.Path:
        art = root / "artifact"
        (art / "install" / "bin").mkdir(parents=True)
        (art / "install" / "lib").mkdir(parents=True)
        binp = art / "install" / "bin" / "oc2"
        libp = art / "install" / "lib" / "libopentui.so"
        arc = art / "oc2-linux-arm64.tar.gz"
        binp.write_bytes(b"binary")
        libp.write_bytes(b"library")
        arc.write_bytes(b"archive")
        manifest["binary_sha256"] = digest(binp)
        manifest["native_library_sha256"] = digest(libp)
        manifest["archive_sha256"] = digest(arc)
        return art

    def test_matching_hashes_pass(self):
        with tempfile.TemporaryDirectory() as td:
            manifest = base_manifest()
            art = self._artifact_dir(pathlib.Path(td), manifest)
            receipts.validate_artifact_hashes(manifest, art)

    def test_swapped_binary_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            manifest = base_manifest()
            art = self._artifact_dir(pathlib.Path(td), manifest)
            (art / "install" / "bin" / "oc2").write_bytes(b"other-binary")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art)

    def test_missing_library_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            manifest = base_manifest()
            art = self._artifact_dir(pathlib.Path(td), manifest)
            (art / "install" / "lib" / "libopentui.so").unlink()
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art)

    def test_frozen_test_mutation_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            src = root / "tests" / "example_gate.py"
            src.parent.mkdir(parents=True)
            src.write_text("print('ok')\n")
            manifest = base_manifest()
            manifest["frozen_tests"]["tests/example_gate.py"] = digest(src)
            receipts.validate_frozen_tests(manifest, root)
            src.write_text("print('weakened')\n")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_frozen_tests(manifest, root)

    def test_runner_tamper_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            runner = pathlib.Path(td) / "runner.py"
            runner.write_text("# retained runner\n")
            manifest = base_manifest()
            manifest["runner_sha256"] = digest(runner)
            receipts.validate_runner(manifest, runner)
            runner.write_text("# patched runner\n")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_runner(manifest, runner)


class ReceiptStaleness(unittest.TestCase):
    def _gate(self, root: pathlib.Path, sha: str, exit_code: int = 0) -> dict:
        log = root / "gate.log"
        log.write_text(f"exit {exit_code}\n")
        return {
            "source_sha": sha, "label": "build", "command": "/bin/true", "cwd": str(root),
            "exit": exit_code, "limit_failure": None, "seconds": 0.1,
            "log_path": str(log), "log_sha256": digest(log),
        }

    def test_matching_receipts_pass(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            receipts_obj = {
                "source_sha": SHA, "platform": "ubuntu",
                "gates": [self._gate(root, SHA)],
            }
            self.assertEqual(receipts.validate_receipts(receipts_obj, SHA, "ubuntu"), [])

    def test_stale_cross_sha_receipt_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            receipts_obj = {
                "source_sha": OTHER_SHA, "platform": "ubuntu",
                "gates": [self._gate(root, SHA)],
            }
            violations = receipts.validate_receipts(receipts_obj, SHA, "ubuntu")
            self.assertTrue(any("stale cross-SHA reuse" in v for v in violations), violations)

    def test_stale_gate_receipt_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            receipts_obj = {
                "source_sha": SHA, "platform": "ubuntu",
                "gates": [self._gate(root, OTHER_SHA)],
            }
            violations = receipts.validate_receipts(receipts_obj, SHA, "ubuntu")
            self.assertTrue(any("stale source_sha" in v for v in violations), violations)

    def test_nonzero_exit_recorded_not_masked(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            receipts_obj = {
                "source_sha": SHA, "platform": "mac",
                "gates": [self._gate(root, SHA, exit_code=3)],
            }
            violations = receipts.validate_receipts(receipts_obj, SHA, "mac")
            self.assertTrue(any("exit=3" in v for v in violations), violations)

    def test_tampered_log_hash_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            gate = self._gate(root, SHA)
            pathlib.Path(gate["log_path"]).write_text("rewritten evidence\n")
            receipts_obj = {"source_sha": SHA, "platform": "web", "gates": [gate]}
            violations = receipts.validate_receipts(receipts_obj, SHA, "web")
            self.assertTrue(any("tampered evidence" in v for v in violations), violations)

    def test_capture_budget_failure_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            gate = self._gate(root, SHA)
            gate["limit_failure"] = "output budget"
            receipts_obj = {"source_sha": SHA, "platform": "web", "gates": [gate]}
            violations = receipts.validate_receipts(receipts_obj, SHA, "web")
            self.assertTrue(any("capture limit" in v for v in violations), violations)

    def test_wrong_platform_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            receipts_obj = {"source_sha": SHA, "platform": "ubuntu",
                            "gates": [self._gate(root, SHA)]}
            violations = receipts.validate_receipts(receipts_obj, SHA, "mac")
            self.assertTrue(any("platform" in v for v in violations), violations)

    def test_evidence_dir_reuse_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            stale = pathlib.Path(td) / "evidence"
            stale.mkdir()
            (stale / "receipts.json").write_text("{}")
            with self.assertRaises(receipts.ReceiptError):
                receipts.reject_nonempty_dir(stale)
            receipts.reject_nonempty_dir(pathlib.Path(td) / "fresh")
            receipts.reject_nonempty_dir(pathlib.Path(td) / "missing")


class EnvironmentHygiene(unittest.TestCase):
    def test_disposable_env_isolates_and_scrubs(self):
        with tempfile.TemporaryDirectory() as td:
            env = receipts.disposable_env(pathlib.Path(td) / "home-root")
            for key in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "TMPDIR"):
                self.assertTrue(env[key].startswith(td), key)
            inherited = {**env, "OPENAI_API_KEY": "leak", "GITHUB_TOKEN": "leak2"}
            scrubbed = receipts.scrub_inherited_env(inherited)
            self.assertNotIn("OPENAI_API_KEY", scrubbed)
            self.assertNotIn("GITHUB_TOKEN", scrubbed)


class DriverContract(unittest.TestCase):
    def run_driver(self, *extra: str) -> subprocess.CompletedProcess:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        cmd = [sys.executable, str(DRIVER), "--source-sha", SHA,
               "--manifest-template", str(MANIFEST), "--evidence-dir", tmp.name + "/evidence"]
        return subprocess.run(cmd + list(extra), capture_output=True, text=True, timeout=60)

    def test_check_contract_passes_against_real_repo(self):
        r = self.run_driver("--platform", "mac", "--platform", "web",
                            "--platform", "ubuntu", "--check-contract")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("contract OK", r.stdout)

    def test_wrong_source_sha_rejected(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        r = subprocess.run(
            [sys.executable, str(DRIVER), "--source-sha", "f" * 40,
             "--manifest", str(MANIFEST), "--evidence-dir", tmp.name + "/e",
             "--platform", "ubuntu", "--check-contract"],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 1)
        self.assertIn("stale", (r.stdout + r.stderr).lower())

    def test_tool_gates_require_ubuntu_selection(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        r = subprocess.run(
            [sys.executable, str(DRIVER), "--source-sha", SHA,
             "--manifest", str(MANIFEST), "--evidence-dir", tmp.name + "/e",
             "--platform", "mac", "--tool-gates", "--check-contract"],
            capture_output=True, text=True, timeout=60)
        self.assertNotEqual(r.returncode, 0)

    def test_frozen_gate_hash_mismatch_is_detected(self):
        # simulate by pointing driver at a temp repo-root copy with one edited file
        with tempfile.TemporaryDirectory() as td:
            repo = pathlib.Path(td) / "repo"
            (repo / "release").mkdir(parents=True)
            (repo / "tests" / "e2e").mkdir(parents=True)
            (repo / "tests" / "bootstrap").mkdir(parents=True)
            for rel in json.loads(MANIFEST.read_text())["artifact_contract"]["frozen_tests"]:
                shutil_copy = pathlib.Path(td)  # noqa: F841
                (repo / rel).write_text("# weakened to force green\n")
            shutil = __import__("shutil")
            shutil.copy(DRIVER, repo / "release" / "run_release_gates.py")
            shutil.copy(ROOT / "release" / "receipts.py", repo / "release" / "receipts.py")
            shutil.copy(ROOT / "release" / "verify-native-utf8.py",
                        repo / "release" / "verify-native-utf8.py")
            shutil.copy(MANIFEST, repo / "release" / "release-gates.manifest.template.json")
            r = subprocess.run(
                [sys.executable, str(repo / "release" / "run_release_gates.py"),
                 "--source-sha", SHA, "--manifest-template",
                 str(repo / "release" / "release-gates.manifest.template.json"),
                 "--evidence-dir", td + "/e", "--repo-root", str(repo),
                 "--platform", "mac", "--check-contract"],
                capture_output=True, text=True, timeout=60)
            self.assertEqual(r.returncode, 1)
            self.assertIn("modified", r.stderr)


class ExitCodeCapture(unittest.TestCase):
    def test_real_exit_code_no_pipeline_masking(self):
        with tempfile.TemporaryDirectory() as td:
            out = pathlib.Path(td)
            log = out / "failing.log"
            command = [sys.executable, "-c", "import sys; print('out'); sys.exit(7)"]
            record = run_release_gates.run_gate(
                "selftest:failing", command, out,
                {"R1_SOURCE_SHA": SHA, "PATH": os.environ.get("PATH", "")},
                30, log, out)
            self.assertEqual(record["exit"], 7, "exit code must not be masked")
            self.assertEqual(record["limit_failure"], None)
            self.assertEqual(record["log_sha256"], digest(log))

    def test_timeout_kills_group_and_records(self):
        with tempfile.TemporaryDirectory() as td:
            out = pathlib.Path(td)
            command = [sys.executable, "-c", "import time; time.sleep(30)"]
            record = run_release_gates.run_gate(
                "selftest:hang", command, out, {"R1_SOURCE_SHA": SHA}, 2,
                out / "hang.log", out)
            self.assertEqual(record["limit_failure"], "timeout")
            self.assertNotEqual(record["exit"], 0)


if __name__ == "__main__":
    unittest.main()

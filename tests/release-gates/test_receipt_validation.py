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
import shutil
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "release"))
import receipts  # noqa: E402
import run_release_gates  # noqa: E402
import materialize_release_manifest  # noqa: E402

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
        "platform": "linux",
        "target": "installed-binary",
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
        arc = art / "oc2-linux-x64.tar.gz"
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
            receipts.validate_artifact_hashes(manifest, art, platform="web")

    def test_swapped_binary_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            manifest = base_manifest()
            art = self._artifact_dir(pathlib.Path(td), manifest)
            (art / "install" / "bin" / "oc2").write_bytes(b"other-binary")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="web")

    def test_missing_library_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            manifest = base_manifest()
            art = self._artifact_dir(pathlib.Path(td), manifest)
            (art / "install" / "lib" / "libopentui.so").unlink()
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="web")

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

    def test_require_archive_false_rejected(self):
        """Artifact-controlled require_archive=false must be rejected."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = base_manifest()
            manifest["require_archive"] = False  # untrusted artifact input
            art = self._artifact_dir(root, manifest)
            # Even with valid archive, require_archive in build.json is rejected
            with self.assertRaises(receipts.ReceiptError) as cm:
                receipts.validate_artifact_hashes(manifest, art, platform="web")
            self.assertIn("require_archive", str(cm.exception))

    def test_missing_archive_rejected(self):
        """Archive is always required, cannot be bypassed."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = base_manifest()
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"binary")
            (art / "install" / "lib" / "libopentui.so").write_bytes(b"library")
            # No archive file
            manifest["binary_sha256"] = digest(art / "install" / "bin" / "oc2")
            manifest["native_library_sha256"] = digest(art / "install" / "lib" / "libopentui.so")
            with self.assertRaises(receipts.ReceiptError) as cm:
                receipts.validate_artifact_hashes(manifest, art, platform="web")
            self.assertIn("archive", str(cm.exception).lower())

    def test_symlink_artifact_rejected(self):
        """Symlinks in artifact directory are rejected."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = base_manifest()
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"binary")
            target = root / "lib-target"
            target.write_bytes(b"library")
            (art / "install" / "lib" / "libopentui.so").symlink_to(target)
            manifest["binary_sha256"] = digest(art / "install" / "bin" / "oc2")
            manifest["native_library_sha256"] = digest(target)
            manifest["archive_sha256"] = "a" * 64
            # Create archive to pass the archive check
            (art / "oc2-linux-x64.tar.gz").write_bytes(b"archive")
            with self.assertRaises(receipts.ReceiptError) as cm:
                receipts.validate_artifact_hashes(manifest, art, platform="web")
            self.assertIn("symlink", str(cm.exception).lower())

    def test_broken_symlink_artifact_rejected(self):
        """Broken symlinks in artifact directory are rejected."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = base_manifest()
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"binary")
            (art / "install" / "lib" / "libopentui.so").symlink_to(root / "nonexistent")
            manifest["binary_sha256"] = digest(art / "install" / "bin" / "oc2")
            # Hash the intended payload, not the dangling link (which cannot be read).
            manifest["native_library_sha256"] = digest(root / "nonexistent") if False else "c" * 64
            manifest["archive_sha256"] = "a" * 64
            (art / "oc2-linux-x64.tar.gz").write_bytes(b"archive")
            with self.assertRaises(receipts.ReceiptError) as cm:
                receipts.validate_artifact_hashes(manifest, art, platform="web")
            self.assertIn("symlink", str(cm.exception).lower())

    def test_mac_dylib_archive_layout(self):
        """Mac platform: libopentui.dylib + oc2-macos-arm64.tar.gz."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = {**base_manifest(), "platform": "mac", "archive_sha256": "a" * 64}
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"BIN")
            (art / "install" / "lib" / "libopentui.dylib").write_bytes(b"LIB")
            (art / "oc2-macos-arm64.tar.gz").write_bytes(b"ARC")
            manifest["binary_sha256"] = digest(art / "install" / "bin" / "oc2")
            manifest["native_library_sha256"] = digest(art / "install" / "lib" / "libopentui.dylib")
            manifest["archive_sha256"] = digest(art / "oc2-macos-arm64.tar.gz")
            receipts.validate_artifact_hashes(manifest, art, platform="mac")

    def test_web_linux_so_x64_archive_layout(self):
        """Web platform (Linux x64): libopentui.so + oc2-linux-x64.tar.gz."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            manifest = {**base_manifest(), "platform": "linux", "archive_sha256": "a" * 64}
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"BIN")
            (art / "install" / "lib" / "libopentui.so").write_bytes(b"LIB")
            (art / "oc2-linux-x64.tar.gz").write_bytes(b"ARC")
            manifest["binary_sha256"] = digest(art / "install" / "bin" / "oc2")
            manifest["native_library_sha256"] = digest(art / "install" / "lib" / "libopentui.so")
            manifest["archive_sha256"] = digest(art / "oc2-linux-x64.tar.gz")
            receipts.validate_artifact_hashes(manifest, art, platform="web")

    def test_ubuntu_arm64_archive_layout(self):
        with tempfile.TemporaryDirectory() as td:
            art = pathlib.Path(td) / "artifact"
            (art / "install/bin").mkdir(parents=True)
            (art / "install/lib").mkdir(parents=True)
            (art / "install/bin/oc2").write_bytes(b"BIN")
            (art / "install/lib/libopentui.so").write_bytes(b"LIB")
            (art / "oc2-linux-arm64.tar.gz").write_bytes(b"ARC")
            manifest = {**base_manifest(), "platform": "linux", "target": "installed-binary"}
            manifest["binary_sha256"] = digest(art / "install/bin/oc2")
            manifest["native_library_sha256"] = digest(art / "install/lib/libopentui.so")
            manifest["archive_sha256"] = digest(art / "oc2-linux-arm64.tar.gz")
            receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")

    def test_archive_override_path_traversal_and_absent_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            art = pathlib.Path(td) / "artifact"
            (art / "install/bin").mkdir(parents=True)
            (art / "install/lib").mkdir(parents=True)
            (art / "install/bin/oc2").write_bytes(b"BIN")
            (art / "install/lib/libopentui.so").write_bytes(b"LIB")
            manifest = {**base_manifest(), "platform": "linux"}
            manifest["binary_sha256"] = digest(art / "install/bin/oc2")
            manifest["native_library_sha256"] = digest(art / "install/lib/libopentui.so")
            manifest["archive_sha256"] = "a" * 64
            with self.assertRaises(TypeError):
                receipts.validate_artifact_hashes(manifest, art, archive_name="../../escape")
            (art / "oc2-linux-arm64.tar.gz").write_bytes(b"ARC")
            (art / "escape.tar.gz").write_bytes(b"ARC")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")
            (art / "escape.tar.gz").unlink()
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")

    def test_archive_symlink_and_extra_files_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            art = root / "artifact"
            (art / "install/bin").mkdir(parents=True)
            (art / "install/lib").mkdir(parents=True)
            for path, value in ((art / "install/bin/oc2", b"BIN"),
                                (art / "install/lib/libopentui.so", b"LIB")):
                path.write_bytes(value)
            target = root / "outside"
            target.write_bytes(b"ARC")
            (art / "oc2-linux-arm64.tar.gz").symlink_to(target)
            manifest = {**base_manifest(), "platform": "linux"}
            manifest["binary_sha256"] = digest(art / "install/bin/oc2")
            manifest["native_library_sha256"] = digest(art / "install/lib/libopentui.so")
            manifest["archive_sha256"] = digest(target)
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")

    def test_archive_override_false_and_target_mismatch_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            art = pathlib.Path(td) / "artifact"
            (art / "install/bin").mkdir(parents=True)
            (art / "install/lib").mkdir(parents=True)
            (art / "install/bin/oc2").write_bytes(b"BIN")
            (art / "install/lib/libopentui.so").write_bytes(b"LIB")
            (art / "oc2-linux-arm64.tar.gz").write_bytes(b"ARC")
            manifest = {**base_manifest(), "platform": "linux", "require_archive": False}
            manifest["binary_sha256"] = digest(art / "install/bin/oc2")
            manifest["native_library_sha256"] = digest(art / "install/lib/libopentui.so")
            manifest["archive_sha256"] = digest(art / "oc2-linux-arm64.tar.gz")
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")
            del manifest["require_archive"]
            manifest["target"] = "x86_64-unknown-linux-gnu"
            with self.assertRaises(receipts.ReceiptError):
                receipts.validate_artifact_hashes(manifest, art, platform="ubuntu")


class AtomicMaterialization(unittest.TestCase):
    def test_atomic_writer_rejects_destinations_preserves_links_and_cleans_tmp(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            output = root / "manifest.json"
            predictable = root / "manifest.json.tmp"
            outside = root / "outside"
            outside.write_text("safe")
            predictable.symlink_to(outside)
            receipts.write_json_atomic_safe(output, {"ok": True})
            self.assertEqual(outside.read_text(), "safe")
            self.assertEqual(json.loads(output.read_text()), {"ok": True})
            self.assertEqual(list(root.glob(".manifest-*.tmp")), [])
            for destination in (root / "regular", root / "live", root / "broken"):
                if destination.name == "regular":
                    destination.write_text("sentinel")
                elif destination.name == "live":
                    destination.symlink_to(outside)
                else:
                    destination.symlink_to(root / "missing-target")
                with self.assertRaises(receipts.ReceiptError):
                    receipts.write_json_atomic_exclusive(destination, {"no": "overwrite"})
            self.assertEqual(outside.read_text(), "safe")

    def test_exclusive_writer_race_destination_preserved_and_temp_cleaned(self):
        with tempfile.TemporaryDirectory() as td:
            output = pathlib.Path(td) / "manifest.json"
            original_link = receipts.os.link
            os_link = receipts.os.link

            def race(source, target, **kwargs):
                pathlib.Path(target).write_text("racer")
                return os_link(source, target, **kwargs)

            receipts.os.link = race
            try:
                with self.assertRaises(receipts.ReceiptError):
                    receipts.write_json_atomic_exclusive(output, {"ours": True})
            finally:
                receipts.os.link = original_link
            self.assertEqual(output.read_text(), "racer")
            self.assertEqual(list(pathlib.Path(td).glob(".manifest-*.tmp")), [])

    def test_materializer_copies_only_hash_facts_and_rejects_drift(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            template = json.loads(MANIFEST.read_text())
            build = receipts.load_json(MANIFEST)
            build["source_sha"] = SHA
            build["frozen_tests"] = template["artifact_contract"]["frozen_tests"]
            build["runner_sha256"] = template["artifact_contract"]["runner_sha256"]
            build["binary_sha256"] = "1" * 64
            build["native_library_sha256"] = "2" * 64
            build["archive_sha256"] = "3" * 64
            build.update({"profile": "release", "native": True, "target": "installed-binary",
                          "platform": "mac", "commands": ["attacker"],
                          "gates": {"mac": [{"command": ["touch", "/tmp/pwned"]}]},
                          "platforms": {"attacker": {"gates": []}}})
            build_path = root / "build.json"
            build_path.write_text(json.dumps(build))
            template_path = root / "template.json"
            template_path.write_text(json.dumps(template))
            result_path = root / "concrete.json"
            concrete = materialize_release_manifest.materialize(template_path, build_path, SHA, result_path)
            self.assertEqual(concrete["platforms"], template["platforms"])
            self.assertNotIn("commands", concrete["artifact_contract"])
            self.assertEqual(concrete["artifact_contract"]["runner_sha256"], template["artifact_contract"]["runner_sha256"])
            with self.assertRaises(receipts.ReceiptError):
                materialize_release_manifest.materialize(template_path, build_path, SHA, result_path)
            build["runner_sha256"] = "f" * 64
            build_path.write_text(json.dumps(build))
            with self.assertRaises(receipts.ReceiptError):
                materialize_release_manifest.materialize(template_path, build_path, SHA, root / "other.json")

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


class MultiPlatformRejection(unittest.TestCase):
    """Tests for --run mode multi-platform rejection."""

    def test_multi_platform_run_rejected(self):
        """--run with multiple platforms must be rejected."""
        with tempfile.TemporaryDirectory() as td:
            # Create a minimal concrete manifest
            manifest = json.loads(MANIFEST.read_text())
            manifest["source_sha"] = SHA
            manifest["artifact_contract"]["source_sha"] = SHA
            manifest_path = pathlib.Path(td) / "manifest.json"
            manifest_path.write_text(json.dumps(manifest))

            evidence_dir = pathlib.Path(td) / "evidence"
            r = subprocess.run(
                [sys.executable, str(DRIVER), "--source-sha", SHA,
                 "--manifest", str(manifest_path),
                 "--evidence-dir", str(evidence_dir),
                 "--platform", "mac", "--platform", "web",
                 "--run"],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(r.returncode, 2)
            self.assertIn("--run accepts exactly one platform", r.stderr)

    def test_single_platform_run_allowed(self):
        """A valid single-platform synthetic artifact binds then runs only its gate."""
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td) / "repo"
            (root / "release").mkdir(parents=True)
            (root / "tests").mkdir()
            import shutil
            template = json.loads(MANIFEST.read_text())
            template["platforms"]["mac"]["gates"] = [{
                "label": "marker", "command": [sys.executable, "-c", "from pathlib import Path; Path('marker').write_text('ran')"],
            }]
            (root / "release" / "release-gates.manifest.template.json").write_text(json.dumps(template))
            shutil.copy(ROOT / "release" / "receipts.py", root / "release" / "receipts.py")
            shutil.copy(ROOT / "release" / "run_release_gates.py", root / "release" / "run_release_gates.py")
            shutil.copy(ROOT / "release" / "verify-native-utf8.py", root / "release" / "verify-native-utf8.py")
            for rel in template["artifact_contract"]["frozen_tests"]:
                dest = root / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(ROOT / rel, dest)
            manifest = template
            manifest["source_sha"] = SHA
            manifest["artifact_contract"]["source_sha"] = SHA
            marker = root / "marker"
            manifest["platforms"]["mac"]["gates"] = [{
                "label": "marker", "command": [sys.executable, "-c", f"open({str(marker)!r}, 'w').write('ran')"],
                "timeout_seconds": 10,
            }]
            (root / "release" / "release-gates.manifest.template.json").write_text(json.dumps(manifest))
            manifest_path = pathlib.Path(td) / "manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            art = pathlib.Path(td) / "artifact"
            (art / "install/bin").mkdir(parents=True)
            (art / "install/lib").mkdir(parents=True)
            (art / "install/bin/oc2").write_bytes(b"BIN")
            (art / "install/lib/libopentui.dylib").write_bytes(b"LIB")
            (art / "oc2-macos-arm64.tar.gz").write_bytes(b"ARC")
            build = {**manifest["artifact_contract"], "source_sha": SHA, "platform": "mac"}
            build["binary_sha256"] = digest(art / "install/bin/oc2")
            build["native_library_sha256"] = digest(art / "install/lib/libopentui.dylib")
            build["archive_sha256"] = digest(art / "oc2-macos-arm64.tar.gz")
            (art / "build.json").write_text(json.dumps(build))
            evidence_dir = pathlib.Path(td) / "evidence"
            r = subprocess.run(
                [sys.executable, str(DRIVER), "--source-sha", SHA,
                 "--manifest", str(manifest_path), "--repo-root", str(root),
                 "--evidence-dir", str(evidence_dir),
                 "--artifact-dir", str(art), "--platform", "mac", "--run"],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertTrue((root / "marker").is_file(), r.stdout + r.stderr)

    def test_rejected_run_modes_start_no_marker_children(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            marker = root / "marker"
            manifest = json.loads(MANIFEST.read_text())
            manifest["source_sha"] = SHA
            manifest["artifact_contract"]["source_sha"] = SHA
            manifest["platforms"]["mac"]["gates"] = [{
                "label": "marker", "command": [sys.executable, "-c", f"open({str(marker)!r}, 'w').write('ran')"],
            }]
            concrete = root / "manifest.json"
            concrete.write_text(json.dumps(manifest))
            common = [sys.executable, str(DRIVER), "--source-sha", SHA,
                      "--manifest", str(concrete), "--evidence-dir"]
            runs = [
                common + [str(root / "template-run"), "--manifest-template", str(MANIFEST), "--platform", "mac", "--run"],
                common + [str(root / "missing-artifact"), "--platform", "mac", "--run"],
                common + [str(root / "multi"), "--platform", "mac", "--platform", "web", "--run"],
                common + [str(root / "duplicate"), "--platform", "mac", "--platform", "mac", "--run"],
            ]
            for args in runs:
                result = subprocess.run(args, capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertFalse(marker.exists(), result.stdout + result.stderr)


class WorkflowContract(unittest.TestCase):
    """Tests for workflow-level contract validation."""

    def test_check_contract_all_platforms(self):
        """Static check-contract must accept all 3 platforms."""
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        r = subprocess.run(
            [sys.executable, str(DRIVER), "--source-sha", SHA,
             "--manifest-template", str(MANIFEST),
             "--evidence-dir", tmp.name + "/evidence",
             "--platform", "mac", "--platform", "web", "--platform", "ubuntu",
             "--check-contract"],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("contract OK", r.stdout)

    def test_check_contract_rejects_artifact_dir_with_require_archive(self):
        """Contract check rejects artifact with require_archive in build.json."""
        with tempfile.TemporaryDirectory() as td:
            # Create artifact with require_archive=false
            root = pathlib.Path(td)
            manifest = json.loads(MANIFEST.read_text())
            manifest["source_sha"] = SHA
            manifest["artifact_contract"]["source_sha"] = SHA
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps(manifest))

            # Create artifact with require_archive=false
            art = root / "artifact"
            (art / "install" / "bin").mkdir(parents=True)
            (art / "install" / "lib").mkdir(parents=True)
            (art / "install" / "bin" / "oc2").write_bytes(b"BIN")
            (art / "install" / "lib" / "libopentui.dylib").write_bytes(b"LIB")
            (art / "oc2-macos-arm64.tar.gz").write_bytes(b"ARC")
            build = {
                "source_sha": SHA,
                "profile": "release",
                "native": True,
                "binary_sha256": digest(art / "install" / "bin" / "oc2"),
                "native_library_sha256": digest(art / "install" / "lib" / "libopentui.dylib"),
                "archive_sha256": digest(art / "oc2-macos-arm64.tar.gz"),
                "runner_sha256": manifest["artifact_contract"]["runner_sha256"],
                "frozen_tests": manifest["artifact_contract"]["frozen_tests"],
                "platform": "mac",
                "target": "installed-binary",
                "require_archive": False,  # Should be rejected
            }
            (art / "build.json").write_text(json.dumps(build))

            evidence_dir = root / "evidence"
            r = subprocess.run(
                [sys.executable, str(DRIVER), "--source-sha", SHA,
                 "--manifest", str(manifest_path),
                 "--evidence-dir", str(evidence_dir),
                 "--artifact-dir", str(art),
                 "--platform", "mac", "--check-contract"],
                capture_output=True, text=True, timeout=30)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("require_archive", r.stderr)

    def test_workflow_exact_sha_env_checkout_upload_permissions(self):
        workflow = (ROOT / ".github/workflows/release-gates.yml").read_text()
        self.assertIn("ref: ${{ inputs.source_sha || github.sha }}", workflow)
        self.assertIn("SOURCE_SHA: ${{ inputs.source_sha || github.sha }}", workflow)
        jobs = workflow.split("jobs:", 1)[1]
        for job in ("mac-native:", "web-journey:", "ubuntu-tool-gates-contract:"):
            self.assertIn(job, jobs)
        self.assertGreaterEqual(workflow.count("ref: ${{ inputs.source_sha || github.sha }}"), 4)
        self.assertGreaterEqual(workflow.count('"$SOURCE_SHA"'), 6)
        self.assertNotRegex(workflow, r"run:.*\$\{\{\s*inputs\.")
        self.assertIn("contents: read", workflow)
        self.assertIn("actions: read", workflow)
        self.assertIn("if: always()", workflow)
        self.assertIn("name: mac-arm64-r1-receipts", workflow)
        self.assertIn("name: web-x64-r1-receipts", workflow)
        self.assertGreaterEqual(workflow.count("actions/upload-artifact@5d5d22a31266ced26887403a3e7a1b2a4a8c88a0"), 2)


if __name__ == "__main__":
    unittest.main()

"""Unit controls for custody.py: real temp hashing, path custody, fail-closed
result states. Standard-library unittest only; no subprocess/socket/PTY; no
real binary execution; no old-hash preimage is ever created."""

import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import custody
from custody import (
    REFUSED_SHA256,
    CaseResult,
    CustodyError,
    VerifiedArtifact,
    create_owned_output,
    guarded_run,
    sha256_file,
    validate_artifact,
    verify_hash_policy,
)

TMP_BASE = (
    "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/"
    "native-model-picker-custody-core-v1"
)


class CustodyTestCase(unittest.TestCase):
    def setUp(self):
        os.makedirs(TMP_BASE, exist_ok=True)
        self.tmp = tempfile.mkdtemp(prefix="t", dir=TMP_BASE)

    def tearDown(self):
        real = os.path.realpath(self.tmp)
        if real.startswith(os.path.realpath(TMP_BASE) + os.sep):
            shutil.rmtree(real, ignore_errors=True)

    # --- artifact custody -------------------------------------------------

    def _write(self, name, content):
        path = os.path.join(self.tmp, name)
        with open(path, "wb") as handle:
            handle.write(content)
        return path

    def _manifest(self, data):
        path = os.path.join(self.tmp, "manifest.json")
        import json

        with open(path, "w", encoding="utf-8") as handle:
            json.dump(data, handle)
        return path

    def test_valid_binary_hash_and_path_match(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        actual = sha256_file(binary)
        manifest = self._manifest({"binary": binary, "sha256": actual})
        artifact = validate_artifact(binary, manifest)
        self.assertIsInstance(artifact, VerifiedArtifact)
        self.assertEqual(artifact.path, os.path.abspath(binary))
        self.assertEqual(artifact.sha256, actual)
        self.assertEqual(artifact.argv_identity(), os.path.abspath(binary))
        self.assertEqual(len(artifact.sha256), 64)

    def test_manifest_sha_mismatch_refused(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        manifest = self._manifest(
            {"binary": binary, "sha256": "0" * 64}
        )
        with self.assertRaises(CustodyError):
            validate_artifact(binary, manifest)

    def test_manifest_different_binary_path_refused(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        other = self._write("other.bin", b"some-other-artifact")
        manifest = self._manifest(
            {"binary": other, "sha256": sha256_file(binary)}
        )
        with self.assertRaises(CustodyError):
            validate_artifact(binary, manifest)

    def test_missing_binary_refused(self):
        missing = os.path.join(self.tmp, "absent.bin")
        manifest = self._manifest({"sha256": "0" * 64})
        with self.assertRaises(CustodyError):
            validate_artifact(missing, manifest)

    def test_missing_manifest_refused(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        with self.assertRaises(CustodyError):
            validate_artifact(binary, os.path.join(self.tmp, "absent.json"))

    def test_manifest_without_sha_refused(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        manifest = self._manifest({"binary": binary})
        with self.assertRaises(CustodyError):
            validate_artifact(binary, manifest)

    def test_symlinked_binary_refused(self):
        binary = self._write("picker.bin", b"fixture-binary-v2")
        link = os.path.join(self.tmp, "link.bin")
        os.symlink(binary, link)
        manifest = self._manifest({"sha256": sha256_file(binary)})
        with self.assertRaises(CustodyError):
            validate_artifact(link, manifest)

    def test_refused_hash_denied_via_policy_helper(self):
        # No preimage of the refused immutable hash is ever created.
        with self.assertRaises(CustodyError):
            verify_hash_policy(REFUSED_SHA256)
        with self.assertRaises(CustodyError):
            verify_hash_policy("a" * 64, REFUSED_SHA256)
        with self.assertRaises(CustodyError):
            verify_hash_policy("A" * 64)  # uppercase not accepted
        with self.assertRaises(CustodyError):
            verify_hash_policy("xyz")
        good = "b" * 64
        self.assertEqual(verify_hash_policy(good, good), good)

    def test_missing_artifact_closes_unfinished_exit3(self):
        case = CaseResult("missing-artifact")

        def launch():
            return validate_artifact(
                os.path.join(self.tmp, "absent.bin"),
                os.path.join(self.tmp, "absent.json"),
            )

        result = guarded_run(case, launch)
        self.assertIsNone(result)
        self.assertEqual(case.state, "HARNESS_UNFINISHED")
        self.assertEqual(case.exit_code(), 3)

    # --- owned output -----------------------------------------------------

    def test_output_created_exclusively_and_contained(self):
        root = os.path.join(self.tmp, "out")
        os.mkdir(root)
        leaf = create_owned_output("run1/grid", root)
        self.assertTrue(os.path.isdir(leaf))
        self.assertEqual(os.path.realpath(leaf), os.path.join(root, "run1/grid"))
        # colliding leaf refused, exclusive, no exist_ok
        with self.assertRaises(CustodyError):
            create_owned_output("run1/grid", root)

    def test_output_parent_escape_refused(self):
        root = os.path.join(self.tmp, "out")
        os.mkdir(root)
        with self.assertRaises(CustodyError):
            create_owned_output("../escape", root)
        with self.assertRaises(CustodyError):
            create_owned_output("/tmp/absolute", root)
        with self.assertRaises(CustodyError):
            create_owned_output("a/../../b", root)
        # nothing was created outside the root
        self.assertFalse(os.path.exists(os.path.join(self.tmp, "escape")))

    def test_output_symlink_component_refused(self):
        root = os.path.join(self.tmp, "out")
        os.mkdir(root)
        outside = os.path.join(self.tmp, "outside")
        os.mkdir(outside)
        os.symlink(outside, os.path.join(root, "linkdir"))
        with self.assertRaises(CustodyError):
            create_owned_output("linkdir/case", root)
        self.assertFalse(os.path.exists(os.path.join(outside, "case")))

    def test_output_symlink_leaf_refused(self):
        root = os.path.join(self.tmp, "out")
        os.mkdir(root)
        target = os.path.join(self.tmp, "elsewhere")
        os.mkdir(target)
        os.symlink(target, os.path.join(root, "leaf"))
        with self.assertRaises(CustodyError):
            create_owned_output("leaf", root)

    def test_output_colliding_grant_root_refused(self):
        colliding = os.path.join(self.tmp, "not-a-dir")
        with open(colliding, "w", encoding="utf-8") as handle:
            handle.write("x")
        with self.assertRaises(CustodyError):
            create_owned_output("case", colliding)
        symlink_root = os.path.join(self.tmp, "rootlink")
        os.symlink(colliding, symlink_root)
        with self.assertRaises(CustodyError):
            create_owned_output("case", symlink_root)

    # --- fail-closed results ---------------------------------------------

    def test_completed_assertion_green_exit0(self):
        case = CaseResult("green")
        case.finish_green(True)
        self.assertEqual(case.state, "GREEN")
        self.assertEqual(case.exit_code(), 0)

    def test_exception_marks_unfinished_not_green(self):
        case = CaseResult("boom")

        def explode():
            raise ValueError("harness blew up")

        guarded_run(case, explode)
        self.assertEqual(case.state, "HARNESS_UNFINISHED")
        self.assertEqual(case.exit_code(), 3)
        self.assertTrue(any("ValueError" in r for r in case.reasons))
        with self.assertRaises(CustodyError):
            case.finish_green(True)

    def test_product_fail_red_exit1(self):
        case = CaseResult("product")
        case.finish_product_fail("picker row mismatch")
        self.assertEqual(case.state, "RED")
        self.assertEqual(case.exit_code(), 1)

    def test_unfinished_cannot_be_overwritten_green(self):
        case = CaseResult("unfinished")
        case.mark_unfinished("artifact went missing")
        with self.assertRaises(CustodyError):
            case.finish_green(True)
        self.assertEqual(case.exit_code(), 3)

    def test_repeated_finish_refused(self):
        case = CaseResult("repeat")
        case.finish_green(True)
        with self.assertRaises(CustodyError):
            case.finish_green(True)
        with self.assertRaises(CustodyError):
            case.mark_unfinished("late note")
        with self.assertRaises(CustodyError):
            case.finish_product_fail("late fail")

    def test_green_requires_explicit_completed_assertion(self):
        case = CaseResult("explicit")
        with self.assertRaises(CustodyError):
            case.finish_green(None)
        with self.assertRaises(CustodyError):
            case.finish_green("done")
        self.assertEqual(case.exit_code(), 3)

    def test_red_precedence_retains_unfinished_reason(self):
        case = CaseResult("mixed")
        case.finish_product_fail("assertion failed")
        case.mark_unfinished("cleanup raised")
        self.assertEqual(case.state, "RED")
        self.assertEqual(case.exit_code(), 1)
        self.assertTrue(any("assertion failed" in r for r in case.reasons))
        self.assertTrue(any("cleanup raised" in r for r in case.reasons))
        with self.assertRaises(CustodyError):
            case.finish_green(True)

    def test_module_import_has_no_launch_side_effects(self):
        self.assertFalse(hasattr(custody, "BIN"))
        self.assertFalse(hasattr(custody, "OUT"))
        self.assertTrue(callable(custody.validate_artifact))
        self.assertTrue(callable(custody.create_owned_output))


if __name__ == "__main__":
    unittest.main(verbosity=2)

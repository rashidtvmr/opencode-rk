import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.coverage_gate import manifest_identity_errors
from tools.source_lock import build_frozen_manifest, checkout_identity_errors, validate_lock, validate_spec


class SourceAuditTests(unittest.TestCase):
    def base_spec(self):
        return {
            "id": "oc",
            "url": "https://github.com/example/repo.git",
            "commit": "a" * 40,
            "treeSha": "b" * 40,
            "licensePath": "LICENSE",
            "licenseBlobSha": "c" * 40,
            "licenseSpdx": "MIT",
        }

    def test_lock_rejects_duplicate_ids_and_credential_urls(self):
        spec = self.base_spec()
        with self.assertRaises(ValueError):
            validate_lock({"schemaVersion": 1, "repositories": [spec, dict(spec)]})
        with self.assertRaises(ValueError):
            validate_spec(dict(spec, url="https://user:token@github.com/example/repo.git"))

    def test_checkout_identity_blocks_changed_attached_or_incomplete_state(self):
        spec = self.base_spec()
        record = {
            "id": "oc",
            "commit": "d" * 40,
            "treeSha": spec["treeSha"],
            "originUrl": spec["url"],
            "branch": "main",
            "dirty": False,
            "licensePath": "LICENSE",
            "licenseBlobSha": spec["licenseBlobSha"],
            "licenseSpdx": "MIT",
            "connectivityVerified": False,
        }
        errors = checkout_identity_errors(record, spec)
        self.assertTrue(any("commit mismatch" in error for error in errors))
        self.assertTrue(any("attached" in error for error in errors))
        self.assertTrue(any("connectivity" in error for error in errors))

    def test_frozen_manifest_drops_local_or_secret_fields(self):
        spec = self.base_spec()
        record = {
            "id": "oc",
            "commit": spec["commit"],
            "treeSha": spec["treeSha"],
            "originUrl": spec["url"],
            "licensePath": "LICENSE",
            "licenseBlobSha": spec["licenseBlobSha"],
            "licenseSpdx": "MIT",
            "connectivityVerified": True,
            "branch": "",
            "dirty": False,
            "localPath": "/home/user/private",
            "token": "secret",
        }
        text = repr(build_frozen_manifest([record]))
        self.assertNotIn("/home/user", text)
        self.assertNotIn("secret", text)

    def test_inventory_manifest_must_match_lock_and_verified_checkout(self):
        spec = self.base_spec()
        lock = {"schemaVersion": 1, "repositories": [spec]}
        record = {
            "repository": "oc",
            "commit": spec["commit"],
            "tree": spec["treeSha"],
            "originUrl": spec["url"],
            "licensePath": "LICENSE",
            "licenseBlobSha": spec["licenseBlobSha"],
            "connectivityVerified": True,
            "detachedHead": True,
            "cleanWorktree": True,
        }
        self.assertEqual(manifest_identity_errors({"schemaVersion": 2, "repositories": [record]}, lock), [])
        stale = dict(record, tree="e" * 40, detachedHead=False)
        errors = manifest_identity_errors({"schemaVersion": 2, "repositories": [stale]}, lock)
        self.assertTrue(any("tree" in error for error in errors))
        self.assertTrue(any("detached" in error for error in errors))


if __name__ == "__main__":
    unittest.main()

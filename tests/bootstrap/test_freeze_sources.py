import importlib.util
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("freeze_sources", ROOT / "tools" / "freeze_sources.py")
freeze = importlib.util.module_from_spec(spec)
assert spec.loader
spec.loader.exec_module(freeze)


class FreezeSourceTests(unittest.TestCase):
    def test_git_environment_drops_credentials_and_prompts(self):
        env = freeze.sanitized_git_env(
            {
                "PATH": "/bin",
                "LANG": "C.UTF-8",
                "GITHUB_TOKEN": "secret",
                "GH_TOKEN": "secret2",
                "AWS_SECRET_ACCESS_KEY": "secret3",
                "HOME": "/home/user",
            }
        )
        self.assertEqual(env["PATH"], "/bin")
        self.assertEqual(env["GIT_TERMINAL_PROMPT"], "0")
        self.assertNotIn("GITHUB_TOKEN", env)
        self.assertNotIn("GH_TOKEN", env)
        self.assertNotIn("AWS_SECRET_ACCESS_KEY", env)
        self.assertNotIn("HOME", env)

    def test_fetch_retries_are_bounded(self):
        calls = []

        def always_fail(*args, **kwargs):
            calls.append((args, kwargs))
            raise subprocess.CalledProcessError(1, args[0])

        with self.assertRaises(subprocess.CalledProcessError):
            freeze.run_bounded(["git", "fetch"], attempts=2, runner=always_fail, sleeper=lambda _: None, env={})
        self.assertEqual(len(calls), 2)

    def test_invalid_commit_tree_or_license_pin_is_rejected(self):
        base = {
            "id": "x",
            "url": "https://github.com/example/repo.git",
            "commit": "a" * 40,
            "treeSha": "b" * 40,
            "licensePath": "LICENSE",
            "licenseBlobSha": "c" * 40,
            "licenseSpdx": "MIT",
        }
        freeze.validate_spec(base)
        for field in ("commit", "treeSha", "licenseBlobSha"):
            bad = dict(base)
            bad[field] = "main"
            with self.assertRaises(ValueError):
                freeze.validate_spec(bad)

    def test_non_github_or_traversing_license_reference_is_rejected(self):
        base = {
            "id": "x",
            "url": "https://github.com/example/repo.git",
            "commit": "a" * 40,
            "treeSha": "b" * 40,
            "licensePath": "LICENSE",
            "licenseBlobSha": "c" * 40,
            "licenseSpdx": "MIT",
        }
        bad_url = dict(base, url="https://evil.invalid/repo.git")
        with self.assertRaises(ValueError):
            freeze.validate_spec(bad_url)
        bad_path = dict(base, licensePath="../LICENSE")
        with self.assertRaises(ValueError):
            freeze.validate_spec(bad_path)


if __name__ == "__main__":
    unittest.main()

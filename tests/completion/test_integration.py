"""RED: serialized integration before acceptance (COORD-005, delivery.json row).

Fails against current tools/completion_integration.py:
1. distinct lock paths do not serialize across processes;
2. write_receipt accepts fabricated revs with no repo ancestry;
3. bare {"passed": True} accepted with no merged-revision binding.
"""
import multiprocessing
import pathlib
import subprocess
import tempfile
import unittest

from tools import completion_integration as m


def git(repo, *args):
    p = subprocess.run(["git", *args], cwd=repo, capture_output=True,
                       text=True, timeout=10)
    assert p.returncode == 0, p.stderr[:200]
    return p.stdout.strip()


def hold_lock(path, ready, hold):
    with m.IntegrationLock(path, timeout=10):
        ready.set()
        hold.wait(timeout=10)


class SerializedIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-q")
        git(self.repo, "branch", "-M", "main")
        git(self.repo, "-c", "user.name=t", "-c", "user.email=t@t",
            "commit", "-q", "--allow-empty", "-m", "root")

    def tearDown(self):
        self.tmp.cleanup()

    def test_parallel_integration_refused(self):
        ctx = multiprocessing.get_context("fork")
        ready, hold = ctx.Event(), ctx.Event()
        p = ctx.Process(target=hold_lock,
                        args=(str(self.root / "a.lock"), ready, hold))
        p.start()
        try:
            self.assertTrue(ready.wait(timeout=10))
            with self.assertRaises(m.RetryableFailure):
                with m.IntegrationLock(str(self.root / "b.lock"), timeout=1):
                    pass
        finally:
            hold.set()
            p.join(timeout=10)

    def test_acceptance_before_integration_rejected(self):
        box = self.root / "box"
        with self.assertRaises(m.Rejected):
            m.write_receipt(box, task_id="COORD-005",
                            candidate_revision="a" * 40,
                            integrated_revision="b" * 40,
                            frozen_tests_sha256="c" * 64,
                            rerun={"passed": True})
        self.assertFalse((box / "COORD-005.json").exists())

    def test_merged_tree_retest_required(self):
        box = self.root / "box"
        rev = git(self.repo, "rev-parse", "HEAD")
        with self.assertRaises(m.Rejected):
            m.write_receipt(box, task_id="COORD-005",
                            candidate_revision=rev,
                            integrated_revision=rev,
                            frozen_tests_sha256="c" * 64,
                            rerun={"passed": True})
        self.assertFalse((box / "COORD-005.json").exists())


if __name__ == "__main__":
    unittest.main()

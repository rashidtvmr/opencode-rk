#!/usr/bin/env python3
"""Repository regression for tools/convergence_v2_salvage.py preservation behavior.

Independent test-owner artifact (package: V2-SALVAGE-PRESERVATION). Frozen from a
standalone RED script; assertion intent B1-B8 is preserved.

The tool under test is imported from:
  * OC2_SALVAGE_SOURCE when set (used to load a preserved pre-fix revision for RED), or
  * <repo>/tools/convergence_v2_salvage.py by default (GREEN).

Disposable fixtures live in a TemporaryDirectory under OC2_SALVAGE_TMP when set,
otherwise the platform default temp. No workstation paths are hardcoded; tests do
not mutate the candidate source, docs, worklog or any existing snapshot.

Run:
  python3 -m unittest discover -s tests/bootstrap \
      -p 'test_convergence_v2_preservation.py' -v
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = REPO_ROOT / "tools" / "convergence_v2_salvage.py"
SOURCE = pathlib.Path(
    os.environ.get("OC2_SALVAGE_SOURCE", str(DEFAULT_SOURCE))
).resolve()

TMP_BASE = os.environ.get("OC2_SALVAGE_TMP") or None

GIT_ENV = {
    "GIT_AUTHOR_NAME": "oc2-test",
    "GIT_AUTHOR_EMAIL": "oc2-test@example.invalid",
    "GIT_COMMITTER_NAME": "oc2-test",
    "GIT_COMMITTER_EMAIL": "oc2-test@example.invalid",
}


def _load_module(path: pathlib.Path):
    spec = importlib.util.spec_from_file_location("salvage_under_test", str(path))
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load module from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SALVAGE = _load_module(SOURCE)


def _git(*args, cwd):
    env = dict(os.environ)
    env.update(GIT_ENV)
    return subprocess.run(
        ["git", *args], cwd=str(cwd), env=env,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
    )


def _snapshot_contains(root: pathlib.Path, needle: str):
    hits = []
    for path in root.rglob("*"):
        if path.is_file():
            try:
                if needle in path.read_text(errors="replace"):
                    hits.append(str(path.relative_to(root)))
            except OSError:
                pass
    return hits


class SalvagePreservationTest(unittest.TestCase):
    def setUp(self):
        self._tmpdir = tempfile.TemporaryDirectory(
            prefix="oc2-salvage-test-", dir=TMP_BASE
        )
        self.tmp = pathlib.Path(self._tmpdir.name)

    def tearDown(self):
        self._tmpdir.cleanup()

    def _repo(self, name):
        repo = self.tmp / name
        repo.mkdir(parents=True, exist_ok=True)
        _git("init", "-q", "-b", "main", cwd=repo)
        return repo

    @staticmethod
    def _empty_report():
        return {"gaps": [], "sensitiveUntouched": [], "overBudget": []}

    @staticmethod
    def _g0(report):
        return "pending" if (report["gaps"] or report["sensitiveUntouched"]) else "clean"

    # B1: repeat snapshot must refuse, never clobber or clear existing data.
    def test_b1_repeat_snapshot_no_clobber(self):
        repo = self._repo("b1")
        (repo / "README.md").write_text("r\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        snap = self.tmp / "b1-snap"
        (repo / "README.md").write_text("r\nV1\n")
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, self._empty_report())
        sub = next(snap.iterdir(), None)
        self.assertIsNotNone(sub)
        v1_hash = hashlib.sha256((sub / "unstaged.patch").read_bytes()).hexdigest()
        _git("checkout", "--", "README.md", cwd=repo)
        (repo / "README.md").write_text("r\nV2\n")
        rep2 = self._empty_report()
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, rep2)
        sub2 = next(snap.iterdir(), None)
        v2_hash = hashlib.sha256((sub2 / "unstaged.patch").read_bytes()).hexdigest()
        self.assertEqual(v2_hash, v1_hash,
                         "existing snapshot artifact was overwritten/cleared")
        self.assertEqual(self._g0(rep2), "pending",
                         f"refusal gap missing: {rep2['gaps']}")

    # B2: status failure must record a gap, never silent clean.
    def test_b2_status_failure_gap(self):
        repo = self._repo("b2")
        (repo / "a.txt").write_text("a\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        (repo / ".git").rename(repo / ".git-gone")
        meta = SALVAGE.worktree_dirty(repo)
        self.assertIsNone(meta.get("dirty"))
        self.assertTrue(meta.get("statusError"))
        rep = self._empty_report()
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": meta.get("dirty"),
              "head": "x", "branch": "main"}], self.tmp / "b2-snap", rep)
        self.assertEqual(self._g0(rep), "pending", f"gap missing: {rep['gaps']}")

    # B3: R100 rename from a sensitive path must not leak any content.
    def test_b3_r100_sensitive_rename_no_leak(self):
        repo = self._repo("b3")
        (repo / "README.md").write_text("r\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        (repo / ".env").write_text("TOKEN=orig\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c1", cwd=repo)
        _git("mv", ".env", "env.txt", cwd=repo)
        (repo / "env.txt").write_text("TOKEN=orig\nTOKEN=LEAKED_X\n")
        snap = self.tmp / "b3-snap"
        rep = self._empty_report()
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, rep)
        self.assertEqual(_snapshot_contains(snap, "LEAKED_X"), [])
        self.assertEqual(_snapshot_contains(snap, "TOKEN=orig"), [])
        self.assertEqual(self._g0(rep), "pending")

    # B4: staged-only tracked change must be captured.
    def test_b4_staged_only_captured(self):
        repo = self._repo("b4")
        (repo / "README.md").write_text("r\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        (repo / "staged_only.rs").write_text("fn main(){}\n")
        _git("add", "staged_only.rs", cwd=repo)
        meta = SALVAGE.worktree_dirty(repo)
        self.assertTrue(meta.get("dirty"))
        self.assertGreaterEqual(meta.get("stagedChanges", 0), 1)
        snap = self.tmp / "b4-snap"
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, self._empty_report())
        sub = next(snap.iterdir(), None)
        self.assertIsNotNone(sub)
        self.assertTrue((sub / "staged.patch").exists())
        self.assertIn("staged_only.rs",
                      (sub / "staged.patch").read_text(errors="replace"))

    # B5: observed HEAD must be stripped of trailing whitespace.
    def test_b5_meta_head_stripped(self):
        repo = self._repo("b5")
        (repo / "README.md").write_text("r\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        (repo / "README.md").write_text("r\nmod\n")
        snap = self.tmp / "b5-snap"
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, self._empty_report())
        sub = next(snap.iterdir(), None)
        head = json.loads((sub / "meta.json").read_text())["head"]
        self.assertTrue(head)
        self.assertFalse(head.endswith("\n") or head.endswith("\r"))

    # B6: CLI --preserve-dirty without --snapshot-dir must fail closed.
    def test_b6_cli_preserve_without_dir_fails(self):
        frozen = self.tmp / "b6-frozen"
        frozen.mkdir()
        for name in ("refs-before.txt", "worktrees-before.txt",
                     "detached-worktrees.tsv", "archive-refs.txt"):
            (frozen / name).write_text("")
        proc = subprocess.run(
            [sys.executable, str(SOURCE), "--preserve-dirty",
             "--base-sha", "0" * 40, "--frozen-root", str(frozen),
             "--json", str(self.tmp / "b6.json"),
             "--markdown", str(self.tmp / "b6.md"), "--no-dirty"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
            cwd=str(REPO_ROOT))
        self.assertNotEqual(proc.returncode, 0)

    # B7: symlink preserved as raw readlink metadata, never dereferenced.
    def test_b7_symlink_readlink_metadata(self):
        repo = self._repo("b7")
        (repo / "README.md").write_text("r\n")
        _git("add", "-A", cwd=repo)
        _git("commit", "-qm", "c0", cwd=repo)
        target = self.tmp / "b7-target.txt"
        target.write_text("TARGET_SECRET_MARKER\n")
        link = repo / "link"
        link.symlink_to(target)
        snap = self.tmp / "b7-snap"
        SALVAGE.preserve_dirty(
            [{"path": str(repo), "exists": True, "dirty": True,
              "head": "x", "branch": "main"}], snap, self._empty_report())
        sub = next(snap.iterdir(), None)
        self.assertIsNotNone(sub)
        links_path = sub / "links.json"
        self.assertTrue(links_path.exists(), "links.json metadata missing")
        entries = json.loads(links_path.read_text())
        entry = next((e for e in entries if e.get("path") == "link"), None)
        self.assertIsNotNone(entry, f"link not recorded: {entries}")
        self.assertEqual(entry.get("target"), os.readlink(link))
        self.assertEqual(_snapshot_contains(snap, "TARGET_SECRET_MARKER"), [])

    # B8: bounded subprocess output and timeout kill/wait.
    def test_b8_run_bounded(self):
        start = time.monotonic()
        timed = SALVAGE.run(["sleep", "5"], timeout=1)
        elapsed = time.monotonic() - start
        self.assertIsNone(timed)
        self.assertLess(elapsed, 4)
        big = SALVAGE.run(["sh", "-c", "yes X | head -c 90000000"], timeout=20)
        if big is not None:
            self.assertLessEqual(len(big.stdout), SALVAGE.MAX_TOTAL_BYTES)


if __name__ == "__main__":
    unittest.main()
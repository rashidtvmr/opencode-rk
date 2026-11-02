"""Retention-guard tests for the automatic workspace collector (W35-RETENTION).

``tools/delivery/cleanup.py`` used to ``shutil.rmtree`` owned execution
workspaces after only a candidate-patch archive. Policy (AGENTS.md
worktree-lifecycle, docs/AGENT_STRATEGY_V2.md section 8) requires proof that
the workspace tip is merged into main-v2 AND present on the push remote AND
that the workspace has no uncommitted/untracked/ignored content. These tests
prove:

- the default path is report-only (lists what it would retire, deletes nothing);
- a merged+pushed+clean workspace is retired on the apply path;
- a dirty workspace (untracked or ignored content) is retained;
- an unmerged workspace is retained;
- a merged-but-unpushed workspace is retained.
"""
import json
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.delivery import cleanup
from tools.delivery.store import Store


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=str(root), text=True,
                                   stderr=subprocess.STDOUT).strip()


class RetentionGuardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="retention-test-")
        self.root = pathlib.Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-q", "-b", "main-v2")
        git(self.repo, "config", "user.name", "Fixture")
        git(self.repo, "config", "user.email", "fixture@localhost.invalid")
        (self.repo / "README.md").write_text("initial\n")
        git(self.repo, "add", ".")
        git(self.repo, "commit", "-qm", "initial")
        self.base = git(self.repo, "rev-parse", "HEAD")
        self.remote = self.root / "push-remote.git"
        git(self.root, "init", "-q", "--bare", str(self.remote))
        git(self.repo, "config", "remote.origin.url", str(self.remote))
        git(self.repo, "config", "remote.origin.pushurl", str(self.remote))
        self.store = Store(self.root / "state")
        self.store.meta("repo", str(self.repo))
        self.store.meta("config", {"branch": "main-v2"})
        self.workspaces = self.store.root / "workspaces"
        self.workspaces.mkdir(exist_ok=True)
        self.counter = 0

    def tearDown(self):
        self.temp.cleanup()

    def workspace(self):
        """A detached owned workspace like jobs.create_workspace, plus a
        consumed job whose owner token matches, so collect() considers it."""
        self.counter += 1
        name = "%032x" % self.counter
        ws = self.workspaces / name
        ws.mkdir()
        git(ws, "init", "-q", "--template=")
        git(ws, "config", "user.name", "Fixture")
        git(ws, "config", "user.email", "fixture@localhost.invalid")
        git(ws, "fetch", "-q", "--no-tags", str(self.repo), "main-v2")
        git(ws, "checkout", "-q", "--detach", "FETCH_HEAD")
        token = "token-" + name
        (ws / ".git" / "delivery-owner.json").write_text(
            json.dumps({"job": name, "token": token}))
        tid = "task-" + name
        now = time.time()
        with self.store.connection(write=True) as cx:
            cx.execute("INSERT INTO tasks(id,spec,spec_hash,state,stage,created,updated)"
                       " VALUES(?,?,?,?,?,?,?)",
                       (tid, json.dumps({"id": tid}), "spec-" + name, "accepted",
                        "implement", now, now))
            cx.execute("INSERT INTO jobs(id,task,stage,token,state,payload,result,"
                       "lease_until,created,updated) VALUES(?,?,?,?,?,?,?,?,?,?)",
                       (name, tid, "implement", token, "consumed",
                        json.dumps({"workspace": str(ws)}), None, now, now, now))
        return ws

    def candidate(self, ws, label):
        (ws / ("feature-%s.txt" % label)).write_text(label + "\n")
        git(ws, "add", ".")
        git(ws, "commit", "-qm", "candidate " + label)
        return git(ws, "rev-parse", "HEAD")

    def record_result(self, ws, tip):
        with self.store.connection(write=True) as cx:
            cx.execute("UPDATE jobs SET result=? WHERE id=?",
                       (json.dumps({"kind": "candidate", "base": self.base,
                                    "candidate": tip, "workspace": str(ws),
                                    "changed_paths": ["feature.txt"]}), ws.name))

    def merge_to_main(self, ws, tip):
        git(self.repo, "fetch", "-q", "--no-tags", str(ws), tip)
        git(self.repo, "merge", "--ff-only", tip)

    def push(self):
        git(self.repo, "push", "-q", "origin", "main-v2")

    def merged_pushed(self, ws, label):
        tip = self.candidate(ws, label)
        self.merge_to_main(ws, tip)
        self.push()
        self.record_result(ws, tip)
        return tip

    def test_report_only_default_lists_would_retire_without_deleting(self):
        ws = self.workspace()
        self.merged_pushed(ws, "merged")
        records = cleanup.collect(self.store)  # apply defaults to False
        self.assertTrue(ws.is_dir())
        self.assertEqual([r["decision"] for r in records], ["retire"])
        self.assertIn("push remote", records[0]["reason"])

    def test_merged_pushed_clean_is_retired_on_apply(self):
        ws = self.workspace()
        tip = self.merged_pushed(ws, "merged")
        records = cleanup.collect(self.store, apply=True)
        self.assertFalse(ws.exists())
        self.assertEqual(records[0]["decision"], "retire")
        self.assertEqual(records[0]["tip"], tip)
        patch = self.store.root / "evidence" / ws.name / "candidate.patch"
        self.assertTrue(patch.is_file())
        self.assertGreater(patch.stat().st_size, 0)
        with self.store.connection() as cx:
            kinds = [row[0] for row in cx.execute("SELECT kind FROM events")]
        self.assertIn("workspace.retired", kinds)

    def test_dirty_workspace_is_retained(self):
        for mode in ("untracked", "ignored"):
            with self.subTest(mode=mode):
                ws = self.workspace()
                self.merged_pushed(ws, "dirty-" + mode)
                if mode == "untracked":
                    (ws / "notes.txt").write_text("unsaved work\n")
                else:
                    info = ws / ".git" / "info"
                    info.mkdir(exist_ok=True)
                    (info / "exclude").write_text("cache/\n")
                    (ws / "cache").mkdir()
                    (ws / "cache" / "artifact.bin").write_text("build output\n")
                records = cleanup.collect(self.store, apply=True)
                by_name = {r["name"]: r for r in records}
                self.assertTrue(ws.is_dir())
                self.assertEqual(by_name[ws.name]["decision"], "retain")
                self.assertIn("not clean", by_name[ws.name]["reason"])

    def test_unmerged_workspace_is_retained(self):
        ws = self.workspace()
        tip = self.candidate(ws, "unmerged")
        self.record_result(ws, tip)  # never merged into main-v2
        records = cleanup.collect(self.store, apply=True)
        self.assertTrue(ws.is_dir())
        self.assertEqual(records[0]["decision"], "retain")
        self.assertIn("not merged", records[0]["reason"])

    def test_merged_but_unpushed_workspace_is_retained(self):
        ws = self.workspace()
        tip = self.candidate(ws, "unpushed")
        self.merge_to_main(ws, tip)  # merged locally; never pushed
        self.record_result(ws, tip)
        records = cleanup.collect(self.store, apply=True)
        self.assertTrue(ws.is_dir())
        self.assertEqual(records[0]["decision"], "retain")
        self.assertIn("absent from the push remote", records[0]["reason"])

    def test_report_path_is_throttled_between_polls(self):
        # The supervisor calls the report-only path every poll tick; it must not
        # re-run the per-workspace git probes on every tick.
        ws = self.workspace()
        self.merged_pushed(ws, "throttle")
        first = cleanup.collect(self.store)
        self.assertEqual([r["decision"] for r in first], ["retire"])
        (ws / "dirty.txt").write_text("unsaved\n")  # change within the interval
        second = cleanup.collect(self.store)
        self.assertEqual([r["decision"] for r in second], ["retire"],
                         "report path re-evaluated inside the throttle interval")
        # The apply path is never throttled: it re-evaluates the exact bytes.
        cleanup._report_cache.clear()
        third = cleanup.collect(self.store)
        self.assertEqual([r["decision"] for r in third], ["retain"])
        self.assertIn("not clean", third[0]["reason"])


if __name__ == "__main__":
    unittest.main()

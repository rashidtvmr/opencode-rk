"""Retries must not inherit a rejected candidate's branch or canonical index."""
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2]))

from tools.delivery.jobs import Git, _select_feature_ref
from tools.delivery.store import Store


class AuthorWorkspaceBaseTests(unittest.TestCase):
    def make_repo(self, root):
        repo = root / "repo"
        repo.mkdir()
        git = Git(repo)
        git.run("init", "--quiet", "--template=")
        (repo / "base.txt").write_text("base")
        git.run("add", "base.txt")
        git.run("commit", "--quiet", "-m", "base")
        return repo, git.head()

    def test_missing_path_with_existing_ref_is_not_an_unused_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            repo, base = self.make_repo(root)
            git = Git(repo)
            git.run("branch", "recovery/feature-01", base)
            store = Store(root / "state")
            store.meta("repo", str(repo))
            store.meta("config", {"workspaces": {
                "root": str(root / "workspaces"),
                "feature_refs": ["refs/heads/recovery/feature-01", "refs/heads/recovery/feature-02"],
            }})
            ref, path = _select_feature_ref(store, "T-1")
            self.assertEqual(ref, "refs/heads/recovery/feature-02")
            self.assertFalse(path.exists())

    def test_dirty_owned_workspace_is_preserved_and_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            repo, base = self.make_repo(root)
            git = Git(repo)
            wsroot = root / "workspaces"
            wsroot.mkdir()
            owned = wsroot / "author-feature-01"
            git.run("worktree", "add", "-b", "recovery/feature-01", str(owned), base)
            (owned / ".delivery-owner.json").write_text(
                '{"task":"T-1","feature_ref":"refs/heads/recovery/feature-01"}'
            )
            (owned / "outside.txt").write_text("preserve me")
            store = Store(root / "state")
            store.meta("repo", str(repo))
            store.meta("config", {"workspaces": {
                "root": str(wsroot),
                "feature_refs": ["refs/heads/recovery/feature-01", "refs/heads/recovery/feature-02"],
            }})
            ref, path = _select_feature_ref(store, "T-1", ["src/*"])
            self.assertEqual(ref, "refs/heads/recovery/feature-02")
            self.assertFalse(path.exists())
            self.assertEqual((owned / "outside.txt").read_text(), "preserve me")

    def test_read_tree_reset_seeds_private_index_and_worktree_from_base(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            repo, base = self.make_repo(root)
            git = Git(repo)
            # Simulate the canonical index carrying a different tree.
            (repo / "base.txt").write_text("canonical-index")
            git.run("add", "base.txt")
            tree = root / "wt"
            git.run("worktree", "add", "-b", "recovery/feature-02", str(tree), base)
            private = Git(tree)
            private.run("read-tree", "--reset", "-u", base)
            self.assertEqual(private.head(), base)
            self.assertEqual((tree / "base.txt").read_text(), "base")
            self.assertEqual(private.run("status", "--porcelain=v1", "--untracked-files=all"), "")


if __name__ == "__main__":
    unittest.main()

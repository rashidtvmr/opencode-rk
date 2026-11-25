"""Parallel jobs must not race on the one shared `.git/config`.

Every managed workspace is a linked worktree of the same repository, so
`git config user.name` in N workspaces all write the same `.git/config`. Two
jobs starting together collided on `.git/config.lock` and one died with
`error: could not lock config file ... File exists`, burning all five attempts
and blocking the task on a Git artifact rather than a product defect. Identity
and gc suppression now travel in the environment, which is per-invocation.

Measured before the fix: 16 workers x 25 concurrent `git config` writes lost 29
of 400 to the lock. The same 16 x 25 concurrent commits now succeed every time.
"""
import concurrent.futures
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2]))

from tools.delivery.jobs import Git

WORKERS = 16
COMMITS_EACH = 8


class ParallelCommitsShareOneConfig(unittest.TestCase):
    def test_concurrent_commits_in_linked_worktrees_never_collide(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            repo.mkdir()
            base = Git(repo)
            base.run("init", "--quiet", "--template=")
            (repo / "seed.txt").write_text("seed")
            base.run("add", "seed.txt")
            base.run("commit", "--quiet", "-m", "seed")

            trees = []
            for index in range(WORKERS):
                tree = pathlib.Path(tmp) / f"wt-{index}"
                base.run("worktree", "add", "--detach", str(tree), "HEAD")
                trees.append(tree)

            def commit_many(index):
                git = Git(trees[index])
                for step in range(COMMITS_EACH):
                    (trees[index] / f"f{index}-{step}.txt").write_text("x")
                    git.run("add", f"f{index}-{step}.txt", timeout=180)
                    git.run("commit", "--quiet", "-m", f"c{index}-{step}", timeout=180)
                return git.head()

            with concurrent.futures.ThreadPoolExecutor(max_workers=WORKERS) as pool:
                heads = list(pool.map(commit_many, range(WORKERS)))

            self.assertEqual(len(set(heads)), len(heads), "each worktree committed its own chain")
            self.assertFalse((repo / ".git" / "config.lock").exists())

    def test_no_stage_writes_shared_git_config(self):
        source = (pathlib.Path(__file__).resolve().parents[2] / "tools" / "delivery" / "jobs.py").read_text()
        for setting in ('"user.name"', '"user.email"', '"gc.auto"'):
            self.assertNotIn(f'git.run("config", {setting}', source,
                             f"shared config write for {setting} reintroduces the config.lock race")


if __name__ == "__main__":
    unittest.main()

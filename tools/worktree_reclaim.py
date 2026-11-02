#!/usr/bin/env python3
"""Reclaim disk from worktrees that are BOTH merged and pushed. (dry-run default)

Implements the user's explicit rule:

    "donot let any work go waste so, once you are sure its merged with our
     main-v2 and its pushed to remote repo, then only delete it"

A worktree is reclaimable only when every one of these is individually verified:

1. its tip commit is an ancestor of the current local `main-v2`;
2. that work is present on the **push** remote, established either by the exact
   tip sha appearing as a remote head, or by the tip being an ancestor of the
   remote `main-v2` tip (which is the real anti-loss guarantee, because the
   commit is then reachable from a published ref);
3. the worktree is clean (no staged, unstaged or untracked work);
4. the worktree is not locked, and is not the canonical checkout;
5. the worktree is not itself a configured remote URL (deleting the `origin`
   fetch path would break the repository).

`refs/remotes/origin/*` is deliberately NOT used as evidence: this repository
fetches `origin` from a local path while pushing to GitHub, so the local
tracking refs can be arbitrarily stale. Verification always goes to the push
remote via `git ls-remote`.

Branch refs are pruned only when Git's own safe delete succeeds. The Recovery
Mode reference guard may refuse the ref deletion; that is reported and the ref
is left untouched. The worktree removal (the part that actually frees disk) is
independent of that.

Usage:
    python3 tools/worktree_reclaim.py              # report only
    python3 tools/worktree_reclaim.py --apply      # remove verified worktrees
    python3 tools/worktree_reclaim.py --json       # machine-readable report
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

CANONICAL = "main-v2"


def git(repo: Path, args: list[str], timeout: float = 60.0) -> tuple[int, str, str]:
    env = os.environ.copy()
    env["GIT_OPTIONAL_LOCKS"] = "0"
    try:
        proc = subprocess.run(
            ["git", *args], cwd=str(repo), text=True, env=env,
            capture_output=True, timeout=timeout,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except (OSError, subprocess.SubprocessError) as exc:
        return 1, "", str(exc)


def rev(repo: Path, ref: str) -> str | None:
    code, out, _ = git(repo, ["rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"])
    value = out.strip()
    return value if code == 0 and len(value) == 40 else None


def push_remote(repo: Path) -> str | None:
    code, out, _ = git(repo, ["config", "--get", "remote.origin.pushurl"])
    return out.strip() or None if code == 0 else None


def remote_state(repo: Path, remote: str, timeout: float = 60.0) -> tuple[dict[str, str], str | None]:
    """Return ({sha: refname}, remote main-v2 sha) from the push remote."""
    env = os.environ.copy()
    env.setdefault("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10")
    try:
        proc = subprocess.run(
            ["git", "ls-remote", "--heads", remote], cwd=str(repo), text=True,
            env=env, capture_output=True, timeout=timeout,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise SystemExit(f"cannot reach push remote {remote}: {exc}")
    if proc.returncode != 0:
        raise SystemExit(f"ls-remote failed for {remote}: {proc.stderr.strip()}")
    heads: dict[str, str] = {}
    canonical: str | None = None
    for line in proc.stdout.splitlines():
        parts = line.split()
        if len(parts) != 2:
            continue
        sha, ref = parts
        heads[sha] = ref
        if ref == f"refs/heads/{CANONICAL}":
            canonical = sha
    return heads, canonical


def worktrees(repo: Path) -> list[dict[str, str]]:
    code, out, err = git(repo, ["worktree", "list", "--porcelain"])
    if code != 0:
        raise SystemExit(f"worktree list failed: {err.strip()}")
    entries: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for line in out.splitlines():
        if not line:
            if current:
                entries.append(current)
            current = {}
            continue
        key, _, value = line.partition(" ")
        current[key] = value
    if current:
        entries.append(current)
    return entries


def dir_bytes(path: Path) -> int:
    total = 0
    for root, _dirs, files in os.walk(path, onerror=lambda _e: None):
        for name in files:
            try:
                st = os.lstat(os.path.join(root, name))
            except OSError:
                continue
            total += st.st_blocks * 512
    return total


def human(n: int) -> str:
    value = float(n)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if value < 1024 or unit == "TiB":
            return f"{value:.1f}{unit}" if unit != "B" else f"{int(value)}B"
        value /= 1024
    return f"{n}B"


def is_dirty(path: Path) -> tuple[bool, str]:
    code, out, err = git(path, ["status", "--porcelain", "--untracked-files=normal"])
    if code != 0:
        return True, f"status failed: {err.strip()}"
    if out.strip():
        return True, f"{len(out.strip().splitlines())} dirty/untracked entr(y|ies)"
    return False, "clean"


def local_remote_paths(repo: Path) -> set[str]:
    """Local filesystem paths configured as remotes; never reclaim these."""
    paths: set[str] = set()
    code, out, _ = git(repo, ["config", "--get-regexp", r"^remote\..*\.url$"])
    if code != 0:
        return paths
    for line in out.splitlines():
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        value = parts[1].strip()
        if value.startswith(("/", ".")):
            paths.add(str(Path(value).resolve()))
    return paths


def classify(repo: Path, entry: dict[str, str], *, canonical_path: Path,
             main_sha: str, heads: dict[str, str], remote_main: str | None,
             protected_paths: set[str]) -> dict[str, object]:
    path = Path(entry.get("worktree", "")).resolve()
    head = entry.get("HEAD", "")
    branch = entry.get("branch", "(detached)").removeprefix("refs/heads/")
    record: dict[str, object] = {
        "path": str(path), "head": head, "branch": branch,
        "bytes": dir_bytes(path) if path.is_dir() else 0,
        "reclaimable": False, "reason": "",
    }

    if path == canonical_path:
        record["reason"] = "canonical checkout"
        return record
    if str(path) in protected_paths:
        record["reason"] = "configured as a remote URL (deleting it would break the repo)"
        return record
    if len(head) != 40:
        record["reason"] = "missing HEAD"
        return record
    if "locked" in entry:
        record["reason"] = "worktree is locked"
        return record
    if not path.is_dir():
        record["reason"] = "path missing (prunable metadata only)"
        record["reclaimable"] = True
        return record

    dirty, detail = is_dirty(path)
    if dirty:
        record["reason"] = f"not clean: {detail}"
        return record

    code, _, _ = git(repo, ["merge-base", "--is-ancestor", head, main_sha])
    if code != 0:
        record["reason"] = f"not merged into {CANONICAL}"
        return record

    on_remote = head in heads
    reachable_from_remote_main = (
        remote_main is not None
        and git(repo, ["merge-base", "--is-ancestor", head, remote_main])[0] == 0
    )
    if not (on_remote or reachable_from_remote_main):
        record["reason"] = "merged locally but absent from the push remote"
        return record

    record["pushed_as"] = heads.get(head, "") or f"ancestor of remote {CANONICAL}"
    record["reclaimable"] = True
    return record


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", default=".", help="canonical repository root")
    parser.add_argument("--apply", action="store_true", help="actually remove verified worktrees")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument("--remote", default=None, help="override push remote (default: remote.origin.pushurl)")
    parser.add_argument("--no-prune-branch", action="store_true", help="leave local branch refs untouched")
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    code, out, err = git(repo, ["rev-parse", "--show-toplevel"])
    if code != 0:
        raise SystemExit(f"not a git repository: {repo} ({err.strip()})")
    top = Path(out.strip())

    main_sha = rev(top, CANONICAL)
    if main_sha is None:
        raise SystemExit(f"cannot resolve local {CANONICAL}")

    remote = args.remote or push_remote(top)
    if not remote:
        raise SystemExit("no push remote configured; refusing to reclaim without a publish target")

    heads, remote_main = remote_state(top, remote)
    if remote_main is None:
        raise SystemExit(f"push remote {remote} has no {CANONICAL}; push first, then reclaim")

    protected = local_remote_paths(top)
    records = [
        classify(top, entry, canonical_path=top, main_sha=main_sha, heads=heads,
                 remote_main=remote_main, protected_paths=protected)
        for entry in worktrees(top)
    ]
    reclaimable = [r for r in records if r["reclaimable"]]
    total = sum(int(r["bytes"]) for r in reclaimable)

    if not args.json:
        print(f"repo          {top}")
        print(f"push remote   {remote}")
        print(f"local {CANONICAL:<8}{main_sha}")
        print(f"remote {CANONICAL:<7}{remote_main}")
        print(f"worktrees     {len(records)} total, {len(reclaimable)} reclaimable, {human(total)}")
        print()
        for record in sorted(records, key=lambda r: -int(r["bytes"])):
            state = "RECLAIM" if record["reclaimable"] else "keep   "
            note = record.get("pushed_as") or record["reason"]
            print(f"{state} {human(int(record['bytes'])):>9}  {record['branch']:<52} {record['path']}")
            print(f"                            {note}")

    if args.apply and reclaimable:
        removed = 0
        if not args.json:
            print()
        for record in reclaimable:
            path = Path(str(record["path"]))
            if path.is_dir():
                cmd = ["worktree", "remove", str(path)]
            else:
                cmd = ["worktree", "prune"]
            code, _out, err = git(top, cmd, timeout=300.0)
            if code != 0:
                if not args.json:
                    print(f"FAILED  {path}: {err.strip()}")
                continue
            removed += 1
            if not args.no_prune_branch:
                branch = str(record["branch"])
                if branch != "(detached)":
                    code, _o, err = git(top, ["branch", "-d", branch])
                    if code != 0 and not args.json:
                        print(f"  note: branch '{branch}' kept ({err.strip().splitlines()[:1]})")
            if not args.json:
                print(f"removed {human(int(record['bytes'])):>9}  {path}")
        if not args.json:
            print(f"\nreclaimed {human(total)} across {removed} worktree(s)")
    elif reclaimable and not args.apply:
        if not args.json:
            print(f"\ndry-run: re-run with --apply to reclaim {human(total)}")

    if args.json:
        print(json.dumps({
            "repo": str(top), "push_remote": remote,
            "local_main": main_sha, "remote_main": remote_main,
            "reclaimable_bytes": total,
            "worktrees": records,
        }, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

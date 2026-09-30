#!/usr/bin/env python3
"""Inventory and preserve pre-V2 refs/worktrees for non-destructive convergence salvage.

Read-only against legacy refs and worktrees. This tool NEVER resets, cleans,
prunes, deletes or force-pushes a historical tree or ref. Its output is an
*intake* inventory and preservation record, not permission to delete anything
and not acceptance of any candidate.

Design fixes over the previous revision:
  * frozen snapshot (refs-before.txt) is the authority for pre-V2 heads/remotes,
    independent of how live refs advanced;
  * exact main-v2 baseline SHA is an explicit context argument;
  * per-tip novelty uses `git cherry` (patch-id equivalence) instead of a
    wholesale three-dot diff, so inherited/cherry-picked commits are not
    misreported as salvage;
  * identical tips are deduplicated with explicit aliases;
  * worktrees, detached anchors, stash/reflog and archive/evidence pointers are
    enumerated; dirty metadata is collected with bounded status checks;
  * unreviewed substantive code is never auto-dispositioned as discarded;
  * intake (`candidate`) is kept explicitly distinct from final acceptance.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_FROZEN = pathlib.Path(
    "/Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130"
)

PROCESS_PREFIXES = (
    "tasks/", "worklog/", "docs/", "prompts/", ".agents/", "sources/",
    "requirements/", "validation/", "workspaces/", ".github/", ".opencode/",
)
PROCESS_FILES = {
    "README.md", "PLAN.md", "FEATURES.md", "FEATURES-COMPLETION.md",
    "AGENTS.md", "MEMORY.md", "prd.json", "ralph.json", "ralph.completion.json",
    "RAW_FEATURE.md", "REVIEW_ITERATION_1.md", "REVIEW_ITERATION_2.md",
    "opencode-rk-rk.md", ".gitignore",
}

SENSITIVE_TOKENS = (
    ".env", "auth.json", "credential", "credentials", "id_rsa", "id_ed25519",
    "id_ecdsa", ".netrc", "secret", "keychain", ".aws", ".ssh", "token",
    "password", ".npmrc", ".pypirc",
)
SENSITIVE_SUFFIXES = (".pem", ".key", ".p12", ".pfx", ".jks", ".keystore")
IGNORE_DIR_PARTS = {
    "target", "node_modules", "dist", "build", ".next", "vendor", ".venv",
    "venv", "__pycache__", ".cargo", ".gradle",
}

MAX_FILE_BYTES = 256 * 1024
MAX_TOTAL_BYTES = 16 * 1024 * 1024
STATUS_TIMEOUT = 25
GIT_TIMEOUT = 60

# disposition labels (subset of the convergence contract)
INTEGRATED_ANCESTOR = "INTEGRATED_ANCESTOR"
SUPERSEDED_BY = "SUPERSEDED_BY"
SALVAGE_PRODUCT = "SALVAGE_PRODUCT"
SALVAGE_TEST = "SALVAGE_TEST"
PROCESS_ONLY = "PROCESS_ONLY"
EVIDENCE_ONLY = "EVIDENCE_ONLY"
PENDING_REVIEW = "PENDING_REVIEW"
CONFLICTING_IMPL_PRESERVED = "CONFLICTING_IMPL_PRESERVED"


# --------------------------------------------------------------------------- #
# bounded git helpers (subprocess arrays only; never a shell string)
# --------------------------------------------------------------------------- #
def run(args, cwd=ROOT, input_bytes=None, timeout=GIT_TIMEOUT):
    try:
        return subprocess.run(
            list(args), cwd=str(cwd), input=input_bytes,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return None


def git_out(*args, cwd=ROOT, timeout=GIT_TIMEOUT):
    p = run(["git", *args], cwd=cwd, timeout=timeout)
    if p is None or p.returncode != 0:
        return None
    return p.stdout.decode("utf-8", "replace")


def git_stdout_bytes(*args, cwd=ROOT, input_bytes=None, timeout=GIT_TIMEOUT):
    p = run(["git", *args], cwd=cwd, input_bytes=input_bytes, timeout=timeout)
    if p is None or p.returncode != 0:
        return None
    return p.stdout


# --------------------------------------------------------------------------- #
# path classification
# --------------------------------------------------------------------------- #
def is_test_path(p: str) -> bool:
    parts = p.split("/")
    name = parts[-1]
    return (
        p.startswith("tests/")
        or "tests" in parts
        or name.startswith("test_")
        or name.endswith((".test.ts", ".test.tsx", ".spec.ts", ".spec.tsx"))
    )


def is_process_path(p: str) -> bool:
    return p in PROCESS_FILES or p.startswith(PROCESS_PREFIXES)


def is_product_path(p: str) -> bool:
    if p.startswith("crates/"):
        return "/src/" in p or p.endswith(("Cargo.toml", "build.rs"))
    if p.startswith("web/"):
        return not is_test_path(p) and not p.startswith("web/docs/")
    if p.startswith("scripts/"):
        return True
    if p.startswith("fixtures/"):
        return False
    return p in {"Cargo.toml", "Cargo.lock", "package.json", "pnpm-lock.yaml"}


def classify(ancestor: bool, novel_paths: list[str]) -> tuple[str, str]:
    """Return (disposition, acceptance). Never auto-discards substantive code."""
    if ancestor:
        return INTEGRATED_ANCESTOR, "INTEGRATED"
    if not novel_paths:
        return SUPERSEDED_BY, "INTEGRATED"
    product = [p for p in novel_paths if is_product_path(p)]
    tests = [p for p in novel_paths if is_test_path(p)]
    process = [p for p in novel_paths if is_process_path(p)]
    if product:
        return SALVAGE_PRODUCT, "NOT_ACCEPTED"
    if tests and len(tests) + len(process) == len(novel_paths):
        return SALVAGE_TEST, "NOT_ACCEPTED"
    if process and len(process) == len(novel_paths):
        return PROCESS_ONLY, "NOT_ACCEPTED"
    if tests:
        return SALVAGE_TEST, "NOT_ACCEPTED"
    return PENDING_REVIEW, "NOT_ACCEPTED"


def is_sensitive_path(p: str) -> bool:
    name = p.rsplit("/", 1)[-1].lower()
    if name.endswith(SENSITIVE_SUFFIXES):
        return True
    for tok in SENSITIVE_TOKENS:
        if tok in name:
            return True
    parts = {x.lower() for x in p.split("/")}
    return bool(parts & {".ssh", ".aws", "keychains"})


# --------------------------------------------------------------------------- #
# frozen snapshot readers
# --------------------------------------------------------------------------- #
def read_frozen_refs(frozen: pathlib.Path):
    heads, remotes, others = [], [], []
    path = frozen / "refs-before.txt"
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        ref, _, sha = line.partition(" ")
        sha = sha.strip()
        if ref.startswith("refs/heads/"):
            heads.append((ref[len("refs/heads/"):], sha))
        elif ref.startswith("refs/remotes/"):
            remotes.append((ref[len("refs/remotes/"):], sha))
        else:
            others.append((ref, sha))
    return heads, remotes, others


def read_frozen_worktrees(frozen: pathlib.Path):
    out, cur = [], {}
    path = frozen / "worktrees-before.txt"
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.rstrip("\n")
        if not line.strip():
            if cur:
                out.append(cur)
                cur = {}
            continue
        key, _, val = line.partition(" ")
        val = val.strip()
        if key == "worktree":
            cur["path"] = val
        elif key == "HEAD":
            cur["head"] = val
        elif key == "branch":
            cur["branch"] = val
        elif key == "detached":
            cur["detached"] = True
    if cur:
        out.append(cur)
    return out


def read_tsv(path: pathlib.Path):
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        sha, _, p = line.partition("\t")
        rows.append({"sha": sha.strip(), "path": p.strip()})
    return rows


# --------------------------------------------------------------------------- #
# novelty analysis
# --------------------------------------------------------------------------- #
class Analyzer:
    def __init__(self, base_sha: str, cwd=ROOT):
        self.base = base_sha
        self.cwd = cwd
        self._counts = {}
        self._novel = {}
        self._paths = {}
        self._exists = {}

    def left_right(self, sha: str):
        if sha not in self._counts:
            out = git_out("rev-list", "--left-right", "--count",
                          f"{self.base}...{sha}", cwd=self.cwd)
            if out is None:
                self._counts[sha] = None
            else:
                parts = out.split()
                behind = int(parts[0]) if parts else 0
                ahead = int(parts[1]) if len(parts) > 1 else 0
                self._counts[sha] = (behind, ahead)
        return self._counts[sha]

    def novelty(self, sha: str):
        """Return (unique_no_merge, novel, inherited)."""
        if sha in self._novel:
            return self._novel[sha]
        unique = git_out("rev-list", "--no-merges", f"{self.base}..{sha}",
                         cwd=self.cwd) or ""
        unique = [s for s in unique.splitlines() if s]
        cherry = git_out("cherry", self.base, sha, cwd=self.cwd) or ""
        sign = {}
        for line in cherry.splitlines():
            if len(line) > 2 and line[0] in "+-":
                sign[line[2:].strip()] = line[0]
        novel = [s for s in unique if sign.get(s) == "+"]
        inherited = [s for s in unique if sign.get(s) != "+"]
        self._novel[sha] = (unique, novel, inherited)
        return self._novel[sha]

    def paths_for(self, shas: list[str]):
        need = [s for s in shas if s not in self._paths]
        if need:
            payload = ("\n".join(need) + "\n").encode()
            out = git_stdout_bytes(
                "diff-tree", "-r", "--name-only", "--no-renames", "--stdin",
                cwd=self.cwd, input_bytes=payload, timeout=max(GIT_TIMEOUT, 120),
            )
            current = None
            for raw in (out or b"").decode("utf-8", "replace").splitlines():
                line = raw.strip()
                if not line:
                    continue
                if len(line) == 40 and all(c in "0123456789abcdef" for c in line):
                    current = line
                    self._paths.setdefault(current, [])
                elif current is not None:
                    self._paths[current].append(line)
            for s in need:
                self._paths.setdefault(s, [])
        result = []
        for s in shas:
            for p in self._paths.get(s, []):
                if p not in result:
                    result.append(p)
        return result

    def object_exists(self, sha: str) -> bool:
        if sha not in self._exists:
            p = run(["git", "cat-file", "-e", f"{sha}^{{commit}}"], cwd=self.cwd)
            self._exists[sha] = bool(p is not None and p.returncode == 0)
        return self._exists[sha]


# --------------------------------------------------------------------------- #
# ref inventory
# --------------------------------------------------------------------------- #
def inventory_refs(an: Analyzer, frozen: pathlib.Path):
    heads, remotes, others = read_frozen_refs(frozen)
    entries = []
    for kind, items in (("head", heads), ("remote", remotes), ("other", others)):
        for short, sha in items:
            entries.append({"kind": kind, "ref": short, "sha": sha})

    by_sha = defaultdict(list)
    for e in entries:
        by_sha[e["sha"]].append(e)

    rows = []
    for sha, group in sorted(by_sha.items()):
        group.sort(key=lambda e: (e["kind"] != "head", e["ref"]))
        primary = group[0]
        aliases = [f"{e['kind']}:{e['ref']}" for e in group[1:]]
        exists = an.object_exists(sha)
        if not exists:
            rows.append({
                "primaryRef": primary["ref"], "primaryKind": primary["kind"],
                "sha": sha, "aliases": aliases, "aliasCount": len(aliases),
                "objectPresent": False, "disposition": "PENDING_REVIEW",
                "acceptance": "NOT_ACCEPTED", "recommendedAction": "human-review",
                "reason": "object missing from local repository",
                "behind": None, "ahead": None,
                "novelCommitCount": None, "inheritedCommitCount": None,
                "novelChangedPaths": [],
            })
            continue
        lr = an.left_right(sha)
        if lr is None:
            behind = ahead = None
            ancestor = False
        else:
            behind, ahead = lr
            ancestor = ahead == 0
        if ancestor:
            novel_paths, novel_n, inherited_n = [], 0, 0
        else:
            unique, novel, inherited = an.novelty(sha)
            novel_n, inherited_n = len(novel), len(inherited)
            novel_paths = an.paths_for(novel)
        disposition, acceptance = classify(ancestor, novel_paths)
        rows.append({
            "primaryRef": primary["ref"], "primaryKind": primary["kind"],
            "sha": sha, "aliases": aliases, "aliasCount": len(aliases),
            "objectPresent": True, "disposition": disposition,
            "acceptance": acceptance,
            "recommendedAction": (
                "heritage-only" if acceptance == "INTEGRATED" else "content-review"
            ),
            "reason": (
                "tip is ancestor of base" if ancestor else
                "all unique commits already in base by patch-id" if novel_n == 0 else
                "novel commits vs base"
            ),
            "behind": behind, "ahead": ahead,
            "novelCommitCount": novel_n, "inheritedCommitCount": inherited_n,
            "novelChangedPaths": novel_paths[:40],
            "novelChangedPathCount": len(novel_paths),
        })
    return rows, {"heads": len(heads), "remotes": len(remotes),
                  "others": len(others), "frozenRefs": len(entries)}


def candidate_groups(rows):
    groups = defaultdict(list)
    for r in rows:
        if r["acceptance"] == "INTEGRATED" or not r["objectPresent"]:
            continue
        groups[r["disposition"]].append(r)
    out = []
    for disp, items in sorted(groups.items()):
        items.sort(key=lambda r: (-(r["novelCommitCount"] or 0), r["primaryRef"]))
        out.append({
            "disposition": disp,
            "uniqueTips": len(items),
            "refs": [i["primaryRef"] for i in items],
            "aliasRefs": sum(i["aliasCount"] for i in items),
            "maxNovelCommits": max((i["novelCommitCount"] or 0) for i in items),
            "totalNovelCommits": sum((i["novelCommitCount"] or 0) for i in items),
        })
    return out


# --------------------------------------------------------------------------- #
# worktree / stash / archive
# --------------------------------------------------------------------------- #
def worktree_dirty(path: pathlib.Path):
    meta = {"exists": path.is_dir(), "dirty": None, "statusTimeout": False,
            "trackedChanges": 0, "stagedChanges": 0, "untrackedCount": 0,
            "sensitivePathsReported": 0, "sensitivePaths": []}
    if not meta["exists"]:
        return meta
    p = run(["git", "-C", str(path), "status", "--porcelain=v1", "-z",
             "--untracked-files=normal"], timeout=STATUS_TIMEOUT)
    if p is None:
        meta["statusTimeout"] = True
        return meta
    fields = [f for f in p.stdout.decode("utf-8", "replace").split("\x00") if f]
    for f in fields:
        code = f[:2]
        name = f[3:]
        if code == "??":
            meta["untrackedCount"] += 1
        elif code[0] not in " ?" and code[1] not in " ?":
            meta["stagedChanges"] += 1
            meta["trackedChanges"] += 1
        elif code[1] != " ":
            meta["trackedChanges"] += 1
        else:
            meta["stagedChanges"] += 1
        if is_sensitive_path(name):
            meta["sensitivePathsReported"] += 1
            if len(meta["sensitivePaths"]) < 20:
                meta["sensitivePaths"].append(name)
    meta["dirty"] = bool(fields)
    return meta


def collect_worktrees(frozen: pathlib.Path, check_dirty: bool):
    descs = read_frozen_worktrees(frozen)
    rows = []
    for d in descs:
        p = pathlib.Path(d["path"])
        row = {
            "path": d["path"],
            "head": d.get("head"),
            "branch": d.get("branch"),
            "detached": bool(d.get("detached") or "branch" not in d),
        }
        if check_dirty:
            row.update(worktree_dirty(p))
        else:
            row["exists"] = p.is_dir()
        rows.append(row)
    return rows


def collect_detached(frozen: pathlib.Path, an: Analyzer):
    rows = []
    for r in read_tsv(frozen / "detached-worktrees.tsv"):
        r["objectPresent"] = an.object_exists(r["sha"])
        rows.append(r)
    return rows


def collect_stash():
    rows = []
    out = git_out("log", "-g", "--format=%H%x00%gs", "refs/stash") or ""
    for line in out.splitlines():
        sha, _, subject = line.partition("\x00")
        rows.append({"stashSha": sha.strip(), "subject": subject.strip()})
    out2 = git_out("for-each-ref", "--format=%(refname) %(objectname)",
                   "refs/stash") or ""
    ref = [{"ref": l.split()[0], "sha": l.split()[1]}
           for l in out2.splitlines() if l.strip()]
    return {"entries": rows, "ref": ref,
            "note": "recorded read-only; never popped or applied"}


def collect_evidence_refs(frozen: pathlib.Path, an: Analyzer):
    rows = []
    seen = set()
    for line in (frozen / "archive-refs.txt").read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        ref, _, sha = line.partition(" ")
        seen.add(ref)
        rows.append({"ref": ref, "sha": sha.strip(), "source": "frozen",
                     "objectPresent": an.object_exists(sha.strip())})
    live = git_out("for-each-ref", "--format=%(refname) %(objectname)",
                   "refs/archive", "refs/ledger") or ""
    for line in live.splitlines():
        if not line.strip():
            continue
        ref, _, sha = line.partition(" ")
        if ref in seen:
            continue
        rows.append({"ref": ref, "sha": sha.strip(), "source": "live",
                     "objectPresent": an.object_exists(sha.strip())})
    return rows


# --------------------------------------------------------------------------- #
# dirty preservation (opt-in, bounded, no owner file mutation)
# --------------------------------------------------------------------------- #
def preserve_dirty(worktrees, snapshot: pathlib.Path, report):
    snapshot.mkdir(parents=True, exist_ok=True)
    total = 0
    preserved = []
    for wt in worktrees:
        if not wt.get("exists") or not wt.get("dirty"):
            continue
        wtpath = pathlib.Path(wt["path"])
        name = hashlib.sha1(str(wtpath).encode()).hexdigest()[:12]
        outdir = snapshot / name
        outdir.mkdir(parents=True, exist_ok=True)
        # changed tracked paths
        raw = git_out("status", "--porcelain=v1", "-z", "--untracked-files=no",
                      cwd=wtpath, timeout=STATUS_TIMEOUT) or ""
        changed = []
        for f in [x for x in raw.split("\x00") if x]:
            changed.append(f[3:])
        patch = []
        for rel in changed:
            if is_sensitive_path(rel):
                report["sensitiveUntouched"].append({"worktree": wt["path"], "path": rel})
                continue
            d = git_out("diff", "--binary", "--", rel, cwd=wtpath, timeout=STATUS_TIMEOUT)
            if d:
                patch.append(d)
            d2 = git_out("diff", "--cached", "--binary", "--", rel, cwd=wtpath, timeout=STATUS_TIMEOUT)
            if d2:
                patch.append(d2)
        if patch:
            data = ("\n".join(patch) + "\n").encode()
            if total + len(data) <= MAX_TOTAL_BYTES:
                (outdir / "tracked.patch").write_bytes(data)
                total += len(data)
            else:
                report["overBudget"].append({"worktree": wt["path"], "reason": "tracked-patch"})
        # untracked non-sensitive useful files
        others = git_out("ls-files", "--others", "--exclude-standard", "-z",
                         cwd=wtpath, timeout=STATUS_TIMEOUT) or ""
        for rel in [x for x in others.split("\x00") if x]:
            relpath = wtpath / rel
            parts = set(rel.split("/"))
            if parts & IGNORE_DIR_PARTS:
                continue
            if is_sensitive_path(rel):
                report["sensitiveUntouched"].append({"worktree": wt["path"], "path": rel})
                continue
            try:
                if relpath.is_symlink() or not relpath.is_file():
                    continue
                size = relpath.stat().st_size
                if size > MAX_FILE_BYTES or total + size > MAX_TOTAL_BYTES:
                    report["overBudget"].append({"worktree": wt["path"], "path": rel})
                    continue
                dest = outdir / "untracked" / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(relpath.read_bytes())
                total += size
            except OSError:
                continue
        preserved.append({"worktree": wt["path"], "dir": str(outdir)})
    report["preservedWorktrees"] = preserved
    report["preservedBytes"] = total
    return report


# --------------------------------------------------------------------------- #
# self-check fixture
# --------------------------------------------------------------------------- #
def self_check() -> int:
    """Build a disposable fixture and assert the classifier defects are fixed."""
    with tempfile.TemporaryDirectory(prefix="salvage-selfcheck-") as td:
        repo = pathlib.Path(td)
        env = {
            "GIT_AUTHOR_NAME": "chk", "GIT_AUTHOR_EMAIL": "chk@x",
            "GIT_COMMITTER_NAME": "chk", "GIT_COMMITTER_EMAIL": "chk@x",
        }

        def g(*args, cwd=repo):
            e = dict(os.environ)
            e.update(env)
            return subprocess.run(["git", *args], cwd=str(cwd), env=e,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  timeout=30)

        g("init", "-q", "-b", "main")
        (repo / "crates" / "a" / "src").mkdir(parents=True)
        (repo / "crates" / "a" / "src" / "lib.rs").write_text("v0\n")
        (repo / "docs").mkdir()
        (repo / "docs" / "PLAN.md").write_text("plan0\n")
        g("add", "-A")
        g("commit", "-q", "-m", "c0")
        c0 = g("rev-parse", "HEAD").stdout.decode().strip()

        # base-advance commit (process path)
        (repo / "docs" / "PLAN.md").write_text("plan1\n")
        g("add", "-A")
        g("commit", "-q", "-m", "c1 process")
        base = g("rev-parse", "HEAD").stdout.decode().strip()

        # ancestor branch
        g("branch", "anc/inside", base)

        # duplicate-content branch: same patch as c1, different parent
        g("checkout", "-q", "-b", "dup/content", c0)
        (repo / "docs" / "PLAN.md").write_text("plan1\n")
        g("add", "-A")
        g("commit", "-q", "-m", "dup of c1")

        # novel product
        g("checkout", "-q", "-b", "novel/product", base)
        (repo / "crates" / "a" / "src" / "lib.rs").write_text("v1\n")
        g("add", "-A")
        g("commit", "-q", "-m", "product")

        # novel test
        g("checkout", "-q", "-b", "novel/test", base)
        (repo / "tests").mkdir()
        (repo / "tests" / "x.rs").write_text("t\n")
        g("add", "-A")
        g("commit", "-q", "-m", "test")

        # novel process
        g("checkout", "-q", "-b", "novel/process", base)
        (repo / "docs" / "PLAN.md").write_text("plan2\n")
        g("add", "-A")
        g("commit", "-q", "-m", "process")

        # alias tip
        g("branch", "alias/a", "novel/product")
        g("branch", "alias/b", "novel/product")
        g("checkout", "-q", "main")

        an = Analyzer(base, cwd=repo)
        rows = {}
        for ref in ["anc/inside", "dup/content", "novel/product", "novel/test",
                    "novel/process"]:
            sha = g("rev-parse", ref).stdout.decode().strip()
            unique, novel, inherited = an.novelty(sha)
            paths = an.paths_for(novel)
            disp, _ = classify(False, paths)
            rows[ref] = (disp, len(novel), len(inherited))
        anc_sha = g("rev-parse", "anc/inside").stdout.decode().strip()
        lr = an.left_right(anc_sha)
        assert lr is not None, "ancestor left_right failed (git timeout)"
        assert lr[1] == 0, f"ancestor ahead != 0: {lr}"

        failures = []
        if rows["dup/content"][1] != 0:
            failures.append("duplicate content counted as novel")
        if rows["dup/content"][0] != SUPERSEDED_BY:
            failures.append(f"dup/content -> {rows['dup/content'][0]}")
        if rows["novel/product"][0] != SALVAGE_PRODUCT:
            failures.append(f"novel/product -> {rows['novel/product'][0]}")
        if rows["novel/test"][0] != SALVAGE_TEST:
            failures.append(f"novel/test -> {rows['novel/test'][0]}")
        if rows["novel/process"][0] != PROCESS_ONLY:
            failures.append(f"novel/process -> {rows['novel/process'][0]}")

        # alias dedup
        pa = g("rev-parse", "alias/a").stdout.decode().strip()
        pb = g("rev-parse", "alias/b").stdout.decode().strip()
        if pa != pb:
            failures.append("alias fixture mismatch")

        if failures:
            print("SELF-CHECK FAILED: " + "; ".join(failures))
            return 1
        print("SELF-CHECK OK: "
              f"dup={rows['dup/content']} product={rows['novel/product']} "
              f"test={rows['novel/test']} process={rows['novel/process']} "
              f"aliases_dedup=1")
        return 0


# --------------------------------------------------------------------------- #
# frozen snapshot metadata
# --------------------------------------------------------------------------- #
def frozen_metadata(frozen: pathlib.Path):
    files = []
    for p in sorted(frozen.iterdir()):
        if not p.is_file():
            continue
        entry = {"name": p.name, "bytes": p.stat().st_size}
        if p.stat().st_size <= 512 * 1024:
            h = hashlib.sha256()
            with p.open("rb") as fh:
                for chunk in iter(lambda: fh.read(65536), b""):
                    h.update(chunk)
            entry["sha256"] = h.hexdigest()
        files.append(entry)
    return files


# --------------------------------------------------------------------------- #
# main
# --------------------------------------------------------------------------- #
def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-sha", default=None,
                    help="exact main-v2 baseline SHA (context)")
    ap.add_argument("--base-ref", default="main-v2")
    ap.add_argument("--frozen-root", default=str(DEFAULT_FROZEN))
    ap.add_argument("--json", default="docs/convergence-v2-salvage.json")
    ap.add_argument("--markdown", default="docs/CONVERGENCE_V2_SALVAGE.md")
    ap.add_argument("--snapshot-dir", default=None)
    ap.add_argument("--preserve-dirty", action="store_true")
    ap.add_argument("--check-dirty", action="store_true", default=True)
    ap.add_argument("--no-dirty", dest="check_dirty", action="store_false")
    ap.add_argument("--self-check", action="store_true")
    args = ap.parse_args()

    if args.self_check:
        return self_check()

    frozen = pathlib.Path(args.frozen_root)
    base_sha = args.base_sha or git_out("rev-parse", args.base_ref)
    base_sha = (base_sha or "").strip()
    if not base_sha:
        print("cannot resolve base sha", file=sys.stderr)
        return 2

    an = Analyzer(base_sha)
    rows, ref_meta = inventory_refs(an, frozen)
    groups = candidate_groups(rows)

    worktrees = collect_worktrees(frozen, args.check_dirty)
    detached = collect_detached(frozen, an)
    stash = collect_stash()
    evidence = collect_evidence_refs(frozen, an)

    ref_counts = Counter(r["disposition"] for r in rows)
    accepted = {"INTEGRATED_ANCESTOR", "SUPERSEDED_BY"}
    intake_counts = Counter(
        r["disposition"] for r in rows if r["disposition"] not in accepted
    )

    payload = {
        "schemaVersion": 2,
        "generatedAt": _dt.datetime.now(_dt.timezone.utc).isoformat(),
        "baseRef": args.base_ref,
        "baseSha": base_sha,
        "frozenRoot": str(frozen),
        "frozenSnapshot": {
            "refsFile": "refs-before.txt",
            "refsAuthority": "pre-V2 heads/remotes",
            "counts": ref_meta,
            "files": frozen_metadata(frozen),
        },
        "counts": {
            "frozenRefs": ref_meta["frozenRefs"],
            "uniqueTips": len(rows),
            "aliasedRefs": sum(r["aliasCount"] for r in rows),
            "byRef": dict(sorted(ref_counts.items())),
            "intakeByUniqueTip": dict(sorted(intake_counts.items())),
            "uniqueTipDispositions": dict(sorted(ref_counts.items())),
        },
        "distinction": {
            "intake": "rows with acceptance NOT_ACCEPTED are unintegrated candidates",
            "final": "acceptance INTEGRATED means ancestry or patch-id-superseded by base; it is not a gate pass for novel content",
        },
        "refs": rows,
        "candidateGroups": groups,
        "worktrees": worktrees,
        "worktreeSummary": {
            "total": len(worktrees),
            "present": sum(1 for w in worktrees if w.get("exists")),
            "missing": sum(1 for w in worktrees if not w.get("exists")),
            "dirty": sum(1 for w in worktrees if w.get("dirty")),
            "statusTimeout": sum(1 for w in worktrees if w.get("statusTimeout")),
        },
        "detachedAnchors": detached,
        "stash": stash,
        "evidenceRefs": evidence,
    }

    report = {"sensitiveUntouched": [], "overBudget": [],
              "preservedWorktrees": [], "preservedBytes": 0}
    if args.preserve_dirty and args.snapshot_dir:
        snapshot = pathlib.Path(args.snapshot_dir)
        preserve_dirty(worktrees, snapshot, report)
        payload["preservation"] = {
            "snapshotDir": str(snapshot),
            "preservedWorktrees": len(report["preservedWorktrees"]),
            "preservedBytes": report["preservedBytes"],
            "sensitiveUntouched": report["sensitiveUntouched"],
            "overBudget": report["overBudget"],
            "g0": "pending" if report["sensitiveUntouched"] else "pending-root-only",
        }

    json_path = ROOT / args.json
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n",
                         encoding="utf-8")

    md = [
        "# Convergence V2 salvage inventory (frozen-authority)",
        "",
        f"Base: `{args.base_ref}` / `{base_sha}`",
        f"Frozen authority: `{frozen}`",
        "",
        "Generated by `tools/convergence_v2_salvage.py`. Intake inventory only; "
        "nothing here authorises deletion. `NOT_ACCEPTED` rows are unintegrated "
        "candidates pending real gate proof.",
        "",
        "## Counts",
        "",
        f"- frozen refs: {ref_meta['frozenRefs']} "
        f"(heads {ref_meta['heads']}, remotes {ref_meta['remotes']}, others {ref_meta['others']})",
        f"- unique tips: {len(rows)} (aliases collapsed: {payload['counts']['aliasedRefs']})",
        "",
        "### By unique tip",
        "",
    ]
    for k, v in sorted(ref_counts.items()):
        md.append(f"- `{k}`: {v}")
    md += ["", "### Intake (unintegrated) groups", ""]
    for g in groups:
        md.append(f"- `{g['disposition']}`: {g['uniqueTips']} tips, "
                  f"{g['totalNovelCommits']} novel commits, "
                  f"{g['aliasRefs']} aliases")
    md += [
        "",
        "## Worktrees",
        "",
        f"- total {payload['worktreeSummary']['total']}, "
        f"present {payload['worktreeSummary']['present']}, "
        f"missing {payload['worktreeSummary']['missing']}, "
        f"dirty {payload['worktreeSummary']['dirty']}, "
        f"status-timeout {payload['worktreeSummary']['statusTimeout']}",
        "",
        "## Anchors and evidence",
        "",
        f"- detached anchors: {len(detached)}",
        f"- stash entries: {len(stash['entries'])} (read-only; never popped)",
        f"- archive/ledger evidence refs: {len(evidence)}",
        "",
    ]
    (ROOT / args.markdown).write_text("\n".join(md), encoding="utf-8")
    print(json.dumps({
        "base": base_sha,
        "frozenRefs": ref_meta["frozenRefs"],
        "uniqueTips": len(rows),
        "counts": dict(sorted(ref_counts.items())),
        "worktrees": payload["worktreeSummary"],
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
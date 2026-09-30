#!/usr/bin/env python3
"""Inventory and preserve pre-V2 refs/worktrees for non-destructive convergence salvage.

Read-only against legacy refs and worktrees. This tool NEVER resets, cleans,
prunes, deletes or force-pushes a historical tree or ref. Its output is an
*intake* inventory and preservation record, not permission to delete anything
and not acceptance of any candidate.

Fail-closed rules (V2-SALVAGE-PRESERVATION):
  * a git command failure/timeout never yields an empty ("no novel") result;
    it produces a PENDING_REVIEW / NOT_ACCEPTED row with analysisFailure=true;
  * merge-only unique history (conflict-resolution-only merge commit) stays
    PENDING_REVIEW until inspected, never SUPERSEDED;
  * a patch-id already present in base is compatibility evidence, not gate
    acceptance: SUPERSEDED_BY carries acceptance NOT_ACCEPTED and records the
    exact base ref + SHA; only an exact ancestry yields INTEGRATED;
  * dirty status is parsed from `--porcelain=v1 -z` with nonzero-returncode
    checks and rename source/destination handling;
  * preservation writes SEPARATE staged.patch / unstaged.patch / untracked/
    plus meta.json (observed HEAD), so staged and unstaged edits restore
    independently;
  * every skip (symlink, sensitive, over-budget, read/write error, ignored)
    is recorded as an explicit gap, never silently omitted.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import pathlib
import selectors
import stat
import subprocess
import sys
import tempfile
import time
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

# exact filenames that are live credential/key stores regardless of extension
SENSITIVE_EXACT_NAMES = {
    ".env", ".env.local", ".env.production", "auth.json", "opencode.json",
    "credentials", "credentials.json", ".netrc", "id_rsa", "id_ed25519",
    "id_ecdsa", ".npmrc", ".pypirc", ".pgpass", "shadow", "passwd",
}
SENSITIVE_SUFFIXES = (".pem", ".key", ".p12", ".pfx", ".jks", ".keystore", ".crt")
SENSITIVE_TOKENS = (
    "credential", "secret", "keychain", "password", "passwd", "token",
    "apikey", "api_key", "privatekey", "private_key", "auth",
)
SENSITIVE_DIRS = {".ssh", ".aws", ".gnupg", "keychains"}
# ordinary source/doc extensions: a sensitive *token in the filename* here is
# not treated as a live credential store (e.g. browser_credential_launch.rs)
SOURCE_EXTS = {
    ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".go", ".java",
    ".rb", ".c", ".h", ".cc", ".cpp", ".hpp", ".cs", ".php", ".swift", ".kt",
    ".scala", ".sh", ".bash", ".zsh", ".md", ".rst", ".sql",
}

IGNORE_DIR_PARTS = {
    "target", "node_modules", "dist", "build", ".next", "vendor", ".venv",
    "venv", "__pycache__", ".cargo", ".gradle", ".git",
}

MAX_FILE_BYTES = 4 * 1024 * 1024        # 4 MiB per file
MAX_TOTAL_BYTES = 64 * 1024 * 1024      # 64 MiB per run
STATUS_TIMEOUT = 25
GIT_TIMEOUT = 60
EXPECTED_DETACHED = 45
EXPECTED_WORKTREES = 112

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
    """Capture bounded output and always join this command's owned process."""
    process = None
    try:
        if input_bytes is not None and len(input_bytes) > MAX_TOTAL_BYTES:
            return None
        with tempfile.TemporaryFile() as stdin, selectors.DefaultSelector() as ready:
            if input_bytes:
                stdin.write(input_bytes)
            stdin.seek(0)
            process = subprocess.Popen(list(args), cwd=str(cwd), stdin=stdin,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            captured = {"stdout": bytearray(), "stderr": bytearray()}
            ready.register(process.stdout, selectors.EVENT_READ, "stdout")
            ready.register(process.stderr, selectors.EVENT_READ, "stderr")
            deadline = time.monotonic() + timeout
            while ready.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    return None
                for key, _ in ready.select(remaining):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        ready.unregister(key.fileobj)
                        continue
                    destination = captured[key.data]
                    limit = MAX_TOTAL_BYTES if key.data == "stdout" else 65536
                    if len(destination) + len(chunk) > limit:
                        return None
                    destination.extend(chunk)
            status = process.wait(timeout=max(0.001, deadline - time.monotonic()))
            return subprocess.CompletedProcess(list(args), status,
                                               bytes(captured["stdout"]),
                                               bytes(captured["stderr"]))
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return None
    finally:
        if process is not None:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()


def git_out(*args, cwd=ROOT, timeout=GIT_TIMEOUT):
    """Return stdout text, or None on any failure (fail-closed signal)."""
    p = run(["git", *args], cwd=cwd, timeout=timeout)
    if p is None or p.returncode != 0:
        return None
    return p.stdout.decode("utf-8", "replace")


def git_bytes(*args, cwd=ROOT, input_bytes=None, timeout=GIT_TIMEOUT):
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
    """Path-based disposition for novel content. Never yields SUPERSEDED_BY."""
    if ancestor:
        return INTEGRATED_ANCESTOR, "INTEGRATED"
    if not novel_paths:
        # no evidence about content: fail closed, never auto-supersede
        return PENDING_REVIEW, "NOT_ACCEPTED"
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


def decide(ok: bool, ancestor: bool, merge_only: bool, merge_count: int,
           novel_n: int, inherited_n: int,
           novel_paths: list[str]) -> tuple[str, str, str]:
    """Fail-closed disposition decision. Returns (disposition, acceptance, reason)."""
    if not ok:
        return PENDING_REVIEW, "NOT_ACCEPTED", "analysis unavailable (git failure); fail-closed"
    if ancestor:
        return INTEGRATED_ANCESTOR, "INTEGRATED", "exact ancestor of base"
    if merge_only:
        return PENDING_REVIEW, "NOT_ACCEPTED", "merge-only unique history; conflict resolution not inspected"
    if novel_n == 0:
        if merge_count > 0:
            # unique merge commits present whose conflict resolution was not
            # inspected for tree equivalence: conservatively never discard.
            return (PENDING_REVIEW, "NOT_ACCEPTED",
                    f"{merge_count} unique merge(s); resolution not inspected")
        return (SUPERSEDED_BY, "NOT_ACCEPTED",
                "all non-merge patches already in base (compatibility evidence, not gate acceptance)")
    disp, acc = classify(False, novel_paths)
    return disp, acc, "novel content vs base"


def is_sensitive_path(p: str) -> bool:
    base = p.rsplit("/", 1)[-1].lower()
    parts = {x.lower() for x in p.split("/")}
    if parts & SENSITIVE_DIRS:
        return True
    if base.endswith(SENSITIVE_SUFFIXES):
        return True
    if base in SENSITIVE_EXACT_NAMES:
        return True
    if not any(tok in base for tok in SENSITIVE_TOKENS):
        return False
    ext = "." + base.rsplit(".", 1)[1] if "." in base[1:] else ""
    if ext in SOURCE_EXTS:
        # ordinary source/doc: token in filename is not a live credential store
        return False
    return True


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
# novelty analysis (fail-closed)
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
        """(behind, ahead) or None on failure."""
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
        """(unique_no_merge, novel, inherited, merge_only, merge_count) or None on failure."""
        if sha in self._novel:
            return self._novel[sha]
        raw = git_out("rev-list", "--no-merges", f"{self.base}..{sha}", cwd=self.cwd)
        if raw is None:
            return None  # do NOT cache failure as empty
        unique = [s for s in raw.splitlines() if s]
        all_count_out = git_out("rev-list", "--count", f"{self.base}..{sha}", cwd=self.cwd)
        if all_count_out is None:
            return None
        ahead_all = int(all_count_out.strip() or "0")
        cherry = git_out("cherry", self.base, sha, cwd=self.cwd)
        if cherry is None:
            return None
        sign = {}
        for line in cherry.splitlines():
            if len(line) > 2 and line[0] in "+-":
                sign[line[2:].strip()] = line[0]
        novel = [s for s in unique if sign.get(s) == "+"]
        inherited = [s for s in unique if sign.get(s) != "+"]
        merge_only = (ahead_all > 0 and len(unique) == 0)
        merge_count = max(0, ahead_all - len(unique))
        result = (unique, novel, inherited, merge_only, merge_count)
        self._novel[sha] = result
        return result

    def paths_for(self, shas: list[str]):
        """List of changed paths, or None on failure. Never caches failure."""
        need = [s for s in shas if s not in self._paths]
        if need:
            payload = ("\n".join(need) + "\n").encode()
            out = git_bytes(
                "diff-tree", "-r", "--name-only", "--no-renames", "--stdin",
                cwd=self.cwd, input_bytes=payload, timeout=max(GIT_TIMEOUT, 120),
            )
            if out is None:
                return None
            current = None
            for raw in out.decode("utf-8", "replace").splitlines():
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
def inventory_refs(an: Analyzer, frozen: pathlib.Path, base_ref: str, base_sha: str):
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
        row = {
            "primaryRef": primary["ref"], "primaryKind": primary["kind"],
            "sha": sha, "aliases": aliases, "aliasCount": len(aliases),
            "objectPresent": False, "analysisFailure": False, "mergeOnly": False,
            "uniqueMergeCount": 0,
            "ancestryExact": False, "disposition": PENDING_REVIEW,
            "acceptance": "NOT_ACCEPTED", "recommendedAction": "human-review",
            "reason": "object missing from local repository",
            "behind": None, "ahead": None, "aheadAll": None,
            "novelCommitCount": None, "inheritedCommitCount": None,
            "novelChangedPaths": [], "novelChangedPathCount": 0,
            "supersededBy": None,
        }
        if not an.object_exists(sha):
            rows.append(row)
            continue
        row["objectPresent"] = True
        lr = an.left_right(sha)
        if lr is None:
            row["analysisFailure"] = True
            row["disposition"], row["acceptance"], row["reason"] = decide(
                False, False, False, 0, 0, 0, [])
            row["recommendedAction"] = "content-review"
            rows.append(row)
            continue
        behind, ahead = lr
        ancestor = ahead == 0
        row["behind"], row["ahead"] = behind, ahead
        ok = True
        novel_n = inherited_n = 0
        merge_count = 0
        novel_paths: list = []
        merge_only = False
        if ancestor:
            row["ancestryExact"] = True
        else:
            res = an.novelty(sha)
            if res is None:
                ok = False
            else:
                unique, novel, inherited, merge_only, merge_count = res
                novel_n, inherited_n = len(novel), len(inherited)
                row["mergeOnly"] = merge_only
                row["uniqueMergeCount"] = merge_count
                if novel_n > 0:
                    paths = an.paths_for(novel)
                    if paths is None:
                        ok = False
                    else:
                        novel_paths = paths
        disp, acc, reason = decide(ok, ancestor, merge_only, merge_count, novel_n, inherited_n, novel_paths)
        row.update({
            "analysisFailure": not ok,
            "disposition": disp, "acceptance": acc, "reason": reason,
            "recommendedAction": ("heritage-only" if acc == "INTEGRATED" else "content-review"),
            "novelCommitCount": novel_n, "inheritedCommitCount": inherited_n,
            "novelChangedPaths": novel_paths[:40], "novelChangedPathCount": len(novel_paths),
            "aheadAll": ahead,
        })
        if disp == SUPERSEDED_BY:
            row["supersededBy"] = {"ref": base_ref, "sha": base_sha}
        rows.append(row)
    meta = {"heads": len(heads), "remotes": len(remotes),
            "others": len(others), "frozenRefs": len(entries)}
    return rows, meta


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
# worktree status (nonzero-returncode + rename-aware -z parsing)
# --------------------------------------------------------------------------- #
def porcelain_entries_z(data: bytes):
    """Yield status, destination and optional rename origin without losing paths."""
    fields = iter(data.split(b"\x00"))
    for field in fields:
        if not field:
            continue
        if len(field) < 4 or field[2:3] != b" ":
            raise ValueError("malformed porcelain record")
        code = field[:2].decode("ascii")
        path = field[3:].decode("utf-8", "surrogateescape")
        origin = None
        if "R" in code or "C" in code:
            raw_origin = next(fields, b"")
            if not raw_origin:
                raise ValueError("missing rename origin")
            origin = raw_origin.decode("utf-8", "surrogateescape")
        yield code, path, origin


def parse_porcelain_z(data: bytes):
    """Parse `git status --porcelain=v1 -z`. Returns (tracked, staged, untracked,
    rename_entries) counting entries; rename records consume an extra token."""
    tracked = staged = untracked = 0
    renames = []
    for code, path, origin in porcelain_entries_z(data):
        if origin is not None:
            renames.append({"code": code, "dest": path, "src": origin})
        if code == "??":
            untracked += 1
        elif code[0] not in " ?" and code[1] not in " ?":
            staged += 1
            tracked += 1
        elif code[1] != " ":
            tracked += 1
        else:
            staged += 1
    return tracked, staged, untracked, renames


def worktree_dirty(path: pathlib.Path):
    meta = {"exists": path.is_dir(), "dirty": None, "statusError": False,
            "statusTimeout": False, "trackedChanges": 0, "stagedChanges": 0,
            "untrackedCount": 0, "renames": [], "sensitivePathsReported": 0,
            "sensitivePaths": []}
    if not meta["exists"]:
        return meta
    p = run(["git", "-C", str(path), "status", "--porcelain=v1", "-z",
             "--untracked-files=normal"], timeout=STATUS_TIMEOUT)
    if p is None:
        meta["statusTimeout"] = True
        return meta
    if p.returncode != 0:
        meta["statusError"] = True
        return meta
    try:
        tracked, staged, untracked, renames = parse_porcelain_z(p.stdout)
    except (ValueError, UnicodeError):
        meta["statusError"] = True
        return meta
    meta["trackedChanges"] = tracked
    meta["stagedChanges"] = staged
    meta["untrackedCount"] = untracked
    meta["renames"] = renames[:20]
    sensitive = set()
    for _, path_name, origin in porcelain_entries_z(p.stdout):
        for name in (path_name, origin):
            if name and is_sensitive_path(name):
                sensitive.add(name)
    meta["sensitivePathsReported"] = len(sensitive)
    meta["sensitivePaths"] = sorted(sensitive)[:20]
    meta["dirty"] = bool(tracked or staged or untracked or renames)
    return meta


def collect_worktrees(frozen: pathlib.Path, check_dirty: bool):
    descs = read_frozen_worktrees(frozen)
    rows = []
    for d in descs:
        p = pathlib.Path(d["path"])
        row = {
            "path": d["path"], "head": d.get("head"), "branch": d.get("branch"),
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
    status = "ok"
    out = git_out("log", "-g", "--format=%H%x00%gs", "refs/stash")
    if out is None:
        status = "unavailable"
        out = ""
    for line in out.splitlines():
        sha, _, subject = line.partition("\x00")
        rows.append({"stashSha": sha.strip(), "subject": subject.strip()})
    out2 = git_out("for-each-ref", "--format=%(refname) %(objectname)", "refs/stash")
    if out2 is None:
        status = "unavailable"
        out2 = ""
    ref = [{"ref": l.split()[0], "sha": l.split()[1]}
           for l in out2.splitlines() if l.strip()]
    return {"entries": rows, "ref": ref, "status": status,
            "note": "recorded read-only; never popped or applied"}


def collect_evidence_refs(frozen: pathlib.Path, an: Analyzer):
    rows, seen = [], set()
    status = "ok"
    for line in (frozen / "archive-refs.txt").read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        ref, _, sha = line.partition(" ")
        seen.add(ref)
        rows.append({"ref": ref, "sha": sha.strip(), "source": "frozen",
                     "objectPresent": an.object_exists(sha.strip())})
    live = git_out("for-each-ref", "--format=%(refname) %(objectname)",
                   "refs/archive", "refs/ledger")
    if live is None:
        status = "unavailable"
        live = ""
    for line in live.splitlines():
        if not line.strip():
            continue
        ref, _, sha = line.partition(" ")
        if ref in seen:
            continue
        rows.append({"ref": ref, "sha": sha.strip(), "source": "live",
                     "objectPresent": an.object_exists(sha.strip())})
    return rows, status


# --------------------------------------------------------------------------- #
# dirty preservation (separate staged/unstaged, bounded, no owner mutation)
# --------------------------------------------------------------------------- #
def preserve_dirty(worktrees, snapshot: pathlib.Path, report):
    report["preservedWorktrees"] = []
    report["preservedBytes"] = 0
    try:
        snapshot.mkdir(parents=True, exist_ok=False)
    except OSError as exc:
        report["gaps"].append({"worktree": "*", "path": str(snapshot),
                               "reason": f"snapshot creation refused ({exc.__class__.__name__}); existing data untouched"})
        return report
    total = 0
    preserved = []
    for wt in worktrees:
        if not wt.get("exists") or wt.get("dirty") is None or wt.get("statusError") or wt.get("statusTimeout"):
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "worktree missing or status unavailable; preservation unverified"})
            continue
        if not wt.get("dirty"):
            continue
        wtpath = pathlib.Path(wt["path"])
        name = hashlib.sha1(str(wtpath).encode()).hexdigest()[:12]
        outdir = snapshot / name
        try:
            outdir.mkdir(exist_ok=False)
        except OSError as exc:
            report["gaps"].append({"worktree": wt["path"], "path": str(outdir),
                                   "reason": f"worktree snapshot refused ({exc.__class__.__name__})"})
            continue
        observed_head = git_out("rev-parse", "HEAD", cwd=wtpath)
        if observed_head is None:
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "observed HEAD unavailable; preservation unverified"})
            continue
        head = observed_head.strip()
        # Disabling rename collapsing exposes BOTH sides, including a sensitive
        # origin renamed to an innocent destination before an unstaged edit.
        staged_names = git_bytes("diff", "--cached", "--no-renames", "--name-only", "-z", cwd=wtpath)
        unstaged_names = git_bytes("diff", "--no-renames", "--name-only", "-z", cwd=wtpath)
        if staged_names is None or unstaged_names is None:
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "diff path inventory unavailable; not preserved"})
            continue
        staged_paths = [p.decode("utf-8", "surrogateescape") for p in staged_names.split(b"\0") if p]
        unstaged_paths = [p.decode("utf-8", "surrogateescape") for p in unstaged_names.split(b"\0") if p]
        # Check names BEFORE materializing any tracked secret content.
        if any(is_sensitive_path(p) for p in staged_paths + unstaged_paths):
            for p in staged_paths + unstaged_paths:
                if is_sensitive_path(p):
                    report["sensitiveUntouched"].append({"worktree": wt["path"], "path": p})
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "tracked diff touches sensitive path; left in place"})
            continue
        staged = git_bytes("diff", "--cached", "--binary", cwd=wtpath,
                           timeout=STATUS_TIMEOUT * 2)
        unstaged = git_bytes("diff", "--binary", cwd=wtpath,
                             timeout=STATUS_TIMEOUT * 2)
        if staged is None or unstaged is None:
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "diff unavailable or output budget exceeded; not preserved"})
            continue
        if total + len(staged) + len(unstaged) > MAX_TOTAL_BYTES:
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "tracked patch run budget exceeded; left in place"})
            report["overBudget"].append({"worktree": wt["path"], "path": "*"})
            continue
        if staged:
            (outdir / "staged.patch").write_bytes(staged)
            total += len(staged)
        if unstaged:
            (outdir / "unstaged.patch").write_bytes(unstaged)
            total += len(unstaged)
        others = git_out("ls-files", "--others", "--exclude-standard", "-z", cwd=wtpath)
        if others is None:
            report["gaps"].append({"worktree": wt["path"], "path": "*",
                                   "reason": "untracked inventory unavailable"})
            others = ""
        untracked_names = [x for x in others.split("\x00") if x]
        copied = []
        links = []
        for rel in untracked_names:
            relpath = wtpath / rel
            parts = set(rel.split("/"))
            if parts & IGNORE_DIR_PARTS:
                report["gaps"].append({"worktree": wt["path"], "path": rel,
                                       "reason": "ignored/known-useful build tree; recorded not copied"})
                continue
            if is_sensitive_path(rel):
                report["sensitiveUntouched"].append({"worktree": wt["path"], "path": rel})
                report["gaps"].append({"worktree": wt["path"], "path": rel,
                                       "reason": "sensitive path; left in place"})
                continue
            try:
                if relpath.is_symlink():
                    target = os.readlink(relpath)
                    size = len(os.fsencode(target))
                    if total + size > MAX_TOTAL_BYTES:
                        report["gaps"].append({"worktree": wt["path"], "path": rel,
                                               "reason": "symlink metadata run budget exceeded; left in place"})
                        report["overBudget"].append({"worktree": wt["path"], "path": rel, "bytes": size})
                        continue
                    links.append({"path": rel, "target": target})
                    total += size
                    continue
                if not relpath.is_file():
                    report["gaps"].append({"worktree": wt["path"], "path": rel,
                                           "reason": "non-regular file skipped"})
                    continue
                size = relpath.stat().st_size
                if size > MAX_FILE_BYTES or total + size > MAX_TOTAL_BYTES:
                    report["gaps"].append({"worktree": wt["path"], "path": rel,
                                           "reason": f"over budget ({size} bytes); left in place"})
                    report["overBudget"].append({"worktree": wt["path"], "path": rel, "bytes": size})
                    continue
                # Read the opened regular file under the actual byte budget;
                # a stale stat must not turn a small file into unbounded capture
                # or follow a final-component symlink introduced during capture.
                descriptor = os.open(relpath, os.O_RDONLY | os.O_NOFOLLOW)
                with os.fdopen(descriptor, "rb") as source:
                    if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                        raise OSError("not a regular file")
                    content = source.read(min(MAX_FILE_BYTES, MAX_TOTAL_BYTES - total) + 1)
                if len(content) > MAX_FILE_BYTES or total + len(content) > MAX_TOTAL_BYTES:
                    report["gaps"].append({"worktree": wt["path"], "path": rel,
                                           "reason": "file grew beyond capture budget; left in place"})
                    report["overBudget"].append({"worktree": wt["path"], "path": rel, "bytes": len(content)})
                    continue
                dest = outdir / "untracked" / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(content)
                total += len(content)
                copied.append(rel)
            except OSError as exc:
                report["gaps"].append({"worktree": wt["path"], "path": rel,
                                       "reason": f"read/write error: {exc.__class__.__name__}"})
        meta = {
            "worktree": wt["path"], "head": head, "branch": wt.get("branch"),
            "observedAt": _dt.datetime.now(_dt.timezone.utc).isoformat(),
            "stagedBytes": len(staged), "unstagedBytes": len(unstaged),
            "stagedPaths": staged_paths, "unstagedPaths": unstaged_paths,
            "untrackedCopied": copied, "untrackedTotal": len(untracked_names),
            "untrackedLinks": len(links),
        }
        if links:
            (outdir / "links.json").write_text(json.dumps(links, indent=2) + "\n",
                                               encoding="utf-8")
        (outdir / "meta.json").write_text(json.dumps(meta, indent=2) + "\n",
                                          encoding="utf-8")
        preserved.append({"worktree": wt["path"], "dir": str(outdir),
                           "head": head, "stagedBytes": len(staged),
                           "unstagedBytes": len(unstaged), "untracked": len(copied),
                           "untrackedLinks": len(links)})
    report["preservedWorktrees"] = preserved
    report["preservedBytes"] = total
    return report


# --------------------------------------------------------------------------- #
# self-check fixtures (fail-closed behaviour)
# --------------------------------------------------------------------------- #
def _mk_git(env):
    def g(*args, cwd):
        e = dict(os.environ)
        e.update(env)
        return subprocess.run(["git", *args], cwd=str(cwd), env=e,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              timeout=30)
    return g


def self_check() -> int:
    failures = []
    with tempfile.TemporaryDirectory(prefix="salvage-selfcheck-") as td:
        root = pathlib.Path(td)
        env = {"GIT_AUTHOR_NAME": "chk", "GIT_AUTHOR_EMAIL": "chk@x",
               "GIT_COMMITTER_NAME": "chk", "GIT_COMMITTER_EMAIL": "chk@x"}
        g = _mk_git(env)

        def init_repo(path):
            path.mkdir(parents=True, exist_ok=True)
            g("init", "-q", "-b", "main", cwd=path)
            return path

        # --- fixture 1: git failure must fail closed -------------------------
        an = Analyzer("0" * 40, cwd=root / "does-not-exist")
        nov = an.novelty("1" * 40)
        if nov is not None:
            failures.append("git failure did not return None from novelty")
        disp, acc, _ = decide(False, False, False, 0, 0, 0, [])
        if (disp, acc) != (PENDING_REVIEW, "NOT_ACCEPTED"):
            failures.append(f"fail-closed decide -> {disp}/{acc}")
        if classify(False, []) != (PENDING_REVIEW, "NOT_ACCEPTED"):
            failures.append("classify(empty) not PENDING")

        # --- fixture 2: merge-only unique history -> PENDING -----------------
        # base already contains A and B (via octopus merge M1). A second merge
        # M2 whose parents A and B are both ancestors of base leaves only the
        # merge commit itself in base..M2 -> merge-only unique history.
        repo = init_repo(root / "mergeonly")
        (repo / "f.txt").write_text("base\n")
        g("add", "-A", cwd=repo)
        g("commit", "-q", "-m", "c0", cwd=repo)
        g("checkout", "-q", "-b", "sideA", cwd=repo)
        (repo / "a.txt").write_text("A\n")
        g("add", "-A", cwd=repo)
        g("commit", "-q", "-m", "A", cwd=repo)
        g("checkout", "-q", "main", cwd=repo)
        g("checkout", "-q", "-b", "sideB", cwd=repo)
        (repo / "b.txt").write_text("B\n")
        g("add", "-A", cwd=repo)
        g("commit", "-q", "-m", "B", cwd=repo)
        g("checkout", "-q", "main", cwd=repo)
        g("merge", "-q", "--no-ff", "-m", "base merge A+B", "sideA", "sideB", cwd=repo)
        base = g("rev-parse", "HEAD", cwd=repo).stdout.decode().strip()
        g("checkout", "-q", "-b", "tmp", "sideA", cwd=repo)
        g("merge", "-q", "--no-ff", "-m", "conflict resolution only", "sideB", cwd=repo)
        merge_head = g("rev-parse", "HEAD", cwd=repo).stdout.decode().strip()
        an2 = Analyzer(base, cwd=repo)
        res = an2.novelty(merge_head)
        if res is None:
            failures.append("merge-only novelty returned None")
        else:
            unique, novel, inherited, merge_only, mcount = res
            if not merge_only:
                failures.append(f"merge-only not detected: unique={len(unique)}")
            d, a, _ = decide(True, False, merge_only, mcount, len(novel), len(inherited), [])
            if (d, a) != (PENDING_REVIEW, "NOT_ACCEPTED"):
                failures.append(f"merge-only decide -> {d}/{a}")

        # --- fixture 3: staged + unstaged + rename separation ----------------
        repo3 = init_repo(root / "dirty")
        (repo3 / "a.txt").write_text("one\n")
        (repo3 / "c.txt").write_text("c\n")
        g("add", "-A", cwd=repo3)
        g("commit", "-q", "-m", "c0", cwd=repo3)
        (repo3 / "a.txt").write_text("two\n")          # unstaged modify
        (repo3 / "b.txt").write_text("new\n")          # staged add
        g("add", "b.txt", cwd=repo3)
        g("mv", "c.txt", "d.txt", cwd=repo3)           # staged rename
        dstat = worktree_dirty(repo3)
        if dstat.get("statusError") or dstat.get("statusTimeout"):
            failures.append("worktree_dirty status error on fixture")
        if dstat["stagedChanges"] < 2:
            failures.append(f"staged count {dstat['stagedChanges']}")
        if dstat["untrackedCount"] != 0 and dstat["trackedChanges"] < 2:
            failures.append("tracked count wrong on fixture")
        snap = root / "snap"
        rep = {"gaps": [], "sensitiveUntouched": [], "overBudget": []}
        preserve_dirty([{"path": str(repo3), "exists": True, "dirty": True,
                         "head": "x", "branch": "main"}], snap, rep)
        sub = next((snap).iterdir(), None)
        if sub is None or not (sub / "staged.patch").exists() or not (sub / "unstaged.patch").exists():
            failures.append("staged.patch/unstaged.patch not separate")
        else:
            sp = (sub / "staged.patch").read_text()
            up = (sub / "unstaged.patch").read_text()
            if "b.txt" not in sp:
                failures.append("staged add missing from staged.patch")
            if "a.txt" not in up:
                failures.append("unstaged modify missing from unstaged.patch")
            if "a.txt" in sp:
                failures.append("unstaged change leaked into staged.patch")
            meta = json.loads((sub / "meta.json").read_text())
            if "d.txt" not in meta["stagedPaths"] and "d.txt" not in sp:
                failures.append("rename dest not recorded")

        # --- fixture 4: safe source name vs real secret ----------------------
        if is_sensitive_path("tests/e2e/browser_credential_launch.rs"):
            failures.append(".rs source false-positive still sensitive")
        if not is_sensitive_path(".env"):
            failures.append(".env not sensitive")
        if not is_sensitive_path("opencode.json"):
            failures.append("opencode.json not sensitive")
        if is_sensitive_path("crates/x/src/credentials.rs"):
            failures.append("crates .rs source false-positive")

        # --- fixture 5: isolated staged-only worktree must be captured --------
        staged_repo = init_repo(root / "stagedonly")
        (staged_repo / "README.md").write_text("r\n")
        g("add", "-A", cwd=staged_repo)
        g("commit", "-q", "-m", "c0", cwd=staged_repo)
        (staged_repo / "staged_only.rs").write_text("fn main(){}\n")
        g("add", "staged_only.rs", cwd=staged_repo)
        dstat = worktree_dirty(staged_repo)
        if dstat["stagedChanges"] != 1 or dstat["untrackedCount"] != 0:
            failures.append(f"staged-only counts wrong: {dstat}")
        if not dstat["dirty"]:
            failures.append("staged-only worktree reported clean (staged omitted from dirty)")
        snap2 = root / "snap-staged"
        rep2 = {"gaps": [], "sensitiveUntouched": [], "overBudget": []}
        preserve_dirty([{"path": str(staged_repo), "exists": True, "dirty": True,
                         "head": "x", "branch": "main"}], snap2, rep2)
        sub2 = next(snap2.iterdir(), None)
        if sub2 is None or not (sub2 / "staged.patch").exists():
            failures.append("staged-only staged.patch missing")
        else:
            if "staged_only.rs" not in (sub2 / "staged.patch").read_text():
                failures.append("staged-only content missing from staged.patch")
            meta2 = json.loads((sub2 / "meta.json").read_text())
            if "staged_only.rs" not in meta2["stagedPaths"]:
                failures.append("staged-only path missing from meta")

        # --- fixture 6: mixed unique-merge (non-merge patches matched) -> PENDING
        # base already contains P by patch-id (cherry-pick). R merges the original
        # P branch: base..R = {P (patch-id matched), M (unique merge)} -> novel 0,
        # merge_count 1 -> PENDING (resolution not inspected).
        repo6 = init_repo(root / "mixedmerge")
        (repo6 / "f.txt").write_text("base\n")
        g("add", "-A", cwd=repo6)
        g("commit", "-q", "-m", "c0", cwd=repo6)
        original_base = g("rev-parse", "HEAD", cwd=repo6).stdout.decode().strip()
        g("checkout", "-q", "-b", "work", cwd=repo6)
        (repo6 / "p.txt").write_text("P\n")
        g("add", "-A", cwd=repo6)
        g("commit", "-q", "-m", "P", cwd=repo6)
        psha = g("rev-parse", "HEAD", cwd=repo6).stdout.decode().strip()
        g("checkout", "-q", "main", cwd=repo6)
        (repo6 / "base-only.txt").write_text("intervening base\n")
        g("add", "base-only.txt", cwd=repo6)
        g("commit", "-q", "-m", "advance base before cherry-pick", cwd=repo6)
        advance = g("rev-parse", "HEAD", cwd=repo6).stdout.decode().strip()
        g("cherry-pick", psha, cwd=repo6)            # patch-id now in base
        base6 = g("rev-parse", "HEAD", cwd=repo6).stdout.decode().strip()
        g("checkout", "-q", "-b", "r6", original_base, cwd=repo6)
        g("cherry-pick", "--no-commit", advance, cwd=repo6)
        g("commit", "-q", "-m", "same advance with separate ancestry", cwd=repo6)
        g("merge", "-q", "--no-ff", "-m", "merge original P", "work", cwd=repo6)
        r6 = g("rev-parse", "HEAD", cwd=repo6).stdout.decode().strip()
        an6 = Analyzer(base6, cwd=repo6)
        res6 = an6.novelty(r6)
        if res6 is None:
            failures.append("mixed-merge novelty returned None")
        else:
            unique, novel, inherited, merge_only, mcount = res6
            if merge_only or len(novel) != 0 or mcount < 1:
                failures.append(f"mixed-merge shape unexpected: novel={len(novel)} mcount={mcount}")
            d6, a6, _ = decide(True, False, merge_only, mcount, len(novel), len(inherited), [])
            if (d6, a6) != (PENDING_REVIEW, "NOT_ACCEPTED"):
                failures.append(f"mixed-merge decide -> {d6}/{a6}")

    if failures:
        print("SELF-CHECK FAILED:")
        for f in failures:
            print("  - " + f)
        return 1
    print("SELF-CHECK OK: fail-closed=PENDING merge-only=PENDING "
          "unique-merge=PENDING staged-only=captured "
          "staged/unstaged/rename=separate safe-source=ok secret-deny=ok")
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
    ap.add_argument("--base-sha", default=None)
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

    if args.preserve_dirty and not args.snapshot_dir:
        ap.error("--preserve-dirty requires --snapshot-dir")
    if args.preserve_dirty and not args.check_dirty:
        ap.error("--preserve-dirty requires worktree status checks")

    if args.self_check:
        return self_check()

    frozen = pathlib.Path(args.frozen_root)
    base_sha = (args.base_sha or git_out("rev-parse", args.base_ref) or "").strip()
    if not base_sha:
        print("cannot resolve base sha", file=sys.stderr)
        return 2

    an = Analyzer(base_sha)
    rows, ref_meta = inventory_refs(an, frozen, args.base_ref, base_sha)
    groups = candidate_groups(rows)
    worktrees = collect_worktrees(frozen, args.check_dirty)
    detached = collect_detached(frozen, an)
    stash = collect_stash()
    evidence, evidence_status = collect_evidence_refs(frozen, an)

    ref_counts = Counter(r["disposition"] for r in rows)
    accepted = {"INTEGRATED_ANCESTOR"}
    intake_counts = Counter(
        r["disposition"] for r in rows if r["disposition"] not in accepted
    )
    alias_sum = sum(r["aliasCount"] for r in rows)
    validation = {
        "refsAccounted": (alias_sum + len(rows)) == ref_meta["frozenRefs"],
        "frozenRefs": ref_meta["frozenRefs"],
        "uniqueTips": len(rows),
        "aliasedRefs": alias_sum,
        "detachedAnchors": len(detached),
        "detachedExpected": EXPECTED_DETACHED,
        "detachedAllValid": all(x["objectPresent"] for x in detached),
        "worktreesObserved": len(worktrees),
        "worktreesExpected": EXPECTED_WORKTREES,
        "analysisFailures": [r["primaryRef"] for r in rows if r.get("analysisFailure")],
        "mergeOnlyPending": [r["primaryRef"] for r in rows if r.get("mergeOnly")],
        "uniqueMergePending": [r["primaryRef"] for r in rows
                               if (r.get("uniqueMergeCount") or 0) > 0
                               and r["disposition"] == PENDING_REVIEW],
        "stashStatus": stash.get("status", "ok"),
        "evidenceStatus": evidence_status,
        "dirtyWorktrees": [w["path"] for w in worktrees if w.get("dirty")],
        "stagedOnlyWorktrees": [w["path"] for w in worktrees
                                if w.get("dirty") and (w.get("stagedChanges") or 0) > 0
                                and (w.get("trackedChanges") or 0) == 0
                                and (w.get("untrackedCount") or 0) == 0],
        "supersededNeedsBase": all(
            (r.get("supersededBy") or {}).get("sha") for r in rows
            if r["disposition"] == SUPERSEDED_BY
        ),
    }

    payload = {
        "schemaVersion": 3,
        "generatedAt": _dt.datetime.now(_dt.timezone.utc).isoformat(),
        "baseRef": args.base_ref,
        "baseSha": base_sha,
        "canonicalContext": ("explicit base context; not stale acceptance"),
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
            "aliasedRefs": alias_sum,
            "byRef": dict(sorted(ref_counts.items())),
            "intakeByUniqueTip": dict(sorted(intake_counts.items())),
        },
        "distinction": {
            "intake": "acceptance NOT_ACCEPTED = unintegrated candidate",
            "final": "INTEGRATED only for exact ancestry; SUPERSEDED_BY is compatibility evidence, not gate acceptance",
        },
        "validation": validation,
        "refs": rows,
        "candidateGroups": groups,
        "worktrees": worktrees,
        "worktreeSummary": {
            "total": len(worktrees),
            "present": sum(1 for w in worktrees if w.get("exists")),
            "missing": sum(1 for w in worktrees if not w.get("exists")),
            "dirty": sum(1 for w in worktrees if w.get("dirty")),
            "statusError": sum(1 for w in worktrees if w.get("statusError")),
            "statusTimeout": sum(1 for w in worktrees if w.get("statusTimeout")),
        },
        "detachedAnchors": detached,
        "stash": stash,
        "evidenceRefs": evidence,
    }

    report = {"gaps": [], "sensitiveUntouched": [], "overBudget": [],
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
            "gaps": report["gaps"],
            "g0": "pending" if (report["gaps"] or report["sensitiveUntouched"]) else "clean",
        }

    json_path = ROOT / args.json
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n",
                         encoding="utf-8")

    md = [
        "# Convergence V2 salvage inventory (frozen-authority, fail-closed)",
        "",
        f"Base: `{args.base_ref}` / `{base_sha}`",
        f"Frozen authority: `{frozen}`",
        "",
        "Generated by `tools/convergence_v2_salvage.py`. Intake inventory only; "
        "nothing here authorises deletion. Git failures and merge-only history are "
        "PENDING_REVIEW. SUPERSEDED_BY is compatibility evidence, not acceptance.",
        "",
        "## Counts",
        "",
        f"- frozen refs: {ref_meta['frozenRefs']} "
        f"(heads {ref_meta['heads']}, remotes {ref_meta['remotes']}, others {ref_meta['others']})",
        f"- unique tips: {len(rows)} (aliases: {alias_sum})",
        f"- refs accounted: {validation['refsAccounted']}",
        "",
        "### By unique tip",
        "",
    ]
    for k, v in sorted(ref_counts.items()):
        md.append(f"- `{k}`: {v}")
    md += ["", "### Intake groups", ""]
    for gg in groups:
        md.append(f"- `{gg['disposition']}`: {gg['uniqueTips']} tips, "
                  f"{gg['totalNovelCommits']} novel commits, {gg['aliasRefs']} aliases")
    md += [
        "",
        "## Validation",
        "",
        f"- analysis failures: {len(validation['analysisFailures'])}",
        f"- merge-only pending: {len(validation['mergeOnlyPending'])}",
        f"- unique-merge pending: {len(validation['uniqueMergePending'])}",
        f"- stash status: {validation['stashStatus']}",
        f"- evidence status: {validation['evidenceStatus']}",
        f"- dirty worktrees: {len(validation['dirtyWorktrees'])}",
        f"- staged-only worktrees: {len(validation['stagedOnlyWorktrees'])}",
        f"- detached: {len(detached)}/{EXPECTED_DETACHED} valid={validation['detachedAllValid']}",
        f"- worktrees: {len(worktrees)}/{EXPECTED_WORKTREES}",
        "",
        "## Worktrees",
        "",
        f"- total {payload['worktreeSummary']['total']}, "
        f"dirty {payload['worktreeSummary']['dirty']}, "
        f"status-error {payload['worktreeSummary']['statusError']}, "
        f"status-timeout {payload['worktreeSummary']['statusTimeout']}",
        "",
        "## Anchors",
        "",
        f"- stash entries: {len(stash['entries'])} (read-only)",
        f"- archive/ledger evidence refs: {len(evidence)}",
        "",
    ]
    if args.preserve_dirty and args.snapshot_dir:
        md += ["## Preservation", "",
               f"- dir: `{args.snapshot_dir}`",
               f"- preserved worktrees: {len(report['preservedWorktrees'])}",
               f"- bytes: {report['preservedBytes']}",
               f"- gaps: {len(report['gaps'])}",
               f"- sensitive untouched: {len(report['sensitiveUntouched'])}",
               ""]
    (ROOT / args.markdown).write_text("\n".join(md), encoding="utf-8")
    print(json.dumps({
        "base": base_sha, "frozenRefs": ref_meta["frozenRefs"],
        "uniqueTips": len(rows), "counts": dict(sorted(ref_counts.items())),
        "worktrees": payload["worktreeSummary"],
        "validation": validation,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

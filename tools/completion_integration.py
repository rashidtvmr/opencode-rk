#!/usr/bin/env python3
"""Serialized VCS integration gate (COORD-005, stdlib + git subprocess only).

Companion to tools/completion_scheduler.py (HEAD 5af7884): scheduler
finalize 189-203 serializes verify -> integrate -> verify_integrated and
turns an ambiguous integration outcome into reconciliation instead of a
blind re-run; tools/ralph_loop.py finalize_worktree 421-448 commits the
lane branch and fast-forwards mainline, record_result 601-608 keeps the
receipt. This module is the trusted git half of that pipeline.

Gate order per candidate, all through list-form git argv (no shell):

1. ff-only ancestry check via ``git merge-base --is-ancestor`` (10 s
   timeout): mainline tip must be an ancestor of the candidate tip.
2. clean-worktree-with-unintegrated-branch rejection: a clean worktree is
   a no-op success only when the branch tip is already contained in the
   mainline; otherwise it raises instead of reporting success.
3. serialized ``git merge --ff-only`` under a cross-process lock. Merge
   failure leaves the task unaccepted (dependents stay blocked) and keeps
   the branch; diverged branches are kept for worker rebase so neither
   contribution is discarded.
4. post-merge rerun hook: the rerun proof must explicitly pass or
   acceptance is refused, even when pre-merge verification passed.
5. atomic acceptance receipt binding integrated revision + frozen test
   hash + successful rerun (temp file + os.replace + fsync).

Ambiguous side effects (merge timeout / killed process) raise
RetryableFailure and must go through reconcile(), which only reads
ancestry and never re-runs the merge. Re-running the rerun hook is safe
(verification has no side effects); re-running the merge blindly is not.

No processes are launched except git; no network, no threads beyond one
process-local guard, no side effects on import.
"""
from __future__ import annotations

import contextlib
import json
import os
import pathlib
import re
import subprocess
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_LOCK_PATH = ROOT / "state" / "completion-integration.lock"
DEFAULT_RECEIPTS_DIR = ROOT / "state" / "completion-integration-receipts"

GIT_TIMEOUT = 10.0
MAX_RECEIPT_BYTES = 64 * 1024

REV = re.compile(r"[0-9a-f]{40}\Z")
HASH = re.compile(r"[0-9a-f]{64}\Z")
TASK_ID = re.compile(r"[A-Z]+-[0-9]{3}\Z")
_REF = re.compile(r"(?:HEAD|main|[A-Za-z0-9][A-Za-z0-9/_.\-]{0,127})\Z")

_LOCAL = threading.Lock()


class Rejected(RuntimeError):
    """Gate refused: task stays unaccepted, dependents stay blocked, no retry."""


class RetryableFailure(RuntimeError):
    """Outcome ambiguous (timeout/unavailable): reconcile first, never blindly retry."""


def _check_ref(ref: str) -> str:
    if not isinstance(ref, str) or not _REF.fullmatch(ref):
        raise Rejected(f"unsafe git ref: {ref!r}")
    if ".." in ref or "@{" in ref or ref.endswith((".", "/")):
        raise Rejected(f"unsafe git ref: {ref!r}")
    return ref


def _run_git(repo: str | os.PathLike, args: list[str] | tuple[str, ...],
             *, timeout: float = GIT_TIMEOUT) -> subprocess.CompletedProcess:
    repo = pathlib.Path(repo)
    if not isinstance(args, (list, tuple)) or not args:
        raise Rejected("git argv must be a nonempty list of strings")
    if any(not isinstance(a, str) or not a for a in args):
        raise Rejected("git argv must be a nonempty list of strings")
    if not isinstance(timeout, (int, float)) or not timeout > 0:
        raise Rejected("positive git timeout required")
    try:
        return subprocess.run(
            ["git", *args], cwd=repo,
            capture_output=True, text=True, timeout=timeout, check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise RetryableFailure(
            f"git {' '.join(args)} timed out after {timeout}s; "
            "outcome unknown, reconcile before any retry") from exc
    except OSError as exc:
        raise RetryableFailure(f"git unavailable: {exc}") from exc


def rev_parse(repo: str | os.PathLike, ref: str = "HEAD",
              *, timeout: float = GIT_TIMEOUT) -> str:
    """Resolve ref to a 40-hex commit; unknown refs raise Rejected."""
    proc = _run_git(repo, ["rev-parse", "--verify", _check_ref(ref)], timeout=timeout)
    if proc.returncode != 0:
        raise Rejected(f"unknown revision: {ref}")
    rev = proc.stdout.strip()
    if not REV.fullmatch(rev):
        raise Rejected(f"unresolvable revision: {ref}")
    return rev


def is_ancestor(repo: str | os.PathLike, ancestor_rev: str, descendant_rev: str,
                *, timeout: float = GIT_TIMEOUT) -> bool:
    """ff-only ancestry probe via ``git merge-base --is-ancestor`` (10 s)."""
    for rev in (ancestor_rev, descendant_rev):
        if not isinstance(rev, str) or not REV.fullmatch(rev):
            raise Rejected("ancestry check needs resolved 40-hex revisions")
    proc = _run_git(repo, ["merge-base", "--is-ancestor", ancestor_rev, descendant_rev],
                    timeout=timeout)
    if proc.returncode in (0, 1):
        return proc.returncode == 0
    raise Rejected(f"merge-base failed: {proc.stderr.strip()[:200]}")


def worktree_clean(repo: str | os.PathLike, *, timeout: float = GIT_TIMEOUT) -> bool:
    """True only when ``git status --porcelain`` reports no changes at all."""
    proc = _run_git(repo, ["status", "--porcelain=v1", "--untracked-files=all"],
                    timeout=timeout)
    if proc.returncode != 0:
        raise Rejected(f"git status failed: {proc.stderr.strip()[:200]}")
    return proc.stdout.strip() == ""


def branch_tip(repo: str | os.PathLike, branch: str,
               *, timeout: float = GIT_TIMEOUT) -> str | None:
    """Branch tip revision, or None when the branch does not exist."""
    proc = _run_git(repo, ["rev-parse", "--verify", _check_ref(branch)], timeout=timeout)
    if proc.returncode != 0:
        return None
    tip = proc.stdout.strip()
    if not REV.fullmatch(tip):
        raise Rejected(f"unresolvable branch: {branch}")
    return tip


def check_clean_means_integrated(repo: str | os.PathLike, branch: str,
                                 *, main_ref: str = "HEAD",
                                 timeout: float = GIT_TIMEOUT) -> bool:
    """A clean worktree is a no-op success only if the branch is contained.

    Raises Rejected when the branch holds precommitted changes the
    mainline does not contain, so a clean worktree is never mistaken for
    an integrated candidate (COORD-005-T02).
    """
    tip = branch_tip(repo, branch, timeout=timeout)
    if tip is None:
        return True
    main_rev = rev_parse(repo, main_ref, timeout=timeout)
    if tip == main_rev or is_ancestor(repo, tip, main_rev, timeout=timeout):
        return True
    raise Rejected(
        f"clean worktree still holds unintegrated branch changes on "
        f"{branch}; refusing no-op success")


class IntegrationLock:
    """Serialized gate: one integrator across threads and processes.

    Process-local threading guard plus a cross-process file lock
    (fcntl, O_EXCL sentinel fallback). Bounded wait; raises
    RetryableFailure instead of queuing without bound.
    """

    def __init__(self, path: str | os.PathLike | None = None, *,
                 timeout: float = 60.0) -> None:
        self.path = pathlib.Path(path) if path is not None else DEFAULT_LOCK_PATH
        self.timeout = timeout
        self._handle = None
        self._sentinel_created = False

    def __enter__(self) -> "IntegrationLock":
        if (type(self.timeout) not in (int, float)) or not self.timeout > 0:
            raise Rejected("positive lock timeout required")
        deadline = time.monotonic() + self.timeout
        while not _LOCAL.acquire(blocking=False):
            if time.monotonic() >= deadline:
                raise RetryableFailure("integration gate busy (local)")
            time.sleep(0.01)
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            try:
                import fcntl  # noqa: PLC0415
            except ImportError:
                fcntl = None  # type: ignore[assignment]
            if fcntl is not None:
                handle = open(self.path, "a+b")
                self._handle = handle
                while True:
                    try:
                        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                        return self
                    except OSError:
                        if time.monotonic() >= deadline:
                            with contextlib.suppress(OSError):
                                handle.close()
                            self._handle = None
                            raise RetryableFailure("integration gate busy; serialized")
                        time.sleep(0.05)
            while True:
                try:
                    fd = os.open(str(self.path), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                    os.close(fd)
                    self._sentinel_created = True
                    return self
                except FileExistsError:
                    if time.monotonic() >= deadline:
                        raise RetryableFailure("integration gate busy; serialized")
                    time.sleep(0.05)
        except BaseException:
            _LOCAL.release()
            raise

    def __exit__(self, *exc: object) -> None:
        try:
            if self._handle is not None:
                handle, self._handle = self._handle, None
                try:
                    import fcntl  # noqa: PLC0415

                    with contextlib.suppress(OSError):
                        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
                except ImportError:
                    pass
                with contextlib.suppress(OSError):
                    handle.close()
            elif self._sentinel_created:
                self._sentinel_created = False
                with contextlib.suppress(OSError):
                    self.path.unlink()
        finally:
            _LOCAL.release()


def integrate(task_id: str, repo: str | os.PathLike, branch: str,
              candidate_revision: str, *, main_ref: str = "HEAD",
              lock_path: str | os.PathLike | None = None,
              lock_timeout: float = 60.0,
              timeout: float = GIT_TIMEOUT) -> str:
    """Merge the verified candidate tip with ``git merge --ff-only``.

    Returns the integrated mainline revision. Already-contained tips are
    an idempotent no-op returning the current tip. Anything else that is
    not a clean fast-forward raises Rejected (task unaccepted, branch
    kept, dependents blocked); ambiguous git outcomes raise
    RetryableFailure for reconcile() instead of a blind retry.
    """
    if not isinstance(task_id, str) or not TASK_ID.fullmatch(task_id):
        raise Rejected("task identity required (e.g. COORD-005)")
    _check_ref(branch)
    if not isinstance(candidate_revision, str) or not REV.fullmatch(candidate_revision):
        raise Rejected("candidate revision must be a 40-hex commit")
    repo = pathlib.Path(repo)
    with IntegrationLock(lock_path, timeout=lock_timeout):
        if not worktree_clean(repo, timeout=timeout):
            raise Rejected("mainline worktree dirty; refusing integration")
        main_rev = rev_parse(repo, main_ref, timeout=timeout)
        tip = branch_tip(repo, branch, timeout=timeout)
        if tip is None:
            raise Rejected(f"candidate branch missing: {branch}")
        if tip != candidate_revision:
            raise Rejected("branch tip moved past the verified candidate; "
                           "reconcile, do not merge blindly")
        if is_ancestor(repo, tip, main_rev, timeout=timeout):
            return main_rev
        if not is_ancestor(repo, main_rev, tip, timeout=timeout):
            raise Rejected(f"diverged mainline; {branch} kept for worker rebase, "
                            "neither contribution discarded")
        proc = _run_git(repo, ["merge", "--ff-only", branch], timeout=timeout)
        if proc.returncode != 0:
            raise Rejected("ff-only merge failed; task unaccepted, dependents "
                            f"blocked: {proc.stderr.strip()[:300]}")
        return rev_parse(repo, main_ref, timeout=timeout)


def check_rerun(rerun_result: object) -> bool:
    """Post-merge rerun must explicitly pass or acceptance is refused."""
    if isinstance(rerun_result, dict):
        passed = rerun_result.get("passed", rerun_result.get("status") == "accepted")
    else:
        passed = getattr(rerun_result, "passed", rerun_result)
    if passed is not True:
        raise Rejected("post-merge rerun did not pass; acceptance refused")
    return True


def write_receipt(receipts_dir: str | os.PathLike | None, *, task_id: str,
                  candidate_revision: str, integrated_revision: str,
                  frozen_tests_sha256: str, rerun: object) -> dict:
    """Atomically record revision + frozen hash + successful rerun (T05).

    Temp-file write plus os.replace and fsync makes the record atomic:
    readers see the full binding or nothing. Repeating the same binding
    is idempotent; a conflicting record raises instead of overwriting.
    """
    if not isinstance(task_id, str) or not TASK_ID.fullmatch(task_id):
        raise Rejected("task identity required (e.g. COORD-005)")
    for label, rev in (("candidate", candidate_revision),
                       ("integrated", integrated_revision)):
        if not isinstance(rev, str) or not REV.fullmatch(rev):
            raise Rejected(f"{label} revision must be a 40-hex commit")
    if not isinstance(frozen_tests_sha256, str) or not HASH.fullmatch(frozen_tests_sha256):
        raise Rejected("frozen test hash must be 64-hex")
    check_rerun(rerun)
    record = {
        "task": task_id,
        "candidate_revision": candidate_revision,
        "integrated_revision": integrated_revision,
        "frozen_tests_sha256": frozen_tests_sha256,
        "rerun": "passed",
    }
    payload = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(payload) > MAX_RECEIPT_BYTES:
        raise Rejected("receipt exceeds byte budget")
    root = pathlib.Path(receipts_dir) if receipts_dir is not None else DEFAULT_RECEIPTS_DIR
    root.mkdir(parents=True, exist_ok=True)
    target = root / f"{task_id}.json"
    if target.is_file():
        try:
            existing = json.loads(target.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise Rejected(f"unreadable acceptance receipt: {exc}") from exc
        if not isinstance(existing, dict) or any(
                existing.get(k) != v for k, v in record.items()):
            raise Rejected("conflicting acceptance receipt; reconcile, do not overwrite")
        return existing
    tmp = root / f".{task_id}.{os.getpid()}.tmp"
    try:
        with open(tmp, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, target)
        with contextlib.suppress(OSError):
            dir_fd = os.open(str(root), os.O_RDONLY)
            try:
                os.fsync(dir_fd)
            finally:
                os.close(dir_fd)
    finally:
        with contextlib.suppress(OSError):
            tmp.unlink()
    return record


def integrate_and_accept(task_id: str, repo: str | os.PathLike, branch: str,
                         candidate_revision: str, frozen_tests_sha256: str,
                         rerun: object, *, main_ref: str = "HEAD",
                         receipts_dir: str | os.PathLike | None = None,
                         lock_path: str | os.PathLike | None = None,
                         lock_timeout: float = 60.0,
                         timeout: float = GIT_TIMEOUT) -> dict:
    """Merge, then rerun outside the git lock, then write the atomic receipt.

    The rerun hook is a side-effect-free verification and may be retried
    by the caller; the merge itself must go through reconcile() after any
    ambiguity and is never blindly retried here.
    """
    integrated = integrate(task_id, repo, branch, candidate_revision,
                           main_ref=main_ref, lock_path=lock_path,
                           lock_timeout=lock_timeout, timeout=timeout)
    result = rerun(integrated) if callable(rerun) else rerun
    if isinstance(result, BaseException):
        raise Rejected("post-merge rerun failed; acceptance refused")
    try:
        check_rerun(result)
    except Rejected as exc:
        raise Rejected(f"post-merge rerun failed; acceptance refused: {exc}") from exc
    return write_receipt(receipts_dir, task_id=task_id,
                         candidate_revision=candidate_revision,
                         integrated_revision=integrated,
                         frozen_tests_sha256=frozen_tests_sha256, rerun=result)


def reconcile(repo: str | os.PathLike, branch: str, candidate_revision: str,
              *, main_ref: str = "HEAD",
              timeout: float = GIT_TIMEOUT) -> str:
    """Classify an ambiguous integration outcome without mutating anything.

    Returns one of "integrated", "not-integrated", "diverged", "unknown".
    Read-only (rev-parse / merge-base / status); never re-runs the merge.
    """
    if not isinstance(candidate_revision, str) or not REV.fullmatch(candidate_revision):
        raise Rejected("candidate revision must be a 40-hex commit")
    repo = pathlib.Path(repo)
    main_rev = rev_parse(repo, main_ref, timeout=timeout)
    tip = branch_tip(repo, branch, timeout=timeout)
    if is_ancestor(repo, candidate_revision, main_rev, timeout=timeout):
        return "integrated"
    if tip is not None and tip != candidate_revision:
        return "unknown"
    if tip is None:
        return "unknown"
    if is_ancestor(repo, main_rev, tip, timeout=timeout):
        return "not-integrated"
    return "diverged"


__all__ = [
    "DEFAULT_LOCK_PATH",
    "DEFAULT_RECEIPTS_DIR",
    "GIT_TIMEOUT",
    "MAX_RECEIPT_BYTES",
    "IntegrationLock",
    "Rejected",
    "RetryableFailure",
    "branch_tip",
    "check_clean_means_integrated",
    "check_rerun",
    "integrate",
    "integrate_and_accept",
    "is_ancestor",
    "reconcile",
    "rev_parse",
    "worktree_clean",
    "write_receipt",
]


if __name__ == "__main__":
    import tempfile

    assert GIT_TIMEOUT == 10.0, "ff-only ancestry check needs a 10s timeout"

    def _git(repo: pathlib.Path, *args: str) -> str:
        proc = subprocess.run(
            ["git", *args], cwd=repo, capture_output=True, text=True, timeout=10)
        assert proc.returncode == 0, f"git {' '.join(args)}: {proc.stderr.strip()[:200]}"
        return proc.stdout.strip()

    _holders = []

    def _make_repo() -> pathlib.Path:
        holder = tempfile.TemporaryDirectory()
        _holders.append(holder)
        repo = pathlib.Path(holder.name) / "repo"
        repo.mkdir()
        _git(repo, "init", "-q")
        _git(repo, "-c", "user.name=t", "-c", "user.email=t@t",
             "commit", "-q", "--allow-empty", "-m", "root")
        _git(repo, "branch", "-M", "main")
        (repo / "a.txt").write_text("root\n", encoding="utf-8")
        _git(repo, "add", "a.txt")
        _git(repo, "-c", "user.name=t", "-c", "user.email=t@t",
             "commit", "-q", "-m", "root files", "a.txt")
        return repo

    def _commit(repo: pathlib.Path, name: str, text: str) -> str:
        (repo / name).write_text(text, encoding="utf-8")
        _git(repo, "add", name)
        _git(repo, "-c", "user.name=t", "-c", "user.email=t@t",
             "commit", "-q", "-m", f"add {name}", name)
        return _git(repo, "rev-parse", "HEAD")

    FROZEN = "ab" * 32
    repo = _make_repo()
    base = _git(repo, "rev-parse", "HEAD")
    assert worktree_clean(repo) is True
    assert is_ancestor(repo, base, base) is True

    # T02: clean worktree + unintegrated branch is not a no-op success.
    _git(repo, "checkout", "-q", "-b", "auto/COORD-005", "main")
    cand = _commit(repo, "lane5.txt", "five\n")
    _git(repo, "checkout", "-q", "main")
    assert worktree_clean(repo) is True
    try:
        check_clean_means_integrated(repo, "auto/COORD-005", main_ref="main")
    except Rejected:
        pass
    else:
        raise AssertionError("clean worktree with unintegrated branch accepted")

    # ff-only merge integrates; receipt binds revision + frozen hash + rerun.
    lock = pathlib.Path(_holders[0].name) / "gate.lock"
    box = pathlib.Path(_holders[0].name) / "receipts"
    integrated = integrate("COORD-005", repo, "auto/COORD-005", cand,
                           main_ref="main", lock_path=lock)
    assert integrated == cand == _git(repo, "rev-parse", "main")
    assert (repo / "lane5.txt").read_text(encoding="utf-8") == "five\n"
    assert check_clean_means_integrated(repo, "auto/COORD-005", main_ref="main") is True
    receipt = write_receipt(box, task_id="COORD-005", candidate_revision=cand,
                            integrated_revision=integrated,
                            frozen_tests_sha256=FROZEN, rerun={"passed": True})
    assert receipt["integrated_revision"] == integrated
    assert receipt["frozen_tests_sha256"] == FROZEN and receipt["rerun"] == "passed"
    on_disk = json.loads((box / "COORD-005.json").read_text(encoding="utf-8"))
    assert on_disk == receipt, "receipt must be atomic and complete"
    assert write_receipt(box, task_id="COORD-005", candidate_revision=cand,
                         integrated_revision=integrated,
                         frozen_tests_sha256=FROZEN,
                         rerun={"passed": True}) == receipt, "receipt not idempotent"

    # T03: diverged nonconflicting candidate kept, then rebased and integrated
    # serially without discarding either contribution.
    _git(repo, "checkout", "-q", "-b", "auto/COORD-006", base)
    cand6 = _commit(repo, "lane6.txt", "six\n")
    _git(repo, "checkout", "-q", "main")
    try:
        integrate("COORD-006", repo, "auto/COORD-006", cand6,
                  main_ref="main", lock_path=lock)
    except Rejected:
        pass
    else:
        raise AssertionError("diverged branch merged instead of kept for rebase")
    assert branch_tip(repo, "auto/COORD-006") == cand6, "diverged tip discarded"
    assert _git(repo, "rev-parse", "main") == integrated, "mainline moved on failure"
    assert not (box / "COORD-006.json").exists(), "failed merge left a receipt"
    _git(repo, "checkout", "-q", "auto/COORD-006")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "rebase", "-q", "main")
    rebased = _git(repo, "rev-parse", "HEAD")
    _git(repo, "checkout", "-q", "main")
    integrated6 = integrate("COORD-006", repo, "auto/COORD-006", rebased,
                            main_ref="main", lock_path=lock)
    assert integrated6 == rebased
    assert (repo / "lane5.txt").read_text(encoding="utf-8") == "five\n"
    assert (repo / "lane6.txt").read_text(encoding="utf-8") == "six\n"

    # T01: conflicting merge failure leaves task unaccepted, dependents blocked.
    _git(repo, "checkout", "-q", "-b", "auto/COORD-007", integrated6)
    (repo / "a.txt").write_text("theirs\n", encoding="utf-8")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t",
         "commit", "-q", "-m", "theirs", "a.txt")
    cand7 = _git(repo, "rev-parse", "HEAD")
    _git(repo, "checkout", "-q", "main")
    (repo / "a.txt").write_text("ours\n", encoding="utf-8")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t",
         "commit", "-q", "-m", "ours", "a.txt")
    try:
        integrate("COORD-007", repo, "auto/COORD-007", cand7,
                  main_ref="main", lock_path=lock)
    except Rejected:
        pass
    else:
        raise AssertionError("conflicting branch accepted")
    assert not (box / "COORD-007.json").exists(), "failed candidate accepted"
    assert reconcile(repo, "auto/COORD-007", cand7, main_ref="main") == "diverged"

    # T04: post-merge rerun failure refuses acceptance even after verification.
    _git(repo, "checkout", "-q", "-b", "auto/COORD-008",
         _git(repo, "rev-parse", "main"))
    cand8 = _commit(repo, "lane8.txt", "eight\n")
    _git(repo, "checkout", "-q", "main")
    try:
        integrate_and_accept("COORD-008", repo, "auto/COORD-008", cand8, FROZEN,
                             {"passed": False}, main_ref="main",
                             receipts_dir=box, lock_path=lock)
    except Rejected:
        pass
    else:
        raise AssertionError("failed post-merge rerun accepted")
    assert not (box / "COORD-008.json").exists(), "failed rerun left a receipt"

    # T05 path: passing rerun records revision + frozen hash + rerun atomically.
    done = integrate_and_accept("COORD-008", repo, "auto/COORD-008", cand8, FROZEN,
                                lambda rev: {"passed": True, "revision": rev},
                                main_ref="main", receipts_dir=box, lock_path=lock)
    assert done["candidate_revision"] == cand8
    assert done["integrated_revision"] == _git(repo, "rev-parse", "main")
    assert done["frozen_tests_sha256"] == FROZEN and done["rerun"] == "passed"

    # Ambiguity path: reconcile is read-only and classifies without merging.
    before = _git(repo, "rev-parse", "main")
    assert reconcile(repo, "auto/COORD-005", cand, main_ref="main") == "integrated"
    try:
        reconcile(repo, "auto/COORD-009", "cd" * 20, main_ref="main")
    except Rejected:
        pass  # fabricated candidate revision correctly fails closed
    try:
        reconcile(repo, "auto/COORD-005", cand, main_ref="main")
    except Rejected:
        raise AssertionError("reconcile of integrated candidate must not raise")
    assert _git(repo, "rev-parse", "main") == before, "reconcile mutated mainline"

    # Conflicting receipt never overwrites; lock serializes holders.
    try:
        write_receipt(box, task_id="COORD-005", candidate_revision="cd" * 20,
                      integrated_revision=integrated,
                      frozen_tests_sha256=FROZEN, rerun={"passed": True})
    except Rejected:
        pass
    else:
        raise AssertionError("conflicting receipt overwrote acceptance")
    with IntegrationLock(lock, timeout=5.0):
        try:
            with IntegrationLock(lock, timeout=0.2):
                raise AssertionError("concurrent integration not serialized")
        except RetryableFailure:
            pass
    try:
        check_rerun({"passed": False})
    except Rejected:
        pass
    else:
        raise AssertionError("falsy rerun passed the gate")

    # Hygiene: list-form git argv only, no placeholders.
    source = pathlib.Path(__file__).read_text(encoding="utf-8")
    assert ("shell" + "=True") not in source, "shell-string concatenation forbidden"
    assert ("TO" + "DO") not in source, "placeholder left in module"
    assert ("FIX" + "ME") not in source, "placeholder left in module"
    assert ("todo" + "!(") not in source
    assert ("unimplemented" + "!(") not in source

    for holder in _holders:
        holder.cleanup()
    print("integration self-check: OK")

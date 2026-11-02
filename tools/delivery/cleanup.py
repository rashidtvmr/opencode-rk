"""Retire owned execution workspaces only after merged+pushed+clean proof.

``collect(store)`` is the report-only default (the variant the supervisor loop
calls every iteration): it lists what a guarded retirement *would* retire and
the reason every other workspace is retained. It never deletes and never writes
inside a workspace.

``collect(store, apply=True)`` (also ``python3 -m tools.delivery.cleanup
--state <dir> --apply``) is the only deletion path. A workspace is retired only
when every guard below is individually verified at evaluation time and
rechecked immediately before the exact-path removal:

1. explicit controller ownership: exact name shape, owner token matching a
   consumed job, no live process, and no unfinished-task/live-job hold;
2. the exact workspace tip is merged into local ``main-v2``
   (``git merge-base --is-ancestor``);
3. that same tip is on the actual push remote: freshly advertised by
   ``git ls-remote``, or verified ancestry from a freshly advertised remote
   ``main-v2``/pushed canonical tip. Stale ``refs/remotes/origin/*`` is never
   evidence (this repository pushes to a different URL than it fetches);
4. no staged, unstaged, untracked or ignored content (``is_dirty`` plus an
   explicit ``--ignored`` probe);
5. the target is not canonical, a configured remote path, and is inside the
   owned workspace root.

Unmerged, unpushed or dirty workspaces are retained and listed with their
reasons; the archived candidate patch alone never authorizes deletion, and
missing push proof fails closed. Never delete on "probably merged".
Policy: AGENTS.md worktree-lifecycle; docs/AGENT_STRATEGY_V2.md section 8.
Reused proven guards: tools/worktree_reclaim.py (git/rev/is_dirty/remote_state/
push_remote/local_remote_paths/dir_bytes).
"""
from __future__ import annotations
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import time

from tools.worktree_reclaim import (dir_bytes, git, is_dirty, local_remote_paths,
                                    push_remote, remote_state, rev)

# Partial writes with these outcomes require explicit human disposition.
FAIL_KINDS = {"failure", "authority", "runtime", "cancelled", "timeout",
              "interrupted", "conflict", "suspended", "output_limit"}
# These outcomes carry a candidate commit worth an exact audit patch.
ARCHIVE_KINDS = {"candidate", "prepared"}
NAME = re.compile(r"[0-9a-f]{32}(?:-red|-post)?")
REMOTE_TTL = 300.0
REMOTE_FAIL_TTL = 30.0
# The supervisor calls the report-only path every poll tick. Evaluating every
# owned workspace costs several git subprocesses each, so the report is
# throttled: it is a status surface, not a per-tick gate. The apply path is
# never throttled and always re-evaluates the exact bytes before removal.
REPORT_INTERVAL = 30.0
_remote_cache = {}
_report_cache = {}


def _push_state(repo, remote):
    """(heads, remote_main) freshly advertised by the push remote.

    Returns (None, None) when push proof cannot be obtained; callers then
    retain instead of deleting. ls-remote is network work: the report path
    caches it briefly, the apply path always refreshes before trusting it.
    """
    key = (str(repo), remote)
    hit = _remote_cache.get(key)
    if hit and time.time() < hit[0]:
        return hit[1], hit[2]
    try:
        heads, remote_main = remote_state(repo, remote)
    except (SystemExit, OSError):
        _remote_cache[key] = (time.time() + REMOTE_FAIL_TTL, None, None)
        return None, None
    _remote_cache[key] = (time.time() + REMOTE_TTL, heads, remote_main)
    return heads, remote_main


def _push_state_fresh(repo, remote):
    _remote_cache.pop((str(repo), remote), None)
    return _push_state(repo, remote)


def _held(store, base):
    """Workspaces a live task/job still needs; never retire these."""
    held = {t['workspace'] for t in store.tasks() if t['workspace'] and t['state'] != 'accepted'}
    for j in store.jobs():
        if j['state'] in {'running', 'finished'}:
            held.add(str(base / j['id']))
            p = json.loads(j['payload'])
            if p.get('workspace'):
                held.add(p['workspace'])
    return held


def _evaluate(directory, base, jobs, held, repo, branch, main_sha, protected, push_state):
    """One guarded retention decision; None means "not an owned candidate"."""
    if directory.is_symlink() or not directory.is_dir() or str(directory) in held:
        return None
    if not NAME.fullmatch(directory.name):
        return None
    owner = directory / '.git' / 'delivery-owner.json'
    if not owner.is_file():
        return None
    try:
        data = json.loads(owner.read_text())
    except (OSError, ValueError):
        return None
    job = jobs.get(data.get('job'))
    if not job or data.get('token') != job['token'] or job['state'] != 'consumed':
        return None
    if job.get('pid'):
        try:
            os.kill(job['pid'], 0)
            return None
        except ProcessLookupError:
            pass
        except PermissionError:
            return None
    record = {"path": str(directory), "name": directory.name, "job": job['id'],
              "kind": None, "tip": None, "decision": "retain", "reason": ""}
    result = json.loads(job['result'] or '{}')
    record["kind"] = result.get('kind', 'failure')
    if result.get('kind') in FAIL_KINDS:
        record["reason"] = "failed/out-of-scope partial writes require explicit human disposition"
        return record
    if result.get('workspace') and result['workspace'] != str(directory):
        record["reason"] = "owner record points at another workspace; identity unproven"
        return record
    tip = rev(directory, "HEAD")
    record["tip"] = tip
    if not tip:
        record["reason"] = "cannot resolve the exact workspace tip"
        return record
    if result.get('candidate') and result['candidate'] != tip:
        record["reason"] = "workspace tip does not match the recorded candidate"
        return record
    resolved = directory.resolve()
    if resolved.parent != base.resolve() or directory.is_symlink():
        record["reason"] = "target escapes the owned workspace root (symlink/replacement race)"
        return record
    if str(resolved) in protected:
        record["reason"] = "target is canonical or a configured remote path"
        return record
    dirty, detail = is_dirty(directory)
    if dirty:
        record["reason"] = "not clean: " + detail
        return record
    code, out, _ = git(directory, ["status", "--porcelain", "--ignored=matching"])
    if code != 0:
        record["reason"] = "cannot prove cleanliness (status failed)"
        return record
    ignored = [line for line in out.splitlines() if line.startswith("!! ")]
    if ignored:
        record["reason"] = "not clean: %d ignored entr(y|ies) carry unique content" % len(ignored)
        return record
    if main_sha is None:
        record["reason"] = "cannot resolve canonical %s; merge proof unavailable" % branch
        return record
    if git(repo, ["merge-base", "--is-ancestor", tip, main_sha])[0] != 0:
        record["reason"] = "tip is not merged into %s" % branch
        return record
    heads, remote_main = push_state()
    if heads is None:
        record["reason"] = "merged, but the push remote is unreachable; push proof unavailable"
        return record
    if tip in heads:
        record["pushed_as"] = heads[tip]
    elif remote_main and git(repo, ["merge-base", "--is-ancestor", tip, remote_main])[0] == 0:
        record["pushed_as"] = "ancestor of freshly advertised remote %s" % branch
    elif main_sha in heads:
        # The local canonical tip is itself advertised by the push remote and
        # the tip's ancestry to it is locally verified: the commit exists there.
        record["pushed_as"] = "ancestor of pushed canonical tip %s" % heads[main_sha]
    else:
        record["reason"] = "merged locally but absent from the push remote"
        return record
    record["decision"] = "retire"
    record["reason"] = "merged into %s; present on the push remote as %s" % (branch, record["pushed_as"])
    return record


def _report_event(store, records):
    """Durable report of what would retire/why retained; emitted on change only."""
    payload = [{k: r[k] for k in ("path", "name", "job", "kind", "tip", "decision", "reason")}
               for r in records]
    key = hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()
    if store.meta("retention_report_hash") == key:
        return
    store.meta("retention_report_hash", key)
    with store.connection(write=True) as cx:
        store.event(cx, 'workspace.retention_report', {"records": payload})


def collect(store, *, apply=False):
    """Report-only by default; apply=True retires verified workspaces only.

    Returns the decision records. ``retire`` means every merged+pushed+clean
    proof held at evaluation time; ``retain`` records carry the blocking reason.
    """
    base = store.root / 'workspaces'
    cfg = store.meta("config") or {}
    repo_value = store.meta("repo")
    if not base.is_dir() or not repo_value:
        return []
    if not apply:
        hit = _report_cache.get(str(store.root))
        if hit and time.time() < hit[0]:
            return hit[1]
    repo = pathlib.Path(repo_value)
    branch = cfg.get('branch', 'main-v2')
    main_sha = rev(repo, branch)
    remote = push_remote(repo)
    protected = local_remote_paths(repo) | {str(repo.resolve())}
    jobs = {j['id']: j for j in store.jobs()}
    held = _held(store, base)
    shared = {}

    def push_state():
        if "value" not in shared:
            if not remote:
                shared["value"] = (None, None)
            elif apply:
                shared["value"] = _push_state_fresh(repo, remote)
            else:
                shared["value"] = _push_state(repo, remote)
        return shared["value"]

    records = []
    for directory in sorted(base.iterdir()):
        record = _evaluate(directory, base, jobs, held, repo, branch, main_sha, protected, push_state)
        if record is not None:
            records.append(record)
    retiring = [r for r in records if r["decision"] == "retire"]
    if not apply:
        _report_cache[str(store.root)] = (time.time() + REPORT_INTERVAL, records)
        _report_event(store, records)
        return records
    reclaimed = 0
    for record in retiring:
        directory = pathlib.Path(record["path"])
        # Recheck identities/ownership/cleanliness/push proof immediately before
        # the exact-path removal (AGENT_STRATEGY_V2 section 8 rule 6).
        fresh_jobs = {j['id']: j for j in store.jobs()}
        again = _evaluate(directory, base, fresh_jobs, _held(store, base), repo, branch,
                          rev(repo, branch), protected, push_state)
        if not again or again["decision"] != "retire" or again["tip"] != record["tip"]:
            record["decision"] = "retain"
            record["reason"] = "recheck before removal disagreed: " + (
                again["reason"] if again else "workspace or ownership changed")
            continue
        job = fresh_jobs[record["job"]]
        result = json.loads(job['result'] or '{}')
        if record["kind"] in ARCHIVE_KINDS:
            # Keep an exact patch for the retired candidate as audit evidence.
            # A patch alone never authorizes deletion; the proofs above did.
            archive = store.root / 'evidence' / job['id'] / 'candidate.patch'
            archive.parent.mkdir(parents=True, exist_ok=True)
            p = subprocess.run(['git', 'diff', '--binary', result['base'], result['candidate']],
                               cwd=directory, capture_output=True, timeout=30)
            if p.returncode:
                record["decision"] = "retain"
                record["reason"] = "candidate patch archival failed; retaining every unique byte"
                continue
            archive.write_bytes(p.stdout)
        # Guard against replacement/symlink races at the owned root boundary.
        if directory.resolve().parent != base.resolve() or directory.is_symlink():
            record["decision"] = "retain"
            record["reason"] = "replacement/symlink race at the owned root boundary"
            continue
        record["bytes"] = dir_bytes(directory)
        shutil.rmtree(directory)
        reclaimed += record["bytes"]
        with store.connection(write=True) as cx:
            store.event(cx, 'workspace.retired',
                        {'job': job['id'], 'path': str(directory), 'tip': record['tip'],
                         'pushed_as': record.get('pushed_as'), 'bytes': record['bytes']},
                        job['task'])
    if retiring:
        with store.connection(write=True) as cx:
            store.event(cx, 'workspace.retention_applied',
                        {'retired': sum(1 for r in retiring if r['decision'] == 'retire'),
                         'reclaimed_bytes': reclaimed})
    return records


def main(argv=None):
    import argparse
    from .store import Store
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--state", required=True, help="delivery state directory")
    parser.add_argument("--apply", action="store_true",
                        help="retire only workspaces proven merged AND pushed AND clean")
    parser.add_argument("--json", action="store_true", help="machine-readable report")
    args = parser.parse_args(argv)
    store = Store(args.state)
    if not store.meta("repo"):
        raise SystemExit("state is not initialized")
    records = collect(store, apply=args.apply)
    if args.json:
        print(json.dumps({"records": records}, indent=2, sort_keys=True))
    else:
        for r in records:
            state = "RETIRE" if r["decision"] == "retire" else "keep   "
            print("%s %-34s %s" % (state, r["name"], r["reason"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

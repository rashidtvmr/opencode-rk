"""Rolling supervisor: one pool, durable stage transitions, dynamic task owners."""
from __future__ import annotations

import argparse
import collections
import contextlib
import fcntl
import json
import os
import pathlib
import signal
import subprocess
import sys
import time

from . import jobs as executors
from . import model, runtime, cleanup
from .store import Store
from .runtime import Failure
from tools.dynamic_agent_allocator import allocate

ROOT = pathlib.Path(__file__).resolve().parents[2]
ENTRY = ROOT / "tools" / "continuous_delivery.py"
ROLE = {"implement": "implement", "review": "review", "test": "test", "prepare": "integration_prepare",
        "publish": "integration_prepare", "postcheck": "test", "diagnose": "repair"}
AGENT_STAGES = {"implement", "review", "diagnose"}
CHECK_STAGES = {"test", "publish", "postcheck"}


@contextlib.contextmanager
def repository_control(repo):
    """Exclude every other supervisor/configuration writer for this repository."""
    lockfile = executors.common_dir(repo) / "delivery-supervisor.lock"
    with lockfile.open("a+") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise Failure("another supervisor owns this repository; stop it before reconfiguration", "authority")
        try:
            yield
        finally:
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)


def setup(repo, state, raw):
    with repository_control(repo):
        return _setup(repo, state, raw)


def _setup(repo, state, raw):
    repo = pathlib.Path(repo).resolve()
    cfg = model.config(raw)
    git = executors.Git(repo)
    git.head(cfg["branch"])
    common = executors.common_dir(repo)
    state = pathlib.Path(state).resolve() if state else common / "delivery-state"
    if state == repo or (repo in state.parents and common not in state.parents):
        raise ValueError("runtime state must be outside tracked working files")
    store = Store(state)
    prior = store.meta("repo")
    if prior and prior != str(repo):
        raise ValueError("state belongs to another repository")
    oldcfg = store.meta("config")
    if oldcfg and model.digest(oldcfg) != model.digest(cfg):
        if store.jobs({"running", "finished"}) or store.meta("publication_barrier") or store.meta("stop_new_publications"):
            raise ValueError("cannot change workflow while jobs need reconciliation")
        # Config changes invalidate live approvals, not completed delivery history.
        with store.connection(write=True) as cx:
            cx.execute("UPDATE tasks SET state='pending',stage='implement',generation=generation+1,reviews='[]',candidate=NULL,base=NULL,updated=? WHERE state NOT IN ('accepted','cancelled','blocked')", (time.time(),))
            store.event(cx, "workflow.changed", {"old": model.digest(oldcfg), "new": model.digest(cfg)})
    if not store.meta("policy_base_sha"):
        store.meta("policy_base_sha", git.head(cfg["branch"]))
    store.meta("repo", str(repo)); store.meta("config", cfg)
    store.meta("config_hash", model.digest(cfg))
    (state / "empty-hooks").mkdir(exist_ok=True)
    return store


def status(store):
    cfg = store.meta("config")
    repo = store.meta("repo")
    git = executors.Git(repo)
    rows = store.tasks()
    current = git.head(cfg["branch"])
    return {"source": repo, "canonical_sha": current, "dirty": bool(git.run("status", "--porcelain=v1")),
            "workers": cfg["workers"], "fixed_role_quotas": False,
            "states": dict(collections.Counter(t["state"] for t in rows)),
            "active_roles": dict(collections.Counter(j["stage"] for j in store.jobs({"running"}))),
            "tasks": [{k: t[k] for k in ("id", "state", "stage", "attempt", "candidate", "accepted_sha", "report", "resume_stage", "eligible_at")} for t in rows],
            "last_verified_sha": store.meta("last_verified_sha"),
            "publication_stop": store.meta("stop_new_publications"),
            "publication_barrier": store.meta("publication_barrier"),
            "policy_base_sha": store.meta("policy_base_sha"),
            "state_directory": str(store.root)}


def failure_values(task, cfg, reason, kind):
    n = task["attempt"] + 1
    if kind == "authority":
        return {"state": "blocked", "report": reason, "reviews": "[]"}
    if n >= cfg["attempts"]:
        if task["diagnoses"] < cfg["diagnoses"]:
            return {"state": "ready", "stage": "diagnose", "attempt": n, "report": reason, "reviews": "[]"}
        return {"state": "blocked", "attempt": n, "report": reason, "reviews": "[]"}
    return {"state": "ready", "stage": "implement", "attempt": n, "report": reason,
            "reviews": "[]", "eligible_at": time.time()+min(2**n, 10)}


def consume(store):
    cfg = store.meta("config")
    for job in store.jobs({"finished"}):
        rows = {t["id"]: t for t in store.tasks()}
        task = rows[job["task"]]
        if task["generation"] != job["generation"]:
            store.transition(job["id"], {}, "job.stale_discarded")
            continue
        result = json.loads(job["result"] or "{}")
        p = json.loads(job["payload"])
        kind = result.get("kind", "failure")
        reason = result.get("error", "")
        values = None
        if task["state"] == "cancelled":
            values = {"state": "cancelled", "report": "result ignored after cancellation"}
        elif kind in {"candidate", "prepared"}:
            values = {"state": "ready", "stage": "review", "base": result["base"], "candidate": result["candidate"],
                      "workspace": result["workspace"], "reviews": "[]", "changed_paths": json.dumps(result["changed_paths"]),
                      "author": result.get("author", task["author"]), "eligible_at": 0}
        elif kind == "review":
            if result.get("candidate") != task["candidate"] or result.get("reviewer") == task["author"]:
                values = {"state": "blocked", "report": "review identity/candidate mismatch"}
            elif result["verdict"] != "APPROVE":
                values = failure_values(task, cfg, json.dumps(result.get("findings")), "review")
            else:
                reviews = json.loads(task["reviews"])
                reviews.append(result)
                spec = json.loads(task["spec"])
                enough = len({r["reviewer"] for r in reviews}) >= spec["reviews"]
                enough = enough and (not spec["security_review"] or any(r.get("security") for r in reviews))
                values = {"state": "ready", "stage": "test" if enough else "review", "reviews": json.dumps(reviews), "eligible_at": 0}
        elif kind == "tested":
            if result.get("candidate") != task["candidate"]:
                values = {"state": "blocked", "report": "test evidence is for the wrong candidate"}
            else:
                values = {"state": "ready", "stage": "prepare", "eligible_at": 0}
        elif kind == "unchanged":
            values = {"state": "ready", "stage": "publish", "eligible_at": 0}
        elif kind == "stale":
            values = {"state": "ready", "stage": "prepare", "report": reason, "eligible_at": 0}
        elif kind == "accepted":
            if result.get("candidate") != task["candidate"]:
                values = {"state": "blocked", "report": "acceptance does not match task candidate"}
            else:
                values = {"state": "accepted", "accepted_sha": result["candidate"], "report": "", "eligible_at": 0}
        elif kind == "diagnosis":
            values = {"state": "ready", "stage": "implement", "attempt": 0, "diagnoses": task["diagnoses"]+1,
                      "report": "Independent diagnosis: "+result["diagnosis"]+"\nNew approach: "+result["approach"], "eligible_at": 0}
        elif kind == "wait":
            values = {"state": "ready", "stage": job["stage"], "report": reason, "eligible_at": time.time()+5}
        elif kind in {"interrupted", "suspended", "timeout", "git", "failure", "cancelled", "ambiguous"} and job["stage"] == "publish" and store.publication(job["id"]):
            reconciliation = executors.reconcile_publication(store, job, lambda: True)
            if reconciliation["kind"] == "accepted":
                values = {"state": "accepted", "accepted_sha": reconciliation["candidate"], "report": "Recovered verified publication"}
            elif reconciliation["kind"] == "retry":
                values = {"state": "ready", "stage": "publish", "report": reconciliation["error"], "eligible_at": time.time()+1}
            elif reconciliation["kind"] == "recheck":
                values = {"state": "ready", "stage": "postcheck", "report": job["id"], "eligible_at": 0}
            else:
                values = {"state": "blocked", "report": reconciliation["error"]}
        elif kind == "suspended":
            values = {"state":"ready", "stage":job["stage"], "eligible_at":0}
        elif kind in {"runtime", "timeout", "interrupted"} and job["stage"] in AGENT_STAGES:
            n = task["attempt"]+1
            if n < cfg["attempts"]:
                values = {"state":"ready", "stage":job["stage"], "attempt":n,
                          "report":reason, "eligible_at":time.time()+min(2**n,10)}
            else:
                values = {"state":"paused", "stage":job["stage"], "attempt":n,
                          "resume_stage":job["stage"], "eligible_at":time.time()+60,
                          "report":"Agent runtime unavailable; candidate preserved for cooldown. "+reason}
        elif job["stage"] == "diagnose":
            values = {"state":"blocked", "diagnoses":task["diagnoses"]+1,
                      "report":"Independent diagnosis failed: "+reason, "reviews":"[]"}
        elif kind == "regression":
            values = failure_values(task, cfg, "Post-merge regression: "+reason, "test")
        elif kind == "ambiguous":
            values = {"state": "blocked", "report": reason+"; inspect publication journal before retry"}
        else:
            values = failure_values(task, cfg, reason or json.dumps(result), kind)
        store.transition(job["id"], values, "task."+values["state"])
        if job["route"]:
            with store.connection(write=True) as cx:
                cx.execute("INSERT OR IGNORE INTO routes(agent) VALUES(?)", (job["route"],))
                if kind == "runtime":
                    row = cx.execute("SELECT failures FROM routes WHERE agent=?", (job["route"],)).fetchone()
                    n = row[0]+1
                    cx.execute("UPDATE routes SET failures=?,unavailable_until=?,last_error=? WHERE agent=?",
                               (n, time.time()+min(300, 5*(2**min(n,6))), reason[-1000:], job["route"]))
                elif kind not in {"interrupted", "cancelled", "suspended"}:
                    cx.execute("UPDATE routes SET failures=0,unavailable_until=0 WHERE agent=?", (job["route"],))


def eligible_tasks(store, publish):
    cfg = store.meta("config")
    rows = store.tasks()
    byid = {t["id"]: t for t in rows}
    active = store.jobs({"running"})
    stop = store.meta("stop_new_publications") or store.meta("publication_barrier") or {}
    turn = store.meta("integration_turn")
    if turn and (turn not in byid or byid[turn]["state"] in {"accepted", "blocked", "cancelled", "paused"}):
        store.meta("integration_turn", "")
        turn = None
    holds = [t for t in rows if t["state"] in {"ready", "running", "paused"}]
    eligible = []
    for row in rows:
        if row["state"] not in {"pending", "ready"} or row["eligible_at"] > time.time():
            continue
        spec = json.loads(row["spec"])
        if any(byid[d]["state"] != "accepted" for d in spec["dependencies"]):
            continue
        if stop and row["id"] != stop.get("task") and row["stage"] in {"implement", "publish"}:
            continue
        if row["stage"] in {"prepare", "publish", "postcheck"} and turn and turn != row["id"]:
            continue
        if row["stage"] == "publish" and (not publish or any(j["stage"] in {"publish", "postcheck"} for j in active)):
            continue
        if row["stage"] in CHECK_STAGES and sum(j["stage"] in CHECK_STAGES for j in active) >= cfg["heavy_checks"]:
            continue
        if row["state"] == "pending" and len(holds) >= cfg["max_wip"]:
            continue
        if any(other["id"] != row["id"] and any(model.overlaps(a,b) for a in spec["paths"] for b in json.loads(other["spec"])["paths"]) for other in holds):
            continue
        eligible.append(row)
    # Downstream work earns priority, but no role owns a fixed reservation.
    weights = {"implement": 0, "review": 12, "test": 14, "prepare": 16, "publish": 20, "postcheck": 25, "diagnose": 22}
    return sorted(eligible, key=lambda r: (-(weights[r["stage"]]+min(30,(time.time()-r["updated"])/30)),
                                           json.loads(r["spec"])["priority"], r["created"], r["id"]))


def choose_route(store):
    cfg = store.meta("config")
    active = collections.Counter(j["route"] for j in store.jobs({"running"}) if j["route"])
    with store.connection() as cx:
        health = {r["agent"]: dict(r) for r in cx.execute("SELECT * FROM routes")}
        unhealthy = {k for k,v in health.items() if v["unavailable_until"]>time.time()}
        total = collections.Counter(r["route"] for r in cx.execute("SELECT route FROM jobs WHERE route IS NOT NULL"))
    possible = [r for r in cfg["routes"] if r["agent"] not in unhealthy
                and active[r["agent"]] < (1 if health.get(r["agent"],{}).get("failures",0) else r["max_parallel"])]
    return min(possible, key=lambda r:(active[r["agent"]],total[r["agent"]]))["agent"] if possible else None


def dispatch(store, publish, children, owned):
    cfg = store.meta("config")
    free = cfg["workers"] - len(store.jobs({"running"}))
    launched = 0
    for _ in range(max(0,free)):
        store.renew(owned, cfg["lease_seconds"])
        options = eligible_tasks(store, publish)
        demand = dict(collections.Counter(ROLE[r["stage"]] for r in options))
        slots = allocate(demand, max(0, cfg["workers"]-len(store.jobs({"running"}))),
                         integration_backlog=sum(r["stage"]=="publish" for r in options))
        claimed = None
        for row in options:
            if slots.get(ROLE[row["stage"]], 0) <= 0:
                continue
            route = choose_route(store) if row["stage"] in AGENT_STAGES else None
            if row["stage"] in AGENT_STAGES and route is None:
                continue
            payload = {"base": row["base"], "base_previous": row["base"], "candidate": row["candidate"],
                       "workspace": row["workspace"], "author": row["author"], "report": row["report"],
                       "reviews": json.loads(row["reviews"]), "publish_enabled": publish,
                       "security_review": bool(json.loads(row["spec"])["security_review"] and not json.loads(row["reviews"])),
                       "config_hash": store.meta("config_hash")}
            claimed = store.claim(row["id"], row["stage"], route, payload, cfg["lease_seconds"])
            if claimed:
                if row["stage"] in {"prepare", "publish", "postcheck"}:
                    store.meta("integration_turn", row["id"])
                break
        if not claimed:
            break
        log = store.root / "workers" / (claimed["id"]+".log")
        log.parent.mkdir(exist_ok=True)
        with log.open("w") as handle:
            try:
                proc = subprocess.Popen([sys.executable, str(ENTRY), "_job", "--state", str(store.root),
                                         "--job", claimed["id"], "--token", claimed["token"]],
                                        stdout=handle, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                                        start_new_session=True, env=runtime.environment(
                                            agent=True, forward=cfg["runtime"].get("forward_env", [])))
            except OSError as exc:
                store.finish(claimed["id"], claimed["token"], {"kind":"runtime","error":str(exc)})
                continue
        store.set_pid(claimed["id"], claimed["token"], proc.pid)
        children[claimed["id"]] = proc
        owned[claimed["id"]] = claimed["token"]
        launched += 1
    return launched


def reconcile_locks(store):
    path = executors.common_dir(store.meta("repo")) / "recovery-state" / "integration.lock"
    if not path.is_file():
        return
    try:
        data = json.loads(path.read_text())
        jid = data.get("job")
        if not jid:
            return  # Legacy/foreign ownership is not ours to clear.
        job = store.job(jid)
        if not job or job["state"] == "running" and job["lease_until"] > time.time():
            return
        if not job["pid"]:
            return
        try:
            os.kill(job["pid"],0)
            return  # Possibly alive or PID reused: never steal.
        except ProcessLookupError:
            pass
        except PermissionError:
            return
        journal = store.publication(jid)
        current = executors.Git(store.meta("repo")).head(store.meta("config")["branch"])
        if journal and current not in {journal["before_sha"], journal["candidate_sha"]}:
            return
        if json.loads(path.read_text()).get("job") == jid:
            path.unlink()
    except (ValueError, OSError):
        return


def supervise(store, *, publish=False, watch=False, max_seconds=0, poll=0.2, tasks_file=None):
    cfg = store.meta("config")
    # One pool per product repository, even when two callers supply different
    # --state directories. A per-state lock alone allows duplicate live owners.
    lockfile = executors.common_dir(store.meta("repo")) / "delivery-supervisor.lock"
    children = {}; owned = {}
    started = time.monotonic()
    with lockfile.open("a+") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise Failure("another supervisor already owns this state", "authority")
        last_emit = None
        manifest_hash = None
        checked_main = None
        try:
            while True:
                if tasks_file:
                    raw = pathlib.Path(tasks_file).read_text()
                    new_hash = model.digest(raw)
                    if new_hash != manifest_hash:
                        value = json.loads(raw)
                        store.admit(value if isinstance(value, list) else value["tasks"], cfg)
                        manifest_hash = new_hash
                # Renew only this supervisor's jobs. Jobs from a crashed process
                # expire and fence themselves; never adopt an unproven child PID.
                store.renew(owned, cfg["lease_seconds"])
                for jid, proc in list(children.items()):
                    if proc.poll() is not None:
                        job = store.job(jid)
                        if job and job["state"] == "running":
                            store.finish(jid, owned[jid], {"kind":"interrupted", "error":"worker exited without a valid result"})
                        children.pop(jid); owned.pop(jid,None)
                store.expired()
                current_main = executors.Git(store.meta("repo")).head(cfg["branch"])
                if current_main != checked_main:
                    last = store.meta("last_verified_sha")
                    if last and executors.Git(store.meta("repo")).run("merge-base", "--is-ancestor", last, current_main, check=False)["returncode"]:
                        store.meta("stop_new_publications", {"task": None, "sha": current_main,
                            "error": "canonical lost verified history; explicit reconciliation required"})
                    checked_main = current_main
                reconcile_locks(store)
                consume(store)
                if watch: store.resume_paused()
                cleanup.collect(store)
                dispatched = dispatch(store, publish, children, owned)
                rows = store.tasks(); active = store.jobs({"running"})
                summary = {"states":dict(collections.Counter(r["state"] for r in rows)),
                           "active":dict(collections.Counter(j["stage"] for j in active))}
                if summary != last_emit:
                    print(json.dumps(summary,sort_keys=True),flush=True); last_emit=summary
                if max_seconds and time.monotonic()-started >= max_seconds:
                    return 3
                if not active and not dispatched:
                    # A worker can finish between consume() and this snapshot.
                    # Drain its durable result before declaring the queue idle.
                    if children or store.jobs({"finished"}):
                        time.sleep(poll)
                        continue
                    if all(r["state"] == "accepted" for r in rows) and rows and not watch:
                        return 0
                    pending_ready = any(r["state"] in {"ready","pending"} for r in rows)
                    future = any(r["eligible_at"] > time.time() for r in rows if r["state"] in {"ready","pending"})
                    with store.connection() as cx:
                        route_wait = bool(cx.execute("SELECT 1 FROM routes WHERE unavailable_until>?",(time.time(),)).fetchone())
                    route_wait = route_wait and pending_ready and any(t["stage"] in AGENT_STAGES for t in eligible_tasks(store, publish))
                    if not watch and not future and not route_wait:
                        return 2  # honest blocked/drained, not fabricated completion
                time.sleep(poll)
        finally:
            # Revoke only owned leases. Job wrappers stop/reap their own children;
            # never issue kill against an unowned or guessed process.
            store.suspend_owned(owned)
            deadline = time.monotonic()+8
            for proc in children.values():
                try:
                    proc.wait(timeout=max(0.1, deadline-time.monotonic()))
                except subprocess.TimeoutExpired:
                    proc.kill(); proc.wait(timeout=5)
            reconcile_locks(store)
            consume(store)


def worker(state, jid, token):
    store = Store(state)
    job = store.job(jid)
    if not job or not store.valid(jid,token):
        return 2
    try:
        p=json.loads(job["payload"])
        if p.get("config_hash") != store.meta("config_hash"):
            raise Failure("workflow changed after job dispatch", "authority")
        result=executors.stage(store,job)
    except Failure as exc:
        result={"kind":exc.kind,"error":runtime.redact(str(exc))[-16000:]}
    except Exception as exc:
        result={"kind":"failure","error":type(exc).__name__+": "+runtime.redact(str(exc))[-8000:]}
    return 0 if store.finish(jid,token,result) else 2

"""Transactional task ownership, fenced job leases, and append-only events."""
from __future__ import annotations

import contextlib
import json
import pathlib
import sqlite3
import time
import uuid

from .model import digest, overlaps, task as validate_task

SCHEMA = """
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tasks(
 id TEXT PRIMARY KEY, spec TEXT NOT NULL, spec_hash TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'pending', stage TEXT NOT NULL DEFAULT 'implement', generation INTEGER NOT NULL DEFAULT 0,
 attempt INTEGER NOT NULL DEFAULT 0, diagnoses INTEGER NOT NULL DEFAULT 0,
 base TEXT, candidate TEXT, workspace TEXT, author TEXT,
 reviews TEXT NOT NULL DEFAULT '[]', report TEXT NOT NULL DEFAULT '', resume_stage TEXT NOT NULL DEFAULT '',
 accepted_sha TEXT, changed_paths TEXT NOT NULL DEFAULT '[]',
 created REAL NOT NULL, updated REAL NOT NULL, eligible_at REAL NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS jobs(
 id TEXT PRIMARY KEY, task TEXT NOT NULL REFERENCES tasks(id), stage TEXT NOT NULL,
 token TEXT NOT NULL, state TEXT NOT NULL, route TEXT, generation INTEGER NOT NULL DEFAULT 0,
 payload TEXT NOT NULL, result TEXT, pid INTEGER,
 lease_until REAL NOT NULL, created REAL NOT NULL, updated REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS job_states ON jobs(state, lease_until);
CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT, at REAL NOT NULL,
 task TEXT, kind TEXT NOT NULL, body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS routes(agent TEXT PRIMARY KEY, failures INTEGER NOT NULL DEFAULT 0,
 unavailable_until REAL NOT NULL DEFAULT 0, last_error TEXT, usage TEXT NOT NULL DEFAULT 'UNKNOWN');
CREATE TABLE IF NOT EXISTS publications(id TEXT PRIMARY KEY, task TEXT NOT NULL,
 before_sha TEXT NOT NULL, candidate_sha TEXT NOT NULL, state TEXT NOT NULL,
 evidence TEXT NOT NULL, created REAL NOT NULL, updated REAL NOT NULL);
"""


class Store:
    def __init__(self, root):
        self.root = pathlib.Path(root).resolve()
        self.root.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.db = self.root / "delivery.sqlite3"
        with self.connection() as cx:
            cx.executescript(SCHEMA)
            cx.execute("BEGIN IMMEDIATE")
            for table in ("tasks", "jobs"):
                if "generation" not in {r["name"] for r in cx.execute("PRAGMA table_info("+table+")")}:
                    cx.execute("ALTER TABLE "+table+" ADD COLUMN generation INTEGER NOT NULL DEFAULT 0")

            if "resume_stage" not in {r["name"] for r in cx.execute("PRAGMA table_info(tasks)")}:
                cx.execute("ALTER TABLE tasks ADD COLUMN resume_stage TEXT NOT NULL DEFAULT ''")

    @contextlib.contextmanager
    def connection(self, write=False):
        cx = sqlite3.connect(str(self.db), timeout=30)
        cx.row_factory = sqlite3.Row
        cx.execute("PRAGMA foreign_keys=ON")
        cx.execute("PRAGMA journal_mode=WAL")
        cx.execute("PRAGMA synchronous=FULL")
        try:
            if write:
                cx.execute("BEGIN IMMEDIATE")
            yield cx
            cx.commit()
        except BaseException:
            cx.rollback()
            raise
        finally:
            cx.close()

    @staticmethod
    def event(cx, kind, body=None, task=None):
        cx.execute("INSERT INTO events(at,task,kind,body) VALUES(?,?,?,?)",
                   (time.time(), task, kind, json.dumps(body or {}, sort_keys=True)))

    def meta(self, key, value=None):
        with self.connection(write=value is not None) as cx:
            if value is not None:
                cx.execute("INSERT OR REPLACE INTO meta VALUES(?,?)", (key, json.dumps(value)))
            row = cx.execute("SELECT value FROM meta WHERE key=?", (key,)).fetchone()
            return json.loads(row[0]) if row else None

    def admit(self, specs, cfg):
        parsed = [validate_task(s) for s in specs]
        with self.connection(write=True) as cx:
            existing = {r["id"]: json.loads(r["spec"]) for r in cx.execute("SELECT id,spec FROM tasks")}
            combined = dict(existing)
            for s in parsed:
                if any(overlaps(p, sec) for p in s["paths"] for sec in cfg["security_paths"]):
                    s["security_review"] = True
                if s["id"] in combined and digest(combined[s["id"]]) != digest(s):
                    raise ValueError("task specification is immutable: " + s["id"])
                for grant in s["paths"]:
                    if any(overlaps(grant, p) for p in cfg["protected_paths"]):
                        raise ValueError("task overlaps protected policy/evaluator path: " + grant)
                if any(overlaps(p, sec) for p in s["paths"] for sec in cfg["security_paths"]):
                    s["security_review"] = True
                combined[s["id"]] = s
            pending = {k: set(v["dependencies"]) for k, v in combined.items()}
            if any(not ds <= pending.keys() for ds in pending.values()):
                raise ValueError("dependency is absent from the admitted graph")
            while pending:
                ready = {k for k, deps in pending.items() if not deps}
                if not ready:
                    raise ValueError("task dependency cycle")
                pending = {k: deps - ready for k, deps in pending.items() if k not in ready}
            for s in parsed:
                if s["id"] in existing:
                    continue
                now = time.time()
                cx.execute("INSERT INTO tasks(id,spec,spec_hash,created,updated) VALUES(?,?,?,?,?)",
                           (s["id"], json.dumps(s), digest(s), now, now))
                self.event(cx, "task.admitted", {"spec_hash": digest(s)}, s["id"])
        return [s["id"] for s in parsed]

    def tasks(self):
        with self.connection() as cx:
            return [dict(r) for r in cx.execute("SELECT * FROM tasks ORDER BY created,id")]

    def jobs(self, states=None):
        with self.connection() as cx:
            if states:
                qs = ",".join("?" for _ in states)
                return [dict(r) for r in cx.execute("SELECT * FROM jobs WHERE state IN ("+qs+") ORDER BY created", tuple(states))]
            return [dict(r) for r in cx.execute("SELECT * FROM jobs ORDER BY created")]

    def job(self, jid):
        with self.connection() as cx:
            row = cx.execute("SELECT * FROM jobs WHERE id=?", (jid,)).fetchone()
            return dict(row) if row else None

    def claim(self, tid, stage, route, payload, lease_seconds):
        """Atomic ownership. The supervisor is single, but claims are independently fenced."""
        now = time.time()
        with self.connection(write=True) as cx:
            row = cx.execute("SELECT * FROM tasks WHERE id=?", (tid,)).fetchone()
            if not row or row["state"] not in {"pending", "ready"} or row["stage"] != stage:
                return None
            if row["eligible_at"] > now:
                return None
            spec = json.loads(row["spec"])
            for dep in spec["dependencies"]:
                d = cx.execute("SELECT state FROM tasks WHERE id=?", (dep,)).fetchone()
                if not d or d[0] != "accepted":
                    return None
            # Grant belongs to an unfinished TASK, not to a permanent named lane.
            for other in cx.execute("SELECT id,spec,state FROM tasks WHERE id!=? AND state NOT IN ('pending','accepted','blocked','cancelled')", (tid,)):
                if any(overlaps(a, b) for a in spec["paths"] for b in json.loads(other["spec"])["paths"]):
                    return None
            jid = uuid.uuid4().hex
            token = uuid.uuid4().hex
            generation = row["generation"] + 1
            payload = dict(payload, task=spec, spec_hash=row["spec_hash"], job=jid, stage=stage, generation=generation)
            cx.execute("INSERT INTO jobs(id,task,stage,token,state,route,payload,lease_until,created,updated,generation) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                       (jid, tid, stage, token, "running", route, json.dumps(payload), now+lease_seconds, now, now, generation))
            cx.execute("UPDATE tasks SET state='running', generation=?, updated=? WHERE id=?", (generation, now, tid))
            self.event(cx, "job.claimed", {"job": jid, "stage": stage, "route": route}, tid)
            return dict(cx.execute("SELECT * FROM jobs WHERE id=?", (jid,)).fetchone())

    def set_pid(self, jid, token, pid):
        with self.connection(write=True) as cx:
            cx.execute("UPDATE jobs SET pid=? WHERE id=? AND token=? AND state='running'", (pid, jid, token))

    def valid(self, jid, token):
        with self.connection() as cx:
            row = cx.execute("SELECT jobs.*, tasks.generation AS current_generation FROM jobs JOIN tasks ON tasks.id=jobs.task WHERE jobs.id=?", (jid,)).fetchone()
            return bool(row and row["token"] == token and row["state"] == "running" and row["lease_until"] > time.time() and row["generation"] == row["current_generation"])

    def renew(self, owned, seconds):
        if not owned:
            return
        with self.connection(write=True) as cx:
            for jid, token in owned.items():
                cx.execute("UPDATE jobs SET lease_until=?,updated=? WHERE id=? AND token=? AND state='running' AND lease_until>?",
                           (time.time()+seconds, time.time(), jid, token, time.time()))

    def finish(self, jid, token, result):
        with self.connection(write=True) as cx:
            row = cx.execute("SELECT * FROM jobs WHERE id=?", (jid,)).fetchone()
            if not row or row["token"] != token or row["state"] != "running" or row["lease_until"] <= time.time():
                return False
            owner = cx.execute("SELECT generation FROM tasks WHERE id=?", (row["task"],)).fetchone()
            if not owner or owner[0] != row["generation"]:
                return False
            cx.execute("UPDATE jobs SET state='finished',result=?,updated=? WHERE id=?",
                       (json.dumps(result), time.time(), jid))
            self.event(cx, "job.finished", {"job": jid, "kind": result.get("kind")}, row["task"])
            return True

    def expired(self):
        now = time.time()
        with self.connection(write=True) as cx:
            rows = list(cx.execute("SELECT * FROM jobs WHERE state='running' AND lease_until<=?", (now,)))
            for row in rows:
                cx.execute("UPDATE jobs SET state='finished',result=?,updated=? WHERE id=?",
                           (json.dumps({"kind": "interrupted", "error": "worker lease expired"}), now, row["id"]))
                self.event(cx, "job.expired", {"job": row["id"], "stage": row["stage"]}, row["task"])
            return len(rows)

    def suspend_owned(self, owned):
        """Fence owner-stopped jobs without counting the stop as a defect."""
        with self.connection(write=True) as cx:
            for jid, token in owned.items():
                row = cx.execute("SELECT * FROM jobs WHERE id=? AND token=? AND state='running'", (jid, token)).fetchone()
                if not row:
                    continue
                cx.execute("UPDATE jobs SET state='finished',lease_until=0,result=?,updated=? WHERE id=?",
                    (json.dumps({"kind":"suspended", "error":"owner stopped supervisor; resume this stage"}), time.time(), jid))
                self.event(cx, "job.suspended", {"job":jid, "stage":row["stage"]}, row["task"])

    def transition(self, jid, values, event):
        columns = {"state", "stage", "attempt", "diagnoses", "base", "candidate", "workspace", "author",
                   "reviews", "report", "resume_stage", "accepted_sha", "changed_paths", "eligible_at"}
        if not set(values) <= columns:
            raise ValueError("invalid task mutation")
        with self.connection(write=True) as cx:
            row = cx.execute("SELECT * FROM jobs WHERE id=? AND state='finished'", (jid,)).fetchone()
            if not row:
                return False
            current = cx.execute("SELECT generation FROM tasks WHERE id=?", (row["task"],)).fetchone()
            if not current or current[0] != row["generation"]:
                cx.execute("UPDATE jobs SET state='consumed',updated=? WHERE id=?", (time.time(), jid))
                self.event(cx, "job.stale_discarded", {"job": jid}, row["task"])
                return False
            values = dict(values, updated=time.time())
            assignments = ",".join(k+"=?" for k in values)
            cx.execute("UPDATE tasks SET "+assignments+" WHERE id=?", tuple(values.values())+(row["task"],))
            cx.execute("UPDATE jobs SET state='consumed',updated=? WHERE id=?", (time.time(), jid))
            if values.get("state") == "accepted":
                for key in ("publication_barrier", "stop_new_publications"):
                    entry = cx.execute("SELECT value FROM meta WHERE key=?", (key,)).fetchone()
                    if entry and json.loads(entry[0]).get("task") == row["task"]:
                        cx.execute("UPDATE meta SET value='{}' WHERE key=?", (key,))
            self.event(cx, event, {"job": jid, **values}, row["task"])
            return True

    def resume_paused(self):
        """Watch mode retries infrastructure, never semantic rejection."""
        with self.connection(write=True) as cx:
            rows=list(cx.execute("SELECT * FROM tasks WHERE state='paused' AND eligible_at<=?",(time.time(),)))
            for row in rows:
                if row["resume_stage"] not in {"implement","review","diagnose"}: continue
                cx.execute("UPDATE tasks SET state='ready',stage=resume_stage,resume_stage='',attempt=0,eligible_at=0,updated=? WHERE id=?",(time.time(),row["id"]))
                self.event(cx,"task.infrastructure_resumed",{"stage":row["resume_stage"]},row["id"])
            return len(rows)

    def retry(self, tid, reason):
        with self.connection(write=True) as cx:
            row = cx.execute("SELECT * FROM tasks WHERE id=?", (tid,)).fetchone()
            if not row or row["state"] not in {"blocked", "cancelled", "paused"}:
                raise ValueError("retry requires a blocked/cancelled/paused task")
            stage = row["resume_stage"] if row["state"] in {"blocked", "paused"} and row["resume_stage"] else "implement"
            if stage not in {"implement", "review", "diagnose"}:
                raise ValueError("invalid infrastructure resume stage")
            reviews = row["reviews"] if stage != "implement" else "[]"
            report = (row["report"]+"\nOwner retry: "+reason)[-16000:]
            cx.execute("UPDATE tasks SET state=?,stage=?,generation=generation+1,attempt=0,diagnoses=0,reviews=?,resume_stage='',report=?,eligible_at=0,updated=? WHERE id=?",
                       ("pending" if stage == "implement" else "ready", stage, reviews, report, time.time(), tid))
            self.event(cx, "task.manual_retry", {"reason": reason}, tid)

    def cancel(self, tid):
        with self.connection(write=True) as cx:
            publishing = cx.execute("SELECT 1 FROM jobs WHERE task=? AND stage IN ('publish','postcheck') AND state IN ('running','finished')", (tid,)).fetchone()
            barrier = cx.execute("SELECT value FROM meta WHERE key='publication_barrier'").fetchone()
            stopped = cx.execute("SELECT value FROM meta WHERE key='stop_new_publications'").fetchone()
            if publishing or any(x and json.loads(x[0]).get("task") == tid for x in (barrier, stopped)):
                raise ValueError("cannot cancel an in-flight publication; stop the supervisor and reconcile its journal first")
            cx.execute("UPDATE jobs SET state='cancelled',lease_until=0,updated=? WHERE task=? AND state='running'", (time.time(), tid))
            cx.execute("UPDATE tasks SET state='cancelled',generation=generation+1,updated=? WHERE id=? AND state!='accepted'", (time.time(), tid))
            self.event(cx, "task.cancelled", {}, tid)

    def publish_intent(self, jid, tid, before, candidate, evidence):
        with self.connection(write=True) as cx:
            cx.execute("INSERT OR IGNORE INTO publications VALUES(?,?,?,?,?,?,?,?)",
                       (jid, tid, before, candidate, "intent", json.dumps(evidence), time.time(), time.time()))
            cx.execute("INSERT OR REPLACE INTO meta VALUES('publication_barrier',?)", (json.dumps({"job": jid, "task": tid, "sha": candidate}),))
            self.event(cx, "publication.intent", {"id": jid, "before": before, "candidate": candidate}, tid)

    def publication(self, jid, state=None):
        with self.connection(write=state is not None) as cx:
            if state:
                cx.execute("UPDATE publications SET state=?,updated=? WHERE id=?", (state, time.time(), jid))
            r = cx.execute("SELECT * FROM publications WHERE id=?", (jid,)).fetchone()
            return dict(r) if r else None

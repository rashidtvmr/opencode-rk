#!/usr/bin/env python3
"""Lean Harness autonomous controller (prototype, serial-or-leased).

Reference: README.md step 3, PLAN.md sections 4 and 8, prompts/START_HERE.md.

What it really does
-------------------
1. Loads the plan (`tools/plan_model.py`) and operator settings
   (`config/controller.settings.json`).
2. Computes the dependency-ready queue and excludes tasks whose ownership lock
   is already held by another in-flight lane.
3. For each selected task, creates an isolated git worktree, invokes a real
   worker through the `opencode run --agent <id> --format json` adapter, and
   waits with a bounded timeout.
4. Runs the configured verification commands (and the repo lane gate). A task
   is accepted only when verification passes; otherwise it is retried up to the
   attempt cap and then recorded `blocked` with the exact failure.
5. Appends an append-only receipt per attempt under `state/receipts.jsonl` and
   keeps `state/controller.json` as the resumable task ledger.

Honesty rules (from AGENTS.md and PLAN.md section 8)
----------------------------------------------------
- Never mark a task accepted without a passing verification command.
- Never fabricate a successful log; worker text is not evidence.
- Stop `blocked` rather than loop forever, weaken tests, or invent credentials.
- Only the controller writes `state/`; workers get an isolated worktree.

Stdlib only. No network except the configured worker adapter.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import json
import math
import os
import pathlib
import shlex
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import asdict, dataclass, field

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.plan_model import Plan, load_plan  # noqa: E402

STATE_DIR = ROOT / "state"
LEDGER = STATE_DIR / "controller.json"
RECEIPTS = STATE_DIR / "receipts.jsonl"
LOCK = STATE_DIR / "loop.lock"
SETTINGS = ROOT / "config" / "controller.settings.json"

DEFAULT_SETTINGS = {
    "maxConcurrentLanes": 6,
    "perTaskTimeoutSeconds": 3600,
    "maxAttemptsPerTask": 3,
    "leaseTtlSeconds": 120,
    "leaseHeartbeatSeconds": 30,
    "workerPool": ["9router-oc-muse-spark-1-3-contributor-free"],
    "verifierWorkerPool": ["9router-tr-glm-5-3-free"],
    "stateDir": "state",
    "worktreeRoot": ".worktrees",
    "verificationCommands": [
        ["python3", "tools/validate_plan.py"],
        ["python3", "tools/lane_gate.py", "--run"],
    ],
    "stopWhenNoReadyWork": True,
}


def load_settings() -> dict:
    settings = dict(DEFAULT_SETTINGS)
    if SETTINGS.is_file():
        settings.update(json.loads(SETTINGS.read_text(encoding="utf-8")))
    return settings


def ownership_lock(task_id: str) -> str:
    return f"feature:{task_id}"


@dataclass
class TaskState:
    id: str
    status: str = "not-started"  # not-started | running | blocked | accepted
    attempts: int = 0
    worker: str | None = None
    worktree: str | None = None
    last_error: str | None = None
    receipts: list[str] = field(default_factory=list)


class LeaseError(RuntimeError):
    """Base error for controller-owned worktree lease decisions."""


class LeaseHeldError(LeaseError):
    pass


class LeaseOwnerError(LeaseError):
    pass


class InvalidLeaseTtlError(LeaseError):
    pass


class LeaseCapacityError(LeaseError):
    pass


@dataclass(frozen=True)
class WorktreeLease:
    task_id: str
    owner: str
    acquired_at: float
    heartbeat_at: float
    expires_at: float


class LeaseTable:
    """Bounded deterministic state machine for controller-owned worktree leases."""

    def __init__(self, max_leases: int):
        if max_leases <= 0:
            raise LeaseCapacityError("max_leases must be positive")
        self.max_leases = max_leases
        self._leases: dict[str, WorktreeLease] = {}

    def get(self, task_id: str) -> WorktreeLease | None:
        return self._leases.get(task_id)

    def acquire(self, task_id: str, owner: str, *, now: float, ttl_seconds: float) -> WorktreeLease:
        self._validate_ttl(ttl_seconds)
        self._validate_identity(task_id, owner)
        self._prune_expired(now, keep_task=task_id)
        current = self._leases.get(task_id)
        if current is not None and current.expires_at > now:
            if current.owner != owner:
                raise LeaseHeldError(f"{task_id} is leased by another owner")
            return current
        if current is None and len(self._leases) >= self.max_leases:
            raise LeaseCapacityError(f"lease capacity {self.max_leases} reached")
        lease = WorktreeLease(task_id, owner, now, now, now + ttl_seconds)
        self._leases[task_id] = lease
        return lease

    def heartbeat(self, task_id: str, owner: str, *, now: float, ttl_seconds: float) -> WorktreeLease:
        self._validate_ttl(ttl_seconds)
        current = self._leases.get(task_id)
        if current is None or current.owner != owner or current.expires_at <= now:
            raise LeaseOwnerError(f"{task_id} lease is missing, expired, or owned by another lane")
        renewed = WorktreeLease(task_id, current.owner, current.acquired_at, now, now + ttl_seconds)
        self._leases[task_id] = renewed
        return renewed

    def release(self, task_id: str, owner: str) -> None:
        current = self._leases.get(task_id)
        if current is None:
            return
        if current.owner != owner:
            raise LeaseOwnerError(f"{task_id} lease is owned by another lane")
        del self._leases[task_id]

    def snapshot(self) -> list[WorktreeLease]:
        return [self._leases[key] for key in sorted(self._leases)]

    @classmethod
    def from_snapshot(cls, max_leases: int, rows: list[dict]) -> "LeaseTable":
        table = cls(max_leases)
        if len(rows) > max_leases:
            raise LeaseCapacityError("persisted lease count exceeds configured capacity")
        for row in rows:
            lease = WorktreeLease(
                task_id=str(row["task_id"]),
                owner=str(row["owner"]),
                acquired_at=float(row["acquired_at"]),
                heartbeat_at=float(row["heartbeat_at"]),
                expires_at=float(row["expires_at"]),
            )
            cls._validate_identity(lease.task_id, lease.owner)
            if not all(math.isfinite(value) for value in (lease.acquired_at, lease.heartbeat_at, lease.expires_at)):
                raise LeaseError("persisted lease contains non-finite time")
            if lease.acquired_at > lease.heartbeat_at or lease.heartbeat_at > lease.expires_at:
                raise LeaseError("persisted lease has invalid time ordering")
            if lease.task_id in table._leases:
                raise LeaseError(f"duplicate persisted lease for {lease.task_id}")
            table._leases[lease.task_id] = lease
        return table

    @staticmethod
    def _validate_ttl(ttl_seconds: float) -> None:
        if not math.isfinite(ttl_seconds) or ttl_seconds <= 0:
            raise InvalidLeaseTtlError("lease ttl must be a positive finite number")

    @staticmethod
    def _validate_identity(task_id: str, owner: str) -> None:
        if not task_id or not owner:
            raise LeaseError("task id and owner are required")

    def _prune_expired(self, now: float, *, keep_task: str) -> None:
        expired = [
            task_id
            for task_id, lease in self._leases.items()
            if task_id != keep_task and lease.expires_at <= now
        ]
        for task_id in expired:
            del self._leases[task_id]


def atomic_write_json(path: pathlib.Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    os.replace(tmp, path)


class DurableLeaseTable:
    """Thread-safe persisted lease table owned by the trusted controller."""

    def __init__(self, path: pathlib.Path, max_leases: int):
        self.path = path
        self._mutex = threading.Lock()
        self._table = self._load(max_leases)

    def _load(self, max_leases: int) -> LeaseTable:
        if not self.path.is_file():
            return LeaseTable(max_leases)
        raw = json.loads(self.path.read_text(encoding="utf-8"))
        if raw.get("schemaVersion") != 1 or not isinstance(raw.get("leases"), list):
            raise LeaseError("invalid persisted worktree lease state")
        now = time.time()
        rows = [row for row in raw["leases"] if float(row["expires_at"]) > now]
        return LeaseTable.from_snapshot(max_leases, rows)

    def _save(self) -> None:
        atomic_write_json(
            self.path,
            {
                "schemaVersion": 1,
                "leases": [asdict(lease) for lease in self._table.snapshot()],
            },
        )

    def get(self, task_id: str) -> WorktreeLease | None:
        with self._mutex:
            return self._table.get(task_id)

    def acquire(self, task_id: str, owner: str, *, now: float, ttl_seconds: float) -> WorktreeLease:
        with self._mutex:
            lease = self._table.acquire(task_id, owner, now=now, ttl_seconds=ttl_seconds)
            self._save()
            return lease

    def heartbeat(self, task_id: str, owner: str, *, now: float, ttl_seconds: float) -> WorktreeLease:
        with self._mutex:
            lease = self._table.heartbeat(task_id, owner, now=now, ttl_seconds=ttl_seconds)
            self._save()
            return lease

    def release(self, task_id: str, owner: str) -> None:
        with self._mutex:
            self._table.release(task_id, owner)
            self._save()


def append_receipt(record: dict) -> None:
    RECEIPTS.parent.mkdir(parents=True, exist_ok=True)
    with RECEIPTS.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")


class Ledger:
    def __init__(self, plan: Plan):
        self.plan = plan
        self.tasks: dict[str, TaskState] = {}
        if LEDGER.is_file():
            raw = json.loads(LEDGER.read_text(encoding="utf-8"))
            for task_id, data in raw.get("tasks", {}).items():
                if task_id in plan.stories:
                    self.tasks[task_id] = TaskState(**data)
        for task_id, story in plan.stories.items():
            self.tasks.setdefault(task_id, TaskState(id=task_id))
            if story.status != "not-started":
                self.tasks[task_id].status = story.status
        # Crash recovery: any task left `running` by a dead controller is
        # requeued. The singleton lock guarantees no live process owns these.
        for state in self.tasks.values():
            if state.status == "running":
                state.status = "not-started"
                state.last_error = "requeued after unclean shutdown"

    def save(self) -> None:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        atomic_write_json(LEDGER, {"tasks": {k: asdict(v) for k, v in self.tasks.items()}})

    @property
    def accepted(self) -> set[str]:
        return {t for t, s in self.tasks.items() if s.status == "accepted"}

    @property
    def blocked(self) -> set[str]:
        return {t for t, s in self.tasks.items() if s.status == "blocked"}

    @property
    def held_locks(self) -> set[str]:
        return {ownership_lock(t) for t, s in self.tasks.items() if s.status == "running"}

    def claim(self, chosen: list[str]) -> list[str]:
        """Return the subset of `chosen` whose ownership locks are free.

        Claiming marks each returned task `running` so a second claim in the same
        process (or a resumed run) cannot start the same slice twice. The caller
        owns resetting a task's status on failure.
        """
        picked: list[str] = []
        held = set(self.held_locks)
        for task_id in chosen:
            lock = ownership_lock(task_id)
            if lock in held:
                continue
            held.add(lock)
            picked.append(task_id)
        for task_id in picked:
            self.tasks[task_id].status = "running"
        return picked


def ready_queue(ledger: Ledger, limit: int, excluded: set[str] | None = None) -> list[str]:
    excluded = excluded or set()
    ready = ledger.plan.ready(ledger.accepted, ledger.blocked)
    ready = [t for t in ready if ledger.tasks[t].status == "not-started" and t not in excluded]
    return ledger.claim(ready[:limit])


def acquire_singleton() -> bool:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if LOCK.exists():
        try:
            payload = json.loads(LOCK.read_text(encoding="utf-8"))
            pid = int(payload.get("pid", 0))
            os.kill(pid, 0)
            return False
        except (ValueError, ProcessLookupError, PermissionError):
            pass
    LOCK.write_text(json.dumps({"pid": os.getpid(), "started": time.time()}), encoding="utf-8")
    return True


def release_singleton() -> None:
    try:
        LOCK.unlink()
    except FileNotFoundError:
        pass


def worker_prompt(task_id: str, story) -> str:
    obligations = "\n".join(f"- {o}" for o in story.test_obligations) or "- (none declared)"
    return f"""Role: senior implementer for the Lean Harness. Follow AGENTS.md and PLAN.md exactly.
Scope: task {task_id} only. Own one worktree. Do NOT edit shared contracts, controller state, budget, or other slices.
Goal: complete task {task_id} with independently verifiable evidence.
Task card: tasks/{task_id}.md (if absent, derive scope from ralph.json userStory and requirementIds).
Declared user story: {story.user_story}
Requirement ids: {", ".join(story.requirement_ids) or "(none)"}
Named test obligations (minimum taxonomy):
{obligations}
Deliverable: the code/documents for this slice plus captured command evidence (exact commands and tails).
Constraints: stdlib/native first; no new deps without an integration proposal; no emojis; no em dashes.
Verification: run the project gates and report exact output:
  python3 tools/validate_plan.py
  python3 tools/lane_gate.py --run
Success criteria: your owned tests pass, and you can state the exact command and result. If the task genuinely
cannot proceed (missing credentials, missing network, missing source pin), STOP and report `blocked` with the exact
reason and reproduction. Never fake success and never mark a task accepted yourself.
"""


def create_worktree(task_id: str, worktree_root: str) -> pathlib.Path:
    """Create (or reuse) an isolated branch-backed worktree for one task.

    A branch is used instead of a detached HEAD so accepted work can be
    fast-forwarded into the mainline by `finalize_worktree`.
    """
    base = (ROOT / worktree_root).resolve()
    base.mkdir(parents=True, exist_ok=True)
    path = base / task_id
    if path.exists():
        return path
    branch = f"auto/{task_id}"
    exists = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", branch],
        cwd=ROOT, capture_output=True, text=True,
    ).returncode == 0
    args = ["git", "worktree", "add"]
    if exists:
        args += [str(path), branch]
    else:
        args += ["-b", branch, str(path), "HEAD"]
    subprocess.run(args, cwd=ROOT, check=True, capture_output=True, text=True)
    return path


def finalize_worktree(task_id: str, worktree: pathlib.Path) -> tuple[bool, str]:
    """Commit worktree changes on its branch and fast-forward mainline if clean.

    Returns (integrated, message). If the mainline has diverged (a concurrent
    writer landed), integration is not attempted; the branch is preserved for
    the integrator per PLAN.md section 5.
    """
    branch = f"auto/{task_id}"
    add = subprocess.run(["git", "add", "-A"], cwd=worktree, capture_output=True, text=True)
    if add.returncode != 0:
        return False, f"git add failed: {add.stderr.strip()}"
    staged = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=worktree)
    if staged.returncode == 0:
        return True, "no changes to integrate"
    commit = subprocess.run(
        ["git", "-c", "user.name=lean-controller", "-c", "user.email=controller@local",
         "commit", "-q", "-m", f"auto({task_id}): accepted by controller"],
        cwd=worktree, capture_output=True, text=True,
    )
    if commit.returncode != 0:
        return False, f"commit failed: {commit.stderr.strip()}"
    merge = subprocess.run(
        ["git", "merge", "--ff-only", branch],
        cwd=ROOT, capture_output=True, text=True,
    )
    if merge.returncode != 0:
        return False, f"branch {branch} kept for integration (ff-only failed): {merge.stderr.strip()}"
    return True, f"integrated {branch}"


def _run_process_with_heartbeat(
    command: list[str],
    workdir: pathlib.Path,
    timeout_s: float,
    *,
    heartbeat=None,
    heartbeat_interval_s: float = 30.0,
) -> tuple[int, str]:
    if timeout_s <= 0 or heartbeat_interval_s <= 0:
        raise ValueError("process timeout and heartbeat interval must be positive")
    proc = subprocess.Popen(
        command,
        cwd=workdir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    deadline = time.monotonic() + timeout_s
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            proc.kill()
            stdout, stderr = proc.communicate()
            return 124, (stdout or "") + (stderr or "") + "\nprocess timed out"
        try:
            stdout, stderr = proc.communicate(timeout=min(heartbeat_interval_s, remaining))
            return proc.returncode, (stdout or "") + (stderr or "")
        except subprocess.TimeoutExpired:
            if heartbeat is None:
                continue
            try:
                heartbeat()
            except LeaseError as exc:
                proc.kill()
                stdout, stderr = proc.communicate()
                return 125, (stdout or "") + (stderr or "") + f"\nlease heartbeat failed: {exc}"


def run_worker(
    worker: str,
    prompt: str,
    workdir: pathlib.Path,
    timeout_s: int,
    *,
    heartbeat=None,
    heartbeat_interval_s: float = 30.0,
) -> tuple[int, str]:
    cmd = ["opencode", "run", "--agent", worker, "--format", "json", prompt]
    return _run_process_with_heartbeat(
        cmd,
        workdir,
        timeout_s,
        heartbeat=heartbeat,
        heartbeat_interval_s=heartbeat_interval_s,
    )


def run_verification(
    commands: list[list[str]],
    workdir: pathlib.Path,
    *,
    timeout_s: int = 3600,
    heartbeat=None,
    heartbeat_interval_s: float = 30.0,
) -> tuple[bool, str]:
    transcript: list[str] = []
    for command in commands:
        code, output = _run_process_with_heartbeat(
            command,
            workdir,
            timeout_s,
            heartbeat=heartbeat,
            heartbeat_interval_s=heartbeat_interval_s,
        )
        transcript.append(f"$ {' '.join(shlex.quote(c) for c in command)}\n{output}")
        if code != 0:
            return False, "\n".join(transcript)
    return True, "\n".join(transcript)


def run_task(
    task_id: str,
    story,
    settings: dict,
    worker: str,
    worktree: pathlib.Path,
    leases: DurableLeaseTable | None = None,
    lease_owner: str | None = None,
) -> dict:
    """Run one worker in its worktree and verify without touching the ledger.

    Safe to call concurrently from multiple threads because each lane owns a
    distinct worktree; optional lease heartbeats are owner-checked and serialized
    by the trusted durable lease table.
    """
    started = time.time()
    lease_ttl = float(settings.get("leaseTtlSeconds", 120))
    heartbeat_interval = float(settings.get("leaseHeartbeatSeconds", 30))

    def heartbeat() -> None:
        if leases is not None and lease_owner is not None:
            leases.heartbeat(task_id, lease_owner, now=time.time(), ttl_seconds=lease_ttl)

    heartbeat()
    code, output = run_worker(
        worker,
        worker_prompt(task_id, story),
        worktree,
        settings["perTaskTimeoutSeconds"],
        heartbeat=heartbeat,
        heartbeat_interval_s=heartbeat_interval,
    )
    tail = "\n".join(output.strip().splitlines()[-12:])
    if code == 0:
        ok, verify_log = run_verification(
            settings["verificationCommands"],
            worktree,
            timeout_s=settings["perTaskTimeoutSeconds"],
            heartbeat=heartbeat,
            heartbeat_interval_s=heartbeat_interval,
        )
    else:
        ok, verify_log = False, "verification skipped because worker did not complete successfully"
    return {
        "task": task_id,
        "worker": worker,
        "exitCode": code,
        "verification": "PASS" if ok else "FAIL",
        "seconds": round(time.time() - started, 1),
        "workerTail": tail,
        "verifyTail": "\n".join(verify_log.strip().splitlines()[-12:]),
        "worktree": str(worktree),
        "ok": code == 0 and ok,
    }


def record_result(ledger: Ledger, result: dict, settings: dict, worktree: pathlib.Path) -> str:
    """Apply a finished lane result to the ledger and integrate accepted work.

    Integration is serial and mainline-only, so it must run on the controller
    thread, never concurrently with another lane's acceptance.
    """
    task_id = result["task"]
    state = ledger.tasks[task_id]
    state.attempts += 1
    receipt = {k: result[k] for k in ("task", "worker", "exitCode", "verification", "seconds", "workerTail", "verifyTail")}
    receipt["attempt"] = state.attempts
    receipt_id = f"{task_id}#{state.attempts}@{int(time.time())}"
    state.receipts.append(receipt_id)

    if result["ok"]:
        state.status = "accepted"
        state.last_error = None
        integrated, message = finalize_worktree(task_id, worktree)
        receipt["integrated"] = integrated
        receipt["integration"] = message
        if not integrated:
            state.last_error = f"accepted but not integrated: {message}"
    elif state.attempts >= settings["maxAttemptsPerTask"]:
        state.status = "blocked"
        state.last_error = "verification failed" if result["verification"] == "FAIL" else f"worker exit {result['exitCode']}"
    else:
        state.status = "not-started"
        state.last_error = "retryable failure"

    append_receipt({**receipt, "id": receipt_id})
    ledger.save()
    return state.status


def status_report(ledger: Ledger, plan: Plan) -> str:
    counts: dict[str, int] = {}
    for state in ledger.tasks.values():
        counts[state.status] = counts.get(state.status, 0) + 1
    ready = plan.ready(ledger.accepted, ledger.blocked)
    waiting = plan.blocked_dependents(ledger.blocked)
    lines = [
        f"stories={len(ledger.tasks)} " + " ".join(f"{k}={v}" for k, v in sorted(counts.items())),
        f"ready={len(ready)} waiting_on_blocker={len(waiting)}",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Lean Harness autonomous controller")
    parser.add_argument("--list-ready", action="store_true")
    parser.add_argument("--status", action="store_true")
    parser.add_argument("--once", action="store_true", help="process one batch of ready lanes, then exit")
    parser.add_argument("--loop", action="store_true", help="keep processing until no ready work remains")
    parser.add_argument("--max-lanes", type=int, default=None)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    plan = load_plan(ROOT)
    if plan.errors:
        print("plan errors:", *plan.errors, sep="\n  ")
        return 2
    ledger = Ledger(plan)
    settings = load_settings()
    limit = args.max_lanes or settings["maxConcurrentLanes"]
    lease_ttl = float(settings.get("leaseTtlSeconds", 120))
    heartbeat_interval = float(settings.get("leaseHeartbeatSeconds", 30))
    if limit <= 0 or not math.isfinite(lease_ttl) or lease_ttl <= 0:
        print("invalid controller lane/lease settings")
        return 2
    if not math.isfinite(heartbeat_interval) or heartbeat_interval <= 0 or heartbeat_interval >= lease_ttl:
        print("leaseHeartbeatSeconds must be positive and less than leaseTtlSeconds")
        return 2

    if args.list_ready or args.status:
        print(status_report(ledger, plan))
        if args.list_ready:
            ready = ready_queue(ledger, limit)
            print("ready:", *ready, sep="\n  ")
        return 0

    if not acquire_singleton():
        print("another controller instance holds state/loop.lock; refusing to start")
        return 3

    try:
        leases = DurableLeaseTable(STATE_DIR / "worktree-leases.json", limit)
        leased_elsewhere: set[str] = set()
        while True:
            batch = ready_queue(ledger, limit, leased_elsewhere)
            if not batch:
                print(status_report(ledger, plan))
                if leased_elsewhere:
                    print(f"no safe ready work remains; {len(leased_elsewhere)} task(s) have live worktree leases")
                else:
                    print("no ready work remains")
                if ledger.blocked:
                    print(f"blocked tasks: {len(ledger.blocked)} (see state/receipts.jsonl)")
                return 0
            print("batch:", *batch, sep="\n  ")
            if args.dry_run:
                return 0
            pool = settings["workerPool"]
            # Each lane owns a durable lease until serial integration completes.
            # Lane threads renew their own lease while worker/verifier processes run.
            lanes = []
            for index, task_id in enumerate(batch):
                worker = pool[index % len(pool)]
                state = ledger.tasks[task_id]
                state.worker = worker
                owner = f"{os.getpid()}:{task_id}:{uuid.uuid4().hex}"
                try:
                    leases.acquire(task_id, owner, now=time.time(), ttl_seconds=lease_ttl)
                except LeaseHeldError as exc:
                    state.status = "not-started"
                    state.last_error = str(exc)
                    leased_elsewhere.add(task_id)
                    continue
                try:
                    worktree = create_worktree(task_id, settings["worktreeRoot"])
                except Exception:
                    leases.release(task_id, owner)
                    state.status = "not-started"
                    raise
                state.worktree = str(worktree)
                lanes.append((task_id, worker, worktree, owner))
            ledger.save()
            if not lanes:
                continue
            results: dict[str, dict] = {}
            with concurrent.futures.ThreadPoolExecutor(max_workers=len(lanes)) as executor:
                futures = {
                    executor.submit(
                        run_task,
                        task_id,
                        ledger.plan.stories[task_id],
                        settings,
                        worker,
                        worktree,
                        leases,
                        owner,
                    ): task_id
                    for task_id, worker, worktree, owner in lanes
                }
                for future in concurrent.futures.as_completed(futures):
                    task_id = futures[future]
                    try:
                        results[task_id] = future.result()
                    except Exception as exc:  # a lane crash must not kill the loop
                        results[task_id] = {"task": task_id, "worker": "", "exitCode": -1,
                                            "verification": "FAIL", "seconds": 0.0,
                                            "workerTail": f"lane raised: {exc}", "verifyTail": "", "ok": False}
            for task_id, worker, worktree, owner in lanes:
                try:
                    leases.heartbeat(task_id, owner, now=time.time(), ttl_seconds=lease_ttl)
                    status = record_result(ledger, results[task_id], settings, worktree)
                    print(f"  {task_id}: {status}")
                except LeaseError as exc:
                    state = ledger.tasks[task_id]
                    state.status = "not-started"
                    state.last_error = f"worktree lease lost before integration: {exc}"
                    ledger.save()
                    print(f"  {task_id}: not-started (lease lost before integration)")
                finally:
                    try:
                        leases.release(task_id, owner)
                    except LeaseOwnerError:
                        # A stale lane must never delete a lease reclaimed by a new owner.
                        pass
            if args.once:
                print(status_report(ledger, plan))
                return 0
    finally:
        release_singleton()


if __name__ == "__main__":
    raise SystemExit(main())

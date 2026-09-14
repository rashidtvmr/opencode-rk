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
import os
import pathlib
import shlex
import subprocess
import sys
import time
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


def atomic_write_json(path: pathlib.Path, payload: object) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    os.replace(tmp, path)


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


def ready_queue(ledger: Ledger, limit: int) -> list[str]:
    ready = ledger.plan.ready(ledger.accepted, ledger.blocked)
    ready = [t for t in ready if ledger.tasks[t].status == "not-started"]
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


def run_worker(worker: str, prompt: str, workdir: pathlib.Path, timeout_s: int) -> tuple[int, str]:
    cmd = ["opencode", "run", "--agent", worker, "--format", "json", prompt]
    try:
        proc = subprocess.run(cmd, cwd=workdir, capture_output=True, text=True, timeout=timeout_s)
    except subprocess.TimeoutExpired:
        return 124, "worker timed out"
    return proc.returncode, (proc.stdout or "") + (proc.stderr or "")


def run_verification(commands: list[list[str]], workdir: pathlib.Path) -> tuple[bool, str]:
    transcript: list[str] = []
    for command in commands:
        proc = subprocess.run(command, cwd=workdir, capture_output=True, text=True)
        transcript.append(f"$ {' '.join(shlex.quote(c) for c in command)}\n{proc.stdout}{proc.stderr}")
        if proc.returncode != 0:
            return False, "\n".join(transcript)
    return True, "\n".join(transcript)


def run_task(task_id: str, story, settings: dict, worker: str, worktree: pathlib.Path) -> dict:
    """Run one worker in its worktree and verify. Pure: does not touch the ledger.

    Safe to call concurrently from multiple threads because each lane owns a
    distinct worktree and never writes the controller state.
    """
    started = time.time()
    code, output = run_worker(worker, worker_prompt(task_id, story), worktree, settings["perTaskTimeoutSeconds"])
    tail = "\n".join(output.strip().splitlines()[-12:])
    ok, verify_log = run_verification(settings["verificationCommands"], worktree)
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
        while True:
            batch = ready_queue(ledger, limit)
            if not batch:
                print(status_report(ledger, plan))
                print("no ready work remains")
                if ledger.blocked:
                    print(f"blocked tasks: {len(ledger.blocked)} (see state/receipts.jsonl)")
                return 0
            print("batch:", *batch, sep="\n  ")
            if args.dry_run:
                return 0
            pool = settings["workerPool"]
            # Parallel lanes: each task gets an isolated worktree, so workers run
            # concurrently. Integration is serialized on this thread after the
            # batch completes. AUTO-003 upgrades leases/heartbeats.
            lanes = []
            for index, task_id in enumerate(batch):
                worker = pool[index % len(pool)]
                state = ledger.tasks[task_id]
                state.worker = worker
                worktree = create_worktree(task_id, settings["worktreeRoot"])
                state.worktree = str(worktree)
                lanes.append((task_id, worker, worktree))
            ledger.save()
            results: dict[str, dict] = {}
            with concurrent.futures.ThreadPoolExecutor(max_workers=len(lanes)) as executor:
                futures = {
                    executor.submit(run_task, task_id, ledger.plan.stories[task_id], settings, worker, worktree): task_id
                    for task_id, worker, worktree in lanes
                }
                for future in concurrent.futures.as_completed(futures):
                    task_id = futures[future]
                    try:
                        results[task_id] = future.result()
                    except Exception as exc:  # a lane crash must not kill the loop
                        results[task_id] = {"task": task_id, "worker": "", "exitCode": -1,
                                            "verification": "FAIL", "seconds": 0.0,
                                            "workerTail": f"lane raised: {exc}", "verifyTail": "", "ok": False}
            for task_id, worker, worktree in lanes:
                status = record_result(ledger, results[task_id], settings, worktree)
                print(f"  {task_id}: {status}")
            if args.once:
                print(status_report(ledger, plan))
                return 0
    finally:
        release_singleton()


if __name__ == "__main__":
    raise SystemExit(main())
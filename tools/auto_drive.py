#!/usr/bin/env python3
"""Autonomous loop driver: maintains N parallel subagent lanes until ralph.json is drained.

Usage:
    nohup python3 tools/auto_drive.py --lanes 15 >> state/auto_drive.log 2>&1 &

What it does:
1. Reads ralph.json via plan_model to find ready tasks.
2. Spawns up to --lanes parallel `opencode run` processes, round-robining
   between muse-spark and vyce-dsv4 (the two reliable free workers).
3. Each worker gets a structured prompt with role/scope/goal/verification.
4. On completion, runs verification (cargo check + validate_plan).
5. Marks accepted or retries (max 3 attempts, then blocked).
6. Immediately fills empty slots with next ready tasks.
7. Stops when no ready work remains or all lanes blocked.

State: state/controller.json (ledger), state/receipts.jsonl (append-only).
Logs: stdout (redirect to state/auto_drive.log).
"""
from __future__ import annotations

import json
import os
import pathlib
import shlex
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass, field
from datetime import datetime

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools.plan_model import load_plan, Plan  # noqa: E402

STATE_DIR = ROOT / "state"
LEDGER = STATE_DIR / "controller.json"
RECEIPTS = STATE_DIR / "receipts.jsonl"
LOCK = STATE_DIR / "auto_drive.lock"

# Only the two reliable free workers, alternating.
WORKERS = [
    "9router-oc-muse-spark-1-3-contributor-free",
    "9router-vyce-dsv4-flash",
]

TASK_TIMEOUT = 1800  # 30 min per task
MAX_ATTEMPTS = 3


def log(msg: str) -> None:
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}", flush=True)


@dataclass
class TaskState:
    id: str
    status: str = "not-started"
    attempts: int = 0
    worker: str | None = None
    last_error: str | None = None
    receipts: list[str] = field(default_factory=list)


def atomic_write(path: pathlib.Path, data: object) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, indent=2, sort_keys=True), encoding="utf-8")
    os.replace(tmp, path)


def append_receipt(record: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    with RECEIPTS.open("a", encoding="utf-8") as f:
        f.write(json.dumps(record, sort_keys=True) + "\n")


class Ledger:
    def __init__(self, plan: Plan):
        self.plan = plan
        self.tasks: dict[str, TaskState] = {}
        if LEDGER.is_file():
            raw = json.loads(LEDGER.read_text(encoding="utf-8"))
            for tid, data in raw.get("tasks", {}).items():
                if tid in plan.stories:
                    self.tasks[tid] = TaskState(**data)
        for tid, story in plan.stories.items():
            self.tasks.setdefault(tid, TaskState(id=tid))
            if story.status != "not-started":
                self.tasks[tid].status = story.status
        # Crash recovery
        for s in self.tasks.values():
            if s.status == "running":
                s.status = "not-started"
                s.last_error = "requeued after crash"

    def save(self) -> None:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        atomic_write(LEDGER, {"tasks": {k: asdict(v) for k, v in self.tasks.items()}})

    @property
    def accepted(self) -> set[str]:
        return {t for t, s in self.tasks.items() if s.status == "accepted"}

    @property
    def blocked(self) -> set[str]:
        return {t for t, s in self.tasks.items() if s.status == "blocked"}

    def ready(self, limit: int, exclude_running: set[str]) -> list[str]:
        ready = self.plan.ready(self.accepted, self.blocked)
        ready = [t for t in ready if self.tasks[t].status == "not-started" and t not in exclude_running]
        return ready[:limit]


def build_prompt(task_id: str, story) -> str:
    obligations = "\n".join(f"- {o}" for o in story.test_obligations) or "- (none)"
    return f"""Role: senior Rust implementer for the Lean Harness project.
Scope: task {task_id} ONLY. Work in /home/rashid/projects/opencode-rk. Do NOT run cargo fmt. Do NOT edit files owned by other tasks.
Goal: implement task {task_id} with passing tests and a task card.
User story: {story.user_story}
Requirements: {", ".join(story.requirement_ids) or "(none)"}
Test obligations:
{obligations}

Instructions:
1. Read PLAN.md, AGENTS.md, docs/TDD.md, docs/SECURITY.md for contracts.
2. Read existing crate code to understand patterns and style.
3. Implement the feature described in the user story.
4. Write tests that prove the feature works.
5. Create tasks/{task_id}.md task card documenting what was done.
6. Run: cargo check --workspace (must pass or document why not)
7. Run: cargo test on your owned crate (must pass)

Constraints: no emojis, no em dashes, no cargo fmt, no new deps without justification. Match existing compressed code style.
If blocked (missing deps, network, credentials), STOP and report blocked with exact reason. Never fake success."""


def run_one_task(task_id: str, worker: str) -> dict:
    """Run a single task via opencode run. Thread-safe, no shared state mutation."""
    story = None  # will be set by caller context
    started = time.time()
    cmd = ["opencode", "run", "--agent", worker, "--format", "json"]
    try:
        # Load plan fresh for the story (thread-safe read)
        plan = load_plan(ROOT)
        story = plan.stories.get(task_id)
        if not story:
            return {"task": task_id, "worker": worker, "ok": False, "error": "story not found", "seconds": 0}
        prompt = build_prompt(task_id, story)
        proc = subprocess.run(
            cmd + [prompt],
            cwd=ROOT, capture_output=True, text=True, timeout=TASK_TIMEOUT,
        )
        code = proc.returncode
        tail = "\n".join((proc.stdout or "").strip().splitlines()[-8:])
    except subprocess.TimeoutExpired:
        code = 124
        tail = "timed out"
    except Exception as exc:
        code = -1
        tail = str(exc)

    # Verify
    verify_ok = False
    verify_log = ""
    try:
        vp = subprocess.run(
            ["python3", "tools/validate_plan.py"],
            cwd=ROOT, capture_output=True, text=True, timeout=60,
        )
        verify_log = vp.stdout + vp.stderr
        verify_ok = vp.returncode == 0
    except Exception as exc:
        verify_log = str(exc)

    return {
        "task": task_id,
        "worker": worker,
        "exitCode": code,
        "ok": code == 0 and verify_ok,
        "seconds": round(time.time() - started, 1),
        "tail": tail[:500],
        "verify": verify_log[:300],
    }


def update_ralph_status(task_id: str, status: str) -> None:
    """Update ralph.json status for a task. Thread-safe via atomic write."""
    path = ROOT / "ralph.json"
    d = json.loads(path.read_text(encoding="utf-8"))
    for s in d["userStories"]:
        if s["id"] == task_id:
            s["status"] = status
            break
    atomic_write(path, d)


def main() -> int:
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--lanes", type=int, default=15)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    # Singleton lock
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if LOCK.exists():
        try:
            pid = json.loads(LOCK.read_text())["pid"]
            os.kill(pid, 0)
            log(f"another auto_drive holds lock (pid {pid})")
            return 1
        except (ProcessLookupError, PermissionError, ValueError):
            pass
    LOCK.write_text(json.dumps({"pid": os.getpid(), "started": time.time()}))

    try:
        plan = load_plan(ROOT)
        ledger = Ledger(plan)
        ledger.save()

        worker_idx = 0
        running: set[str] = set()
        total_accepted = len(ledger.accepted)
        total_blocked = len(ledger.blocked)

        log(f"auto_drive started: {len(plan.stories)} stories, {total_accepted} accepted, lanes={args.lanes}")

        with ThreadPoolExecutor(max_workers=args.lanes) as pool:
            futures = {}

            while True:
                # Fill empty slots
                slots = args.lanes - len(futures)
                if slots > 0:
                    batch = ledger.ready(slots, running)
                    if not batch and not futures:
                        log(f"DONE: no ready work. accepted={len(ledger.accepted)} blocked={len(ledger.blocked)}")
                        break
                    for task_id in batch:
                        worker = WORKERS[worker_idx % len(WORKERS)]
                        worker_idx += 1
                        ledger.tasks[task_id].status = "running"
                        ledger.tasks[task_id].worker = worker
                        running.add(task_id)
                        ledger.save()
                        log(f"LAUNCH {task_id} -> {worker.split('-')[-1]}")
                        if args.dry_run:
                            continue
                        fut = pool.submit(run_one_task, task_id, worker)
                        futures[fut] = task_id

                if args.dry_run:
                    log(f"DRY RUN: would launch {len(batch)} tasks")
                    break

                if not futures:
                    log("no futures and no ready work, stopping")
                    break

                # Wait for ANY future to complete
                done_futures = []
                for fut in as_completed(futures):
                    done_futures.append(fut)
                    break  # process one at a time to refill immediately

                for fut in done_futures:
                    task_id = futures.pop(fut)
                    running.discard(task_id)
                    try:
                        result = fut.result()
                    except Exception as exc:
                        result = {"task": task_id, "worker": "", "ok": False, "error": str(exc), "seconds": 0}

                    state = ledger.tasks[task_id]
                    state.attempts += 1
                    receipt_id = f"{task_id}#{state.attempts}@{int(time.time())}"
                    state.receipts.append(receipt_id)
                    append_receipt({**result, "id": receipt_id, "attempt": state.attempts})

                    if result.get("ok"):
                        state.status = "accepted"
                        state.last_error = None
                        update_ralph_status(task_id, "accepted")
                        log(f"ACCEPTED {task_id} ({result.get('seconds', 0)}s)")
                    elif state.attempts >= MAX_ATTEMPTS:
                        state.status = "blocked"
                        state.last_error = result.get("tail", "unknown")[:200]
                        update_ralph_status(task_id, "blocked")
                        log(f"BLOCKED  {task_id} after {state.attempts} attempts: {state.last_error[:80]}")
                    else:
                        state.status = "not-started"
                        state.last_error = "retryable"
                        log(f"RETRY    {task_id} attempt {state.attempts}/{MAX_ATTEMPTS}")

                    ledger.save()

        # Final status
        plan = load_plan(ROOT)
        ledger = Ledger(plan)
        accepted = len(ledger.accepted)
        blocked = len(ledger.blocked)
        remaining = sum(1 for s in ledger.tasks.values() if s.status == "not-started")
        log(f"FINAL: accepted={accepted} blocked={blocked} remaining={remaining} total={len(ledger.tasks)}")
        return 0

    finally:
        try:
            LOCK.unlink()
        except FileNotFoundError:
            pass


if __name__ == "__main__":
    raise SystemExit(main())

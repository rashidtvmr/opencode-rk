#!/usr/bin/env python3
"""Conservative event-driven delivery pump over the existing Recovery Mode state.

It does not spawn workers or invent approvals. An external agent orchestrator may
consume `status --json` and write candidate/review evidence through recovery_state.
Only INTEGRATION_READY candidates can be landed, by the trusted state manager.
"""
from __future__ import annotations

import argparse
import fcntl
import json
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def snapshot() -> dict:
    from tools import recovery_state as rs

    config = rs.load_json(rs.CONFIG_PATH)
    meta, failures, gates, _, paths = rs.require_state(config)
    canonical = rs.main_sha(config)
    rows = failures.get("failures", [])
    occupied_lanes = {r.get("lane") for r in rows if r.get("status") in rs.ACTIVE_STATES}
    ready = sorted(
        [r for r in rows if r.get("status") == "INTEGRATION_READY"],
        key=lambda r: (r.get("priority", "P9"), r.get("integrationReadyAt", ""), r["id"]),
    )
    dirty = bool(git("status", "--porcelain"))
    lock = paths["integration_lock"].exists()
    return {
        "canonicalSha": canonical,
        "stateRevision": meta.get("stateRevision"),
        "dirtyCanonical": dirty,
        "integrationLocked": lock,
        "ready": [{"id": r["id"], "lane": r.get("lane"), "baseSha": r.get("baseSha"),
                   "candidateSha": r.get("candidateSha"), "stale": r.get("baseSha") != canonical}
                  for r in ready],
        "availableLanes": [lane for lane in config["executionLanes"] if lane not in occupied_lanes
                           and git("rev-parse", lane) == canonical],
        "open": [{"id": r["id"], "priority": r.get("priority"), "title": r.get("title"),
                  "status": r.get("status")}
                 for r in sorted(rows, key=lambda r: (r.get("priority", "P9"), r["id"]))
                 if r.get("status") in {"OPEN", "UNPROVEN"}],
        "integrationBacklog": len(ready),
        "acceptedGates": sum(1 for g in gates.get("gates", [])
                             if g.get("status") == "ACCEPTED" and g.get("acceptedSha") == canonical),
        "gateCount": len(gates.get("gates", [])),
    }


def integrate_one(info: dict, *, owner: str, enabled: bool) -> dict:
    if not info["ready"]:
        return {"action": "idle"}
    candidate = info["ready"][0]
    if info["integrationLocked"]:
        return {"action": "blocked", "reason": "integration lock present; operator must reconcile"}
    if info["dirtyCanonical"]:
        return {"action": "blocked", "reason": "canonical worktree is dirty; preserve user changes"}
    if candidate["stale"]:
        return {"action": "blocked", "reason": "candidate base is stale; reopen/reapply and review exact new SHA",
                "id": candidate["id"]}
    if not enabled:
        return {"action": "would-integrate", "id": candidate["id"]}
    guard = subprocess.run([sys.executable, str(ROOT / "tools/recovery_guard.py")],
                           cwd=ROOT, text=True, capture_output=True)
    if guard.returncode:
        return {"action": "blocked", "reason": "recovery guard failed; reconcile before unattended integration",
                "guardOutput": (guard.stdout + guard.stderr)[-1600:]}
    before = git("rev-parse", "main-v2")
    if before != info["canonicalSha"]:
        return {"action": "blocked", "reason": "canonical SHA moved; resnapshot first"}
    result = subprocess.run([sys.executable, str(ROOT / "tools/recovery_state.py"), "integrate",
                             "--id", candidate["id"], "--owner", owner],
                            cwd=ROOT, text=True, capture_output=True)
    if result.returncode:
        return {"action": "failed", "id": candidate["id"],
                "reason": (result.stderr or result.stdout)[-1600:]}
    return {"action": "integrated", "id": candidate["id"], "sha": git("rev-parse", "main-v2"),
            "requiresJudge": True}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--once", action="store_true", help="print one actionable snapshot and exit")
    parser.add_argument("--execute-integration", action="store_true", help="opt-in trusted merge of reviewed ready candidate")
    parser.add_argument("--owner", default="recovery-delivery", help="unique integration owner identity")
    parser.add_argument("--interval", type=float, default=10.0)
    args = parser.parse_args()
    if not 1 <= args.interval <= 3600:
        parser.error("interval must be between 1 and 3600 seconds")
    from tools import recovery_state as rs
    lock_path = rs.common_git_dir() / "recovery-state" / "delivery-pump.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+") as fh:
        try:
            fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print(json.dumps({"action": "blocked", "reason": "delivery pump already running"}))
            return 2
        while True:
            try:
                info = snapshot()
                action = integrate_one(info, owner=args.owner, enabled=args.execute_integration)
                print(json.dumps({"snapshot": info, "delivery": action}, sort_keys=True), flush=True)
                # The installed application judge requires a verified release artifact.
                # Never mark work accepted or auto-run a quick judge as acceptance.
            except Exception as exc:
                print(json.dumps({"action": "blocked", "reason": str(exc)}), flush=True)
                if args.once:
                    return 2
            if args.once:
                return 0
            time.sleep(args.interval)


if __name__ == "__main__":
    raise SystemExit(main())

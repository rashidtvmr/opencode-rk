#!/usr/bin/env python3
"""Task claim/status ledger (agent coordination, stdlib only).

Workers coordinate through this checked-in ledger instead of touching
ralph.json or each other's files:

- CLAIM: a worker about to start APP-005 writes its session id and scratchpad
  path with status "in-progress". The claim is advisory fencing (last writer
  wins on disk), but the ledger is committed to git, so a second agent that
  pulls and sees a foreign claim for its target task must stop and pick
  different work.
- STATUS: a worker may only move its own claim forward:
  not-started -> in-progress -> completed. Nothing else is a valid transition.
- COMPLETED means: all test code for the task written (RED frozen before
  implementation), all frozen tests green on the integrated tree, and the
  feature/journey documented in the task's FEATURES.md section or worklog —
  without modifying any frozen test to make it pass.
- RELEASE: the orchestrator (or a human) is the only role allowed to clear a
  foreign stale claim (heartbeat expired) or reject a completed row.

Pure, bounded, no processes/network/writes. The file is data; this module
validates transitions and renders/merges rows.
"""
from __future__ import annotations

import json
import pathlib
import sys

LEDGER_RELPATH = "tasks/completion/claims.json"
ROOT = pathlib.Path(__file__).resolve().parent.parent
STATUSES = ("not-started", "in-progress", "completed", "blocked")
TRANSITIONS = {
    "not-started": {"in-progress", "blocked"},
    "in-progress": {"completed", "blocked"},
    "completed": set(),
    "blocked": {"in-progress"},
}
MAX_ROWS = 500
MAX_SESSION = 120
MAX_SCRATCHPAD = 200
MAX_NOTE = 400


class ClaimError(ValueError):
    """Invalid ledger operation."""


def load_ledger(root: pathlib.Path) -> dict:
    path = root / LEDGER_RELPATH
    if not path.is_file():
        return {"schemaVersion": 1, "claims": {}}
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("schemaVersion") != 1:
        raise ClaimError("unsupported claims ledger schemaVersion")
    claims = document.get("claims")
    if not isinstance(claims, dict) or len(claims) > MAX_ROWS:
        raise ClaimError("claims must be a bounded mapping")
    for tid, row in claims.items():
        validate_row(tid, row)
    return document


def validate_row(tid: str, row: dict) -> None:
    if not isinstance(tid, str) or not tid:
        raise ClaimError("claim id must be a non-empty string")
    if not isinstance(row, dict):
        raise ClaimError(f"{tid}: claim row must be an object")
    status = row.get("status")
    if status not in STATUSES:
        raise ClaimError(f"{tid}: invalid status {status!r}")
    session = row.get("session")
    if not isinstance(session, str) or len(session) > MAX_SESSION:
        raise ClaimError(f"{tid}: session must be a short string")
    if status != "not-started" and not session.strip():
        raise ClaimError(f"{tid}: claimed statuses require an owning session")
    scratchpad = row.get("scratchpad")
    if (
        not isinstance(scratchpad, str)
        or not scratchpad.strip()
        or len(scratchpad) > MAX_SCRATCHPAD
        or scratchpad.startswith("/")
        or ".." in pathlib.PurePosixPath(scratchpad).parts
        or "\\" in scratchpad
    ):
        raise ClaimError(f"{tid}: scratchpad must be a safe repo-relative path")


def validate_transition(old_status: str, new_status: str) -> None:
    if new_status not in TRANSITIONS.get(old_status, set()):
        raise ClaimError(f"illegal status transition {old_status!r} -> {new_status!r}")


def claim(root: pathlib.Path, tid: str, session: str, scratchpad: str) -> dict:
    """Claim a not-started task for one session (fenced against live claims).

    Write-through: the ledger file is updated so a concurrent worker that
    reloads sees the claim immediately. A task held `in-progress` OR `blocked`
    by another session is fenced: no new claim may take it; only the
    orchestrator's `reclaim` (with a recorded evidence note) can clear it.
    """
    document = load_ledger(root)
    claims = document["claims"]
    existing = claims.get(tid)
    if existing is not None and existing.get("status") in {"in-progress", "blocked"}:
        raise ClaimError(
            f"{tid}: fenced by session {existing.get('session')!r} ({existing.get('status')})"
        )
    claims[tid] = {
        "status": "in-progress",
        "session": session,
        "scratchpad": scratchpad,
    }
    save_ledger(root, document)
    return document


def reclaim(root: pathlib.Path, tid: str, session: str, evidence: str) -> dict:
    """Orchestrator-only: clear a foreign claim after proving the owner stopped.

    Requires a non-empty evidence note (e.g. heartbeat expired, worker exited).
    Returns the row to `not-started` with the original scratchpad retained so
    the prior session's worklog survives for reconciliation.
    """
    if not evidence.strip():
        raise ClaimError(f"{tid}: reclaim requires an evidence note")
    document = load_ledger(root)
    row = document["claims"].get(tid)
    if row is None:
        raise ClaimError(f"{tid}: nothing to reclaim")
    if row.get("status") not in {"in-progress", "blocked"}:
        raise ClaimError(f"{tid}: only a live claim can be reclaimed")
    scratchpad = row.get("scratchpad", "")
    document["claims"][tid] = {
        "status": "not-started",
        "session": "",
        "scratchpad": scratchpad,
        "reclaimedBy": session,
    }
    save_ledger(root, document)
    return document


def update(root: pathlib.Path, tid: str, session: str, new_status: str, note: str = "") -> dict:
    """Move an owned claim through a legal transition (write-through).

    `completed` and `blocked` require a non-empty evidence note: `completed`
    must carry the exact test commands/results proving frozen tests are green
    with zero test edits; `blocked` must carry the exact blocker.
    """
    if len(note) > MAX_NOTE:
        raise ClaimError(f"{tid}: note too long")
    if new_status in {"completed", "blocked"} and not note.strip():
        raise ClaimError(f"{tid}: {new_status} requires an evidence note")
    document = load_ledger(root)
    row = document["claims"].get(tid)
    if row is None:
        raise ClaimError(f"{tid}: no claim to update")
    if row.get("session") != session:
        raise ClaimError(f"{tid}: session {session!r} does not own this claim")
    validate_transition(row.get("status", "not-started"), new_status)
    row["status"] = new_status
    if new_status == "completed":
        row["completedNote"] = note
    elif new_status == "blocked":
        row["blockedNote"] = note
    save_ledger(root, document)
    return document


def release(root: pathlib.Path, tid: str, session: str) -> dict:
    """Release own in-progress claim back to not-started (write-through)."""
    document = load_ledger(root)
    row = document["claims"].get(tid)
    if row is None:
        raise ClaimError(f"{tid}: nothing to release")
    if row.get("session") != session:
        raise ClaimError(f"{tid}: session {session!r} does not own this claim")
    if row.get("status") != "in-progress":
        raise ClaimError(f"{tid}: only in-progress claims can be released")
    claims = document["claims"]
    claims[tid] = {"status": "not-started", "session": "", "scratchpad": row.get("scratchpad", "")}
    save_ledger(root, document)
    return document


def save_ledger(root: pathlib.Path, document: dict) -> None:
    for tid, row in document.get("claims", {}).items():
        validate_row(tid, row)
    path = root / LEDGER_RELPATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def ready_tasks(root: pathlib.Path, plan_stories: dict[str, dict]) -> list[str]:
    """Task ids whose deps are all completed and which no live claim holds."""
    document = load_ledger(root)
    claims = document["claims"]
    ready = []
    for tid, story in plan_stories.items():
        row = claims.get(tid, {})
        if row.get("status") in {"in-progress", "blocked"}:
            continue
        deps = story.get("deps", [])
        if all(claims.get(dep, {}).get("status") == "completed" for dep in deps):
            ready.append(tid)
    return sorted(ready)


def plan_stories(root: pathlib.Path) -> dict[str, dict]:
    """Union plan stories (id -> {deps,...}) for ledger coordination.

    Reuses the completion plan loader so the ledger can never disagree with
    the plan about task ids or dependencies. The loader forces every story
    status to `not-started`, which is exactly why progress lives HERE, not in
    the plan files.
    """
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import completion_plan

    plan = completion_plan.load(root)
    return {tid: {"deps": list(story.get("deps", []))} for tid, story in plan["stories"].items()}


def drift_errors(document: dict, plan_stories: dict[str, dict]) -> list[str]:
    """Claims must reference real plan tasks; completed claims must exist there."""
    errors = []
    for tid in document.get("claims", {}):
        if tid not in plan_stories:
            errors.append(f"{tid}: claim references unknown plan task")
    return errors


def scratchpad_report(document: dict, session: str) -> list[str]:
    """Scratchpad paths owned by one session (for the orchestrator handback)."""
    return sorted(
        row.get("scratchpad", "")
        for tid, row in document.get("claims", {}).items()
        if row.get("session") == session and row.get("scratchpad")
    )


__all__ = [
    "ClaimError", "LEDGER_RELPATH", "ROOT", "STATUSES", "TRANSITIONS",
    "claim", "drift_errors", "load_ledger", "plan_stories", "ready_tasks",
    "reclaim", "release", "save_ledger", "scratchpad_report", "update",
    "validate_row", "validate_transition",
]


if __name__ == "__main__":
    import sys

    root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(".")
    document = load_ledger(root)
    claims = document.get("claims", {})
    in_progress = sorted(t for t, r in claims.items() if r.get("status") == "in-progress")
    completed = sorted(t for t, r in claims.items() if r.get("status") == "completed")
    print(f"claims: {len(claims)} total, {len(in_progress)} in-progress, {len(completed)} completed")
    for tid in in_progress:
        row = claims[tid]
        print(f"  {tid}: {row.get('session')} -> {row.get('scratchpad')}")
    raise SystemExit(0)

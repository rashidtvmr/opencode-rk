#!/usr/bin/env python3
"""Plan model for the Lean Harness autonomous controller.

Loads `ralph.json` (the canonical story list) and `requirements/user-requirements.json`,
validates their shape, and derives the dependency edges needed to compute a
ready queue.

The shipped `ralph.json` was reconstructed and its `dependencyIds` are empty, so
this module synthesizes conservative edges from the milestone table in PLAN.md
section 4 and the requirement groups. Synthesized edges are recorded in the
returned model (`deps_synthesized=True`) so no consumer mistakes them for
authored dependencies.

Stdlib only. No network, no side effects on import.
"""
from __future__ import annotations

import json
import pathlib
from dataclasses import dataclass, field

ROOT = pathlib.Path(__file__).resolve().parents[1]

# Milestone rank from PLAN.md section 4. A lower rank must be Accepted before a
# higher rank may start. Within a rank, tasks may run in parallel unless they
# share an ownership lock (prefix collision is the ownership hint).
PHASE_RANK = {
    "DISC": 0,
    "BASE": 1,
    "SEC": 1,
    "DB": 1,
    "CAT": 2,
    "PROV": 2,
    "TOOL": 2,
    "SESS": 2,
    "AGENT": 3,
    "ROUTE": 3,
    "UI": 3,
    "SHARE": 4,
    "EXT": 4,
    "INT": 4,
    "WEB": 5,
    "OPS": 6,
    "AUTO": 6,
    "REL": 6,
    # Legacy reconciliation families (DISC-003 manifest / COORD-008
    # tools/completion_reconcile.py mirrors this table as its local copy).
    "SYNC": 2,
    "RUN": 2,
    "ACP": 2,
    "WSX": 5,
    "SDK": 5,
    "HEAD": 1,
}

# AUTO-001/002 are M0 (reference and trusted pipeline) despite the AUTO prefix.
PHASE_OVERRIDE = {"AUTO-001": 0, "AUTO-002": 0}

VALID_STATUS = {"not-started", "in-progress", "blocked", "accepted"}


class PlanError(ValueError):
    """Raised when the plan files are structurally invalid."""


@dataclass
class Story:
    id: str
    status: str
    user_story: str
    requirement_ids: list[str]
    dependency_ids: list[str]
    test_obligations: list[str]
    rank: int
    synthesized_deps: list[str] = field(default_factory=list)

    @property
    def allowed_status(self) -> bool:
        return self.status in VALID_STATUS


@dataclass
class Plan:
    stories: dict[str, Story]
    requirements: dict[str, list[str]]
    deps_synthesized: bool
    errors: list[str] = field(default_factory=list)

    def rank(self, task_id: str) -> int:
        return self.stories[task_id].rank

    def dependencies(self, task_id: str) -> list[str]:
        story = self.stories[task_id]
        return list(dict.fromkeys(story.dependency_ids + story.synthesized_deps))

    def ready(self, accepted: set[str], blocked: set[str]) -> list[str]:
        """Tasks that are not-started and whose dependencies are all accepted.

        Blocked tasks are never returned. A dependency that is blocked holds its
        dependents out of the ready set (they are not ready, but not blocked
        either) so the controller can report them as waiting on a blocker.
        """
        out: list[str] = []
        for task_id, story in self.stories.items():
            if story.status != "not-started":
                continue
            deps = self.dependencies(task_id)
            if any(dep in blocked for dep in deps):
                continue
            if all(dep in accepted for dep in deps):
                out.append(task_id)
        return sorted(out, key=lambda t: (self.rank(t), t))

    def blocked_dependents(self, blocked: set[str]) -> set[str]:
        """Tasks held out of the ready queue solely by a blocked dependency."""
        result: set[str] = set()
        for task_id, story in self.stories.items():
            if story.status != "not-started":
                continue
            deps = self.dependencies(task_id)
            if any(dep in blocked for dep in deps):
                result.add(task_id)
        return result


def _rank_for(task_id: str) -> int:
    if task_id in PHASE_OVERRIDE:
        return PHASE_OVERRIDE[task_id]
    prefix = task_id.split("-", 1)[0]
    if prefix not in PHASE_RANK:
        raise PlanError(f"Unknown task prefix for {task_id!r}")
    return PHASE_RANK[prefix]


def load_plan(root: pathlib.Path | None = None) -> Plan:
    root = root or ROOT
    ralph_path = root / "ralph.json"
    req_path = root / "requirements" / "user-requirements.json"
    if not ralph_path.is_file():
        raise PlanError(f"missing {ralph_path}")
    if not req_path.is_file():
        raise PlanError(f"missing {req_path}")

    ralph = json.loads(ralph_path.read_text(encoding="utf-8"))
    reqs_raw = json.loads(req_path.read_text(encoding="utf-8"))

    if ralph.get("schemaVersion") != 1:
        raise PlanError("unsupported ralph.json schemaVersion")
    raw_stories = ralph.get("userStories")
    if not isinstance(raw_stories, list):
        raise PlanError("ralph.json.userStories must be a list")

    errors: list[str] = []
    requirements: dict[str, list[str]] = {}
    for req in reqs_raw.get("requirements", []):
        rid = req.get("id")
        if not isinstance(rid, str):
            errors.append("requirement without an id")
            continue
        requirements[rid] = list(req.get("tasks", []))

    stories: dict[str, Story] = {}
    for raw in raw_stories:
        tid = raw.get("id")
        if not isinstance(tid, str):
            errors.append(f"story without an id: {raw!r}")
            continue
        if tid in stories:
            errors.append(f"duplicate story id {tid}")
            continue
        status = raw.get("status", "not-started")
        if status not in VALID_STATUS:
            errors.append(f"{tid}: invalid status {status!r}")
        try:
            rank = _rank_for(tid)
        except PlanError as exc:
            errors.append(str(exc))
            rank = max(PHASE_RANK.values())
        stories[tid] = Story(
            id=tid,
            status=status,
            user_story=str(raw.get("userStory", "")),
            requirement_ids=list(raw.get("requirementIds", [])),
            dependency_ids=list(raw.get("dependencyIds", [])),
            test_obligations=list(raw.get("testObligations", [])),
            rank=rank,
        )

    # Validate authored dependencies reference real stories.
    for story in stories.values():
        for dep in story.dependency_ids:
            if dep not in stories:
                errors.append(f"{story.id}: dependency {dep} does not exist")

    # Validate requirement task references.
    for rid, tasks in requirements.items():
        for task in tasks:
            if task not in stories:
                errors.append(f"{rid}: references unknown task {task}")

    deps_synthesized = not any(s.dependency_ids for s in stories.values())
    if deps_synthesized:
        # Milestone gating: a story in rank R depends on every story in a lower
        # rank. This is the conservative reading of PLAN.md section 4 (M0..M6)
        # and keeps higher-milestone work out of the queue until earlier
        # milestones are accepted. Within a rank, work runs in parallel subject
        # to the ownership-lock rule enforced by the controller.
        for story in stories.values():
            if story.rank == 0:
                continue
            story.synthesized_deps = sorted(
                t for t, other in stories.items() if other.rank < story.rank
            )

    return Plan(stories=stories, requirements=requirements, deps_synthesized=deps_synthesized, errors=errors)


if __name__ == "__main__":  # pragma: no cover - debug helper
    import sys

    plan = load_plan()
    if plan.errors:
        print("PLAN ERRORS:", *plan.errors, sep="\n  ")
        sys.exit(1)
    ready = plan.ready(set(), set())
    print(f"stories={len(plan.stories)} requirements={len(plan.requirements)} deps_synthesized={plan.deps_synthesized}")
    print(f"ready={len(ready)}: {ready[:20]}")

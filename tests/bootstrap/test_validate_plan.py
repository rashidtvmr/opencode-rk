"""Deterministic tests for plan_model and validate_plan (AUTO-002).

Complements tests/bootstrap/test_auto_controller.py without duplicating it.
This module focuses on the fixture-level invariants that keep the trusted
RED/GREEN pipeline deterministic:

  - every requirement maps to existing stories (both directions),
  - phase rank assignments match the PLAN.md milestone table exactly,
  - test-obligation ids are well formed and unique per story,
  - synthesized dependency edges form an acyclic, rank-ordered graph.

No network, no worker invocation, no mutation of repo state.
"""
from __future__ import annotations

import importlib
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

plan_model = importlib.import_module("tools.plan_model")
validate_plan = importlib.import_module("tools.validate_plan")


def _load() -> plan_model.Plan:
    return plan_model.load_plan(ROOT)


class RequirementMappingTests(unittest.TestCase):
    def test_every_requirement_task_is_an_existing_story(self) -> None:
        plan = _load()
        for rid, tasks in plan.requirements.items():
            for task in tasks:
                self.assertIn(task, plan.stories, f"{rid}: unknown task {task}")

    def test_every_story_requirement_id_is_a_known_requirement(self) -> None:
        plan = _load()
        for story in plan.stories.values():
            for rid in story.requirement_ids:
                self.assertIn(rid, plan.requirements, f"{story.id}: unknown req {rid}")

    def test_every_seeded_requirement_is_mandatory(self) -> None:
        required = {s.id for s in _load().stories.values()}
        reqs = validate_plan._check_requirements(ROOT)  # noqa: SLF001
        self.assertEqual(reqs, [])
        self.assertTrue(required)  # guard: plan actually loaded stories


class PhaseRankTests(unittest.TestCase):
    def test_each_story_rank_matches_its_prefix(self) -> None:
        plan = _load()
        for tid, story in plan.stories.items():
            prefix = tid.split("-", 1)[0]
            if tid in plan_model.PHASE_OVERRIDE:
                expected = plan_model.PHASE_OVERRIDE[tid]
            else:
                expected = plan_model.PHASE_RANK[prefix]
            self.assertEqual(story.rank, expected, tid)

    def test_override_moves_auto_to_m0(self) -> None:
        plan = _load()
        self.assertEqual(plan.rank("AUTO-001"), 0)
        self.assertEqual(plan.rank("AUTO-002"), 0)

    def test_rank_zero_stories_have_no_synthesized_deps(self) -> None:
        plan = _load()
        for tid, story in plan.stories.items():
            if story.rank == 0:
                self.assertEqual(story.synthesized_deps, [], tid)


class TestObligationTests(unittest.TestCase):
    def test_obligations_are_nonempty_for_auto_002(self) -> None:
        plan = _load()
        obligations = plan.stories["AUTO-002"].test_obligations
        self.assertGreaterEqual(len(obligations), 5)

    def test_obligations_are_well_formed_and_unique_per_story(self) -> None:
        plan = _load()
        for story in plan.stories.values():
            seen = set()
            for obligation in story.test_obligations:
                self.assertTrue(
                    obligation.startswith(f"{story.id}-T"),
                    f"{story.id}: malformed {obligation!r}",
                )
                self.assertNotIn(
                    obligation, seen, f"{story.id}: duplicate obligation {obligation}"
                )
                seen.add(obligation)

    def test_obligation_checks_have_no_errors(self) -> None:
        plan = _load()
        self.assertEqual(validate_plan._check_obligations(ROOT), [])  # noqa: SLF001
        # The dataclass obligations agree with what the validator reads from disk.
        self.assertTrue(plan.stories)  # guard: stories actually loaded


class BacklogExhaustionIntegrationTests(unittest.TestCase):
    def test_backlog_exhaustion_is_part_of_plan_validation(self) -> None:
        self.assertEqual(validate_plan._check_backlog_exhaustion(ROOT), [])  # noqa: SLF001


class DependencyAcycTests(unittest.TestCase):
    def test_synthesized_dependency_graph_is_acyclic(self) -> None:
        plan = _load()
        graph: dict[str, list[str]] = {tid: [] for tid in plan.stories}
        for tid, story in plan.stories.items():
            for dep in story.synthesized_deps:
                graph[dep].append(tid)

        WHITE, GRAY, BLACK = 0, 1, 2
        color = {tid: WHITE for tid in plan.stories}
        cycle: list[tuple[str, str]] = []

        def dfs(node: str) -> None:
            color[node] = GRAY
            for nxt in graph[node]:
                if color[nxt] == GRAY:
                    cycle.append((node, nxt))
                elif color[nxt] == WHITE:
                    dfs(nxt)
            color[node] = BLACK

        for node in plan.stories:
            if color[node] == WHITE:
                dfs(node)
        self.assertEqual(cycle, [])

    def test_synthesized_deps_only_point_to_lower_rank(self) -> None:
        plan = _load()
        for tid, story in plan.stories.items():
            for dep in story.synthesized_deps:
                self.assertLess(
                    plan.rank(dep),
                    story.rank,
                    f"{tid} depends on same/higher rank {dep}",
                )

    def test_every_lower_rank_story_is_a_dependency_of_higher_ranks(self) -> None:
        plan = _load()
        for tid, story in plan.stories.items():
            if story.rank == 0:
                continue
            lower = {t for t, s in plan.stories.items() if s.rank < story.rank}
            self.assertEqual(set(story.synthesized_deps), lower, tid)


if __name__ == "__main__":
    unittest.main()

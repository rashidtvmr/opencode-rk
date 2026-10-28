"""Validated, immutable task and workflow inputs.

Agent output is never interpreted as a command, a test definition, or approval of
its own change. Only the owner-supplied task manifest defines these obligations.
"""
from __future__ import annotations

import hashlib
import json
import math
import pathlib
import re
from typing import Any

ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,95}\Z")
SHA = re.compile(r"[0-9a-f]{40,64}\Z")
STAGES = ("implement", "review", "test", "prepare", "publish", "diagnose")
TERMINAL = {"accepted", "blocked", "cancelled"}
ACTIVE = {"implement", "review", "test", "prepare", "publish", "diagnose"}
DEFAULT_PROTECTED = [".git", ".github", "AGENTS.md", "opencode.json", "opencode.jsonc", "DELIVERY_REVIEW.diff",
                     ".opencode", "config", "tools/delivery", "tools/continuous_delivery.py",
                     "tools/recovery_state.py", "tools/recovery_guard.py", "tools/phase1_judge.py",
                     "tools/recovery_ref_hook.py", "docs/PHASE1_ACCEPTANCE.md", "recovery"]


def digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def path(value: str) -> str:
    if not isinstance(value, str) or not value or value.startswith("/") or "\\" in value:
        raise ValueError("grant must be a nonempty relative POSIX path")
    if any(c in value for c in "*?[]\n\r\x00") or any(p in ("", ".", "..") for p in value.split("/")):
        raise ValueError("path wildcards, traversal and control characters are forbidden")
    return value


def overlaps(a: str, b: str) -> bool:
    return a == b or a.startswith(b + "/") or b.startswith(a + "/")


def within(child: str, parent: str) -> bool:
    return child == parent or child.startswith(parent + "/")


def positive(value: Any, label: str, maximum: int = 1000000) -> int:
    if type(value) is not int or not 1 <= value <= maximum:
        raise ValueError("%s must be an integer in 1..%s" % (label, maximum))
    return value


def checks(value: Any, label: str, required: bool = True) -> list[dict]:
    if not isinstance(value, list) or (required and not value):
        raise ValueError(label + " needs a nonempty list of trusted test commands")
    output = []
    names = set()
    for entry in value:
        if not isinstance(entry, dict) or not ID.fullmatch(str(entry.get("id", ""))):
            raise ValueError(label + " check needs a stable id")
        if entry["id"] in names:
            raise ValueError("duplicate check id")
        names.add(entry["id"])
        argv = entry.get("argv")
        if not isinstance(argv, list) or not argv or not all(isinstance(a, str) and a and "\x00" not in a for a in argv):
            raise ValueError("check argv must be an argument array, not a shell command")
        # A manifest supplied by the owner is trusted, but reject obvious fake checks.
        if pathlib.PurePath(argv[0]).name in {"true", "echo", "printf", "sleep"}:
            raise ValueError("a no-op is not an acceptance command")
        assertions = entry.get("required_output", [])
        if not isinstance(assertions, list) or not all(isinstance(a, str) and a for a in assertions):
            raise ValueError("required_output must be a list of regular expressions")
        for pattern in assertions:
            re.compile(pattern)
        output.append({"id": entry["id"], "argv": list(argv), "required_output": assertions,
                       "timeout": positive(entry.get("timeout", 900), "check timeout", 86400)})
    return output


def task(raw: dict) -> dict:
    if not isinstance(raw, dict) or not ID.fullmatch(str(raw.get("id", ""))):
        raise ValueError("task id must be a bounded identifier")
    goal = raw.get("goal", "")
    evidence = raw.get("source", "")
    if not isinstance(goal, str) or len(goal.strip()) < 10 or len(goal) > 24000:
        raise ValueError("task needs an observable goal (10..24000 characters)")
    if not isinstance(evidence, str) or not evidence.strip():
        raise ValueError("task requires a user requirement or source/failure reference")
    grants = raw.get("paths", [])
    if not isinstance(grants, list) or not grants or len(grants) > 100:
        raise ValueError("task requires bounded explicit path grants")
    grants = sorted(set(path(x) for x in grants))
    if any(overlaps(x, y) for i, x in enumerate(grants) for y in grants[i+1:]):
        raise ValueError("redundant overlapping grants within task")
    deps = raw.get("dependencies", [])
    if not isinstance(deps, list) or len(set(deps)) != len(deps) or not all(ID.fullmatch(str(x)) for x in deps):
        raise ValueError("invalid dependencies")
    if raw["id"] in deps:
        raise ValueError("self dependency")
    priority = raw.get("priority", 2)
    if type(priority) is not int or not 0 <= priority <= 3:
        raise ValueError("priority must be 0..3")
    reviews = positive(raw.get("reviews", 2 if priority <= 1 else 1), "reviews", 4)
    if priority <= 1 and reviews < 2:
        raise ValueError("P0/P1 requires two independent reviews")
    return {"id": raw["id"], "goal": goal, "source": evidence, "paths": grants,
            "dependencies": sorted(deps), "priority": priority, "reviews": reviews,
            "checks": checks(raw.get("checks"), "task"),
            "red_checks": checks(raw.get("red_checks", []), "red_checks", required=False),
            "security_review": bool(raw.get("security_review", False))}


def config(raw: dict) -> dict:
    if not isinstance(raw, dict):
        raise ValueError("workflow configuration must be an object")
    if raw.get("schema", 1) != 1 or type(raw.get("schema", 1)) is not int:
        raise ValueError("unsupported workflow schema")
    c = dict(raw)
    c["schema"] = 1
    c["branch"] = raw.get("branch", "main-v2")
    if not isinstance(c["branch"], str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_./-]{0,100}", c["branch"]):
        raise ValueError("invalid canonical branch")
    if ".." in c["branch"] or c["branch"].endswith(("/", ".")) or "//" in c["branch"] or any(part.startswith(".") or part.endswith(".lock") for part in c["branch"].split("/")):
        raise ValueError("invalid canonical branch")
    c["workers"] = positive(raw.get("workers", 20), "workers", 48)
    c["max_wip"] = positive(raw.get("max_wip", c["workers"] * 2), "max_wip", 192)
    c["heavy_checks"] = positive(raw.get("heavy_checks", 1), "heavy_checks", 1)
    c["lease_seconds"] = positive(raw.get("lease_seconds", 60), "lease_seconds", 3600)
    c["min_free_bytes"] = positive(raw.get("min_free_bytes", 536870912), "min_free_bytes", 1099511627776)
    c["stage_timeout"] = positive(raw.get("stage_timeout", 1800), "stage_timeout", 86400)
    c["attempts"] = positive(raw.get("attempts", 3), "attempts", 10)
    c["diagnoses"] = positive(raw.get("diagnoses", 1), "diagnoses", 3)
    c["max_output_bytes"] = positive(raw.get("max_output_bytes", 2097152), "max_output_bytes", 16777216)
    c["full_checks"] = checks(raw.get("full_checks"), "full_checks")
    c["post_merge_checks"] = checks(raw.get("post_merge_checks", raw.get("full_checks")), "post_merge_checks")
    c["guard_checks"] = checks(raw.get("guard_checks", []), "guard_checks", required=False)
    c["protected_paths"] = sorted(set(DEFAULT_PROTECTED + [path(x) for x in raw.get("protected_paths", [])]))
    c["frozen_tests"] = sorted(set(path(x) for x in raw.get("frozen_tests", ["tests/e2e"])))
    c["security_paths"] = sorted(set(path(x) for x in raw.get("security_paths", ["crates/security", "crates/providers", "crates/tools"])))
    c["runtime"] = dict(raw.get("runtime", {}))
    if c["runtime"].get("kind") not in {"opencode", "command"}:
        raise ValueError("runtime.kind must be opencode or an explicitly configured command")
    if c["runtime"]["kind"] == "command":
        argv = c["runtime"].get("argv")
        if not isinstance(argv, list) or not argv or not all(isinstance(x, str) and x for x in argv):
            raise ValueError("command runtime requires explicit argv")
    else:
        c["runtime"].setdefault("executable", "opencode")
        exe = c["runtime"]["executable"]
        if not isinstance(exe, str) or not exe or "\x00" in exe:
            raise ValueError("runtime executable must be a nonempty path/name")
        if type(c["runtime"].get("standalone", False)) is not bool:
            raise ValueError("runtime.standalone must be boolean")
    if type(c.get("use_recovery_capability", False)) is not bool:
        raise ValueError("use_recovery_capability must be boolean")
    forwarded = c["runtime"].setdefault("forward_env", [])
    if (not isinstance(forwarded, list) or len(forwarded) > 32
            or not all(isinstance(x, str) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", x)
                       and not x.startswith(("OC2_RECOVERY_", "DELIVERY_", "GIT_")) for x in forwarded)
            or len(set(forwarded)) != len(forwarded)):
        raise ValueError("runtime.forward_env must explicitly name allowed non-controller environment keys")
    routes = raw.get("routes", [])
    if not isinstance(routes, list) or not routes:
        raise ValueError("explicit authorized routes are required; no implicit fallback")
    seen = set()
    for r in routes:
        if not isinstance(r, dict) or not isinstance(r.get("agent"), str) or not r["agent"] or r["agent"].startswith("-"):
            raise ValueError("invalid authorized agent route")
        if r["agent"] in seen:
            raise ValueError("duplicate agent route")
        seen.add(r["agent"])
        positive(r.get("max_parallel", c["workers"]), "route parallel cap", 48)
    c["routes"] = [{"agent": r["agent"], "max_parallel": r.get("max_parallel", c["workers"])} for r in routes]
    return c

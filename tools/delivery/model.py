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
    retained = raw.get("retained_package")
    if retained is not None:
        if not isinstance(retained, dict) or not isinstance(retained.get("workspace"), str) or not pathlib.Path(retained["workspace"]).is_absolute():
            raise ValueError("retained_package.workspace must be absolute")
        if not SHA.fullmatch(str(retained.get("base", ""))) or not SHA.fullmatch(str(retained.get("source_head", ""))):
            raise ValueError("retained_package base/source_head must be exact Git identities")
        files = retained.get("files")
        if not isinstance(files, dict) or not files or any(not path(k) or not re.fullmatch(r"[0-9a-f]{64}", str(v)) for k, v in files.items()):
            raise ValueError("retained_package.files must be an exact path/hash manifest")
        if not isinstance(retained.get("feature_ref"), str) or not re.fullmatch(r"refs/heads/[A-Za-z0-9][A-Za-z0-9_.-]*(?:/[A-Za-z0-9][A-Za-z0-9_.-]*)*", retained["feature_ref"]):
            raise ValueError("retained_package.feature_ref is invalid")
        if not isinstance(retained.get("common_git_dir"), str) or not pathlib.Path(retained["common_git_dir"]).is_absolute():
            raise ValueError("retained_package.common_git_dir must be absolute")
        custody = retained.get("custody")
        if custody is None:
            custody = files
        if not isinstance(custody, dict) or any(not path(k) or not re.fullmatch(r"[0-9a-f]{64}", str(v)) for k, v in custody.items()):
            raise ValueError("retained_package.custody must be an exact path/hash manifest")
        if not set(files) <= set(custody):
            raise ValueError("retained_package.custody must include every package file")
        had_custody = "custody" in retained
        retained = {"workspace": str(pathlib.Path(retained["workspace"]).resolve()), "common_git_dir": str(pathlib.Path(retained["common_git_dir"]).resolve()),
                    "base": retained["base"], "source_head": retained["source_head"], "files": dict(sorted(files.items())), "feature_ref": retained["feature_ref"]}
        if had_custody:
            retained["custody"] = dict(sorted(custody.items()))
    pinned = raw.get("pinned_red_fixture")
    if pinned is not None:
        if not isinstance(pinned, dict):
            raise ValueError("pinned_red_fixture must be an object")
        required = {"task_id", "base", "candidate", "expires_at", "fixture_sha256", "fixture_path", "argv", "required_output", "target_path", "module_name", "declaration"}
        if set(pinned) < required or pinned.get("task_id") != raw["id"]:
            raise ValueError("pinned_red_fixture authorization is incomplete or wrong task")
        if not SHA.fullmatch(str(pinned["base"])) or not SHA.fullmatch(str(pinned["candidate"])):
            raise ValueError("pinned_red_fixture revisions must be exact Git identities")
        if not re.fullmatch(r"[0-9a-f]{64}", str(pinned["fixture_sha256"])):
            raise ValueError("pinned_red_fixture fixture hash is malformed")
        if not isinstance(pinned["fixture_path"], str) or pathlib.PurePosixPath(pinned["fixture_path"]).is_absolute() or ".." in pathlib.PurePosixPath(pinned["fixture_path"]).parts:
            raise ValueError("pinned_red_fixture fixture path is unsafe")
        if not isinstance(pinned["argv"], list) or not pinned["argv"] or not all(isinstance(x, str) and x for x in pinned["argv"]):
            raise ValueError("pinned_red_fixture argv must be an argument array")
        if not isinstance(pinned["required_output"], list) or not pinned["required_output"] or not all(isinstance(x, str) and x for x in pinned["required_output"]):
            raise ValueError("pinned_red_fixture requires assertion patterns")
        for key in ("target_path", "module_name", "declaration"):
            if not isinstance(pinned[key], str) or not pinned[key] or ".." in pathlib.PurePosixPath(pinned[key]).parts or pathlib.PurePosixPath(pinned[key]).is_absolute():
                raise ValueError("pinned_red_fixture injection metadata is unsafe")
        if "fixture_source" in pinned:
            if not isinstance(pinned["fixture_source"], str) or not pathlib.Path(pinned["fixture_source"]).is_absolute():
                raise ValueError("pinned_red_fixture fixture_source must be absolute")
            if not re.fullmatch(r"[0-9a-f]{64}", str(pinned.get("fixture_source_sha256", ""))):
                raise ValueError("pinned_red_fixture fixture_source hash is malformed")
        try:
            expiry = float(pinned["expires_at"])
        except (TypeError, ValueError):
            raise ValueError("pinned_red_fixture expiry is malformed")
        if not math.isfinite(expiry) or expiry <= 0:
            raise ValueError("pinned_red_fixture authorization is expired")
        had_fixture_source = "fixture_source" in pinned
        fixture_source = pinned.get("fixture_source")
        fixture_source_sha256 = pinned.get("fixture_source_sha256")
        pinned = {"task_id": raw["id"], "base": pinned["base"], "candidate": pinned["candidate"],
                  "expires_at": expiry, "fixture_sha256": pinned["fixture_sha256"],
                  "fixture_path": pinned["fixture_path"], "argv": list(pinned["argv"]),
                  "required_output": list(pinned["required_output"]), "target_path": pinned["target_path"],
                  "module_name": pinned["module_name"], "declaration": pinned["declaration"],
                  "production_files": dict(sorted((pinned.get("production_files") or {}).items())),
                  "base_production_files": dict(sorted((pinned.get("base_production_files") or pinned.get("production_files") or {}).items())),
                  "candidate_production_files": dict(sorted((pinned.get("candidate_production_files") or pinned.get("production_files") or {}).items()))}
        if had_fixture_source:
            pinned["fixture_source"] = str(pathlib.Path(fixture_source).resolve())
            pinned["fixture_source_sha256"] = fixture_source_sha256
        for field in ("base_production_files", "candidate_production_files"):
            if any(not path(k) or not re.fullmatch(r"[0-9a-f]{64}", str(v)) for k, v in pinned[field].items()):
                raise ValueError("pinned production manifest is malformed")
    result = {"id": raw["id"], "goal": goal, "source": evidence, "paths": grants,
            "dependencies": sorted(deps), "priority": priority, "reviews": reviews,
            "checks": checks(raw.get("checks"), "task"), "retained_package": retained,
            "red_checks": checks(raw.get("red_checks", []), "red_checks", required=False),
            "security_review": bool(raw.get("security_review", False))}
    if pinned is not None:
        result["pinned_red_fixture"] = pinned
    return result


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
    c["workers"] = positive(raw.get("workers", 30), "workers", 128)
    c["max_wip"] = positive(raw.get("max_wip", c["workers"] * 2), "max_wip", 512)
    c["heavy_checks"] = positive(raw.get("heavy_checks", 3), "heavy_checks", 8)
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
    scoped = raw.get("retained_package_authorization")
    if scoped is not None:
        if (not isinstance(scoped, dict) or not isinstance(scoped.get("task_id"), str) or
                not isinstance(scoped.get("paths"), list) or not isinstance(scoped.get("workspace"), str) or
                not pathlib.Path(scoped["workspace"]).is_absolute() or
                not SHA.fullmatch(str(scoped.get("base", ""))) or
                not SHA.fullmatch(str(scoped.get("source_head", ""))) or
                not isinstance(scoped.get("common_git_dir"), str) or not pathlib.Path(scoped["common_git_dir"]).is_absolute() or
                not isinstance(scoped.get("feature_ref"), str) or not re.fullmatch(r"refs/heads/[A-Za-z0-9][A-Za-z0-9_.-]*(?:/[A-Za-z0-9][A-Za-z0-9_.-]*)*", scoped["feature_ref"]) or
                not isinstance(scoped.get("files"), dict) or not scoped["files"] or
                any(not re.fullmatch(r"[0-9a-f]{64}", str(v)) for v in scoped["files"].values())):
            raise ValueError("retained_package_authorization is malformed")
        exact_paths = sorted(set(path(x) for x in scoped["paths"]))
        if set(exact_paths) != set(scoped["files"]):
            raise ValueError("retained authorization paths must equal the exact hash manifest paths")
        custody = scoped.get("custody", scoped["files"])
        if not isinstance(custody, dict) or any(not path(k) or not re.fullmatch(r"[0-9a-f]{64}", str(v)) for k, v in custody.items()) or not set(scoped["files"]) <= set(custody):
            raise ValueError("retained_package_authorization custody manifest is malformed")
        c["retained_package_authorization"] = {"task_id": scoped["task_id"], "paths": exact_paths,
            "workspace": str(pathlib.Path(scoped["workspace"]).resolve()), "common_git_dir": str(pathlib.Path(scoped["common_git_dir"]).resolve()),
            "base": scoped["base"], "source_head": scoped["source_head"], "feature_ref": scoped["feature_ref"],
            "files": dict(sorted(scoped["files"].items()))}
        if "custody" in scoped:
            c["retained_package_authorization"]["custody"] = dict(sorted(custody.items()))
    fixture = raw.get("pinned_red_fixture_authorization")
    if fixture is not None:
        if not isinstance(fixture, dict):
            raise ValueError("pinned_red_fixture_authorization is malformed")
        # The owner manifest is immutable and is copied into the Store config.
        required = {"task_id", "base", "candidate", "expires_at", "fixture_sha256", "fixture_path", "argv", "required_output", "target_path", "module_name", "declaration"}
        if not required <= set(fixture) or not SHA.fullmatch(str(fixture.get("base"))) or not SHA.fullmatch(str(fixture.get("candidate"))):
            raise ValueError("pinned_red_fixture_authorization is incomplete")
        try:
            fixture_expiry = float(fixture["expires_at"])
        except (TypeError, ValueError):
            raise ValueError("pinned_red_fixture_authorization expiry is malformed")
        if not math.isfinite(fixture_expiry) or fixture_expiry <= 0:
            raise ValueError("pinned_red_fixture_authorization expiry must be finite and positive")
        fixture = dict(fixture, expires_at=fixture_expiry)
        if not re.fullmatch(r"[0-9a-f]{64}", str(fixture["fixture_sha256"])):
            raise ValueError("pinned_red_fixture_authorization fixture hash is malformed")
        if not isinstance(fixture["fixture_path"], str) or pathlib.PurePosixPath(fixture["fixture_path"]).is_absolute() or ".." in pathlib.PurePosixPath(fixture["fixture_path"]).parts:
            raise ValueError("pinned_red_fixture_authorization fixture path is unsafe")
        if not isinstance(fixture["argv"], list) or not fixture["argv"] or not all(isinstance(x, str) and x for x in fixture["argv"]):
            raise ValueError("pinned_red_fixture_authorization argv is malformed")
        if not isinstance(fixture["required_output"], list) or not fixture["required_output"]:
            raise ValueError("pinned_red_fixture_authorization assertions are required")
        for key in ("target_path", "module_name", "declaration"):
            if not isinstance(fixture[key], str) or not fixture[key] or ".." in pathlib.PurePosixPath(fixture[key]).parts or pathlib.PurePosixPath(fixture[key]).is_absolute():
                raise ValueError("pinned_red_fixture_authorization injection metadata is unsafe")
        if "fixture_source" in fixture:
            if not isinstance(fixture["fixture_source"], str) or not pathlib.Path(fixture["fixture_source"]).is_absolute() or not re.fullmatch(r"[0-9a-f]{64}", str(fixture.get("fixture_source_sha256", ""))):
                raise ValueError("pinned_red_fixture_authorization fixture source is malformed")
        c["pinned_red_fixture_authorization"] = dict(fixture)
        for field in ("base_production_files", "candidate_production_files", "production_files"):
            manifest = fixture.get(field, {})
            if not isinstance(manifest, dict) or any(not path(k) or not re.fullmatch(r"[0-9a-f]{64}", str(v)) for k, v in manifest.items()):
                raise ValueError("pinned production manifest is malformed")
        c["pinned_red_fixture_authorization"].setdefault("base_production_files", fixture.get("production_files", {}))
        c["pinned_red_fixture_authorization"].setdefault("candidate_production_files", fixture.get("production_files", {}))
        if "red_checks" in fixture:
            c["pinned_red_fixture_authorization"]["red_checks"] = checks(fixture["red_checks"], "pinned red_checks")
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
    if "workspaces" in raw:
        ws = raw["workspaces"]
        if not isinstance(ws, dict):
            raise ValueError("workspaces must be an object")
        root = ws.get("root")
        if not isinstance(root, str) or not root or root.startswith("~") or "\x00" in root:
            raise ValueError("workspaces.root must be an explicit absolute persistent path")
        rootp = pathlib.Path(root)
        if not rootp.is_absolute():
            raise ValueError("workspaces.root must be an explicit absolute persistent path")
        try:
            resolved = rootp.resolve()
        except OSError as exc:
            raise ValueError("workspaces.root cannot be resolved") from exc
        if not rootp.is_dir():
            raise ValueError("workspaces.root must exist on persistent storage")
        # fail closed: never under canonical/product .git or tracked source tree internals
        repo_git = None
        # cannot know repo here; callers must ensure outside .git; reject obvious .git paths
        if any(part == ".git" for part in resolved.parts):
            raise ValueError("workspaces.root must be outside .git")
        allowed = ws.get("allowed_refs")
        if not isinstance(allowed, list) or not allowed:
            raise ValueError("workspaces.allowed_refs must be a nonempty list of existing allowed refs")
        refs = []
        for ref in allowed:
            if not isinstance(ref, str) or not re.fullmatch(r"refs/[A-Za-z0-9][A-Za-z0-9_./-]{0,180}", ref):
                raise ValueError("invalid allowed ref")
            if ".." in ref or "//" in ref or ref.endswith(("/", ".lock", ".")):
                raise ValueError("invalid allowed ref")
            refs.append(ref)
        refs = sorted(set(refs))
        c["workspaces"] = {"root": str(resolved), "allowed_refs": refs}
        author_ref = ws.get("author_ref")
        if author_ref is not None:
            if not isinstance(author_ref, str) or author_ref not in refs:
                raise ValueError("workspaces.author_ref must be an explicitly allowed existing ref")
            c["workspaces"]["author_ref"] = author_ref
        feature_refs = ws.get("feature_refs")
        if feature_refs is not None:
            if not isinstance(feature_refs, list):
                raise ValueError("workspaces.feature_refs must be a list of explicit refs")
            frefs = []
            for ref in feature_refs:
                if not isinstance(ref, str) or ref not in refs:
                    raise ValueError("feature ref must be an explicitly allowed existing ref")
                frefs.append(ref)
            c["workspaces"]["feature_refs"] = list(dict.fromkeys(frefs))
        evidence_root = ws.get("evidence_root")
        if evidence_root is not None:
            if not isinstance(evidence_root, str) or not pathlib.Path(evidence_root).is_absolute():
                raise ValueError("workspaces.evidence_root must be an explicit absolute path")
            c["workspaces"]["evidence_root"] = str(pathlib.Path(evidence_root).resolve())
        build_root = ws.get("build_root")
        if build_root is not None:
            if not isinstance(build_root, str) or not pathlib.Path(build_root).is_absolute():
                raise ValueError("workspaces.build_root must be an explicit absolute path")
            c["workspaces"]["build_root"] = str(pathlib.Path(build_root).resolve())
    return c

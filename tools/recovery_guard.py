#!/usr/bin/env python3
"""Fail-closed guard for OpenCode-RK Recovery Mode.

This protects convergence topology and queue invariants. It is not a product
judge; product correctness is decided by tools/phase1_judge.py on exact main-v2.
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config" / "recovery-mode.json"
FAILURES = ROOT / "recovery" / "failures.json"
GATES = ROOT / "recovery" / "phase1-gates.json"
SNAPSHOT = ROOT / "recovery" / "legacy-snapshot.json"
SHA40 = re.compile(r"^[0-9a-f]{40}$")


def load(path: pathlib.Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        try:
            label = str(path.relative_to(ROOT))
        except ValueError:
            label = str(path)
        raise RuntimeError(
            f"cannot load {label}: {type(exc).__name__}: {exc}"
        ) from exc


def git(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, stderr=subprocess.STDOUT
    ).strip()


def git_lines(*args: str) -> list[str]:
    out = git(*args)
    return out.splitlines() if out else []


def common_git_dir() -> pathlib.Path:
    raw = pathlib.Path(git("rev-parse", "--git-common-dir"))
    if not raw.is_absolute():
        raw = (ROOT / raw).resolve()
    return raw


def paths_overlap(a: str, b: str) -> bool:
    aa = a.rstrip("/")
    bb = b.rstrip("/")
    return aa == bb or aa.startswith(bb + "/") or bb.startswith(aa + "/")


def worktrees() -> dict[str, str | None]:
    result: dict[str, str | None] = {}
    current_path: str | None = None
    for line in git_lines("worktree", "list", "--porcelain"):
        if line.startswith("worktree "):
            current_path = line[len("worktree ") :]
            result[current_path] = None
        elif current_path and line.startswith("branch refs/heads/"):
            result[current_path] = line[len("branch refs/heads/") :]
    return result


def main() -> int:
    errors: list[str] = []
    try:
        cfg = load(CONFIG)
        runtime = common_git_dir() / "recovery-state"
        meta = load(runtime / "meta.json")
        failures = load(runtime / "failures.json")
        gates = load(runtime / "gates.json")
        salvage = load(runtime / "salvage-index.json")
        policy = load(runtime / "policy.json")
        snapshot = load(SNAPSHOT)
    except RuntimeError as exc:
        print(f"RECOVERY GUARD BLOCKED\n- {exc}")
        return 1

    if cfg.get("status") != "ACTIVE":
        errors.append("config/recovery-mode.json is not ACTIVE")
    if cfg.get("branchInventoryIsExecutionQueue") is not False:
        errors.append("branch inventory must not be an execution queue")
    if cfg.get("bulkLegacyMergeAllowed") is not False:
        errors.append("bulk legacy merge must remain disabled")
    if cfg.get("legacyRefsMutable") is not False:
        errors.append("legacy refs must remain immutable")

    revision = meta.get("stateRevision")
    if not isinstance(revision, int) or revision < 1:
        errors.append("runtime meta has invalid stateRevision")
    if failures.get("queueRevision") != revision:
        errors.append(
            f"runtime failures revision {failures.get('queueRevision')!r} != meta {revision!r}; run recovery_state.py recover-state"
        )
    if gates.get("stateRevision") != revision:
        errors.append(
            f"runtime gates revision {gates.get('stateRevision')!r} != meta {revision!r}; run recovery_state.py recover-state"
        )
    if salvage.get("stateRevision") != revision:
        errors.append(
            f"runtime salvage revision {salvage.get('stateRevision')!r} != meta {revision!r}; run recovery_state.py recover-state"
        )

    canonical = str(cfg.get("canonicalBranch", ""))
    bootstrap = str(cfg.get("bootstrapBranch", ""))
    fixed = set(cfg.get("fixedMutableBranches", []))
    execution_lanes = set(cfg.get("executionLanes", []))
    if canonical not in fixed:
        errors.append("canonical branch is missing from fixedMutableBranches")
    if not execution_lanes or not execution_lanes <= fixed:
        errors.append("executionLanes must be a non-empty subset of fixedMutableBranches")

    try:
        current = git("branch", "--show-current")
    except Exception as exc:
        current = ""
        errors.append(f"cannot identify current branch: {type(exc).__name__}")
    allowed_current = set(fixed)
    if cfg.get("bootstrapMutable", False) and bootstrap:
        allowed_current.add(bootstrap)
    if current and current not in allowed_current:
        errors.append(
            f"current branch {current!r} is not mutable in Recovery Mode; do not mutate it"
        )

    snapshot_refs = snapshot.get("refs", {})
    if not isinstance(snapshot_refs, dict) or not snapshot_refs:
        errors.append("legacy snapshot has no refs")
        snapshot_refs = {}

    actual_heads = set(
        git_lines("for-each-ref", "--format=%(refname:short)", "refs/heads/")
    )
    allowed_heads = set(snapshot_refs) | fixed
    if bootstrap:
        allowed_heads.add(bootstrap)
    unexpected_heads = sorted(actual_heads - allowed_heads)
    if unexpected_heads:
        errors.append(
            f"unexpected local branches created during Recovery Mode: {unexpected_heads[:5]}"
        )

    moved: list[str] = []
    missing: list[str] = []
    for ref, expected in snapshot_refs.items():
        if ref in fixed:
            continue
        try:
            actual = git("rev-parse", ref)
        except subprocess.CalledProcessError:
            missing.append(ref)
            continue
        if actual != expected:
            moved.append(f"{ref}: {str(expected)[:12]} -> {actual[:12]}")
    if missing:
        errors.append(f"{len(missing)} frozen legacy refs are missing; first={missing[0]}")
    if moved:
        errors.append(f"{len(moved)} frozen legacy refs moved; first={moved[0]}")

    snapshot_paths = {
        str(w.get("path")) for w in snapshot.get("worktrees", []) if w.get("path")
    }
    approved_map = cfg.get("approvedRecoveryWorktrees", {})
    if not isinstance(approved_map, dict):
        errors.append("approvedRecoveryWorktrees must be an object of branch -> path")
        approved_map = {}
    actual_worktrees = worktrees()
    unexpected_paths = sorted(
        set(actual_worktrees) - snapshot_paths - set(str(p) for p in approved_map.values())
    )
    if unexpected_paths:
        errors.append(
            f"unexpected persistent worktrees created during Recovery Mode: {unexpected_paths[:5]}"
        )
    for branch, path in approved_map.items():
        p = str(path)
        if p not in actual_worktrees:
            errors.append(f"approved recovery worktree is missing: {branch} -> {p}")
        elif actual_worktrees[p] != branch:
            errors.append(
                f"approved recovery worktree {p} has branch {actual_worktrees[p]!r}, expected {branch!r}"
            )

    if current != bootstrap and cfg.get("requireCanonicalRecoveryConfig", True):
        try:
            canonical_blob = git("rev-parse", f"{canonical}:config/recovery-mode.json")
            working_blob = git("hash-object", "config/recovery-mode.json")
            if canonical_blob != working_blob:
                errors.append("canonical main-v2 does not contain the active Recovery Mode config")
        except subprocess.CalledProcessError:
            errors.append("canonical main-v2 does not contain config/recovery-mode.json")

    protected = policy.get("protectedEvaluatorBlobs", {})
    if not isinstance(protected, dict) or not protected:
        errors.append("runtime policy has no protectedEvaluatorBlobs")
    else:
        for rel, expected_blob in protected.items():
            try:
                actual_blob = git("rev-parse", f"{canonical}:{rel}")
            except subprocess.CalledProcessError:
                errors.append(f"protected evaluator path disappeared from {canonical}: {rel}")
                continue
            if actual_blob != expected_blob:
                errors.append(
                    f"protected evaluator drift: {rel} {str(expected_blob)[:12]} -> {actual_blob[:12]}"
                )

    hook = common_git_dir() / "hooks" / "reference-transaction"
    expected_hook = policy.get("referenceHookSha256")
    if cfg.get("activationPolicy", {}).get("installReferenceTransactionHook", False):
        if not hook.is_file() or not os.access(hook, os.X_OK):
            errors.append("Recovery Mode reference-transaction hook is missing or not executable")
        elif expected_hook:
            actual_hook = hashlib.sha256(hook.read_bytes()).hexdigest()
            if actual_hook != expected_hook:
                errors.append("Recovery Mode reference-transaction hook hash changed")

    unresolved_states = {
        "UNPROVEN",
        "OPEN",
        "CLAIMED",
        "CANDIDATE",
        "PREVERIFIED",
        "INTEGRATION_READY",
        "INTEGRATING",
        "INTEGRATED",
    }
    allowed_states = unresolved_states | {"REJECTED", "SUPERSEDED", "ACCEPTED"}
    queue = failures.get("failures", [])
    ids = [str(item.get("id", "")) for item in queue]
    if len(ids) != len(set(ids)):
        errors.append("recovery/failures.json contains duplicate failure IDs")
    bad_states = sorted(
        {str(item.get("status")) for item in queue if item.get("status") not in allowed_states}
    )
    if bad_states:
        errors.append(f"recovery/failures.json contains invalid states: {bad_states}")

    active_states = {
        "CLAIMED",
        "CANDIDATE",
        "PREVERIFIED",
        "INTEGRATION_READY",
        "INTEGRATING",
    }
    active = [item for item in queue if item.get("status") in active_states]
    grants: list[tuple[str, str]] = []
    protected_paths = cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", [])
    for item in active:
        fid = str(item.get("id", ""))
        lane = item.get("lane")
        base_sha = item.get("baseSha")
        paths = item.get("pathGrant")
        if lane not in execution_lanes:
            errors.append(f"{fid} active claim has invalid lane {lane!r}")
        if not isinstance(base_sha, str) or not SHA40.fullmatch(base_sha):
            errors.append(f"{fid} active claim is missing a 40-hex baseSha")
        if not isinstance(paths, list) or not paths:
            errors.append(f"{fid} active claim has no explicit pathGrant")
            continue
        intents = cfg.get("lanePathIntent", {}).get(lane, []) if lane else []
        for path in paths:
            p = str(path)
            if not any(
                p == str(prefix).rstrip("/") or p.startswith(str(prefix).rstrip("/") + "/")
                for prefix in intents
            ):
                errors.append(f"{fid} path grant {p!r} is outside lane intent for {lane!r}")
            if any(paths_overlap(p, str(rel)) for rel in protected_paths):
                errors.append(f"{fid} path grant {p!r} overlaps protected evaluator state")
            for other_path, other_id in grants:
                if other_id != fid and paths_overlap(p, other_path):
                    errors.append(
                        f"path grant collision: {p} ({fid}) overlaps {other_path} ({other_id})"
                    )
            grants.append((p, fid))
        candidate = item.get("candidateSha")
        if candidate and isinstance(base_sha, str) and SHA40.fullmatch(base_sha):
            try:
                changed = git_lines("diff", "--name-only", f"{base_sha}..{candidate}")
                outside = [
                    path for path in changed
                    if not any(
                        path == str(grant).rstrip("/")
                        or path.startswith(str(grant).rstrip("/") + "/")
                        for grant in paths
                    )
                ]
                if outside:
                    errors.append(f"{fid} candidate diff escapes path grant: {outside[:8]}")
                evaluator_changes = [
                    path for path in changed
                    if any(paths_overlap(path, str(rel)) for rel in protected_paths)
                ]
                if evaluator_changes:
                    errors.append(
                        f"{fid} candidate modifies protected evaluator paths: {evaluator_changes[:8]}"
                    )
            except subprocess.CalledProcessError:
                errors.append(f"cannot inspect candidate diff for {fid}")

    salvage_queries = salvage.get("queries", {})
    terminal_dispositions = set(
        cfg.get("salvagePolicy", {}).get("terminalDispositions", [])
    )
    max_salvage = int(
        cfg.get("salvagePolicy", {}).get("maxMaterializedCandidatesPerFailure", 8)
    )
    queue_ids = set(ids)
    for fid, query in salvage_queries.items():
        if fid not in queue_ids:
            errors.append(f"salvage query references unknown failure {fid!r}")
            continue
        if not isinstance(query, dict):
            errors.append(f"salvage query {fid!r} is not an object")
            continue
        if query.get("refsScanned") != len(snapshot_refs):
            errors.append(
                f"salvage query {fid} did not scan full frozen snapshot: {query.get('refsScanned')} != {len(snapshot_refs)}"
            )
        candidates = query.get("candidates", [])
        if not isinstance(candidates, list) or len(candidates) > max_salvage:
            errors.append(f"salvage query {fid} exceeds bounded candidate limit {max_salvage}")
            continue
        unresolved_salvage = 0
        for candidate in candidates:
            ref = candidate.get("sourceRef")
            sha = candidate.get("sourceSha")
            aliases = candidate.get("aliases", [])
            if not isinstance(aliases, list) or ref not in aliases:
                errors.append(f"salvage candidate {fid}/{ref} has invalid aliases")
            if snapshot_refs.get(ref) != sha:
                errors.append(f"salvage candidate {fid}/{ref} does not match frozen ref/SHA")
            disposition = candidate.get("disposition")
            if disposition is None:
                unresolved_salvage += 1
            elif disposition not in terminal_dispositions:
                errors.append(
                    f"salvage candidate {fid}/{ref} has non-terminal/invalid disposition {disposition!r}"
                )
        row = next((item for item in queue if item.get("id") == fid), None)
        if unresolved_salvage and row and row.get("status") in {"INTEGRATION_READY", "INTEGRATING"}:
            errors.append(
                f"{fid} entered integration with {unresolved_salvage} unresolved salvage candidates"
            )

    integrating = [item for item in queue if item.get("status") == "INTEGRATING"]
    integration_lock = common_git_dir() / "recovery-state" / "integration.lock"
    if len(integrating) > 1:
        errors.append(f"more than one failure is INTEGRATING: {[x.get('id') for x in integrating]}")
    if integrating:
        if not integration_lock.is_file():
            errors.append("failure is INTEGRATING but runtime integration.lock is missing")
        else:
            try:
                lock_data = load(integration_lock)
                if lock_data.get("failure") != integrating[0].get("id"):
                    errors.append(
                        f"integration.lock belongs to {lock_data.get('failure')!r}, not {integrating[0].get('id')!r}"
                    )
            except RuntimeError as exc:
                errors.append(str(exc))
    elif integration_lock.exists():
        errors.append("stale integration.lock exists with no INTEGRATING failure; reopen/repair runtime state")

    high_water = int(cfg.get("maxVerifiedUnintegratedCandidates", 4))
    verified = [
        item
        for item in queue
        if item.get("status") in {"PREVERIFIED", "INTEGRATION_READY", "INTEGRATING"}
    ]
    if len(verified) > high_water:
        errors.append(
            f"verified candidate high-water exceeded: {len(verified)} > {high_water}"
        )

    max_ahead = int(cfg.get("lanePolicy", {}).get("maxAheadCommits", 6))
    for lane in sorted(execution_lanes):
        try:
            _left, right = git(
                "rev-list", "--left-right", "--count", f"{canonical}...{lane}"
            ).split()
            ahead = int(right)
            if ahead > max_ahead:
                errors.append(
                    f"{lane} is {ahead} commits ahead of {canonical}; max is {max_ahead}"
                )
        except Exception:
            errors.append(f"cannot measure recovery lane divergence for {lane}")

    current_main = git("rev-parse", canonical)
    accepted_gates = []
    for gate in gates.get("gates", []):
        if gate.get("status") == "ACCEPTED":
            if gate.get("acceptedSha") != current_main:
                errors.append(
                    f"{gate.get('id')} ACCEPTED SHA {gate.get('acceptedSha')!r} is stale; "
                    f"current {canonical} is {current_main}"
                )
            else:
                accepted_gates.append(gate)
    for item in queue:
        if item.get("status") == "ACCEPTED":
            accepted_sha = item.get("acceptedSha")
            if accepted_sha != current_main:
                errors.append(
                    f"{item.get('id')} ACCEPTED SHA {accepted_sha!r} is stale; current {canonical} is {current_main}"
                )

    unresolved = [item for item in queue if item.get("status") in unresolved_states]
    total_gates = len(gates.get("gates", []))

    if errors:
        print("RECOVERY GUARD BLOCKED")
        for error in errors:
            print(f"- {error}")
        print(f"total={len(errors)}")
        return 1

    print("RECOVERY GUARD GREEN")
    print(f"branch={current or '<detached>'}")
    print(f"canonical_sha={current_main}")
    print(f"frozen_legacy_refs={len(snapshot_refs)}")
    print(f"phase1_gates_accepted={len(accepted_gates)}/{total_gates}")
    print(f"unresolved_failures={len(unresolved)}")
    if unresolved:
        ordered = sorted(
            unresolved,
            key=lambda item: (
                str(item.get("priority", "P9")),
                str(item.get("id", "")),
            ),
        )
        nxt = ordered[0]
        print(
            f"next_failure={nxt.get('id')} [{nxt.get('priority')}] {nxt.get('title')}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

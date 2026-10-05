#!/usr/bin/env python3
"""Fail-closed guard for OpenCode-RK Recovery Mode.

This guard protects the bounded-mutable-state model. It does not prove product
correctness; it prevents the controller from silently falling back to the old
branch/worktree explosion model.
"""

from __future__ import annotations

import json
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config" / "recovery-mode.json"
FAILURES = ROOT / "recovery" / "failures.json"
GATES = ROOT / "recovery" / "phase1-gates.json"
SNAPSHOT = ROOT / "recovery" / "legacy-snapshot.json"


def load(path: pathlib.Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise RuntimeError(
            f"cannot load {path.relative_to(ROOT)}: {type(exc).__name__}: {exc}"
        ) from exc


def git(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, stderr=subprocess.STDOUT
    ).strip()


def main() -> int:
    errors: list[str] = []

    try:
        cfg = load(CONFIG)
        failures = load(FAILURES)
        gates = load(GATES)
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
        errors.append("legacy refs must remain immutable during Recovery Mode")

    fixed = set(cfg.get("fixedMutableBranches", []))
    canonical = str(cfg.get("canonicalBranch", ""))
    bootstrap = str(cfg.get("bootstrapBranch", ""))
    if canonical not in fixed:
        errors.append("canonical branch is missing from fixedMutableBranches")
    if bootstrap and bootstrap not in fixed:
        errors.append("bootstrap branch is missing from fixedMutableBranches")

    try:
        current = git("branch", "--show-current")
    except Exception as exc:
        errors.append(f"cannot identify current branch: {type(exc).__name__}")
        current = ""
    if current and current not in fixed:
        errors.append(
            f"current branch {current!r} is not a fixed Recovery Mode branch; "
            "do not mutate it"
        )

    snapshot_refs = snapshot.get("refs", {})
    if not isinstance(snapshot_refs, dict) or not snapshot_refs:
        errors.append("legacy snapshot has no refs")
    else:
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
                moved.append(f"{ref}: {expected[:12]} -> {actual[:12]}")
        if missing:
            errors.append(
                f"{len(missing)} frozen legacy refs are missing; first={missing[0]}"
            )
        if moved:
            errors.append(
                f"{len(moved)} frozen legacy refs moved; first={moved[0]}"
            )

    unresolved_states = {
        "UNPROVEN",
        "OPEN",
        "CLAIMED",
        "CANDIDATE",
        "PREVERIFIED",
        "INTEGRATION_READY",
        "INTEGRATING",
    }
    queue = failures.get("failures", [])
    unresolved = [f for f in queue if f.get("status") in unresolved_states]
    accepted_gates = [g for g in gates.get("gates", []) if g.get("status") == "ACCEPTED"]
    total_gates = len(gates.get("gates", []))

    if errors:
        print("RECOVERY GUARD BLOCKED")
        for error in errors:
            print(f"- {error}")
        print(f"total={len(errors)}")
        return 1

    print("RECOVERY GUARD GREEN")
    print(f"branch={current or '<detached>'}")
    print(f"frozen_legacy_refs={len(snapshot_refs)}")
    print(f"phase1_gates_accepted={len(accepted_gates)}/{total_gates}")
    print(f"unresolved_failures={len(unresolved)}")
    if unresolved:
        ordered = sorted(
            unresolved,
            key=lambda x: (str(x.get("priority", "P9")), str(x.get("id", ""))),
        )
        nxt = ordered[0]
        print(
            "next_failure="
            f"{nxt.get('id')} [{nxt.get('priority')}] {nxt.get('title')}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

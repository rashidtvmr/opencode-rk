#!/usr/bin/env python3
"""Atomic runtime state manager for OpenCode-RK Recovery Mode.

Mutable orchestration state lives in Git's shared common directory, never in
tracked files. This prevents queue/gate bookkeeping commits from invalidating
the exact source SHA they claim to have accepted.
"""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import fcntl
import json
import os
import pathlib
import re
import subprocess
import sys
from contextlib import contextmanager

ROOT = pathlib.Path(__file__).resolve().parents[1]
CONFIG_PATH = ROOT / "config" / "recovery-mode.json"
SHA40 = re.compile(r"^[0-9a-f]{40}$")
ACTIVE_STATES = {"CLAIMED", "CANDIDATE", "PREVERIFIED", "INTEGRATION_READY", "INTEGRATING"}


def load_json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def atomic_write(path: pathlib.Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    tmp.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def git(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, stderr=subprocess.STDOUT
    ).strip()


def common_git_dir() -> pathlib.Path:
    raw = pathlib.Path(git("rev-parse", "--git-common-dir"))
    if not raw.is_absolute():
        raw = (ROOT / raw).resolve()
    return raw


def runtime_dir(cfg: dict) -> pathlib.Path:
    return common_git_dir() / "recovery-state"


def state_paths(cfg: dict) -> dict[str, pathlib.Path]:
    base = runtime_dir(cfg)
    return {
        "meta": base / "meta.json",
        "failures": base / "failures.json",
        "gates": base / "gates.json",
        "salvage": base / "salvage-index.json",
        "lock": base / "state.lock",
        "integration_lock": base / "integration.lock",
        "evidence": common_git_dir() / "recovery-evidence",
    }


@contextmanager
def locked(path: pathlib.Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def main_sha(cfg: dict) -> str:
    return git("rev-parse", str(cfg["canonicalBranch"]))


def paths_overlap(a: str, b: str) -> bool:
    aa = a.rstrip("/")
    bb = b.rstrip("/")
    return aa == bb or aa.startswith(bb + "/") or bb.startswith(aa + "/")


def bump(meta: dict) -> None:
    meta["stateRevision"] = int(meta.get("stateRevision", 0)) + 1
    meta["updatedAt"] = now()


def init_state(cfg: dict, force: bool = False) -> None:
    paths = state_paths(cfg)
    paths["meta"].parent.mkdir(parents=True, exist_ok=True)
    with locked(paths["lock"]):
        if paths["meta"].exists() and not force:
            print("RECOVERY STATE ALREADY INITIALIZED")
            return
        seed_failures = load_json(ROOT / "recovery" / "failures.json")
        gate_defs = load_json(ROOT / "recovery" / "phase1-gates.json")
        salvage_seed = load_json(ROOT / "recovery" / "salvage-index.json")
        sha = main_sha(cfg)
        failures = copy.deepcopy(seed_failures)
        failures["runtime"] = True
        failures["queueRevision"] = 1
        failures["canonicalShaAtInit"] = sha
        gates = copy.deepcopy(gate_defs)
        gates["runtime"] = True
        gates["stateRevision"] = 1
        for gate in gates.get("gates", []):
            gate["status"] = "UNPROVEN"
            gate["acceptedSha"] = None
            gate["evidence"] = []
        salvage = copy.deepcopy(salvage_seed)
        salvage["runtime"] = True
        salvage["stateRevision"] = 1
        meta = {
            "schemaVersion": 1,
            "initializedAt": now(),
            "updatedAt": now(),
            "stateRevision": 1,
            "canonicalBranch": cfg["canonicalBranch"],
            "canonicalShaAtInit": sha,
            "note": "Operational state is intentionally outside tracked Git history.",
        }
        atomic_write(paths["failures"], failures)
        atomic_write(paths["gates"], gates)
        atomic_write(paths["salvage"], salvage)
        atomic_write(paths["meta"], meta)
    print(f"RECOVERY STATE INITIALIZED {paths['meta'].parent}")


def require_state(cfg: dict) -> tuple[dict, dict, dict, dict, dict[str, pathlib.Path]]:
    paths = state_paths(cfg)
    for key in ("meta", "failures", "gates", "salvage"):
        if not paths[key].is_file():
            raise RuntimeError("runtime state missing; run tools/recovery_state.py init")
    return (
        load_json(paths["meta"]),
        load_json(paths["failures"]),
        load_json(paths["gates"]),
        load_json(paths["salvage"]),
        paths,
    )


def active_failures(failures: dict) -> list[dict]:
    return [row for row in failures.get("failures", []) if row.get("status") in ACTIVE_STATES]


def failure_by_id(failures: dict, fid: str) -> dict:
    rows = [row for row in failures.get("failures", []) if row.get("id") == fid]
    if len(rows) != 1:
        raise RuntimeError(f"failure id {fid!r} not found exactly once")
    return rows[0]


def save_all(meta: dict, failures: dict, gates: dict, salvage: dict, paths: dict[str, pathlib.Path]) -> None:
    bump(meta)
    failures["queueRevision"] = meta["stateRevision"]
    gates["stateRevision"] = meta["stateRevision"]
    salvage["stateRevision"] = meta["stateRevision"]
    atomic_write(paths["failures"], failures)
    atomic_write(paths["gates"], gates)
    atomic_write(paths["salvage"], salvage)
    atomic_write(paths["meta"], meta)


def cmd_status(cfg: dict) -> int:
    meta, failures, gates, _, paths = require_state(cfg)
    main = main_sha(cfg)
    unresolved = [
        row
        for row in failures.get("failures", [])
        if row.get("status") in {"UNPROVEN", "OPEN", "INTEGRATED", *ACTIVE_STATES}
    ]
    print(f"runtime_dir={paths['meta'].parent}")
    print(f"state_revision={meta.get('stateRevision')}")
    print(f"canonical_sha={main}")
    print(f"unresolved={len(unresolved)}")
    for row in sorted(unresolved, key=lambda x: (x.get("priority", "P9"), x.get("id", "")))[:20]:
        print(
            f"{row.get('priority','P9')} {row.get('id')} {row.get('status')} "
            f"lane={row.get('lane','-')} {row.get('title','')}"
        )
    accepted = [g for g in gates.get("gates", []) if g.get("status") == "ACCEPTED" and g.get("acceptedSha") == main]
    print(f"accepted_gates={len(accepted)}/{len(gates.get('gates', []))}")
    return 0


def cmd_claim(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"OPEN", "UNPROVEN"}:
            raise RuntimeError(f"{args.id} is {row.get('status')}, not OPEN/UNPROVEN")
        lanes = set(cfg.get("executionLanes", []))
        if args.lane not in lanes:
            raise RuntimeError(f"lane {args.lane!r} is not an execution lane")
        canonical = main_sha(cfg)
        lane_sha = git("rev-parse", args.lane)
        if lane_sha != canonical:
            raise RuntimeError(
                f"lane {args.lane} is stale: {lane_sha} != current {cfg['canonicalBranch']} {canonical}"
            )
        for other in active_failures(failures):
            if other.get("lane") == args.lane:
                raise RuntimeError(f"lane {args.lane} already owns {other.get('id')}")
            for wanted in args.path:
                for held in other.get("pathGrant", []):
                    if paths_overlap(wanted, held):
                        raise RuntimeError(
                            f"path grant {wanted!r} overlaps {held!r} owned by {other.get('id')}"
                        )
        row.update(
            {
                "status": "CLAIMED",
                "owner": args.owner,
                "lane": args.lane,
                "baseSha": canonical,
                "pathGrant": sorted(set(args.path)),
                "claimedAt": now(),
                "reviews": [],
            }
        )
        save_all(meta, failures, gates, salvage, paths)
    print(f"CLAIMED {args.id} lane={args.lane} base={canonical}")
    return 0


def cmd_candidate(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") != "CLAIMED":
            raise RuntimeError(f"{args.id} must be CLAIMED before CANDIDATE")
        if not SHA40.fullmatch(args.sha):
            raise RuntimeError("candidate SHA must be 40 lowercase hex")
        git("merge-base", "--is-ancestor", row["baseSha"], args.sha)
        lane_head = git("rev-parse", row["lane"])
        if lane_head != args.sha:
            raise RuntimeError(
                f"candidate SHA {args.sha} is not current lane head {row['lane']} {lane_head}"
            )
        row["status"] = "CANDIDATE"
        row["candidateSha"] = args.sha
        row["candidateAt"] = now()
        row["reviews"] = []
        save_all(meta, failures, gates, salvage, paths)
    print(f"CANDIDATE {args.id} {args.sha}")
    return 0


def cmd_review(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"CANDIDATE", "PREVERIFIED"}:
            raise RuntimeError(f"{args.id} has no reviewable candidate")
        if args.reviewer == row.get("owner"):
            raise RuntimeError("candidate author may not review own candidate")
        review = {
            "reviewer": args.reviewer,
            "verdict": args.verdict,
            "role": args.role,
            "evidence": args.evidence,
            "at": now(),
            "candidateSha": row.get("candidateSha"),
        }
        reviews = [r for r in row.get("reviews", []) if r.get("reviewer") != args.reviewer]
        reviews.append(review)
        row["reviews"] = reviews
        if args.verdict != "APPROVE":
            row["status"] = "CANDIDATE"
        save_all(meta, failures, gates, salvage, paths)
    print(f"REVIEW {args.id} {args.reviewer} {args.verdict}")
    return 0


def required_approvals(cfg: dict, row: dict) -> int:
    policy = cfg.get("evaluationPolicy", {}).get("requiredApprovalsByPriority", {})
    return int(policy.get(row.get("priority", "P3"), 1))


def cmd_preverify(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") != "CANDIDATE":
            raise RuntimeError(f"{args.id} must be CANDIDATE")
        candidate = row.get("candidateSha")
        approvals = {
            r.get("reviewer")
            for r in row.get("reviews", [])
            if r.get("verdict") == "APPROVE" and r.get("candidateSha") == candidate
        }
        need = required_approvals(cfg, row)
        if len(approvals) < need:
            raise RuntimeError(f"{args.id} needs {need} independent approvals; has {len(approvals)}")
        security_change = any(
            str(p).startswith("crates/security") for p in row.get("pathGrant", [])
        )
        if security_change:
            security_ok = any(
                r.get("verdict") == "APPROVE"
                and r.get("role") == "security"
                and r.get("candidateSha") == candidate
                for r in row.get("reviews", [])
            )
            if not security_ok:
                raise RuntimeError("security path change requires an approving security evaluator")
        row["status"] = "PREVERIFIED"
        row["preverifiedAt"] = now()
        row["preverifyEvidence"] = args.evidence
        save_all(meta, failures, gates, salvage, paths)
    print(f"PREVERIFIED {args.id}")
    return 0


def cmd_ready(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") != "PREVERIFIED":
            raise RuntimeError(f"{args.id} must be PREVERIFIED")
        canonical = main_sha(cfg)
        if row.get("baseSha") != canonical:
            raise RuntimeError(
                f"{args.id} candidate is stale: base {row.get('baseSha')} != current main-v2 {canonical}; reopen, fast-forward lane, and reapply"
            )
        high_water = int(cfg.get("maxVerifiedUnintegratedCandidates", 4))
        waiting = [
            x
            for x in failures.get("failures", [])
            if x.get("status") in {"INTEGRATION_READY", "INTEGRATING"}
        ]
        if len(waiting) >= high_water:
            raise RuntimeError("candidate high-water reached")
        row["status"] = "INTEGRATION_READY"
        row["integrationReadyAt"] = now()
        save_all(meta, failures, gates, salvage, paths)
    print(f"INTEGRATION_READY {args.id}")
    return 0


def cmd_reopen(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        previous = row.get("status")
        if previous == "INTEGRATING" and paths["integration_lock"].is_file():
            lock_data = load_json(paths["integration_lock"])
            if lock_data.get("failure") == args.id:
                paths["integration_lock"].unlink()
        for key in (
            "owner",
            "lane",
            "baseSha",
            "pathGrant",
            "candidateSha",
            "claimedAt",
            "candidateAt",
            "reviews",
            "preverifiedAt",
            "preverifyEvidence",
            "integrationReadyAt",
        ):
            row.pop(key, None)
        row["status"] = "OPEN"
        row["lastReopenedAt"] = now()
        row["lastReopenReason"] = args.reason
        row["lastState"] = previous
        save_all(meta, failures, gates, salvage, paths)
    print(f"REOPENED {args.id} from={previous}")
    return 0


def cmd_accept(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"INTEGRATED", "OPEN", "UNPROVEN"}:
            raise RuntimeError(f"{args.id} must be INTEGRATED, or an OPEN/UNPROVEN probe failure")
        canonical = main_sha(cfg)
        if args.sha != canonical:
            raise RuntimeError(f"accept SHA {args.sha} != current main-v2 {canonical}")
        evidence = pathlib.Path(args.evidence)
        if not evidence.is_absolute():
            raise RuntimeError("accept evidence path must be absolute")
        allowed = paths["evidence"].resolve()
        resolved = evidence.resolve()
        if allowed not in resolved.parents:
            raise RuntimeError(f"accept evidence must live under {allowed}")
        if not resolved.is_file():
            raise RuntimeError("accept evidence file does not exist")
        receipt = load_json(resolved)
        if receipt.get("sourceSha") != canonical or receipt.get("artifactMode") is not True:
            raise RuntimeError("accept evidence is not installed-artifact evidence for current main-v2")
        gate_id = row.get("gate")
        gate_result = receipt.get("gateResults", {}).get(gate_id, {})
        if gate_result.get("status") != "PASS":
            raise RuntimeError(f"accept evidence does not PASS failure gate {gate_id}")
        candidate = row.get("candidateSha")
        if row.get("status") == "INTEGRATED" and candidate:
            git("merge-base", "--is-ancestor", candidate, canonical)
        row["status"] = "ACCEPTED"
        row["acceptedSha"] = canonical
        row["acceptedAt"] = now()
        row["acceptEvidence"] = str(resolved)
        save_all(meta, failures, gates, salvage, paths)
        if paths["integration_lock"].is_file():
            lock_data = load_json(paths["integration_lock"])
            if lock_data.get("failure") == args.id:
                paths["integration_lock"].unlink()
    print(f"ACCEPTED {args.id} sha={canonical}")
    return 0


def cmd_complete_integration(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") != "INTEGRATING":
            raise RuntimeError(f"{args.id} must be INTEGRATING")
        canonical = main_sha(cfg)
        if args.sha != canonical:
            raise RuntimeError(f"integrated SHA {args.sha} != current main-v2 {canonical}")
        candidate = row.get("candidateSha")
        if not candidate:
            raise RuntimeError("integrating failure has no candidateSha")
        git("merge-base", "--is-ancestor", candidate, canonical)
        if not paths["integration_lock"].is_file():
            raise RuntimeError("integration lock is missing")
        lock_data = load_json(paths["integration_lock"])
        if lock_data.get("failure") != args.id or lock_data.get("owner") != row.get("integrationOwner"):
            raise RuntimeError("integration lock does not match runtime failure/owner")
        row["status"] = "INTEGRATED"
        row["integratedSha"] = canonical
        row["integratedAt"] = now()
        save_all(meta, failures, gates, salvage, paths)
        paths["integration_lock"].unlink()
    print(f"INTEGRATED {args.id} sha={canonical}")
    return 0


def cmd_sync_gates(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    evidence = pathlib.Path(args.evidence).resolve()
    allowed = paths["evidence"].resolve()
    if allowed not in evidence.parents or not evidence.is_file():
        raise RuntimeError(f"judge evidence must be an existing file under {allowed}")
    result = load_json(evidence)
    canonical = main_sha(cfg)
    if result.get("sourceSha") != canonical:
        raise RuntimeError(
            f"judge source {result.get('sourceSha')!r} != current main-v2 {canonical}"
        )
    if result.get("artifactMode") is not True or result.get("acceptanceEligible") is not True:
        raise RuntimeError("only an acceptance-eligible installed-artifact judge may sync gates")
    gate_results = result.get("gateResults", {})
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        if main_sha(cfg) != canonical:
            raise RuntimeError("main-v2 moved while syncing judge results; rerun judge")
        staged = copy.deepcopy(gates)
        for gate in staged.get("gates", []):
            verdict = gate_results.get(gate.get("id"), {})
            if verdict.get("status") == "PASS":
                gate["status"] = "ACCEPTED"
                gate["acceptedSha"] = canonical
                gate["acceptedAt"] = now()
                gate["evidence"] = [str(evidence)]
            else:
                gate["status"] = "UNPROVEN"
                gate["acceptedSha"] = None
                gate["evidence"] = [str(evidence)]
        if not all(g.get("status") == "ACCEPTED" for g in staged.get("gates", [])):
            raise RuntimeError("judge evidence did not PASS every Phase-1 gate")
        gates = staged
        save_all(meta, failures, gates, salvage, paths)
    print(f"PHASE1 GATES ACCEPTED sha={canonical}")
    return 0


def cmd_begin_integration(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    lock = paths["integration_lock"]
    lock.parent.mkdir(parents=True, exist_ok=True)
    try:
        fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError:
        raise RuntimeError(f"integration lock exists: {lock}")
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump({"owner": args.owner, "failure": args.id, "acquiredAt": now()}, handle)
            handle.write("\n")
        with locked(paths["lock"]):
            meta, failures, gates, salvage, paths = require_state(cfg)
            row = failure_by_id(failures, args.id)
            if row.get("status") != "INTEGRATION_READY":
                raise RuntimeError(f"{args.id} must be INTEGRATION_READY")
            canonical = main_sha(cfg)
            if row.get("baseSha") != canonical:
                raise RuntimeError(
                    f"{args.id} became stale before integration: base {row.get('baseSha')} != {canonical}"
                )
            if git("rev-parse", row["lane"]) != row.get("candidateSha"):
                raise RuntimeError("lane head changed after candidate review; reopen the failure")
            row["status"] = "INTEGRATING"
            row["integrationOwner"] = args.owner
            row["integrationStartedAt"] = now()
            save_all(meta, failures, gates, salvage, paths)
    except Exception:
        lock.unlink(missing_ok=True)
        raise
    print(f"INTEGRATING {args.id} owner={args.owner}")
    return 0


def cmd_end_integration(cfg: dict, args: argparse.Namespace) -> int:
    paths = state_paths(cfg)
    lock = paths["integration_lock"]
    if not lock.is_file():
        raise RuntimeError("integration lock does not exist")
    data = load_json(lock)
    if data.get("owner") != args.owner:
        raise RuntimeError(f"integration lock owner is {data.get('owner')!r}, not {args.owner!r}")
    lock.unlink()
    print("INTEGRATION LOCK RELEASED")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("init").add_argument("--force", action="store_true")
    sub.add_parser("status")

    p = sub.add_parser("claim")
    p.add_argument("--id", required=True)
    p.add_argument("--lane", required=True)
    p.add_argument("--owner", required=True)
    p.add_argument("--path", action="append", required=True)

    p = sub.add_parser("candidate")
    p.add_argument("--id", required=True)
    p.add_argument("--sha", required=True)

    p = sub.add_parser("review")
    p.add_argument("--id", required=True)
    p.add_argument("--reviewer", required=True)
    p.add_argument("--role", default="general", choices=["general", "security", "test", "upstream"])
    p.add_argument("--verdict", required=True, choices=["APPROVE", "REJECT", "NEEDS_FIX"])
    p.add_argument("--evidence", required=True)

    p = sub.add_parser("preverify")
    p.add_argument("--id", required=True)
    p.add_argument("--evidence", required=True)

    p = sub.add_parser("ready")
    p.add_argument("--id", required=True)

    p = sub.add_parser("reopen")
    p.add_argument("--id", required=True)
    p.add_argument("--reason", required=True)

    p = sub.add_parser("begin-integration")
    p.add_argument("--id", required=True)
    p.add_argument("--owner", required=True)

    p = sub.add_parser("end-integration")
    p.add_argument("--owner", required=True)

    p = sub.add_parser("complete-integration")
    p.add_argument("--id", required=True)
    p.add_argument("--sha", required=True)

    p = sub.add_parser("accept")
    p.add_argument("--id", required=True)
    p.add_argument("--sha", required=True)
    p.add_argument("--evidence", required=True)

    p = sub.add_parser("sync-gates")
    p.add_argument("--evidence", required=True)
    return parser


def main() -> int:
    cfg = load_json(CONFIG_PATH)
    args = build_parser().parse_args()
    try:
        if args.cmd == "init":
            init_state(cfg, args.force)
            return 0
        if args.cmd == "status":
            return cmd_status(cfg)
        if args.cmd == "claim":
            return cmd_claim(cfg, args)
        if args.cmd == "candidate":
            return cmd_candidate(cfg, args)
        if args.cmd == "review":
            return cmd_review(cfg, args)
        if args.cmd == "preverify":
            return cmd_preverify(cfg, args)
        if args.cmd == "ready":
            return cmd_ready(cfg, args)
        if args.cmd == "reopen":
            return cmd_reopen(cfg, args)
        if args.cmd == "begin-integration":
            return cmd_begin_integration(cfg, args)
        if args.cmd == "end-integration":
            return cmd_end_integration(cfg, args)
        if args.cmd == "complete-integration":
            return cmd_complete_integration(cfg, args)
        if args.cmd == "accept":
            return cmd_accept(cfg, args)
        if args.cmd == "sync-gates":
            return cmd_sync_gates(cfg, args)
    except (RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"RECOVERY STATE BLOCKED: {exc}", file=sys.stderr)
        return 1
    raise AssertionError(args.cmd)


if __name__ == "__main__":
    raise SystemExit(main())

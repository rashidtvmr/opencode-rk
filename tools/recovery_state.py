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
import hashlib
import json
import os
import pathlib
import re
import secrets
import shlex
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
        "policy": base / "policy.json",
        "ref_policy": base / "ref-policy.json",
        "canonical_token": base / "canonical-write.token",
        "backup": base / "last-good.json",
        "lock": base / "state.lock",
        "integration_lock": base / "integration.lock",
        "heavy_lock": base / "heavy-validation.lock",
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


def path_within(path: str, prefix: str) -> bool:
    value = path.rstrip("/")
    root = prefix.rstrip("/")
    return value == root or value.startswith(root + "/")


def normalize_repo_path(raw: str) -> str:
    value = raw.replace("\\", "/").strip().rstrip("/")
    pure = pathlib.PurePosixPath(value)
    if (
        not value
        or pure.is_absolute()
        or ".." in pure.parts
        or value == ".git"
        or value.startswith(".git/")
    ):
        raise RuntimeError(f"unsafe repository path grant: {raw!r}")
    return pure.as_posix()


def changed_paths(base: str, tip: str) -> list[str]:
    out = git("diff", "--name-only", f"{base}..{tip}")
    return [line for line in out.splitlines() if line]


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def bump(meta: dict) -> None:
    meta["stateRevision"] = int(meta.get("stateRevision", 0)) + 1
    meta["updatedAt"] = now()


def install_reference_hook(cfg: dict, paths: dict[str, pathlib.Path]) -> str:
    canonical = str(cfg["canonicalBranch"])
    canonical_root = pathlib.Path(cfg["approvedRecoveryWorktrees"][canonical]).resolve()
    hook_driver = canonical_root / "tools" / "recovery_ref_hook.py"
    if not hook_driver.is_file():
        raise RuntimeError(f"reference hook driver missing: {hook_driver}")
    hook = common_git_dir() / "hooks" / "reference-transaction"
    hook.parent.mkdir(parents=True, exist_ok=True)
    wrapper = (
        "#!/bin/sh\n"
        f"exec /usr/bin/env python3 {shlex.quote(str(hook_driver))} \"$@\"\n"
    )
    if hook.exists():
        existing = hook.read_text(encoding="utf-8", errors="replace")
        if "recovery_ref_hook.py" not in existing:
            raise RuntimeError(
                f"existing reference-transaction hook is not Recovery Mode owned: {hook}"
            )
    hook.write_text(wrapper, encoding="utf-8")
    hook.chmod(0o755)
    return sha256_bytes(wrapper.encode())


def build_runtime_policy(cfg: dict, sha: str, paths: dict[str, pathlib.Path]) -> dict:
    snapshot = load_json(ROOT / cfg["legacySnapshot"])
    frozen = snapshot.get("refs", {})
    if not isinstance(frozen, dict) or not frozen:
        raise RuntimeError("legacy snapshot has no frozen refs")
    mutable = list(cfg.get("fixedMutableBranches", []))
    canonical_token = secrets.token_hex(32)
    paths["canonical_token"].write_text(canonical_token + "\n", encoding="utf-8")
    paths["canonical_token"].chmod(0o600)
    ref_policy = {
        "schemaVersion": 1,
        "activationSha": sha,
        "mutableHeads": mutable,
        "canonicalHead": cfg["canonicalBranch"],
        "canonicalWriteTokenSha256": sha256_bytes(canonical_token.encode()),
        "frozenHeads": frozen,
        "protectedPaths": cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", []),
    }
    atomic_write(paths["ref_policy"], ref_policy)

    protected: dict[str, str] = {}
    for rel in cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", []):
        path = ROOT / rel
        if not path.is_file():
            raise RuntimeError(f"protected evaluator path missing at activation: {rel}")
        protected[rel] = git("rev-parse", f"{cfg['canonicalBranch']}:{rel}")

    hook_sha = install_reference_hook(cfg, paths)
    return {
        "schemaVersion": 1,
        "activationSha": sha,
        "createdAt": now(),
        "protectedEvaluatorBlobs": protected,
        "referenceHookSha256": hook_sha,
    }


def init_state(cfg: dict) -> None:
    paths = state_paths(cfg)
    paths["meta"].parent.mkdir(parents=True, exist_ok=True)
    with locked(paths["lock"]):
        if paths["meta"].exists():
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
        salvage.setdefault("queries", {})
        salvage.setdefault("dispositions", {})
        meta = {
            "schemaVersion": 1,
            "initializedAt": now(),
            "updatedAt": now(),
            "stateRevision": 1,
            "canonicalBranch": cfg["canonicalBranch"],
            "canonicalShaAtInit": sha,
            "note": "Operational state is intentionally outside tracked Git history.",
        }
        policy = build_runtime_policy(cfg, sha, paths)
        atomic_write(paths["failures"], failures)
        atomic_write(paths["gates"], gates)
        atomic_write(paths["salvage"], salvage)
        atomic_write(paths["policy"], policy)
        atomic_write(paths["meta"], meta)
        atomic_write(
            paths["backup"],
            {"meta": meta, "failures": failures, "gates": gates, "salvage": salvage},
        )
    print(f"RECOVERY STATE INITIALIZED {paths['meta'].parent}")


def require_state(cfg: dict) -> tuple[dict, dict, dict, dict, dict[str, pathlib.Path]]:
    paths = state_paths(cfg)
    for key in ("meta", "failures", "gates", "salvage"):
        if not paths[key].is_file():
            raise RuntimeError("runtime state missing; run tools/recovery_state.py init")
    meta = load_json(paths["meta"])
    failures = load_json(paths["failures"])
    gates = load_json(paths["gates"])
    salvage = load_json(paths["salvage"])
    revision = meta.get("stateRevision")
    if not isinstance(revision, int) or revision < 1:
        raise RuntimeError("runtime state has invalid meta revision; run recovery_state.py recover-state")
    if failures.get("queueRevision") != revision:
        raise RuntimeError("runtime failures revision mismatch; run recovery_state.py recover-state")
    if gates.get("stateRevision") != revision:
        raise RuntimeError("runtime gates revision mismatch; run recovery_state.py recover-state")
    if salvage.get("stateRevision") != revision:
        raise RuntimeError("runtime salvage revision mismatch; run recovery_state.py recover-state")
    return meta, failures, gates, salvage, paths


def active_failures(failures: dict) -> list[dict]:
    return [row for row in failures.get("failures", []) if row.get("status") in ACTIVE_STATES]


def failure_by_id(failures: dict, fid: str) -> dict:
    rows = [row for row in failures.get("failures", []) if row.get("id") == fid]
    if len(rows) != 1:
        raise RuntimeError(f"failure id {fid!r} not found exactly once")
    return rows[0]


def save_all(meta: dict, failures: dict, gates: dict, salvage: dict, paths: dict[str, pathlib.Path]) -> None:
    if all(paths[key].is_file() for key in ("meta", "failures", "gates", "salvage")):
        atomic_write(
            paths["backup"],
            {
                "meta": load_json(paths["meta"]),
                "failures": load_json(paths["failures"]),
                "gates": load_json(paths["gates"]),
                "salvage": load_json(paths["salvage"]),
            },
        )
    bump(meta)
    failures["queueRevision"] = meta["stateRevision"]
    gates["stateRevision"] = meta["stateRevision"]
    salvage["stateRevision"] = meta["stateRevision"]
    atomic_write(paths["failures"], failures)
    atomic_write(paths["gates"], gates)
    atomic_write(paths["salvage"], salvage)
    atomic_write(paths["meta"], meta)


def cmd_recover_state(cfg: dict) -> int:
    paths = state_paths(cfg)
    with locked(paths["lock"]):
        if not paths["backup"].is_file():
            raise RuntimeError("last-good runtime state backup is missing")
        backup = load_json(paths["backup"])
        for key in ("meta", "failures", "gates", "salvage"):
            value = backup.get(key)
            if not isinstance(value, dict):
                raise RuntimeError(f"last-good backup is missing {key}")
        atomic_write(paths["failures"], backup["failures"])
        atomic_write(paths["gates"], backup["gates"])
        atomic_write(paths["salvage"], backup["salvage"])
        atomic_write(paths["meta"], backup["meta"])
    print(f"RECOVERY STATE RESTORED revision={backup['meta'].get('stateRevision')}")
    return 0


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


def salvage_query_open(salvage: dict, fid: str) -> bool:
    query = salvage.get("queries", {}).get(fid)
    if not isinstance(query, dict):
        return False
    return any(not candidate.get("disposition") for candidate in query.get("candidates", []))


def cmd_salvage_status(cfg: dict, args: argparse.Namespace) -> int:
    _, _, _, salvage, _ = require_state(cfg)
    queries = salvage.get("queries", {})
    if args.failure:
        queries = {args.failure: queries.get(args.failure)}
    for fid, query in sorted(queries.items()):
        if not isinstance(query, dict):
            continue
        unresolved = [c for c in query.get("candidates", []) if not c.get("disposition")]
        print(
            f"{fid} query={query.get('queryHash','-')} scanned={query.get('refsScanned',0)} "
            f"matches={query.get('matchCount',0)} selected={query.get('selectedCount',0)} unresolved={len(unresolved)}"
        )
        for row in unresolved[:8]:
            print(f"  {row.get('sourceRef')} {row.get('sourceSha','')[:12]} score={row.get('score')}")
    return 0


def cmd_salvage_query(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.failure)
        if row.get("status") not in ACTIVE_STATES | {"OPEN", "UNPROVEN"}:
            raise RuntimeError("salvage query requires an unresolved current failure")
        if salvage_query_open(salvage, args.failure):
            raise RuntimeError("prior salvage query still has unresolved candidates")
        base = main_sha(cfg)
        grants = row.get("pathGrant", []) or [normalize_repo_path(path) for path in args.path]
        if not grants:
            raise RuntimeError("salvage query needs failure pathGrant or explicit --path")
        terms = sorted({term.casefold() for term in args.term if term.strip()})
        snapshot = load_json(ROOT / cfg["legacySnapshot"])
        refs = snapshot.get("refs", {})
        if not isinstance(refs, dict) or not refs:
            raise RuntimeError("legacy snapshot has no refs")
        groups: dict[str, list[str]] = {}
        for ref, sha in refs.items():
            groups.setdefault(str(sha), []).append(str(ref))
        scored: list[dict] = []
        for sha, aliases in groups.items():
            try:
                files = changed_paths(base, sha)
            except subprocess.CalledProcessError:
                continue
            path_hits = [
                changed for changed in files
                if any(paths_overlap(changed, grant) for grant in grants)
            ]
            alias_text = " ".join(aliases).casefold()
            term_hits = [term for term in terms if term in alias_text]
            score = len(path_hits) * 100 + len(term_hits) * 10
            if score <= 0:
                continue
            scored.append(
                {
                    "sourceSha": sha,
                    "sourceRef": sorted(aliases)[0],
                    "aliases": sorted(aliases),
                    "score": score,
                    "pathHits": path_hits[:40],
                    "termHits": term_hits,
                    "disposition": None,
                }
            )
        scored.sort(key=lambda item: (-int(item["score"]), item["sourceSha"]))
        limit = int(cfg.get("salvagePolicy", {}).get("maxMaterializedCandidatesPerFailure", 8))
        if len(scored) > limit and scored[limit - 1]["score"] == scored[limit]["score"]:
            raise RuntimeError(
                "salvage candidate tie crosses materialization cutoff; narrow --path/--term before continuing"
            )
        selected = scored[:limit]
        query_value = {
            "failureId": args.failure,
            "baseSha": base,
            "pathGrant": grants,
            "terms": terms,
            "refsScanned": len(refs),
            "uniqueTipsScanned": len(groups),
            "matchCount": len(scored),
            "selectedCount": len(selected),
            "truncatedCount": max(0, len(scored) - len(selected)),
            "candidates": selected,
            "createdAt": now(),
        }
        material = json.dumps(
            {key: query_value[key] for key in ("failureId", "baseSha", "pathGrant", "terms")},
            sort_keys=True,
        ).encode()
        query_value["queryHash"] = sha256_bytes(material)
        salvage.setdefault("queries", {})[args.failure] = query_value
        save_all(meta, failures, gates, salvage, paths)
    print(
        f"SALVAGE QUERY {args.failure} scanned={len(refs)} matches={len(scored)} selected={len(selected)}"
    )
    for candidate in selected:
        print(f"{candidate['sourceRef']} {candidate['sourceSha']} score={candidate['score']}")
    return 0


def cmd_salvage_dispose(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    terminal = set(cfg.get("salvagePolicy", {}).get("terminalDispositions", []))
    if args.disposition not in terminal:
        raise RuntimeError(f"invalid terminal salvage disposition {args.disposition!r}")
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        query = salvage.get("queries", {}).get(args.failure)
        if not isinstance(query, dict):
            raise RuntimeError("no salvage query exists for this failure")
        candidates = [
            item for item in query.get("candidates", [])
            if item.get("sourceRef") == args.source_ref or args.source_ref in item.get("aliases", [])
        ]
        if len(candidates) != 1:
            raise RuntimeError("source ref is not exactly one materialized candidate")
        candidate = candidates[0]
        if candidate.get("disposition"):
            raise RuntimeError("salvage candidate is already disposed")
        snapshot = load_json(ROOT / cfg["legacySnapshot"])
        expected = snapshot.get("refs", {}).get(args.source_ref)
        if expected != candidate.get("sourceSha"):
            raise RuntimeError("salvage source ref/SHA does not match frozen snapshot")
        if args.disposition in {"SUPERSEDED_BY", "DUPLICATE"} and not args.target:
            raise RuntimeError(f"{args.disposition} requires --target")
        if args.disposition in {"SALVAGED_PRODUCT", "SALVAGED_TEST"} and not args.candidate_sha:
            raise RuntimeError(f"{args.disposition} requires --candidate-sha")
        candidate.update(
            {
                "disposition": args.disposition,
                "evidence": args.evidence,
                "target": args.target,
                "candidateSha": args.candidate_sha,
                "disposedAt": now(),
            }
        )
        salvage.setdefault("dispositions", {})[candidate["sourceSha"]] = {
            "failureId": args.failure,
            "sourceRef": candidate["sourceRef"],
            "sourceSha": candidate["sourceSha"],
            "disposition": args.disposition,
            "evidence": args.evidence,
            "target": args.target,
            "candidateSha": args.candidate_sha,
            "queryHash": query.get("queryHash"),
            "baseSha": query.get("baseSha"),
            "at": now(),
        }
        save_all(meta, failures, gates, salvage, paths)
    print(f"SALVAGE DISPOSED {args.source_ref} {args.disposition}")
    return 0


def cmd_claim(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"OPEN", "UNPROVEN"}:
            raise RuntimeError(f"{args.id} is {row.get('status')}, not OPEN/UNPROVEN")
        if row.get("strategyReviewRequired"):
            raise RuntimeError(
                f"{args.id} reached the failed-candidate circuit breaker; "
                "run strategy-review with an independent evaluator before claiming it again"
            )
        lanes = set(cfg.get("executionLanes", []))
        if args.lane not in lanes:
            raise RuntimeError(f"lane {args.lane!r} is not an execution lane")
        grants = sorted({normalize_repo_path(path) for path in args.path})
        intents = cfg.get("lanePathIntent", {}).get(args.lane, [])
        if not intents:
            raise RuntimeError(f"lane {args.lane} has no configured path intent")
        for grant in grants:
            if not any(path_within(grant, prefix) for prefix in intents):
                raise RuntimeError(
                    f"path grant {grant!r} is outside configured intent for {args.lane}: {intents}"
                )
        protected = cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", [])
        for grant in grants:
            if any(paths_overlap(grant, rel) for rel in protected):
                raise RuntimeError(f"path grant {grant!r} overlaps protected evaluator state")
        canonical = main_sha(cfg)
        lane_sha = git("rev-parse", args.lane)
        if lane_sha != canonical:
            raise RuntimeError(
                f"lane {args.lane} is stale: {lane_sha} != current {cfg['canonicalBranch']} {canonical}"
            )
        for other in active_failures(failures):
            if other.get("lane") == args.lane:
                raise RuntimeError(f"lane {args.lane} already owns {other.get('id')}")
            for wanted in grants:
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
                "pathGrant": grants,
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
        diff = changed_paths(row["baseSha"], args.sha)
        if not diff:
            raise RuntimeError("candidate has no changed paths")
        grants = row.get("pathGrant", [])
        outside = [path for path in diff if not any(path_within(path, grant) for grant in grants)]
        if outside:
            raise RuntimeError(
                f"candidate changed paths outside its lease: {outside[:8]}"
            )
        protected = cfg.get("evaluationPolicy", {}).get("protectedEvaluatorPaths", [])
        evaluator_changes = [path for path in diff if any(paths_overlap(path, rel) for rel in protected)]
        if evaluator_changes:
            raise RuntimeError(
                f"ordinary candidates may not modify frozen evaluator paths: {evaluator_changes[:8]}"
            )
        row["status"] = "CANDIDATE"
        row["candidateSha"] = args.sha
        row["changedPaths"] = diff
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
        current_reviews = [
            r
            for r in row.get("reviews", [])
            if r.get("candidateSha") == candidate
        ]
        negative = [
            r
            for r in current_reviews
            if r.get("verdict") in {"REJECT", "NEEDS_FIX"}
        ]
        if negative:
            reviewers = sorted(str(r.get("reviewer")) for r in negative)
            raise RuntimeError(
                f"{args.id} has unresolved negative evaluator verdicts from {reviewers}; "
                "reopen the failure, produce a new candidate SHA, and review again"
            )
        approvals = {
            r.get("reviewer")
            for r in current_reviews
            if r.get("verdict") == "APPROVE" and r.get("candidateSha") == candidate
        }
        need = required_approvals(cfg, row)
        evaluator_sensitive = cfg.get("evaluationPolicy", {}).get(
            "evaluatorSensitivePaths", []
        )
        evaluator_change = any(
            any(path_within(str(changed), prefix) for prefix in evaluator_sensitive)
            for changed in row.get("changedPaths", [])
        )
        if evaluator_change:
            need = max(
                need,
                int(
                    cfg.get("evaluationPolicy", {}).get(
                        "evaluatorChangeMinimumApprovals", 2
                    )
                ),
            )
        if len(approvals) < need:
            raise RuntimeError(f"{args.id} needs {need} independent approvals; has {len(approvals)}")
        if evaluator_change and cfg.get("evaluationPolicy", {}).get(
            "evaluatorChangeRequiresTestEvaluator", True
        ):
            test_ok = any(
                r.get("verdict") == "APPROVE"
                and r.get("role") == "test"
                and r.get("candidateSha") == candidate
                for r in row.get("reviews", [])
            )
            if not test_ok:
                raise RuntimeError(
                    "evaluator/judge change requires an approving independent test evaluator"
                )
        sensitive = cfg.get("evaluationPolicy", {}).get("securitySensitivePaths", [])
        security_change = any(
            any(path_within(str(changed), prefix) for prefix in sensitive)
            for changed in row.get("changedPaths", [])
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
        if cfg.get("salvagePolicy", {}).get("unresolvedCandidatesBlockIntegration", True):
            if salvage_query_open(salvage, args.id):
                raise RuntimeError(
                    f"{args.id} still has unresolved legacy salvage candidates; dispose them before integration"
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
        last_owner = row.get("owner")
        counted_states = {
            "CANDIDATE",
            "PREVERIFIED",
            "INTEGRATION_READY",
            "INTEGRATING",
            "INTEGRATED",
        }
        if previous in counted_states:
            cycles = int(row.get("failedCandidateCyclesSinceReview", 0)) + 1
            row["failedCandidateCyclesSinceReview"] = cycles
            limit = int(
                cfg.get("optimizerPolicy", {}).get(
                    "maxFailedCandidateCyclesBeforeStrategyReview", 3
                )
            )
            if cycles >= limit:
                row["strategyReviewRequired"] = True
        if last_owner:
            row["lastOwner"] = last_owner
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
            "changedPaths",
            "claimedAt",
            "candidateAt",
            "reviews",
            "preverifiedAt",
            "preverifyEvidence",
            "integrationReadyAt",
            "integrationOwner",
            "integrationStartedAt",
        ):
            row.pop(key, None)
        row["status"] = "OPEN"
        row["lastReopenedAt"] = now()
        row["lastReopenReason"] = args.reason
        row["lastState"] = previous
        save_all(meta, failures, gates, salvage, paths)
    print(f"REOPENED {args.id} from={previous}")
    return 0


def cmd_strategy_review(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"OPEN", "UNPROVEN"}:
            raise RuntimeError("strategy-review requires an OPEN/UNPROVEN failure")
        if not row.get("strategyReviewRequired"):
            raise RuntimeError("strategy-review is not required for this failure")
        if (
            cfg.get("optimizerPolicy", {}).get(
                "strategyReviewRequiresIndependentEvaluator", True
            )
            and args.reviewer == row.get("lastOwner")
        ):
            raise RuntimeError(
                "strategy reviewer must be independent from the last candidate owner"
            )
        if not args.diagnosis.strip() or not args.new_approach.strip():
            raise RuntimeError("strategy review requires diagnosis and a new approach")
        row.setdefault("strategyReviews", []).append(
            {
                "reviewer": args.reviewer,
                "diagnosis": args.diagnosis,
                "newApproach": args.new_approach,
                "at": now(),
            }
        )
        row["strategyReviewRequired"] = False
        row["failedCandidateCyclesSinceReview"] = 0
        row["lastStrategyReviewAt"] = now()
        save_all(meta, failures, gates, salvage, paths)
    print(f"STRATEGY REVIEWED {args.id} reviewer={args.reviewer}")
    return 0


def cmd_accept(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        row = failure_by_id(failures, args.id)
        if row.get("status") not in {"INTEGRATED", "OPEN", "UNPROVEN"}:
            raise RuntimeError(f"{args.id} must be INTEGRATED, or an OPEN/UNPROVEN probe failure")
        if row.get("status") in {"OPEN", "UNPROVEN"} and row.get("requiresImplementation", True):
            raise RuntimeError(
                f"{args.id} requires implementation and must pass the full candidate/review/integration lifecycle before acceptance"
            )
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
        for other in failures.get("failures", []):
            if other is row:
                continue
            if other.get("status") == "ACCEPTED" and other.get("acceptedSha") != canonical:
                other["lastAcceptedSha"] = other.get("acceptedSha")
                other["lastAcceptEvidence"] = other.get("acceptEvidence")
                other["status"] = "UNPROVEN"
                other.pop("acceptedSha", None)
                other.pop("acceptEvidence", None)
        for gate in gates.get("gates", []):
            if gate.get("status") == "ACCEPTED" and gate.get("acceptedSha") != canonical:
                gate["lastAcceptedSha"] = gate.get("acceptedSha")
                gate["lastAcceptedEvidence"] = gate.get("evidence", [])
                gate["status"] = "UNPROVEN"
                gate["acceptedSha"] = None
                gate["evidence"] = []
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


def observed_failure_id(check: str) -> str:
    safe = re.sub(r"[^A-Za-z0-9]+", "-", check).strip("-").upper()
    return f"OBS-{safe}"[:120]


def cmd_observe_judge(cfg: dict, args: argparse.Namespace) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    evidence = pathlib.Path(args.evidence).resolve()
    allowed = paths["evidence"].resolve()
    if allowed not in evidence.parents or not evidence.is_file():
        raise RuntimeError(f"judge evidence must be an existing file under {allowed}")
    result = load_json(evidence)
    canonical = main_sha(cfg)
    if result.get("sourceSha") != canonical or result.get("canonicalSha") != canonical:
        raise RuntimeError("judge evidence is not bound to current main-v2")
    checks = {str(row.get("name")): row for row in result.get("checks", [])}
    gate_results = result.get("gateResults", {})
    failed_checks: dict[str, set[str]] = {}
    artifact_mode = result.get("artifactMode") is True
    for gate_id, gate in gate_results.items():
        if artifact_mode:
            for check in gate.get("missingChecks", []):
                failed_checks.setdefault(str(check), set()).add(str(gate_id))
        for check in gate.get("failedChecks", []):
            failed_checks.setdefault(str(check), set()).add(str(gate_id))
    all_installed_pass = artifact_mode and all(
        gate.get("status") == "PASS" for gate in gate_results.values()
    )

    with locked(paths["lock"]):
        meta, failures, gates, salvage, paths = require_state(cfg)
        if main_sha(cfg) != canonical:
            raise RuntimeError("main-v2 moved while ingesting judge evidence; rerun judge")
        rows = failures.setdefault("failures", [])
        by_check = {
            str(row.get("check")): row
            for row in rows
            if isinstance(row.get("check"), str)
        }

        for check, gate_ids in sorted(failed_checks.items()):
            existing = by_check.get(check)
            if existing is None:
                gate = sorted(gate_ids)[0] if gate_ids else "P1-RELEASE"
                existing = {
                    "id": observed_failure_id(check),
                    "priority": "P1",
                    "gate": gate,
                    "status": "OPEN",
                    "kind": "judge-observed",
                    "check": check,
                    "requiresImplementation": True,
                    "title": f"Installed judge check fails: {check}",
                    "evidence": str(evidence),
                    "nextAction": "Diagnose the exact observed failure, claim a fixed recovery lane, and use the evaluator-optimizer lifecycle.",
                }
                rows.append(existing)
                by_check[check] = existing
            if existing.get("status") in {"ACCEPTED", "SUPERSEDED", "REJECTED"}:
                existing["status"] = "OPEN"
                existing.pop("acceptedSha", None)
                existing.pop("acceptEvidence", None)
            existing["lastObservedAt"] = now()
            existing["lastObservedSha"] = canonical
            existing["lastJudgeEvidence"] = str(evidence)

        passing = {
            name for name, check in checks.items() if check.get("exit") == 0
        }
        if all_installed_pass:
            passing.add("phase1_all_installed")
        for check, row in list(by_check.items()):
            if check not in passing or check in failed_checks:
                continue
            status = row.get("status")
            if status == "INTEGRATED":
                row["status"] = "ACCEPTED"
                row["acceptedSha"] = canonical
                row["acceptedAt"] = now()
                row["acceptEvidence"] = str(evidence)
            elif row.get("requiresImplementation") is False and status in {"OPEN", "UNPROVEN"}:
                row["status"] = "ACCEPTED"
                row["acceptedSha"] = canonical
                row["acceptedAt"] = now()
                row["acceptEvidence"] = str(evidence)
            elif status in {"OPEN", "UNPROVEN"}:
                row["status"] = "SUPERSEDED"
                row["supersededReason"] = (
                    "Exact current installed judge passes this check; another integrated/reviewed change resolved the observed failure."
                )
                row["supersededEvidence"] = str(evidence)
        save_all(meta, failures, gates, salvage, paths)
    print(
        f"JUDGE OBSERVED sha={canonical} failed_checks={len(failed_checks)} artifact={artifact_mode}"
    )
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


def cmd_integrate(cfg: dict, args: argparse.Namespace) -> int:
    canonical_root = pathlib.Path(
        cfg.get("approvedRecoveryWorktrees", {}).get(cfg["canonicalBranch"], ROOT)
    ).resolve()
    if subprocess.check_output(
        ["git", "-C", str(canonical_root), "status", "--porcelain"], text=True
    ).strip():
        raise RuntimeError("canonical main-v2 worktree is dirty; refuse integration")
    before = main_sha(cfg)
    cmd_begin_integration(cfg, args)
    _, failures, _, _, paths = require_state(cfg)
    row = failure_by_id(failures, args.id)
    candidate = str(row.get("candidateSha"))
    try:
        ref_env = dict(os.environ)
        ref_env["OC2_RECOVERY_CANONICAL_TOKEN"] = paths["canonical_token"].read_text(
            encoding="utf-8"
        ).strip()
        subprocess.check_call(
            ["git", "-C", str(canonical_root), "merge", "--ff-only", candidate],
            env=ref_env,
        )
        after = main_sha(cfg)
        if after != candidate:
            raise RuntimeError(
                f"fast-forward integration ended at {after}, expected reviewed candidate {candidate}"
            )
        return cmd_complete_integration(
            cfg, argparse.Namespace(id=args.id, sha=after)
        )
    except Exception:
        after = main_sha(cfg)
        if after == before:
            with locked(paths["lock"]):
                meta, failures, gates, salvage, paths = require_state(cfg)
                row = failure_by_id(failures, args.id)
                if row.get("status") == "INTEGRATING":
                    row["status"] = "INTEGRATION_READY"
                    row.pop("integrationOwner", None)
                    row.pop("integrationStartedAt", None)
                    save_all(meta, failures, gates, salvage, paths)
            paths["integration_lock"].unlink(missing_ok=True)
        else:
            print(
                "INTEGRATION INTERRUPTED AFTER MAIN MOVED; keep lock/state and run complete-integration after inspection",
                file=sys.stderr,
            )
        raise


def cmd_sync_lane(cfg: dict, args: argparse.Namespace) -> int:
    """Adopt the current canonical tip under a private, one-ref authorization.

    Exclusion order is integration.lock (created O_EXCL), then state.lock, then
    capability file. Both locks remain held through Git's full ref transaction.
    A crash leaves the integration lock/capability and blocks future work; it
    never silently reuses authority.
    """
    paths = state_paths(cfg)
    lane = str(args.lane)
    fixed = set(cfg.get("executionLanes", [])) & set(cfg.get("fixedMutableBranches", []))
    if lane not in fixed:
        raise RuntimeError(f"{lane!r} is not a fixed execution lane")
    lane_root = pathlib.Path(cfg["approvedRecoveryWorktrees"][lane]).resolve()
    capability = paths["lock"].parent / "lane-sync-capability.json"
    operation = "lane-sync-" + secrets.token_hex(16)
    try:
        fd = os.open(paths["integration_lock"], os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError as exc:
        raise RuntimeError("integration lock exists; lane sync fails closed") from exc
    lock_owned = True
    preserve_lock = False
    capability_minted = False
    ref_committed = False
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump({"kind": "lane-sync", "lane": lane, "operation": operation, "startedAt": now()}, handle)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        with locked(paths["lock"]):
            _, failures, _, _, paths = require_state(cfg)
            if capability.exists():
                preserve_lock = True
                raise RuntimeError("stale lane-sync capability exists; fail closed and inspect controller runtime")
            if any(row.get("lane") == lane and row.get("status") in ACTIVE_STATES
                   for row in failures.get("failures", [])):
                raise RuntimeError(f"{lane} has active work and cannot be synchronized")
            dirty = subprocess.check_output(
                ["git", "-C", str(lane_root), "status", "--porcelain", "--untracked-files=all"], text=True
            ).strip()
            if dirty:
                raise RuntimeError(f"{lane} worktree is dirty")
            canonical = main_sha(cfg)
            old = git("rev-parse", lane)
            checked_out = subprocess.check_output(
                ["git", "-C", str(lane_root), "symbolic-ref", "--short", "HEAD"], text=True
            ).strip()
            worktree_head = subprocess.check_output(
                ["git", "-C", str(lane_root), "rev-parse", "HEAD"], text=True
            ).strip()
            if checked_out != lane or worktree_head != old:
                raise RuntimeError(f"{lane} worktree HEAD/ref mismatch: {checked_out}@{worktree_head} != {lane}@{old}")
            if git("rev-parse", lane) != old or main_sha(cfg) != canonical:
                raise RuntimeError("lane or canonical moved during sync preflight")
            # Dry-run Git's two-tree worktree update before granting ref
            # authority. This detects ignored/untracked collisions that status
            # may omit, so a later index refresh cannot strand a moved lane.
            subprocess.run(
                ["git", "-C", str(lane_root), "read-tree", "--dry-run", "-m", "-u", old, canonical],
                check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            record = {
                "schemaVersion": 1, "status": "ARMED", "operation": operation,
                "lane": lane, "laneRef": f"refs/heads/{lane}", "oldLaneSha": old,
                "canonicalRef": f"refs/heads/{cfg['canonicalBranch']}", "canonicalSha": canonical,
                "targetTree": git("rev-parse", f"{canonical}^{{tree}}"),
                "expiresAt": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=60)).isoformat(),
            }
            atomic_write(capability, record)
            os.chmod(capability, 0o600)
            capability_minted = True
            # Repeat preflight immediately before entering Git's prepared phase.
            # No dirty user data may be hidden by the subsequent index refresh.
            if (main_sha(cfg) != canonical or git("rev-parse", lane) != old
                    or subprocess.check_output(["git", "-C", str(lane_root), "status", "--porcelain", "--untracked-files=all"], text=True).strip()):
                capability.unlink(missing_ok=True)
                raise RuntimeError("canonical, lane, or worktree changed after capability mint")
            env = dict(os.environ)
            env["OC2_RECOVERY_LANE_SYNC_CAP"] = operation
            try:
                # update-ref yields one atomic ref transaction. Unlike merge,
                # it does not propose AUTO_MERGE or any side ref to the hook.
                transaction = (
                    "start\n"
                    f"update refs/heads/{lane} {canonical} {old}\n"
                    "prepare\ncommit\n"
                )
                subprocess.run(
                    ["git", "-C", str(lane_root), "update-ref", "--stdin"],
                    input=transaction, text=True, check=True, env=env,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                )
                if git("rev-parse", lane) != canonical or main_sha(cfg) != canonical:
                    raise RuntimeError("post-sync lane/canonical SHA differs from authorized target")
                ref_committed = True
                # Ref transaction has committed and the hook consumed authority.
                # If a concurrent local edit appeared, preserve it and stop;
                # otherwise refresh only index/worktree, never refs.
                index_tree = subprocess.check_output(["git", "-C", str(lane_root), "write-tree"], text=True).strip()
                old_tree = git("rev-parse", f"{old}^{{tree}}")
                worktree_delta = subprocess.check_output(["git", "-C", str(lane_root), "diff", "--name-only"], text=True).strip()
                untracked = subprocess.check_output(["git", "-C", str(lane_root), "ls-files", "--others", "--exclude-standard"], text=True).strip()
                if index_tree != old_tree or worktree_delta or untracked:
                    raise RuntimeError("lane ref synchronized but worktree/index changed concurrently; preserve work and inspect")
                subprocess.check_call(["git", "-C", str(lane_root), "read-tree", "--reset", "-u", canonical])
            except Exception:
                if not ref_committed:
                    capability.unlink(missing_ok=True)
                else:
                    preserve_lock = True
                raise
            capability.unlink(missing_ok=True)
    finally:
        if lock_owned and capability_minted and capability.exists():
            # An interrupted or failed operation with an armed record blocks
            # follow-on integration/sync until explicit diagnosis.
            preserve_lock = True
        if lock_owned and not preserve_lock:
            try:
                owner = load_json(paths["integration_lock"])
            except (OSError, ValueError, TypeError):
                owner = {}
            if owner.get("operation") == operation and owner.get("kind") == "lane-sync":
                paths["integration_lock"].unlink(missing_ok=True)
    print(f"LANE SYNCED {lane} {canonical}")
    return 0


def cmd_recover_integration(cfg: dict) -> int:
    meta, failures, gates, salvage, paths = require_state(cfg)
    integrating = [
        row for row in failures.get("failures", [])
        if row.get("status") == "INTEGRATING"
    ]
    if len(integrating) != 1:
        raise RuntimeError(
            f"recover-integration requires exactly one INTEGRATING failure; found {len(integrating)}"
        )
    row = integrating[0]
    current = main_sha(cfg)
    candidate = str(row.get("candidateSha", ""))
    base = str(row.get("baseSha", ""))
    if candidate and subprocess.run(
        ["git", "merge-base", "--is-ancestor", candidate, current], cwd=ROOT
    ).returncode == 0:
        return cmd_complete_integration(
            cfg, argparse.Namespace(id=row["id"], sha=current)
        )
    if current == base:
        with locked(paths["lock"]):
            meta, failures, gates, salvage, paths = require_state(cfg)
            row = failure_by_id(failures, row["id"])
            row["status"] = "INTEGRATION_READY"
            row.pop("integrationOwner", None)
            row.pop("integrationStartedAt", None)
            save_all(meta, failures, gates, salvage, paths)
        paths["integration_lock"].unlink(missing_ok=True)
        print(f"INTEGRATION RECOVERED TO READY {row['id']}")
        return 0
    raise RuntimeError(
        f"integration recovery is ambiguous: current main {current}, base {base}, candidate {candidate}; inspect before mutation"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("init")
    sub.add_parser("recover-state")
    sub.add_parser("status")

    p = sub.add_parser("salvage-status")
    p.add_argument("--failure")

    p = sub.add_parser("salvage-query")
    p.add_argument("--failure", required=True)
    p.add_argument("--path", action="append", default=[])
    p.add_argument("--term", action="append", default=[])

    p = sub.add_parser("salvage-dispose")
    p.add_argument("--failure", required=True)
    p.add_argument("--source-ref", required=True)
    p.add_argument("--disposition", required=True)
    p.add_argument("--evidence", required=True)
    p.add_argument("--target")
    p.add_argument("--candidate-sha")

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

    p = sub.add_parser("strategy-review")
    p.add_argument("--id", required=True)
    p.add_argument("--reviewer", required=True)
    p.add_argument("--diagnosis", required=True)
    p.add_argument("--new-approach", required=True)

    p = sub.add_parser("begin-integration")
    p.add_argument("--id", required=True)
    p.add_argument("--owner", required=True)

    p = sub.add_parser("integrate")
    p.add_argument("--id", required=True)
    p.add_argument("--owner", required=True)

    p = sub.add_parser("sync-lane")
    p.add_argument("--lane", required=True)

    sub.add_parser("recover-integration")

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

    p = sub.add_parser("observe-judge")
    p.add_argument("--evidence", required=True)
    return parser


def main() -> int:
    cfg = load_json(CONFIG_PATH)
    args = build_parser().parse_args()
    try:
        if args.cmd == "init":
            init_state(cfg)
            return 0
        if args.cmd == "recover-state":
            return cmd_recover_state(cfg)
        if args.cmd == "status":
            return cmd_status(cfg)
        if args.cmd == "salvage-status":
            return cmd_salvage_status(cfg, args)
        if args.cmd == "salvage-query":
            return cmd_salvage_query(cfg, args)
        if args.cmd == "salvage-dispose":
            return cmd_salvage_dispose(cfg, args)
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
        if args.cmd == "strategy-review":
            return cmd_strategy_review(cfg, args)
        if args.cmd == "begin-integration":
            return cmd_begin_integration(cfg, args)
        if args.cmd == "integrate":
            return cmd_integrate(cfg, args)
        if args.cmd == "sync-lane":
            return cmd_sync_lane(cfg, args)
        if args.cmd == "recover-integration":
            return cmd_recover_integration(cfg)
        if args.cmd == "end-integration":
            return cmd_end_integration(cfg, args)
        if args.cmd == "complete-integration":
            return cmd_complete_integration(cfg, args)
        if args.cmd == "accept":
            return cmd_accept(cfg, args)
        if args.cmd == "sync-gates":
            return cmd_sync_gates(cfg, args)
        if args.cmd == "observe-judge":
            return cmd_observe_judge(cfg, args)
    except (RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"RECOVERY STATE BLOCKED: {exc}", file=sys.stderr)
        return 1
    raise AssertionError(args.cmd)


if __name__ == "__main__":
    raise SystemExit(main())

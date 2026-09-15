#!/usr/bin/env python3
"""REL-001 release-feature-accounting validator (additive, read-only).

Entrypoint: python3 tools/check_release_accounting.py --snapshot <dir> --out <report.json>

Reads a read-only repository snapshot (user-requirements.json, ralph.json,
release-ledger.json, controller-status.json, evidence_rev.txt, optional
backlog-ledger.json) and emits a deterministic machine-verifiable report:
{"passed": bool, "missing": [ids], "misclassified": [ids],
 "evidence_rev": "<git rev>", "partition": "release-feature-accounting-validator"}.

Exit 0 on accounting pass, exit 2 on accounting failure, exit 1 on
tool/usage error (no report written). Stdlib only. No network, no threads,
no wall-clock in verdict. Never emits an acceptance grant.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
import time

PARTITION = "release-feature-accounting-validator"
PIN_TASKS = frozenset({"DISC-001", "DISC-010"})
MAX_FILES = 256
MAX_TOTAL_BYTES = 8 * 1024 * 1024
MAX_REPORT_BYTES = 64 * 1024
DEFAULT_TIMEOUT = 60.0


class ToolError(Exception):
    pass


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="REL-001 release-feature-accounting validator"
    )
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    # argparse exits 2 on usage errors; contract requires exit 1.
    try:
        return parser.parse_args(argv)
    except SystemExit as exc:
        raise ToolError(f"usage error: exit {exc.code}") from exc


def _read_snapshot_files(snapshot: pathlib.Path, deadline: float) -> dict[str, bytes]:
    if not snapshot.is_dir():
        raise ToolError(f"unreadable snapshot: {snapshot}")
    try:
        names = sorted(os.listdir(snapshot))
    except OSError as exc:
        raise ToolError(f"unreadable snapshot: {exc}") from exc
    if len(names) > MAX_FILES:
        raise ToolError(f"snapshot exceeds file cap ({len(names)} > {MAX_FILES})")
    total = 0
    blobs: dict[str, bytes] = {}
    for name in names:
        if time.monotonic() > deadline:
            raise ToolError("timeout")
        path = snapshot / name
        if not path.is_file() or path.is_symlink():
            continue
        try:
            data = path.read_bytes()
        except OSError as exc:
            raise ToolError(f"unreadable snapshot file: {name}: {exc}") from exc
        total += len(data)
        if total > MAX_TOTAL_BYTES:
            raise ToolError("snapshot exceeds byte cap (8 MiB)")
        blobs[name] = data
    return blobs


def _load_json(blobs: dict[str, bytes], name: str, *, required: bool) -> object | None:
    if name not in blobs:
        if required:
            raise ToolError(f"missing snapshot file: {name}")
        return None
    try:
        return json.loads(blobs[name].decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as exc:
        raise ToolError(f"malformed JSON: {name}: {exc}") from exc


def _check(reqs: object, ralph: object, ledger: object) -> tuple[list[str], list[str], dict[str, str], dict[str, str]]:
    """Return (missing, misclassified, hints, reasons). All sorted."""
    if not isinstance(reqs, dict) or not isinstance(reqs.get("requirements"), list):
        raise ToolError("malformed JSON: user-requirements.json: bad requirements")
    if not isinstance(ralph, dict) or not isinstance(ralph.get("userStories"), list):
        raise ToolError("malformed JSON: ralph.json: bad userStories")
    if not isinstance(ledger, dict) or not isinstance(ledger.get("edges"), list):
        raise ToolError("malformed JSON: release-ledger.json: bad edges")

    stories: dict[str, dict] = {}
    for story in ralph["userStories"]:
        if isinstance(story, dict) and isinstance(story.get("id"), str):
            stories[story["id"]] = story

    mandatory: dict[str, list[str]] = {}
    for req in reqs["requirements"]:
        if isinstance(req, dict) and req.get("mandatory") is True and isinstance(req.get("id"), str):
            tasks = [t for t in req.get("tasks", []) if isinstance(t, str)]
            mandatory[req["id"]] = tasks

    mis: set[str] = set()
    reasons: dict[str, str] = {}
    evidenced_tasks: set[str] = set()
    evidenced_reqs: set[str] = set()
    for edge in ledger["edges"]:
        if not isinstance(edge, dict):
            continue
        req_id = edge.get("requirement")
        task_id = edge.get("task")
        card = edge.get("card")
        tests = edge.get("tests")
        if not isinstance(req_id, str) or not isinstance(task_id, str):
            continue
        ok = True
        reason = ""
        if not isinstance(card, str) or not card or not isinstance(tests, list) or not tests:
            ok = False
            reason = "inference-without-evidence"
        else:
            story = stories.get(task_id)
            if story is None or req_id not in story.get("requirementIds", []):
                ok = False
                reason = "inference-without-evidence"
            elif not all(isinstance(t, str) and t in story.get("testObligations", []) for t in tests):
                ok = False
                reason = "inference-without-evidence"
        if ok:
            evidenced_tasks.add(task_id)
            evidenced_reqs.add(req_id)
        else:
            mis.add(req_id)
            mis.add(task_id)
            reasons[req_id] = reason
            reasons[task_id] = reason

    missing: set[str] = set()
    hints: dict[str, str] = {}
    for req_id, tasks in mandatory.items():
        for task_id in tasks:
            if task_id not in evidenced_tasks:
                missing.add(task_id)
                hints[task_id] = (
                    f"expected edge: requirement={req_id} task={task_id} "
                    f"card=tasks/{task_id}.md tests=[{task_id}-T01..T05]"
                )
        if req_id not in evidenced_reqs:
            missing.add(req_id)
            hints[req_id] = (
                f"expected edge: requirement={req_id} "
                f"task=<one of {','.join(tasks) or 'none'}> card=tasks/<id>.md"
            )
    return sorted(missing), sorted(mis), hints, reasons


def main(argv: list[str] | None = None) -> int:
    deadline = time.monotonic() + DEFAULT_TIMEOUT
    try:
        args = _parse_args(sys.argv[1:] if argv is None else argv)
    except ToolError as exc:
        print(f"check_release_accounting: error: {exc}", file=sys.stderr)
        return 1
    deadline = time.monotonic() + (args.timeout if args.timeout > 0 else DEFAULT_TIMEOUT)
    snapshot = pathlib.Path(args.snapshot)
    out = pathlib.Path(args.out)
    try:
        blobs = _read_snapshot_files(snapshot, deadline)
        reqs = _load_json(blobs, "user-requirements.json", required=True)
        ralph = _load_json(blobs, "ralph.json", required=True)
        _load_json(blobs, "controller-status.json", required=True)
        if time.monotonic() > deadline:
            raise ToolError("timeout")
        if "release-ledger.json" not in blobs:
            ledger: object = {"partition": "absent", "edges": []}
            ledger_missing = True
        else:
            ledger = _load_json(blobs, "release-ledger.json", required=True)
            ledger_missing = False
        rev_blob = blobs.get("evidence_rev.txt", b"").decode("utf-8", "replace").strip()
        evidence_rev = rev_blob.split()[0] if rev_blob.split() else "unknown"
        missing, misclassified, hints, reasons = _check(reqs, ralph, ledger)

        reason = ""
        detail = ""
        partition = PARTITION
        if isinstance(ledger, dict):
            partition = ledger.get("partition", PARTITION) if isinstance(ledger.get("partition"), str) else PARTITION
        edge_tasks: set[str] = set()
        if isinstance(ledger, dict) and isinstance(ledger.get("edges"), list):
            for edge in ledger["edges"]:
                if isinstance(edge, dict) and isinstance(edge.get("task"), str):
                    edge_tasks.add(edge["task"])
        # Distinct-guarantee disqualifier: ledger with no release partition or
        # only pin/coverage edges duplicates DISC-001/DISC-010 ownership.
        if ledger_missing or partition != PARTITION or (edge_tasks and edge_tasks <= set(PIN_TASKS)):
            reason = "duplicate-of-existing-owner"
            owners = sorted(edge_tasks & set(PIN_TASKS)) or sorted(PIN_TASKS)
            detail = f"ledger duplicates existing owners: {','.join(owners)} (DISC-001 pin / DISC-010 coverage); no release ledger"

        passed = not missing and not misclassified and not reason
        report: dict[str, object] = {
            "passed": passed,
            "missing": missing,
            "misclassified": misclassified,
            "evidence_rev": evidence_rev,
            "partition": PARTITION,
        }
        if hints and not passed:
            report["hints"] = {k: hints[k] for k in sorted(hints) if k in set(missing)}
        if reasons and not passed:
            report["reasons"] = {k: reasons[k] for k in sorted(reasons) if k in set(misclassified)}
        if reason:
            report["reason"] = reason
            report["detail"] = detail
        text = json.dumps(report, sort_keys=True, indent=2) + "\n"
        if len(text.encode("utf-8")) > MAX_REPORT_BYTES:
            # Never drop fail entries; truncate auxiliary maps, count overflow.
            dropped = 0
            for key in ("hints", "reasons", "detail"):
                if key in report:
                    del report[key]
                    dropped += 1
            report["truncated"] = dropped
            text = json.dumps(report, sort_keys=True, indent=2) + "\n"
        out.write_text(text, encoding="utf-8")
        print(text, end="")
        return 0 if passed else 2
    except ToolError as exc:
        print(f"check_release_accounting: error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

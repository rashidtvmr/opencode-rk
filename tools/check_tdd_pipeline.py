#!/usr/bin/env python3
"""AUTO-005: trusted RED/GREEN pipeline enforcer fragment.

Additive pipeline-enforcement tool. Consumes a controller-frozen test
manifest plus implementer/verifier receipts (read-only) and emits a
deterministic accept/reject report. Enforces propose -> freeze (hash +
command manifest) -> verify (rerun frozen suite on exact revision) ->
accept; rejects edited tests, self-reports, and wrong-revision runs;
blocked pipelines stop with the exact blocker.

Contract (tasks/AUTO-005.md):
  python3 tools/check_tdd_pipeline.py --manifest <manifest.json> \
      --rev <rev> --receipts <dir> --out <report.json>
  exit 0 pass, exit 2 assurance failure, exit 1 tool error.

Stdlib only. Single process, no threads, no network. Reads inputs
read-only; writes only <out>. Bounded: 256 files / 8 MiB inputs,
64 KiB report. Never logs secrets (inputs are hashes/commands only).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import time

PARTITION = "trusted-red-green-pipeline"
MAX_FILES = 256
MAX_TOTAL_BYTES = 8 * 1024 * 1024
MAX_REPORT_BYTES = 64 * 1024

PASS = "pass"
FAIL = "fail"


class ToolError(Exception):
    """Malformed/unreadable inputs, bound violations, timeout. Exit 1."""


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="check_tdd_pipeline.py",
        description="AUTO-005 trusted RED/GREEN pipeline enforcer.",
    )
    # Unknown flags are tool errors (exit 1), not assurance failures.
    parser.error = lambda message: (_emit_tool_error(message), sys.exit(1))  # type: ignore[method-assign]
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--rev", required=True)
    parser.add_argument("--receipts", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser.parse_args(argv)


def _emit_tool_error(message: str) -> None:
    sys.stderr.write(f"check_tdd_pipeline: tool-error: {message}\n")


def _load_json(path: pathlib.Path, what: str) -> object:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ToolError(f"unreadable {what}: {path}: {exc.strerror or exc}") from exc
    if len(raw) > MAX_TOTAL_BYTES:
        raise ToolError(f"bound exceeded: {what} {path} > {MAX_TOTAL_BYTES} bytes")
    try:
        return json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError) as exc:
        raise ToolError(f"malformed {what}: {path}: {exc}") from exc


def _sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(65536), b""):
                digest.update(chunk)
    except OSError as exc:
        raise ToolError(f"unreadable test file: {path}: {exc.strerror or exc}") from exc
    return digest.hexdigest()


def _check_deadline(start: float, timeout: float) -> None:
    if timeout > 0 and (time.monotonic() - start) > timeout:
        raise ToolError("timeout: --timeout exceeded, aborting")


def main(argv: list[str] | None = None) -> int:
    start = time.monotonic()
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    manifest_path = pathlib.Path(args.manifest)
    receipts_dir = pathlib.Path(args.receipts)
    out_path = pathlib.Path(args.out)

    try:
        manifest = _load_json(manifest_path, "manifest")
        if not isinstance(manifest, dict):
            raise ToolError(f"malformed manifest: {manifest_path}: top level must be object")
        base = manifest_path.parent

        # Distinct-slice guard: pipeline partition only. Release-assurance
        # material (REL-002) is owned elsewhere, never passed here.
        partition = manifest.get("partition")
        owner = manifest.get("owner")
        if partition != PARTITION:
            report = {
                "passed": False,
                "checks": {
                    "red_proof": FAIL,
                    "frozen_intact": FAIL,
                    "green_on_frozen": FAIL,
                    "verifier_rerun": FAIL,
                },
                "failing_fixture": str(manifest_path),
                "evidence_rev": args.rev,
                "partition": PARTITION,
                "reason": f"duplicate-of-existing-owner:{owner or 'REL-002'}",
            }
            return _write_report(out_path, report, 2, start, args.timeout)

        files = manifest.get("files")
        if not isinstance(files, list) or not files:
            raise ToolError(f"malformed manifest: {manifest_path}: 'files' must be a non-empty list")
        if len(files) > MAX_FILES:
            raise ToolError(f"bound exceeded: {len(files)} files > {MAX_FILES}")
        total_bytes = 0
        frozen: dict[str, str] = {}
        for entry in files:
            if not isinstance(entry, dict) or not isinstance(entry.get("path"), str) \
                    or not isinstance(entry.get("sha256"), str):
                raise ToolError(f"malformed manifest: {manifest_path}: bad file entry {entry!r}")
            rel = entry["path"]
            candidate_probe = pathlib.Path(rel)
            if candidate_probe.is_absolute() or ".." in candidate_probe.parts:
                raise ToolError(f"malformed manifest: {manifest_path}: unsafe path {rel!r}")
            frozen[entry["path"]] = entry["sha256"]
        _check_deadline(start, args.timeout)

        # freeze: controller re-read of disk bytes must hash-equal frozen pins.
        mismatched: list[str] = []
        for rel in frozen:
            candidate = base / rel
            try:
                size = candidate.stat().st_size
            except OSError as exc:
                raise ToolError(f"unreadable test file: {candidate}: {exc.strerror or exc}") from exc
            total_bytes += size
            if total_bytes > MAX_TOTAL_BYTES:
                raise ToolError(f"bound exceeded: test bytes > {MAX_TOTAL_BYTES}")
            if _sha256_file(candidate) != frozen[rel]:
                mismatched.append(rel)
            _check_deadline(start, args.timeout)
        frozen_ok = not mismatched

        receipts: dict[str, object] = {}
        if receipts_dir.is_dir():
            children = sorted(receipts_dir.iterdir())
            if len(children) > MAX_FILES:
                raise ToolError(f"bound exceeded: {len(children)} receipt files > {MAX_FILES}")
            for child in children:
                if child.is_file() and not child.is_symlink() and child.suffix == ".json":
                    receipts[child.stem] = _load_json(child, "receipt")
                _check_deadline(start, args.timeout)

        # Blocked pipeline stops with the exact blocker; never a pass.
        blocked = receipts.get("blocked")
        if isinstance(blocked, dict) and blocked.get("blocked") is True:
            _emit_tool_error(f"blocked-pending-authority: {blocked.get('detail') or 'pipeline blocked'}")
            return 1

        def receipt(name: str) -> dict | None:
            value = receipts.get(name)
            return value if isinstance(value, dict) else None

        red = receipt("red")
        green = receipt("green")
        verifier = receipt("verifier")
        worker_only = receipt("worker") is not None

        # red_proof: compiled RED failing for missing behavior on exact rev.
        red_ok = (
            red is not None
            and red.get("compiled") is True
            and isinstance(red.get("exit_code"), int)
            and bool(red["exit_code"])
            and red.get("failure_reason") == "missing-behavior"
            and red.get("revision") == args.rev
            and all(red.get("test_hash") == digest for digest in frozen.values())
        )

        # green_on_frozen: GREEN ran the unedited frozen suite on exact rev.
        green_ok = (
            frozen_ok
            and green is not None
            and green.get("exit_code") == 0
            and green.get("revision") == args.rev
            and all(green.get("test_hash") == digest for digest in frozen.values())
        )

        # verifier_rerun: independent rerun on exact rev. Worker
        # self-report (passes:true) never counts as evidence.
        verifier_ok = (
            verifier is not None
            and verifier.get("independent") is True
            and verifier.get("exit_code") == 0
            and verifier.get("revision") == args.rev
            and all(verifier.get("test_hash") == digest for digest in frozen.values())
        )

        checks = {
            "red_proof": PASS if red_ok else FAIL,
            "frozen_intact": PASS if frozen_ok else FAIL,
            "green_on_frozen": PASS if green_ok else FAIL,
            "verifier_rerun": PASS if verifier_ok else FAIL,
        }
        passed = all(value == PASS for value in checks.values())

        failing_fixture: str | None = None
        reason: str | None = None
        if not passed:
            if not red_ok:
                failing_fixture = str(receipts_dir / "red.json")
                if red is None:
                    reason = "missing-red-proof"
                elif red.get("failure_reason") != "missing-behavior" or red.get("compiled") is not True:
                    reason = "red-failed-for-wrong-reason"
                else:
                    reason = "red-proof-mismatch"
            elif not frozen_ok:
                failing_fixture = str(base / mismatched[0])
                reason = "mutated-frozen-evidence"
            elif not green_ok:
                failing_fixture = str(receipts_dir / "green.json")
                reason = "green-not-on-frozen"
            else:
                failing_fixture = str(receipts_dir / "verifier.json")
                if verifier is None and worker_only:
                    reason = "self-report-not-evidence"
                elif verifier is not None and verifier.get("revision") != args.rev:
                    reason = "verifier-revision-mismatch"
                else:
                    reason = "verifier-rerun-missing"

        report = {
            "passed": passed,
            "checks": checks,
            "failing_fixture": failing_fixture,
            "evidence_rev": args.rev,
            "partition": PARTITION,
            "reason": reason,
        }
        return _write_report(out_path, report, 0 if passed else 2, start, args.timeout)
    except ToolError as exc:
        _emit_tool_error(str(exc))
        return 1


def _write_report(out: pathlib.Path, report: dict, code: int, start: float, timeout: float) -> int:
    _check_deadline(start, timeout)
    payload = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
    if len(payload) > MAX_REPORT_BYTES:
        _emit_tool_error(f"bound exceeded: report {len(payload)} bytes > {MAX_REPORT_BYTES}")
        return 1
    try:
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(payload)
    except OSError as exc:
        _emit_tool_error(f"cannot write report {out}: {exc.strerror or exc}")
        return 1
    sys.stdout.write(payload.decode("utf-8"))
    return code


if __name__ == "__main__":
    raise SystemExit(main())

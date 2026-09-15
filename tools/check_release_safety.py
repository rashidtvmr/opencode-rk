#!/usr/bin/env python3
"""REL-003: safety-and-resource-correctness release validator.

Additive release-assurance tool. Consumes read-only security/resource gate
outputs plus capability and quota fixtures and emits a deterministic
pass/fail report proving the candidate preserved capability-based safety
and bounded-resource behavior at release time.

Contract (tasks/REL-003.md):
  python3 tools/check_release_safety.py --revision <rev> --gates <dir> \\
      --caps <caps.json> --quotas <quotas.json> --out <report.json>
  exit 0 pass, exit 2 assurance failure, exit 1 tool error (no report).

Partition: safety-resource-correctness-release-validator. Consumes gate
outputs only; never re-owns SEC-017 control semantics, DISC-008 inventory,
or the REL-002 TDD-proof guarantee (TDD-only input => exit 2
unverified-release-state, never pass on file parity alone).

Stdlib only. Single process, no threads, no network, no wall-clock in the
verdict. Reads gates/caps/quotas read-only; writes only <out>. Bounded:
256 files / 8 MiB inputs, 64 KiB report. Never logs secret bytes.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import time

PARTITION = "safety-resource-correctness-release-validator"
MAX_FILES = 256
MAX_TOTAL_BYTES = 8 * 1024 * 1024
MAX_REPORT_BYTES = 64 * 1024
MAX_ENTRIES = 4096

PASS = "pass"
FAIL = "fail"

# Gate receipts owned by this partition. Anything else (e.g. REL-002
# red/green TDD receipts) is not safety/resource evidence.
SAFETY_RECEIPTS = ("mandatory.json", "side_effects.json")
RESOURCE_RECEIPTS = ("resources.json", "retention.json")


class ToolError(Exception):
    """Malformed/unreadable inputs, bound violations, timeout. Exit 1."""


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="check_release_safety.py",
        description="REL-003 safety/resource release validator.",
    )
    # Unknown flags are tool errors (exit 1), not assurance failures.
    parser.error = lambda message: (_emit_tool_error(message), sys.exit(1))  # type: ignore[method-assign]
    parser.add_argument("--revision", required=True)
    parser.add_argument("--gates", required=True)
    parser.add_argument("--caps", required=True)
    parser.add_argument("--quotas", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser.parse_args(argv)


def _emit_tool_error(message: str) -> None:
    sys.stderr.write(f"check_release_safety: tool-error: {message}\n")


def _load_json(path: pathlib.Path, what: str) -> object:
    try:
        size = path.stat().st_size
    except OSError as exc:
        raise ToolError(f"unreadable {what}: {path}: {exc.strerror or exc}") from exc
    if size > MAX_TOTAL_BYTES:
        raise ToolError(f"bound exceeded: {what} {path} {size} bytes > {MAX_TOTAL_BYTES}")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ToolError(f"unreadable {what}: {path}: {exc.strerror or exc}") from exc
    try:
        return json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError) as exc:
        raise ToolError(f"malformed {what}: {path}: {exc}") from exc


def _load_json_capped(path: pathlib.Path, what: str, budget: list[int]) -> object:
    """Bounded single-file load; deducts bytes from the shared input budget."""
    try:
        size = path.stat().st_size
    except OSError as exc:
        raise ToolError(f"unreadable {what}: {path}: {exc.strerror or exc}") from exc
    if size > MAX_TOTAL_BYTES:
        raise ToolError(f"bound exceeded: {what} {path} {size} bytes > {MAX_TOTAL_BYTES}")
    budget[0] += size
    if budget[0] > MAX_TOTAL_BYTES:
        raise ToolError(f"bound exceeded: input bytes > {MAX_TOTAL_BYTES}")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ToolError(f"unreadable {what}: {path}: {exc.strerror or exc}") from exc
    try:
        return json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError) as exc:
        raise ToolError(f"malformed {what}: {path}: {exc}") from exc


def _check_deadline(start: float, timeout: float) -> None:
    if timeout > 0 and (time.monotonic() - start) > timeout:
        raise ToolError("timeout: --timeout exceeded, aborting")


def _read_gates(
    gates_dir: pathlib.Path, start: float, timeout: float, budget: list[int]
) -> dict[str, object]:
    if not gates_dir.is_dir():
        raise ToolError(f"unreadable gates: {gates_dir}: not a directory")
    try:
        children = sorted(gates_dir.iterdir())
    except OSError as exc:
        raise ToolError(f"unreadable gates: {gates_dir}: {exc.strerror or exc}") from exc
    files = [c for c in children if c.is_file() and not c.is_symlink()]
    if len(files) > MAX_FILES:
        raise ToolError(f"bound exceeded: {len(files)} gate files > {MAX_FILES}")
    receipts: dict[str, object] = {}
    for child in files:
        _check_deadline(start, timeout)
        try:
            size = child.stat().st_size
        except OSError as exc:
            raise ToolError(f"unreadable gate output: {child}: {exc.strerror or exc}") from exc
        budget[0] += size
        if budget[0] > MAX_TOTAL_BYTES:
            raise ToolError(f"bound exceeded: input bytes > {MAX_TOTAL_BYTES}")
        if child.suffix == ".json":
            receipts[child.name] = _load_json(child, "gate output")
    return receipts


def main(argv: list[str] | None = None) -> int:
    start = time.monotonic()
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    gates_dir = pathlib.Path(args.gates)
    caps_path = pathlib.Path(args.caps)
    quotas_path = pathlib.Path(args.quotas)
    out_path = pathlib.Path(args.out)

    try:
        budget = [0]
        receipts = _read_gates(gates_dir, start, args.timeout, budget)
        caps = _load_json_capped(caps_path, "capability fixture", budget)
        quotas = _load_json_capped(quotas_path, "quota fixture", budget)
        if not isinstance(caps, dict):
            raise ToolError(f"malformed capability fixture: {caps_path}: top level must be object")
        if not isinstance(quotas, dict):
            raise ToolError(f"malformed quota fixture: {quotas_path}: top level must be object")
        _check_deadline(start, args.timeout)

        # Distinct-slice guard: TDD-only / blind-translation input carries no
        # safety/resource gate receipts, so the release state is unverified.
        # Never pass on file parity or TDD proof alone (not REL-002).
        if not any(name in receipts for name in (*SAFETY_RECEIPTS, *RESOURCE_RECEIPTS)):
            report = {
                "passed": False,
                "checks": {
                    "mandatory_intact": FAIL,
                    "denied_no_side_effects": FAIL,
                    "bytes_bounded": FAIL,
                    "quotas_enforced": FAIL,
                },
                "failing_fixture": str(gates_dir),
                "evidence_rev": args.revision,
                "partition": PARTITION,
                "reason": "unverified-release-state",
            }
            return _write_report(out_path, report, 2, start, args.timeout, secrets=_secrets_of(caps))

        mandatory = receipts.get("mandatory.json")
        side_effects = receipts.get("side_effects.json")
        resources = receipts.get("resources.json")
        retention = receipts.get("retention.json")

        # mandatory_intact: every mandatory control denial held, even under a
        # broad "*" grant (SEC-017 semantics consumed, never re-owned).
        bypass_control: str | None = None
        controls = mandatory.get("controls") if isinstance(mandatory, dict) else None
        if isinstance(controls, list) and controls:
            if len(controls) > MAX_ENTRIES:
                raise ToolError(f"bound exceeded: {len(controls)} controls > {MAX_ENTRIES}")
            mandatory_ok = True
            for entry in controls:
                _check_deadline(start, args.timeout)
                if not isinstance(entry, dict) or not isinstance(entry.get("control"), str):
                    raise ToolError(f"malformed gate output: {gates_dir / 'mandatory.json'}: bad control entry")
                if entry.get("denied") is not True or entry.get("bypassed_by"):
                    mandatory_ok = False
                    bypass_control = entry["control"]
                    break
        else:
            mandatory_ok = False
        mandatory_fixture = str(gates_dir / "mandatory.json") + (f"#{bypass_control}" if bypass_control else "")

        # denied_no_side_effects: each denied op left an empty fs/proc diff
        # and logged zero secret bytes.
        bad_op: str | None = None
        denied_ops = side_effects.get("denied_ops") if isinstance(side_effects, dict) else None
        if isinstance(denied_ops, list) and denied_ops:
            if len(denied_ops) > MAX_ENTRIES:
                raise ToolError(f"bound exceeded: {len(denied_ops)} denied ops > {MAX_ENTRIES}")
            side_ok = True
            for op in denied_ops:
                _check_deadline(start, args.timeout)
                if not isinstance(op, dict) or not isinstance(op.get("op"), str):
                    raise ToolError(f"malformed gate output: {gates_dir / 'side_effects.json'}: bad op entry")
                if op.get("denied") is not True or op.get("fs_diff") \
                        or op.get("proc_started") is True or op.get("secret_bytes_logged"):
                    side_ok = False
                    bad_op = op["op"]
                    break
        else:
            side_ok = False
        side_fixture = str(gates_dir / "side_effects.json") + (f"#{bad_op}" if bad_op else "")

        # bytes_bounded: retained bytes within the explicit quota.
        quota_bytes = None
        for source in (resources, quotas):
            if isinstance(source, dict):
                value = source.get("quota_bytes" if source is resources else "max_bytes")
                if isinstance(value, int) and not isinstance(value, bool):
                    quota_bytes = value if quota_bytes is None else min(quota_bytes, value)
        retained = resources.get("retained_bytes") if isinstance(resources, dict) else None
        bytes_ok = (
            isinstance(retained, int) and not isinstance(retained, bool)
            and quota_bytes is not None and retained >= 0 and retained <= quota_bytes
        )

        # quotas_enforced: queue counts, bytes and retention policy all held;
        # precious history never silently deleted (archival receipt required).
        max_queue = quotas.get("max_queue") if isinstance(quotas, dict) else None
        queue_quota = resources.get("queue_quota") if isinstance(resources, dict) else None
        queue_count = resources.get("queue_count") if isinstance(resources, dict) else None
        queue_cap = None
        for value in (max_queue, queue_quota):
            if isinstance(value, int) and not isinstance(value, bool):
                queue_cap = value if queue_cap is None else min(queue_cap, value)
        queue_ok = (
            isinstance(queue_count, int) and not isinstance(queue_count, bool)
            and queue_cap is not None and 0 <= queue_count <= queue_cap
        )
        retention_ok = False
        retention_bad = False
        if isinstance(retention, dict) and isinstance(retention.get("retention"), dict):
            policy = retention["retention"]
            want = quotas.get("retention_policy") if isinstance(quotas, dict) else None
            precious_deleted = policy.get("precious_deleted") is True
            archived = bool(policy.get("archival_receipt"))
            policy_ok = want is None or policy.get("policy") == want
            retention_ok = policy_ok and not (precious_deleted and not archived)
            retention_bad = precious_deleted and not archived
        quotas_ok = queue_ok and retention_ok
        if not quotas_ok:
            quotas_fixture = str(gates_dir / ("retention.json" if retention_bad or queue_ok else "resources.json"))
        else:
            quotas_fixture = str(gates_dir / "retention.json")

        checks = {
            "mandatory_intact": PASS if mandatory_ok else FAIL,
            "denied_no_side_effects": PASS if side_ok else FAIL,
            "bytes_bounded": PASS if bytes_ok else FAIL,
            "quotas_enforced": PASS if quotas_ok else FAIL,
        }
        passed = all(value == PASS for value in checks.values())

        failing_fixture: str | None = None
        reason: str | None = None
        if not passed:
            if not mandatory_ok:
                failing_fixture = mandatory_fixture
                reason = "mandatory-bypass"
            elif not side_ok:
                failing_fixture = side_fixture
                reason = "denied-side-effect"
            elif not bytes_ok:
                failing_fixture = str(gates_dir / "resources.json")
                reason = "unbounded-retention"
            else:
                failing_fixture = quotas_fixture
                reason = "silent-history-deletion" if retention_bad else "quota-violation"

        report = {
            "passed": passed,
            "checks": checks,
            "failing_fixture": failing_fixture,
            "evidence_rev": args.revision,
            "partition": PARTITION,
            "reason": reason,
        }
        return _write_report(out_path, report, 0 if passed else 2, start, args.timeout, secrets=_secrets_of(caps))
    except ToolError as exc:
        _emit_tool_error(str(exc))
        return 1


def _secrets_of(caps: object) -> list[str]:
    """Canary values that must never appear in the report or logs."""
    secrets: list[str] = []
    if isinstance(caps, dict):
        canary = caps.get("secret_canary")
        if isinstance(canary, str) and canary:
            secrets.append(canary)
    return secrets


def _write_report(
    out: pathlib.Path,
    report: dict,
    code: int,
    start: float,
    timeout: float,
    secrets: list[str] | None = None,
) -> int:
    _check_deadline(start, timeout)
    payload = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
    for secret in secrets or []:
        if secret.encode("utf-8") in payload:
            _emit_tool_error("secret-safety: refusing to emit report containing secret bytes")
            return 1
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

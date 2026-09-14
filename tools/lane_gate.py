#!/usr/bin/env python3
"""Deterministic lane gate for the storage format-2 wave.

Never trust a subagent's own completion message. Each lane declares the file it
owns plus the exact evidence that must exist; this script verifies the artifact
on disk and, for a shrinking set of mandatory lanes, that its Rust test target
compiles and passes. Emits machine-readable JSON so a driver (or a human) can
re-delegate any lane whose status is not PASS.

Usage:
    python3 tools/lane_gate.py            # verify all lanes, print JSON + summary
    python3 tools/lane_gate.py --json     # JSON only
    python3 tools/lane_gate.py --run      # also run cargo test for --test lanes

Exit code is 0 only when every lane is PASS.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
STORAGE = ROOT / "crates" / "storage"

# A lane is a unit of delegated work: exactly one owned file plus the
# verification that must hold. `module` lanes need a non-stub Rust file with a
# public API. `test` lanes need a Rust test file whose harness passes.
LANES: list[dict] = [
    {"id": "gc_v2", "kind": "module", "owner": "muse-spark", "path": "crates/storage/src/gc_v2.rs", "markers": ["pub fn claim_unreferenced_for_deletion", "pub fn finish_deletion", "pub fn prune_outbox_prefix", "pub fn retention_counts"]},
    {"id": "admission_v2", "kind": "module", "owner": "muse-spark", "path": "crates/storage/src/admission_v2.rs", "markers": ["pub fn submit_input", "pub fn promote_input", "pub fn receipt_lookup", "pub fn receipt_store"]},
    {"id": "execution_v2", "kind": "module", "owner": "muse-spark", "path": "crates/storage/src/execution_v2.rs", "markers": ["pub fn start_execution", "pub fn transition_execution", "pub fn record_attempt", "pub fn finish_attempt", "pub fn plan_tool", "pub fn finish_tool"]},
    {"id": "approvals_v2", "kind": "module", "owner": "muse-spark", "path": "crates/storage/src/approvals_v2.rs", "markers": ["pub fn request", "pub fn resolve", "pub fn expire_sweep", "pub fn add_resource"]},
    {"id": "snapshot_v2", "kind": "module", "owner": "muse-spark", "path": "crates/storage/src/snapshot_v2.rs", "markers": ["pub fn open_epoch", "pub fn close_epoch", "pub fn checkpoint", "pub fn pin", "pub fn unpin", "pub fn export_page", "pub fn outbox_page"]},
    {"id": "import_v2", "kind": "module", "owner": "gonkagate", "path": "crates/storage/src/import_v2.rs", "markers": ["pub fn import_session", "pub fn verify_counts", "pub fn import_is_resumable"]},
    {"id": "quota_v2", "kind": "module", "owner": "gonkagate", "path": "crates/storage/src/quota_v2.rs", "markers": ["pub fn measure", "pub fn admit", "pub fn reclaim"]},
    {"id": "retention_v2", "kind": "module", "owner": "gonkagate", "path": "crates/storage/src/retention_v2.rs", "markers": ["pub fn sweep_expired_receipts", "pub fn sweep_resolved_approvals", "pub fn sweep_orphan_inline_payloads", "pub fn retention_backlog"]},
    {"id": "test:integration_v2", "kind": "test", "owner": "cf-glm47", "target": "integration_v2", "path": "crates/storage/tests/integration_v2.rs", "min_tests": 2},
    {"id": "test:import_v2", "kind": "test", "owner": "tr-glm-5-3", "target": "import_v2", "path": "crates/storage/tests/import_v2.rs", "min_tests": 4},
    {"id": "test:quota_v2", "kind": "test", "owner": "vyce-dsv4", "target": "quota_v2", "path": "crates/storage/tests/quota_v2.rs", "min_tests": 4},
    {"id": "test:perf_modules_v2", "kind": "test", "owner": "bai-qwen38", "target": "perf_modules_v2", "path": "crates/storage/tests/perf_modules_v2.rs", "min_tests": 4},
]


def check_module(lane: dict) -> dict:
    path = ROOT / lane["path"]
    if not path.is_file():
        return {"status": "MISSING", "detail": "file does not exist"}
    text = path.read_text(encoding="utf-8")
    if "filled by a dedicated lane" in text or len(text.splitlines()) < 20:
        return {"status": "STUB", "detail": f"{len(text.splitlines())} lines"}
    missing = [m for m in lane["markers"] if m not in text]
    if missing:
        return {"status": "INCOMPLETE", "detail": f"missing API: {', '.join(missing)}"}
    return {"status": "PASS", "detail": f"{len(text.splitlines())} lines, all markers present"}


def parse_test_count(output: str) -> int | None:
    for line in output.splitlines():
        if line.startswith("test result:"):
            for token in line.replace(";", " ").split():
                if token.isdigit():
                    return int(token)
    return None


def check_test(lane: dict, run: bool) -> dict:
    path = ROOT / lane["path"]
    if not path.is_file():
        return {"status": "MISSING", "detail": "test file does not exist"}
    if not run:
        return {"status": "UNRUN", "detail": "pass --run to execute the harness"}
    proc = subprocess.run(
        ["cargo", "test", "-p", "opencode-rk-storage", "--test", lane["target"]],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    output = proc.stdout + proc.stderr
    if proc.returncode != 0:
        tail = "\n".join(output.strip().splitlines()[-6:])
        return {"status": "FAIL", "detail": tail}
    count = parse_test_count(output)
    if count is None or count < lane["min_tests"]:
        return {"status": "FAIL", "detail": f"only {count} tests, need >= {lane['min_tests']}"}
    return {"status": "PASS", "detail": f"{count} tests"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--run", action="store_true", help="execute cargo test for test lanes")
    args = parser.parse_args()

    results = []
    for lane in LANES:
        record = check_module(lane) if lane["kind"] == "module" else check_test(lane, args.run)
        results.append({"lane": lane["id"], "owner": lane["owner"], **record})

    if not args.json:
        for r in results:
            print(f"{r['status']:10} {r['lane']:24} {r['detail']}")
    else:
        print(json.dumps(results, indent=2))

    failed = [r for r in results if r["status"] != "PASS"]
    if failed and not args.json:
        print(f"\n{len(failed)} lane(s) need attention: {', '.join(r['lane'] for r in failed)}")
        print("Re-delegate a failed lane to an allowed worker (see AGENTS.md subagent policy).")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())

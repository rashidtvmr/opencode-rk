#!/usr/bin/env python3
"""Capture one deterministic fixture through the upstream and Rust adapters.

This is deliberately a small, side-effect-free PAR-03 driver.  It keeps the
two renderer commands independent, but sends the exact same request to both
and compares the portable screen envelope (rather than ANSI bytes).
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


def run(argv: list[str], request: dict, cwd: Path) -> dict:
    process = subprocess.run(
        argv,
        cwd=cwd,
        input=json.dumps(request),
        text=True,
        capture_output=True,
        check=False,
    )
    if process.returncode:
        raise RuntimeError(f"{argv[0]} exited {process.returncode}: {process.stderr.strip()}")
    try:
        result = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{argv[0]} did not emit JSON: {error}") from error
    if result.get("schema") != "oc2.ui-parity.screen/v1":
        raise RuntimeError(f"{argv[0]} emitted unexpected schema")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--rust-bin", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--upstream", type=Path, default=None)
    args = parser.parse_args()

    fixture = json.loads(args.fixture.read_text(encoding="utf-8"))
    viewport = fixture["viewport"]
    request = {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": fixture,
        "fixture_class": fixture.get("class", "baseline"),
        "viewport": {"cols": int(viewport["columns"]), "rows": int(viewport["rows"])},
        "theme": fixture.get("theme", {"name": "opencode", "mode": "dark"}),
        "pins": {"upstream_commit": "450e7b92412c780a7b5a75226bb8d49aaca24e22",
                 "opentui_commit": "8d6065a3e60039d16c6ee697692dcfbc46d3c786"},
    }
    wrapper = args.repo / "tools/ui_parity/renderers/upstream_opentui.py"
    upstream = args.upstream or Path("/Users/mymac/Projects/.oc2-features-20261103/artifacts/upstream-opentui068")
    up = run([sys.executable, str(wrapper), "--oracle", str(upstream), "--component"], request, args.repo)
    rust = run([sys.executable, str(args.repo / "tools/ui_parity/renderers/oc2_pty.py"), "--bin", str(args.rust_bin)], request, args.repo)
    comparable = ("viewport", "default_style", "rows", "cursor", "focus", "hit_targets", "actions")
    diffs = [key for key in comparable if up.get(key) != rust.get(key)]
    result = {"schema": "oc2.ui-parity.par03/v1", "fixture_id": fixture["id"],
              "upstream": up, "rust": rust, "diff_keys": diffs,
              "identical": not diffs}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=1, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"fixture_id": fixture["id"], "identical": not diffs, "diff_keys": diffs}))
    return 0 if not diffs else 1


if __name__ == "__main__":
    raise SystemExit(main())

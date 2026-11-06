#!/usr/bin/env python3
"""Run the pinned OpenCode/OpenTUI oracle capture adapter.

The adapter emits the same ``oc2.ui-parity.screen/v1`` protocol as the local
oc2 renderers, but sources its styled cells from the *pinned upstream* app
mounted headlessly through OpenTUI's ``createTestRenderer``/``captureSpans``.
That lets the comparator diff upstream vs oc2 at the same OpenTUI version.

The capture script lives in this repository
(``tools/ui_parity/renderers/upstream_capture.ts``). Because it imports bare
``@opentui/*`` packages and the upstream app fixture by relative path, it must
be executed from inside the frozen upstream workspace root, so this wrapper
copies it there (as a dot-prefixed scratch file) before running ``bun``.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = HERE.parents[3]
# In-repo capture adapter (the source of truth, committed with the repo).
CAPTURE = HERE.with_name("upstream_capture.ts")
ORACLE = Path("/Users/mymac/Projects/.oc2-features-20261103/artifacts/upstream-opentui068")
# The capture script runs from packages/tui/ because that is where the pinned
# workspace links @opentui/core (and siblings) into node_modules.
STAGE_DIR = "packages/tui"
# Scratch name used inside the oracle workspace; overwritten on every run.
STAGED_NAME = ".oc2-upstream-capture.ts"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle", default=str(ORACLE))
    parser.add_argument("--capture", default=str(CAPTURE))
    # Anything else (e.g. --synthetic) is forwarded verbatim to the capture
    # script so callers can exercise the adapter without a full app mount.
    args, extra = parser.parse_known_args()

    oracle = Path(args.oracle)
    capture = Path(args.capture)
    if not oracle.is_dir():
        sys.stderr.write(f"oracle workspace not found: {oracle}\n")
        return 2
    if not capture.is_file():
        sys.stderr.write(f"capture adapter not found: {capture}\n")
        return 2

    stage_dir = oracle / STAGE_DIR
    if not stage_dir.is_dir():
        sys.stderr.write(f"oracle stage dir not found: {stage_dir}\n")
        return 2
    staged = stage_dir / STAGED_NAME
    shutil.copyfile(capture, staged)

    request = json.load(sys.stdin)
    env = os.environ.copy()
    env.setdefault("NO_COLOR", "1")
    proc = subprocess.run(
        ["bun", "run", f"{STAGE_DIR}/{STAGED_NAME}", *extra],
        cwd=str(oracle),
        input=json.dumps(request),
        text=True,
        capture_output=True,
        env=env,
    )
    if proc.returncode:
        sys.stderr.write(proc.stderr)
        return proc.returncode
    sys.stdout.write(proc.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

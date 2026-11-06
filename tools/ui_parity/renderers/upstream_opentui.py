#!/usr/bin/env python3
"""Run the pinned OpenCode/OpenTUI oracle capture adapter."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = HERE.parents[3]
ORACLE = Path("/Users/mymac/Projects/.oc2-features-20261103/artifacts/upstream-opentui068")
CAPTURE = Path("/Users/mymac/Projects/.oc2-features-20261103/artifacts/upstream-capture/capture.ts")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle", default=str(ORACLE))
    parser.add_argument("--capture", default=str(CAPTURE))
    args = parser.parse_args()
    request = json.load(sys.stdin)
    env = os.environ.copy()
    env.setdefault("NO_COLOR", "1")
    proc = subprocess.run(
        ["bun", "run", args.capture],
        cwd=args.oracle,
        input=json.dumps(request), text=True, capture_output=True, env=env,
    )
    if proc.returncode:
        sys.stderr.write(proc.stderr)
        return proc.returncode
    sys.stdout.write(proc.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

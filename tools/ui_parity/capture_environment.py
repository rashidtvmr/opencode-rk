#!/usr/bin/env python3
"""Print and validate the deterministic environment used by the parity PTY.

This intentionally reports logical terminal cells, not raster pixels.  The
PTY renderer has no access to the emulator's font or DPI, so those values are
explicitly represented as unknown rather than guessed.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys


EXPECTED = {
    "TERM": "xterm-256color",
    "COLORTERM": "truecolor",
    "LANG": "C.UTF-8",
    "LC_ALL": "C.UTF-8",
    "TZ": "UTC",
}


def host_terminal() -> dict[str, str]:
    return {
        "emulator": os.environ.get("TERM_PROGRAM", "unknown"),
        "emulator_version": os.environ.get("TERM_PROGRAM_VERSION", "unknown"),
        "font_family": "unknown (PTY capture does not expose raster font)",
        "font_size_pt": "unknown (PTY capture does not expose raster font)",
        "cell_width_px": "unknown (logical-cell capture)",
        "cell_height_px": "unknown (logical-cell capture)",
        "scale_factor": "unknown (logical-cell capture)",
    }


def report() -> dict[str, object]:
    return {
        "schema": "oc2.ui-parity.capture-environment/v1",
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "terminal": host_terminal(),
        "logical_cells": {"width": 1, "height": 1},
        "renderer_environment": {key: os.environ.get(key) for key in EXPECTED},
        "font_and_pixel_note": (
            "Font, DPI, and pixel cell dimensions are not observable through the "
            "isolated PTY; do not use this report as pixel evidence."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="require the PTY baseline")
    args = parser.parse_args()
    payload = report()
    print(json.dumps(payload, indent=2, sort_keys=True))
    if not args.check:
        return 0
    mismatches = {
        key: (os.environ.get(key), expected)
        for key, expected in EXPECTED.items()
        if os.environ.get(key) != expected
    }
    if mismatches:
        print("baseline mismatch: " + json.dumps(mismatches, sort_keys=True), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

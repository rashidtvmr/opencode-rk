#!/usr/bin/env python3
"""Renderer adapter: OC2 `--once` bounded text frame (no color styling).

This is the deterministic fallback when the native renderer cannot be launched.
It runs the real binary under a PTY at the fixture viewport, feeds the plain
text frame through the same screen model, and marks the capture as unstyled so a
styled-cell golden cannot silently pass against it.
"""

from __future__ import annotations

import argparse
import json
import os
import pty
import select
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from oc2_pty import isolated_env, resolve_binary, sha256_file  # noqa: E402
from vt_screen import Screen  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--max-ms", type=float, default=20000)
    args = parser.parse_args()
    request = json.load(sys.stdin)
    binary = resolve_binary(args.bin)
    if not binary.exists():
        print(f"error: OC2 binary not found: {binary}", file=sys.stderr)
        return 2
    cols = int(request["viewport"]["cols"])
    rows = int(request["viewport"]["rows"])
    root = Path(tempfile.mkdtemp(prefix="oc2-ui-parity-once-"))
    master = None
    proc = None
    try:
        env, work = isolated_env(request, root)
        screen = Screen(cols, rows)
        master, slave = pty.openpty()
        import fcntl
        import struct
        import termios

        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        proc = subprocess.Popen(
            [str(binary), "--once"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            cwd=str(work),
            env=env,
            start_new_session=True,
            close_fds=True,
        )
        os.close(slave)
        deadline = time.monotonic() + args.max_ms / 1000.0
        chunks = []
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], 0.05)
            if ready:
                try:
                    data = os.read(master, 1 << 16)
                except OSError:
                    data = b""
                if data:
                    chunks.append(data)
                    screen.feed(data)
            if proc.poll() is not None:
                break
        if not chunks:
            raise RuntimeError("OC2 --once produced no output")
        screen.visible = False
        snapshot = screen.snapshot()
        version = ""
        try:
            result = subprocess.run(
                [str(binary), "--version"],
                capture_output=True,
                text=True,
                timeout=15,
                env=env,
                cwd=str(work),
            )
            version = (result.stdout or "").strip().splitlines()[0] if result.stdout else ""
        except Exception:
            version = ""
        theme = dict(request.get("theme") or {})
        theme["mode"] = "unstyled"
        theme["source"] = "once_plain_text_has_no_color"
        snapshot.update(
            {
                "schema": "oc2.ui-parity.screen/v1",
                "fixture_id": request["fixture"]["id"],
                "theme": theme,
                "focus": {
                    "value": None,
                    "source": "once_plain_text_has_no_focus_channel",
                },
                "actions": [],
                "environment": {
                    "adapter": "oc2-once",
                    "styled": False,
                    "product": {
                        "name": "opencode-rk",
                        "version": version,
                        "sha256": sha256_file(binary),
                        "executable": binary.name,
                    },
                    "terminal": {
                        "term": env.get("TERM"),
                        "cols": cols,
                        "rows": rows,
                        "platform": sys.platform,
                    },
                    "captured_with": [str(binary), "--once"],
                },
            }
        )
        json.dump(snapshot, sys.stdout, ensure_ascii=False, sort_keys=True)
        sys.stdout.write("\n")
        return 0
    except Exception as error:  # noqa: BLE001
        print(f"error: {error}", file=sys.stderr)
        return 1
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait(timeout=3)
        if master is not None:
            try:
                os.close(master)
            except OSError:
                pass
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())

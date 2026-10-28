"""POSIX owner-death guardian; process supervision, not a hostile-code sandbox."""
from __future__ import annotations
import contextlib
import json
import os
import select
import signal
import subprocess
import sys


def terminate(proc, first_signal):
    with contextlib.suppress(ProcessLookupError):
        os.killpg(proc.pid, first_signal)
    try:
        proc.wait(timeout=0.8)
    except subprocess.TimeoutExpired:
        pass
    # Darwin can report EPERM for a group already dying after TERM.
    with contextlib.suppress(ProcessLookupError, PermissionError):
        os.killpg(proc.pid, signal.SIGKILL)
    with contextlib.suppress(subprocess.TimeoutExpired):
        proc.wait(timeout=0.8)


def main():
    with os.fdopen(int(sys.argv[1]), "r") as spec_file:
        spec = json.loads(spec_file.read(2097152))
    argv = spec["argv"]
    first_signal = int(spec["cancel_signal"])
    if (not isinstance(argv, list) or not argv or not all(isinstance(x, str) for x in argv)
            or first_signal not in {signal.SIGINT, signal.SIGTERM}):
        raise ValueError("invalid trusted command specification")
    stopped = False
    def stop(_signal, _frame):
        nonlocal stopped
        stopped = True
    signal.signal(signal.SIGINT, stop); signal.signal(signal.SIGTERM, stop)
    readable, _, _ = select.select([sys.stdin.buffer], [], [], 0)
    if stopped or (readable and not os.read(sys.stdin.fileno(), 1)):
        return 125
    proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, start_new_session=True)
    try:
        while proc.poll() is None:
            readable, _, _ = select.select([sys.stdin.buffer], [], [], 0.02)
            if stopped or (readable and not os.read(sys.stdin.fileno(), 1)):
                return 125
        return proc.returncode if proc.returncode >= 0 else 128-proc.returncode
    finally:
        terminate(proc, first_signal)


if __name__ == "__main__":
    raise SystemExit(main())

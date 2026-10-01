#!/usr/bin/env python3
"""NATIVE-RESIZE: an installed native CLI must follow a live PTY resize.

This is intentionally a real-PTY contract.  The kernel window size is changed
with TIOCSWINSZ while the application is idle; no COLUMNS/LINES environment
change, input event, or provider request is used as a resize substitute.
"""
import argparse
import fcntl
import json
import os
import pathlib
import pty
import re
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX,
    checked_artifact,
    descriptor,
    free_loopback_port,
    read_until,
    stop_owned_daemon,
)
from native_stream_tool import (  # noqa: E402
    ASSISTANT_FIRST,
    FINAL,
    FIRST,
    Handler,
    Fixture,
    MARKER,
    MODEL,
    PARTIAL,
    ProviderState,
    SECOND,
    SECOND_FINAL,
    reap_cli_then_daemon,
)

MAX_CAPTURE = 256 * 1024
TIMEOUT = 20
ROWS, COLS = 24, 80
SHRUNK_ROWS, SHRUNK_COLS = 8, 60


def sha256(path):
    import hashlib
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resize(fd, rows, cols):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))


def cursor_rows(data):
    """Return explicit CUP/HVP rows in one repaint, ignoring erase controls."""
    rows = []
    for match in re.finditer(rb"\x1b\[([0-9]+);([0-9]+)[Hf]", data):
        rows.append(int(match.group(1)))
    return rows


def run(binary, library, manifest, artifacts):
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    root = pathlib.Path(tempfile.mkdtemp(prefix="nresize-", dir=os.environ.get("TMPDIR", "/tmp")))
    install = root / "install"
    (install / "bin").mkdir(parents=True)
    (install / "lib").mkdir()
    shutil.copyfile(binary, install / "bin" / "oc2")
    shutil.copyfile(library, install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX))
    os.chmod(install / "bin" / "oc2", 0o755)
    for name in ("home", "project", "data"):
        (root / name).mkdir()
    (root / "data" / "runtime").mkdir()
    key = "fixture-stream-generated-5e7c"
    auth = root / "data" / "opencode" / "auth.json"
    auth.parent.mkdir()
    auth.write_text(json.dumps({"openai": {"type": "api", "key": key}}) + "\n")
    os.chmod(auth, 0o600)
    catalog = root / "data" / "catalog" / "models.dev.api.json"
    catalog.parent.mkdir()
    catalog.write_text(json.dumps({"openai": {"name": "fixture", "models": {
        MODEL: {"name": "resize fixture", "limit": {"context": 200000}},
    }}}) + "\n")
    state = ProviderState(root / "project")
    server = Fixture(("127.0.0.1", 0), Handler)
    server.state = state
    import threading
    provider_thread = threading.Thread(target=server.serve_forever, daemon=False)
    provider_thread.start()
    master = slave = child = None
    descriptor_value = None
    captured = bytearray()
    try:
        port = free_loopback_port()
        runtime = root / "data" / "runtime" / "opencode-rk.sock"
        if len(os.fsencode(runtime)) > 100:
            raise AssertionError("fixture Unix socket path exceeds 100 bytes")
        env = {
            "HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""),
            "TERM": "xterm-256color", "TMPDIR": str(root),
            "XDG_CONFIG_HOME": str(root / "home" / ".config"),
            "XDG_DATA_HOME": str(root / "data"), "XDG_RUNTIME_DIR": str(root / "data" / "runtime"),
            "OPENCODE_RK_HOME": str(root / "data"), "OPENCODE_PROJECT_DIR": str(root / "project"),
            "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{port}",
            "OPENAI_BASE_URL": f"http://127.0.0.1:{server.server_port}/v1",
            "OPENCODE_RK_TURN_TOOLS": "write",
        }
        (root / "home" / ".config").mkdir()
        master, slave = pty.openpty()
        resize(slave, ROWS, COLS)
        child = subprocess.Popen([str(install / "bin" / "oc2"), "--data-dir", str(root / "data")],
                                 cwd=root / "project", env=env, stdin=slave, stdout=slave,
                                 stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        descriptor_value = descriptor(root / "data", port, child, time.monotonic() + TIMEOUT)
        read_until(master, time.monotonic() + TIMEOUT, captured, b"OpenCode", 0)
        os.write(master, (FIRST + "\r").encode())
        read_until(master, time.monotonic() + TIMEOUT, captured, PARTIAL.encode(), 0)
        state.partial_visible.set(); state.release_partial.set()
        read_until(master, time.monotonic() + TIMEOUT, captured, FINAL.encode(), 0)
        os.write(master, (SECOND + "\r").encode())
        read_until(master, time.monotonic() + TIMEOUT, captured, SECOND_FINAL.encode(), 0)
        if state.error or len(state.requests) != 2:
            raise AssertionError(state.error or f"expected exactly two provider requests, got {len(state.requests)}")

        # The application is idle here.  This ioctl is the observable user
        # action; it must cause a native repaint constrained to the new frame.
        before = len(captured)
        resize(master, SHRUNK_ROWS, SHRUNK_COLS)
        # A resize event is asynchronous; bounded draining must observe a fresh
        # full repaint, not merely the pre-resize transcript.
        # Need terminal-specific fresh paint, so wait for any new output and
        # analyze the whole bounded repaint rather than assuming one CSI order.
        import select
        deadline = time.monotonic() + TIMEOUT
        while time.monotonic() < deadline and len(captured) < MAX_CAPTURE:
            ready, _, _ = select.select([master], [], [], min(.1, deadline - time.monotonic()))
            if not ready:
                continue
            chunk = os.read(master, min(65536, MAX_CAPTURE - len(captured)))
            if not chunk:
                break
            captured.extend(chunk)
            if len(captured) > before and b"\x1b[2J" in bytes(captured[before:]):
                break
        repaint = bytes(captured[before:])
        rows = cursor_rows(repaint)
        if not rows:
            raise AssertionError("resize produced no addressable native repaint")
        if max(rows) > SHRUNK_ROWS:
            raise AssertionError(f"native repaint used row {max(rows)} after {SHRUNK_ROWS}-row PTY resize")
        if state.error or len(state.requests) != 2:
            raise AssertionError("idle resize submitted provider input")
        if not (b"assistant:" in repaint or b"completed" in repaint or b">" in repaint):
            raise AssertionError("resized frame did not retain the session/composer surface")
        result = {
            "phase": "candidate-observed",
            "source_sha": source, "binary_sha256": binary_sha, "native_library_sha256": library_sha,
            "provider_requests": len(state.requests), "initial_geometry": [COLS, ROWS],
            "resized_geometry": [SHRUNK_COLS, SHRUNK_ROWS], "repaint_cursor_rows": rows,
        }
        artifacts.mkdir(parents=True, exist_ok=True)
        (artifacts / "native-resize-result.json").write_text(json.dumps(result, indent=2) + "\n")
    finally:
        state.partial_visible.set(); state.release_partial.set()
        if child is not None:
            try:
                reap_cli_then_daemon(child, descriptor_value, master)
            except (OSError, RuntimeError, AssertionError):
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=3)
        if slave is not None:
            os.close(slave)
        if master is not None:
            os.close(master)
        server.shutdown(); server.server_close(); provider_thread.join(timeout=10)
        if provider_thread.is_alive():
            raise RuntimeError("fixture server failed bounded shutdown")
        shutil.rmtree(root, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--native-library", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--artifact-dir", required=True)
    args = parser.parse_args()
    run(pathlib.Path(args.binary), pathlib.Path(args.native_library), pathlib.Path(args.build_json), pathlib.Path(args.artifact_dir))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""NATIVE-RESIZE: live kernel PTY geometry must reach the native repaint.

This source-only contract deliberately uses the already-frozen two-turn
fixture from ``native_escape_input``.  It does not change COLUMNS/LINES and it
does not write input when resizing: the only resize stimulus is TIOCSWINSZ.
"""
import argparse
import fcntl
import hashlib
import json
import os
import pathlib
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX, MAX_PTY, TerminalScreen, checked_artifact, descriptor,
    free_loopback_port, read_until, stop_owned_daemon,
)
from native_escape_input import (  # noqa: E402
    FIRST, FIRST_REPLY, KEY, MODEL, SECOND, SECOND_REPLY, Fixture, Handler,
    State, history, send_fragments,
)

MAX_REQUEST, MAX_RESPONSE = 128 * 1024, 256 * 1024
TIMEOUT, TOTAL_TIMEOUT = 20, 45
INITIAL_COLS, INITIAL_ROWS = 80, 24
RESIZED_COLS, RESIZED_ROWS = 60, 8


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def set_winsize(fd, rows, cols):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))


def cup_rows(data):
    """Extract explicit CUP/HVP rows without assuming a particular repaint."""
    return [int(m.group(1)) for m in re.finditer(rb"\x1b\[([0-9]+);[0-9]+[Hf]", data)]


def bounded_resize_read(master, captured, screen, deadline, start):
    """Drain only a bounded post-ioctl frame and require visible new content."""
    while time.monotonic() < deadline and len(captured) < MAX_PTY:
        ready, _, _ = select.select([master], [], [], min(.1, deadline - time.monotonic()))
        if not ready:
            continue
        try:
            chunk = os.read(master, min(65536, MAX_PTY - len(captured)))
        except OSError as exc:
            raise AssertionError(f"PTY read failed after resize: {exc}") from exc
        if not chunk:
            raise AssertionError("PTY EOF before resized native frame")
        captured.extend(chunk)
        screen.feed(chunk)
        if len(captured) > start and screen.contains("OpenCode RK") and screen.contains(">"):
            return
    if len(captured) >= MAX_PTY:
        raise AssertionError("PTY capture bound reached while awaiting resized frame")
    raise AssertionError("PTY timeout awaiting a visible native frame after TIOCSWINSZ")


def run(binary, library, manifest, artifacts):
    started = time.monotonic()
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    build = json.loads(manifest.read_text())
    if build.get("native") is not True or build.get("profile") != "release":
        raise RuntimeError("the contract requires an attested native release")
    for value in (binary, library, manifest, artifacts):
        if not pathlib.Path(value).is_absolute():
            raise RuntimeError("artifact paths must be absolute")

    # Keep the forensic fixture root after the run; its path is included in the
    # evidence even on failure.  TMPDIR is supplied by the parent runner.
    tmpdir = os.environ.get("TMPDIR")
    if not tmpdir:
        raise RuntimeError("TMPDIR must name the parent-approved short forensic directory")
    root = pathlib.Path(tempfile.mkdtemp(prefix="nresize-", dir=tmpdir))
    for name in ("home", "project", "data"):
        (root / name).mkdir()
    runtime = root / "data" / "runtime"
    runtime.mkdir()
    if len(os.fsencode(runtime / "opencode-rk.sock")) > 100:
        raise RuntimeError("fixture Unix socket path exceeds 100 bytes")
    install = root / "install"
    (install / "bin").mkdir(parents=True)
    (install / "lib").mkdir()
    shutil.copyfile(binary, install / "bin" / "oc2")
    shutil.copyfile(library, install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX))
    os.chmod(install / "bin" / "oc2", 0o755)
    auth = root / "data" / "opencode" / "auth.json"
    auth.parent.mkdir()
    auth.write_text(json.dumps({"openai": {"type": "api", "key": KEY}}) + "\n")
    os.chmod(auth, 0o600)
    catalog = root / "data" / "catalog" / "models.dev.api.json"
    catalog.parent.mkdir()
    catalog.write_text(json.dumps({"openai": {"name": "fixture", "models": {
        MODEL: {"name": "Resize fixture", "limit": {"context": 200000}},
        "gpt-5.6-mini": {"name": "Readiness fixture", "limit": {"context": 200000}},
    }}}) + "\n")

    state = State()
    server = Fixture(("127.0.0.1", 0), Handler)
    server.state = state
    provider_thread = threading.Thread(target=server.serve_forever, daemon=False)
    provider_thread.start()
    master = slave = child = None
    descriptor_value = None
    saved_attrs = None
    captured = bytearray()
    initial_screen = TerminalScreen(INITIAL_COLS, INITIAL_ROWS)
    resized_screen = TerminalScreen(RESIZED_COLS, RESIZED_ROWS)
    semantic_error = None
    raw_mode_seen = False
    termios_restored = False
    child_reaped = False
    daemon_gone = False
    provider_joined = False
    forced_kill = False
    secret_echo = False
    messages = []
    resize_observed = False
    resize_rows = []

    try:
        port = free_loopback_port()
        env = {
            "HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""),
            "TERM": "xterm-256color", "TMPDIR": str(root),
            "XDG_CONFIG_HOME": str(root / "home" / ".config"),
            "XDG_DATA_HOME": str(root / "data"), "XDG_STATE_HOME": str(root / "home" / "state"),
            "XDG_CACHE_HOME": str(root / "home" / "cache"), "XDG_RUNTIME_DIR": str(runtime),
            "OPENCODE_RK_HOME": str(root / "data"), "OPENCODE_PROJECT_DIR": str(root / "project"),
            "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{port}",
            "OPENAI_BASE_URL": f"http://127.0.0.1:{server.server_port}/v1",
        }
        for path in (env["XDG_CONFIG_HOME"], env["XDG_STATE_HOME"], env["XDG_CACHE_HOME"]):
            pathlib.Path(path).mkdir(parents=True)
        master, slave = pty.openpty()
        set_winsize(slave, INITIAL_ROWS, INITIAL_COLS)
        os.set_blocking(master, False)
        saved_attrs = termios.tcgetattr(master)
        child = subprocess.Popen([str(install / "bin" / "oc2"), "--data-dir", str(root / "data")],
                                 cwd=root / "project", env=env, stdin=slave, stdout=slave,
                                 stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        descriptor_value = descriptor(root / "data", port, child, time.monotonic() + TIMEOUT)
        if descriptor_value.get("schema_version") != 1:
            raise AssertionError("unexpected daemon descriptor schema")
        if not isinstance(descriptor_value.get("_validated_pid"), int):
            raise AssertionError("descriptor did not validate owned daemon PID")
        token = descriptor_value.get("auth_token", "")
        if not isinstance(token, str) or len(token) != 64 or not re.fullmatch(r"[0-9a-fA-F]{64}", token):
            raise AssertionError("descriptor auth token is not a 64-hex capability")
        read_until(master, time.monotonic() + TIMEOUT, captured, b"OpenCode", 0,
                    initial_screen, lambda: initial_screen.contains("model: openai/" + MODEL))
        raw_mode_seen = not bool(termios.tcgetattr(master)[3] & termios.ICANON)
        send_fragments(master, FIRST.encode() + b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captured, FIRST_REPLY.encode(),
                    len(captured), initial_screen, lambda: initial_screen.contains("assistant: " + FIRST_REPLY))
        send_fragments(master, SECOND.encode() + b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captured, SECOND_REPLY.encode(),
                    len(captured), initial_screen, lambda: initial_screen.contains("assistant: " + SECOND_REPLY))
        messages = history(descriptor_value)
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError(state.error or state.semantic_errors or "expected exactly two provider requests")

        # Idle resize: no byte is written to the PTY and no provider call may
        # result.  The kernel geometry is the sole stimulus.
        before = len(captured)
        set_winsize(master, RESIZED_ROWS, RESIZED_COLS)
        bounded_resize_read(master, captured, resized_screen,
                            min(started + TOTAL_TIMEOUT, time.monotonic() + TIMEOUT), before)
        resize_bytes = bytes(captured[before:])
        resize_rows = cup_rows(resize_bytes)
        if not resize_rows:
            raise AssertionError("resized frame contained no explicit terminal positioning")
        if max(resize_rows) > RESIZED_ROWS:
            raise AssertionError(f"resized frame addressed row {max(resize_rows)} beyond {RESIZED_ROWS}")
        if not resized_screen.contains(">"):
            raise AssertionError("resized native frame did not retain the composer")
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError("idle resize caused provider input")
        resize_observed = True
    except BaseException as exc:
        semantic_error = str(exc)
    finally:
        # Product cleanup is attempted before any descriptor/FD is closed.
        if child is not None and child.poll() is None:
            try:
                send_fragments(master, b"\x03", delay=0)
                child.wait(timeout=10)
            except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
                semantic_error = semantic_error or f"normal CLI reap failed: {exc}"
                forced_kill = True
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=3)
                except (OSError, subprocess.TimeoutExpired) as kill_error:
                    semantic_error = semantic_error or f"forced CLI cleanup failed: {kill_error}"
        child_reaped = child is not None and child.poll() is not None
        if master is not None and saved_attrs is not None:
            try:
                termios_restored = termios.tcgetattr(master) == saved_attrs
            except OSError as exc:
                semantic_error = semantic_error or f"terminal restoration probe failed: {exc}"
        if descriptor_value is not None:
            try:
                stop_owned_daemon(descriptor_value)
            except Exception as exc:
                semantic_error = semantic_error or f"owned daemon stop failed: {exc}"
            try:
                os.kill(descriptor_value["_validated_pid"], 0)
            except ProcessLookupError:
                daemon_gone = True
            except OSError as exc:
                semantic_error = semantic_error or f"owned daemon liveness probe failed: {exc}"
        if master is not None and saved_attrs is not None and not termios_restored:
            # Do not repair the product result here; preserve the failed
            # observation.  The descriptor and PTY remain auditable below.
            pass
        for fd in (slave, master):
            if fd is not None:
                try:
                    os.close(fd)
                except OSError as exc:
                    semantic_error = semantic_error or f"PTY close failed: {exc}"
        server.shutdown(); server.server_close(); provider_thread.join(timeout=10)
        provider_joined = not provider_thread.is_alive()
        if not provider_joined:
            semantic_error = semantic_error or "fixture provider thread did not join"
        secret_echo = KEY.encode() in captured or KEY in initial_screen.text() or KEY in resized_screen.text()
        if secret_echo:
            semantic_error = semantic_error or "fixture API key appeared in terminal output"
        capture_bound = len(captured) <= MAX_PTY
        if not capture_bound:
            semantic_error = semantic_error or "PTY capture bound exceeded"
        success = (semantic_error is None and state.error is None and not state.semantic_errors and
                   len(state.requests) == 2 and len(messages) == 4 and resize_observed and
                   child_reaped and child.returncode == 0 and not forced_kill and daemon_gone and
                   termios_restored and raw_mode_seen and provider_joined and capture_bound and
                   not secret_echo and time.monotonic() - started <= TOTAL_TIMEOUT)
        evidence = {
            "phase": "success" if success else "failed", "error": semantic_error or state.error or "",
            "source_sha": source, "binary_sha256": binary_sha, "native_library_sha256": library_sha,
            "test_sha256": sha256(pathlib.Path(__file__)), "fixture_root": str(root),
            "requests": len(state.requests), "semantic_errors": state.semantic_errors,
            "initial_geometry": [INITIAL_COLS, INITIAL_ROWS], "resized_geometry": [RESIZED_COLS, RESIZED_ROWS],
            "resize_observed": resize_observed, "resize_cursor_rows": resize_rows,
            "termios_restored": termios_restored, "raw_mode_seen": raw_mode_seen,
            "child_reaped": child_reaped, "cli_exit_code": child.returncode if child else None,
            "daemon_gone": daemon_gone, "provider_thread_joined": provider_joined,
            "forced_kill": forced_kill, "secret_echo": secret_echo,
            "captured_bytes": len(captured), "max_capture": MAX_PTY,
        }
        output = json.dumps(evidence, indent=2) + "\n"
        if len(output.encode()) > 512 * 1024:
            raise RuntimeError("evidence exceeded 512 KiB")
        artifacts.mkdir(parents=True, exist_ok=True)
        (artifacts / ("native-resize-result.json" if success else "native-resize-failure.json")).write_text(output)
        if not success:
            raise AssertionError(evidence["error"] or "native resize contract failed")


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

#!/usr/bin/env python3
"""Renderer adapter: launch the real OC2 native TUI in an isolated PTY.

Reads an `oc2.ui-parity.request/v1` JSON document on stdin, replays the
fixture's events, captures the resulting styled cell grid from the emitted
ANSI stream, and writes an `oc2.ui-parity.screen/v1` document on stdout.

Isolation: HOME, XDG_* and OPENCODE_RK_HOME point at a disposable temp tree, so
the user's database, preferences and daemon are never touched.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import platform
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from vt_screen import Screen  # noqa: E402


def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_binary(raw: str) -> Path:
    path = Path(raw).expanduser()
    if not path.is_absolute():
        path = (Path.cwd() / path).resolve()
    return path


def isolated_env(request, root: Path):
    env = dict(os.environ)
    for key in (
        "OPENCODE_RK_HOME",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_CLI_CONFIG_CONTENT",
        "OPENCODE_CONFIG",
        "OPENCODE_CONFIG_CONTENT",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
    ):
        env.pop(key, None)
    home = root / "home"
    config = root / "config"
    data = root / "data"
    work = root / "work"
    tmp = root / "tmp"
    for path in (home, config, data, work, tmp):
        path.mkdir(parents=True, exist_ok=True)
    env.update(
        {
            "HOME": str(home),
            "XDG_CONFIG_HOME": str(config),
            "XDG_DATA_HOME": str(root / "xdg-data"),
            "XDG_STATE_HOME": str(root / "xdg-state"),
            "OPENCODE_RK_HOME": str(data),
            "PWD": str(work),
            "TMPDIR": str(tmp),
            "TMP": str(tmp),
            "TEMP": str(tmp),
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "TZ": "UTC",
        }
    )
    fixture_env = request.get("environment") or {}
    inline = fixture_env.get("inline_cli_config")
    if inline:
        env["OPENCODE_CLI_CONFIG_CONTENT"] = inline
    for key, value in (fixture_env.get("env") or {}).items():
        if isinstance(key, str) and isinstance(value, str):
            env[key] = value
    native_lib = (
        repo_root()
        / "crates"
        / "opentui-bridge"
        / "native"
        / "lib"
        / f"{rust_target_arch()}-apple-darwin"
    )
    if native_lib.is_dir():
        existing = env.get("DYLD_LIBRARY_PATH", "")
        env["DYLD_LIBRARY_PATH"] = str(native_lib) + (":" + existing if existing else "")
    linux_lib = (
        repo_root()
        / "crates"
        / "opentui-bridge"
        / "native"
        / "lib"
        / f"{rust_target_arch()}-unknown-linux-gnu"
    )
    if linux_lib.is_dir():
        existing = env.get("LD_LIBRARY_PATH", "")
        env["LD_LIBRARY_PATH"] = str(linux_lib) + (":" + existing if existing else "")
    return env, work


def rust_target_arch() -> str:
    machine = platform.machine().lower()
    if machine in ("arm64", "aarch64"):
        return "aarch64"
    return machine


KEY_SEQUENCES = {
    "enter": "\r",
    "return": "\r",
    "tab": "\t",
    "esc": "\x1b",
    "escape": "\x1b",
    "backspace": "\x7f",
    "space": " ",
    "up": "\x1b[A",
    "down": "\x1b[B",
    "right": "\x1b[C",
    "left": "\x1b[D",
    "home": "\x1b[H",
    "end": "\x1b[F",
    "pageup": "\x1b[5~",
    "pagedown": "\x1b[6~",
    "delete": "\x1b[3~",
}


def key_sequence(name: str) -> str:
    lowered = name.lower()
    if lowered in KEY_SEQUENCES:
        return KEY_SEQUENCES[lowered]
    if lowered.startswith("ctrl-") and len(lowered) == 6 and "a" <= lowered[5] <= "z":
        return chr(ord(lowered[5]) - ord("a") + 1)
    if len(name) == 1:
        return name
    raise ValueError(f"unsupported key: {name!r}")


def read_until_quiet(master, screen: Screen, proc, min_total: float, quiet: float, max_total: float):
    deadline = time.monotonic() + max_total
    first = None
    last = None
    chunks = []
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        now = time.monotonic()
        if ready:
            try:
                data = os.read(master, 1 << 16)
            except OSError:
                data = b""
            if data:
                screen.feed(data)
                chunks.append(data)
                first = first or now
                last = now
        if proc.poll() is not None and (last is None or now - last > 0.25):
            break
        if first and now - first >= min_total and now - last >= quiet:
            break
    return b"".join(chunks)


def redact_workdir(screen: Screen, token: str) -> None:
    """Replace the environment-specific cwd path shown in the footer.

    The path is a test-fixture artifact, not product UI.  Redacting the whole
    visible path (including a truncation ellipsis and the machine-specific temp
    prefix) keeps the rest of the footer/layout comparable on any machine.
    """
    for row in screen.grid:
        plain = []
        columns = []
        for col, cell in enumerate(row):
            if cell["cont"]:
                continue
            plain.append(cell["ch"])
            columns.append(col)
        text = "".join(plain)
        index = text.find(token)
        if index < 0:
            continue
        start = index
        while start > 0 and text[start - 1] != " ":
            start -= 1
        end = min(index + len(token), len(columns))
        if start >= len(columns):
            continue
        start_col = columns[start]
        end_col = columns[end - 1] + 1
        style = row[start_col]
        fg, bg, attrs = style["fg"], style["bg"], style["attrs"]
        for col in range(start_col, end_col):
            row[col] = {
                "ch": " ",
                "fg": fg,
                "bg": bg,
                "attrs": attrs,
                "cont": False,
            }
        for offset, ch in enumerate("<WORKDIR>"):
            if start_col + offset < end_col:
                row[start_col + offset]["ch"] = ch


def infer_theme_mode(screen: Screen) -> str:
    counts = {}
    for row in screen.grid:
        for cell in row:
            counts[cell["bg"]] = counts.get(cell["bg"], 0) + 1
    background = max(counts, key=counts.get) if counts else "#000000"
    try:
        value = background.lstrip("#")
        red, green, blue = (int(value[index : index + 2], 16) for index in (0, 2, 4))
        luminance = (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255.0
    except ValueError:
        luminance = 0.0
    return "light" if luminance > 0.5 else "dark"


def replay_events(master, screen, proc, events, quiet: float):
    trace = []
    for index, event in enumerate(events or []):
        kind = event.get("type") or "key"
        applied = True
        entry = {"seq": index, "type": kind}
        try:
            if kind == "wait":
                time.sleep(float(event.get("ms", 100)) / 1000.0)
            elif kind == "text":
                text = event.get("text", "")
                entry["text"] = text
                os.write(master, text.encode("utf-8"))
            elif kind == "key":
                key = event.get("key") or event.get("name") or ""
                entry["key"] = key
                sequence = event.get("sequence") or key_sequence(key)
                os.write(master, sequence.encode("utf-8"))
            else:
                applied = False
                entry["reason"] = "unsupported_event_type"
            if kind in ("key", "text"):
                read_until_quiet(master, screen, proc, 0.05, quiet, 5.0)
        except Exception as error:  # noqa: BLE001 - surface in the trace
            applied = False
            entry["error"] = str(error)
        entry["applied"] = applied
        entry["cursor_after"] = {
            "row": screen.row,
            "col": screen.col,
            "visible": screen.visible,
        }
        trace.append(entry)
    return trace


def safe_fixture_id(fixture_id: str) -> str:
    cleaned = "".join(ch if ch.isalnum() or ch in "-_." else "_" for ch in fixture_id)
    return cleaned[:96] or "fixture"


def stable_root(fixture_id: str):
    base = Path(tempfile.gettempdir()) / "oc2-ui-parity-slots" / safe_fixture_id(fixture_id)
    base.mkdir(parents=True, exist_ok=True)
    return base


def capture(request, binary: Path, min_total: float, quiet: float, max_total: float):
    cols = int(request["viewport"]["cols"])
    rows = int(request["viewport"]["rows"])
    base = stable_root(request["fixture"]["id"])
    lock_handle = (base / "run.lock").open("w")
    fcntl.flock(lock_handle, fcntl.LOCK_EX)
    root = base / "run"
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True, exist_ok=True)
    master = None
    proc = None
    try:
        env, work = isolated_env(request, root)
        screen = Screen(cols, rows)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        proc = subprocess.Popen(
            [str(binary), "--native"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            cwd=str(work),
            env=env,
            start_new_session=True,
            close_fds=True,
        )
        os.close(slave)
        chunks = read_until_quiet(master, screen, proc, min_total, quiet, max_total)
        if not chunks:
            raise RuntimeError("OC2 native TUI produced no output")
        actions = replay_events(master, screen, proc, request["fixture"].get("events"), quiet)
        if request["fixture"].get("events"):
            read_until_quiet(master, screen, proc, 0.05, quiet, 5.0)
        workdir_token = f"oc2-ui-parity-slots/{safe_fixture_id(request['fixture']['id'])}/run/work"
        redact_workdir(screen, workdir_token)
        snapshot = screen.snapshot()
        version = ""
        try:
            version_result = subprocess.run(
                [str(binary), "--version"],
                capture_output=True,
                text=True,
                timeout=15,
                env=env,
                cwd=str(work),
            )
            version = (version_result.stdout or "").strip().splitlines()[0] if version_result.stdout else ""
        except Exception:
            version = ""
        theme = dict(request.get("theme") or {})
        theme["mode"] = infer_theme_mode(screen)
        theme["source"] = "inferred_from_background"
        snapshot.update(
            {
                "schema": "oc2.ui-parity.screen/v1",
                "fixture_id": request["fixture"]["id"],
                "theme": theme,
                "focus": {
                    "value": None,
                    "source": "terminal_has_no_focus_channel",
                },
                "actions": actions,
                "environment": {
                    "adapter": "oc2-pty",
                    "product": {
                        "name": "opencode-rk",
                        "version": version,
                        "sha256": sha256_file(binary),
                        "executable": binary.name,
                    },
                    "terminal": {
                        "term": env.get("TERM"),
                        "colorterm": env.get("COLORTERM"),
                        "cols": cols,
                        "rows": rows,
                        "platform": sys.platform,
                        "machine": platform.machine(),
                    },
                    "captured_with": [str(binary), "--native"],
                    "normalized": ["workdir_path"],
                },
            }
        )
        return snapshot
    finally:
        if proc is not None and proc.poll() is None:
            try:
                os.killpg(proc.pid, signal.SIGINT)
            except ProcessLookupError:
                pass
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                proc.wait(timeout=3)
        if master is not None:
            try:
                os.close(master)
            except OSError:
                pass
        shutil.rmtree(root, ignore_errors=True)
        fcntl.flock(lock_handle, fcntl.LOCK_UN)
        lock_handle.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--min-ms", type=float, default=900)
    parser.add_argument("--quiet-ms", type=float, default=450)
    parser.add_argument("--max-ms", type=float, default=25000)
    args = parser.parse_args()
    request = json.load(sys.stdin)
    binary = resolve_binary(args.bin)
    if not binary.exists():
        print(f"error: OC2 binary not found: {binary}", file=sys.stderr)
        return 2
    try:
        screen = capture(
            request,
            binary,
            args.min_ms / 1000.0,
            args.quiet_ms / 1000.0,
            args.max_ms / 1000.0,
        )
    except Exception as error:  # noqa: BLE001
        print(f"error: {error}", file=sys.stderr)
        return 1
    json.dump(screen, sys.stdout, ensure_ascii=False, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

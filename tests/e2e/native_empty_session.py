#!/usr/bin/env python3
"""Installed native regression for empty-session routing.

Runs two real PTY journeys against disposable authenticated daemon state:
``/new`` must bind a newly-created empty session, and startup ``--session``
must bind an already-existing empty session.  A small local ANSI cell observer
keeps the contract independent of external recovery artifacts and whitespace
in PTY snapshots.
"""

import argparse
import fcntl
import hashlib
import http.server
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
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX,
    MAX_PTY,
    checked_artifact,
    descriptor,
    free_loopback_port,
    stop_owned_daemon,
)

COLS, ROWS = 160, 40
DEADLINE = 18
MAX_API = 256 * 1024
KEY = "fixture-empty-session-key-4c91"
UUID_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$", re.I)
LOGO_ROWS = (
    "█▀▀█ █▀▀█ █▀▀█ █▀▀▄ █▀▀▀ █▀▀█ █▀▀█ █▀▀█",
    "█  █ █  █ █▀▀▀ █  █ █    █  █ █  █ █▀▀▀",
    "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
)
MAX_ESC = 1024


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


class Screen:
    """Bounded ANSI/UTF-8 screen model; predicates inspect fresh settled cells."""

    def __init__(self):
        self.cells = [[" "] * COLS for _ in range(ROWS)]
        self.bg = [[None] * COLS for _ in range(ROWS)]
        self.row = self.col = 0
        self.saved = (0, 0)
        self.fg = self.background = None
        self.pending = bytearray()
        self.decoder = __import__("codecs").getincrementaldecoder("utf-8")("replace")
        self.frame_counter = 0
        self.frame_protocol = bytearray()

    def text(self):
        return "\n".join("".join(row).rstrip() for row in self.cells)

    def clear_cells(self, row, start, end):
        for col in range(max(0, start), min(COLS, end)):
            self.cells[row][col] = " "
            self.bg[row][col] = self.background

    def put(self, char):
        if char == "\n":
            self.row = min(ROWS - 1, self.row + 1)
            self.col = 0
        elif char == "\r":
            self.col = 0
        elif char == "\b":
            self.col = max(0, self.col - 1)
        elif ord(char) >= 0x20:
            if self.col >= COLS:
                self.col = 0
                self.row = min(ROWS - 1, self.row + 1)
            self.cells[self.row][self.col] = char
            self.bg[self.row][self.col] = self.background
            self.col += 1

    def csi(self, raw):
        final, body = chr(raw[-1]), raw[:-1]
        if body[:1] in (b"?", b">", b"!"):
            return
        try:
            values = [int(part) if part else 0 for part in body.decode("ascii").split(";")] if body else [0]
        except ValueError:
            return
        n = values[0] or 1
        if final in "Hf":
            self.row = max(0, min(ROWS - 1, (values[0] or 1) - 1))
            self.col = max(0, min(COLS - 1, (values[1] if len(values) > 1 else 1) - 1))
        elif final == "G": self.col = max(0, min(COLS - 1, n - 1))
        elif final == "A": self.row = max(0, self.row - n)
        elif final == "B": self.row = min(ROWS - 1, self.row + n)
        elif final == "C": self.col = min(COLS - 1, self.col + n)
        elif final == "D": self.col = max(0, self.col - n)
        elif final == "J":
            mode = values[0]
            if mode == 2:
                for row in range(ROWS): self.clear_cells(row, 0, COLS)
            elif mode == 0:
                self.clear_cells(self.row, self.col, COLS)
                for row in range(self.row + 1, ROWS): self.clear_cells(row, 0, COLS)
            elif mode == 1:
                for row in range(self.row): self.clear_cells(row, 0, COLS)
                self.clear_cells(self.row, 0, self.col + 1)
        elif final == "K":
            mode = values[0]
            if mode == 0: self.clear_cells(self.row, self.col, COLS)
            elif mode == 1: self.clear_cells(self.row, 0, self.col + 1)
            elif mode == 2: self.clear_cells(self.row, 0, COLS)
        elif final == "s": self.saved = (self.row, self.col)
        elif final == "u": self.row, self.col = self.saved
        elif final == "m":
            i = 0
            while i < len(values):
                code = values[i]
                if code == 0: self.fg = self.background = None
                elif code == 39: self.fg = None
                elif code == 49: self.background = None
                elif code in (38, 48) and i + 4 < len(values) and values[i + 1] == 2:
                    color = ("rgb", values[i + 2], values[i + 3], values[i + 4])
                    if code == 38: self.fg = color
                    else: self.background = color
                    i += 4
                i += 1

    def feed(self, data):
        opening, closing = b"\x1b[?2026h", b"\x1b[?2026l"
        self.frame_protocol.extend(data)
        if len(self.frame_protocol) > MAX_PTY:
            raise AssertionError("synchronized frame exceeded PTY bound")
        while True:
            begin = self.frame_protocol.find(opening)
            if begin < 0:
                self.frame_protocol[:] = self.frame_protocol[-(len(opening) - 1):]
                break
            end = self.frame_protocol.find(closing, begin + len(opening))
            if end < 0:
                del self.frame_protocol[:begin]
                break
            # Parse only a complete update. Later partial updates must never
            # contaminate the cells exposed to the semantic predicates.
            self._feed_cells(bytes(self.frame_protocol[begin + len(opening):end]))
            self.frame_counter += 1
            del self.frame_protocol[:end + len(closing)]

    def _feed_cells(self, data):
        self.pending.extend(data)
        if len(self.pending) > MAX_ESC + MAX_PTY:
            raise AssertionError("ANSI pending buffer exceeded bound")
        raw = bytes(self.pending)
        self.pending.clear()
        i, plain = 0, bytearray()
        while i < len(raw):
            if raw[i] != 0x1b:
                plain.append(raw[i]); i += 1; continue
            if plain:
                for char in self.decoder.decode(bytes(plain)): self.put(char)
                plain.clear()
            if i + 1 >= len(raw): self.pending.extend(raw[i:]); break
            if raw[i + 1] == ord("["):
                match = re.match(rb"\x1b\[([0-?]*[ -/]*[@-~])", raw[i:])
                if not match:
                    if len(raw) - i > MAX_ESC: raise AssertionError("malformed oversized CSI")
                    self.pending.extend(raw[i:]); break
                self.csi(match.group(1)); i += len(match.group(0)); continue
            if raw[i + 1] in (ord("]"), ord("P"), ord("^"), ord("_")):
                bel = raw.find(b"\x07", i + 2)
                st = raw.find(b"\x1b\\", i + 2)
                ends = [(bel, 1), (st, 2)]
                ends = [(end, size) for end, size in ends if end >= 0]
                if not ends:
                    if len(raw) - i > MAX_ESC: raise AssertionError("oversized OSC/DCS")
                    self.pending.extend(raw[i:]); break
                end, size = min(ends)
                i = end + size; continue
            i += 2
        if plain:
            for char in self.decoder.decode(bytes(plain)): self.put(char)

    def home_absent(self):
        rows = self.text().splitlines()
        return not any(row.strip() in LOGO_ROWS for row in rows)

    def composer(self):
        return any(line.strip() == ">" for line in self.text().splitlines())

    def sidebar_at_expected_geometry(self, session_id):
        for r, row in enumerate(self.cells):
            text = "".join(row)
            pos = text.find(session_id)
            if pos >= 117:
                # 160 columns; upstream's right-hand session sidebar is 42 cells.
                return self.bg[r][117] == ("rgb", 10, 10, 10) and all(
                    self.bg[r][c] == ("rgb", 20, 20, 20) for c in range(118, 160))
        return False

    def selected_sidebar_identity(self, session_id, other_ids=()):
        identities = [(number, "".join(row[118:]).strip())
                      for number, row in enumerate(self.cells)
                      if UUID_RE.fullmatch("".join(row[118:]).strip())]
        return (len(identities) == 1 and identities[0][1] == session_id and
                session_id not in other_ids and
                self.bg[identities[0][0]][117] == ("rgb", 10, 10, 10) and
                all(self.bg[identities[0][0]][c] == ("rgb", 20, 20, 20)
                    for c in range(118, 160)))


class Provider(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.server.requests += 1
        self.server.error = "unexpected provider request during empty-session journey"
        length = int(self.headers.get("Content-Length", "0"))
        if length < 0 or length > 128 * 1024:
            self.send_error(413)
            return
        self.rfile.read(length)
        self.send_error(500, "provider requests are forbidden in this test")

    def log_message(self, *_):
        pass


def api(origin, token, path, method="GET", body=None):
    raw = None if body is None else json.dumps(body, separators=(",", ":")).encode()
    request = urllib.request.Request(origin + path, data=raw, method=method, headers={
        "Authorization": "Bearer " + token,
        "Content-Type": "application/json",
    })
    with urllib.request.urlopen(request, timeout=4) as response:
        data = response.read(MAX_API + 1)
        if response.status not in (200, 201) or len(data) > MAX_API:
            raise AssertionError("authenticated API response status/size invalid")
        return json.loads(data) if data else {}


def wait_frame(master, child, screen, captured, predicate, start, timeout=DEADLINE,
               start_frame=None):
    if start_frame is None:
        start_frame = screen.frame_counter
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise AssertionError("native TUI exited while awaiting fresh frame")
        if screen.frame_counter > start_frame and predicate(screen):
            return
        remaining = min(0.1, deadline - time.monotonic())
        ready, _, _ = select.select([master], [], [], max(0, remaining))
        if not ready:
            continue
        chunk = os.read(master, min(65536, MAX_PTY - len(captured)))
        if not chunk:
            raise AssertionError("PTY reached EOF before expected fresh frame")
        captured.extend(chunk)
        if len(captured) >= MAX_PTY:
            raise AssertionError("PTY output exceeded 256 KiB bound")
        screen.feed(chunk)
    raise AssertionError("no predicate-satisfying fresh settled frame within deadline")


def get_sessions(origin, token):
    value = api(origin, token, "/api/sessions")
    rows = value.get("sessions")
    if not isinstance(rows, list):
        raise AssertionError("session listing lacks sessions array")
    return rows


class Journey:
    def __init__(self, binary, root, provider, explicit_session=None, all_assertions=None,
                 label="journey", cleanup_daemon=True):
        self.binary = binary
        self.root = root
        self.provider = provider
        self.explicit_session = explicit_session
        self.data = root / "data"
        self.project = root / "project"
        self.env = None
        self.master = self.slave = self.child = None
        self.saved_termios = None
        self.captured = bytearray()
        self.screen = Screen()
        self.daemon_descriptor = None
        self.assertions = all_assertions if all_assertions is not None else []
        self.label = label
        self.cleanup_daemon = cleanup_daemon

    def check(self, name, condition):
        self.assertions.append({"journey": self.label, "name": name, "pass": bool(condition)})
        if not condition:
            raise AssertionError("assertion failed: " + name)

    def start(self, existing_descriptor=None):
        self.master, self.slave = pty.openpty()
        self.saved_termios = termios.tcgetattr(self.master)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        args = [str(self.binary), "--native", "--data-dir", str(self.data), "tui"]
        if self.explicit_session:
            args.extend(["--session", self.explicit_session])
        self.child = subprocess.Popen(
            args, cwd=self.project, env=self.env, stdin=self.slave, stdout=self.slave,
            stderr=self.slave, start_new_session=True, close_fds=True,
        )
        os.close(self.slave); self.slave = None
        port = int(self.env["OPENCODE_RK_DAEMON_ADDR"].rsplit(":", 1)[1])
        if existing_descriptor is None:
            self.daemon_descriptor = descriptor(self.data, port, self.child, time.monotonic() + 20)
        else:
            # The already-running bootstrap client owns this validated daemon;
            # the second TUI deliberately joins that daemon's process group.
            self.daemon_descriptor = existing_descriptor
        return self

    def _wait_for_exit_draining_pty(self, timeout):
        """Wait for the CLI while boundedly draining output from its PTY."""
        deadline = time.monotonic() + timeout
        while self.child.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(self.child.args, timeout)
            if self.master is None or len(self.captured) >= MAX_PTY:
                time.sleep(min(remaining, .05))
                continue
            try:
                readable, _, _ = select.select([self.master], [], [], min(remaining, .05))
            except (OSError, ValueError):
                readable = []
            if not readable:
                continue
            try:
                chunk = os.read(self.master, min(65536, MAX_PTY - len(self.captured)))
            except OSError as error:
                # Linux PTYs report the peer closing as EIO rather than EOF.
                if error.errno == 5:
                    break
                raise
            if not chunk:
                break
            self.captured.extend(chunk)
            self.screen.feed(chunk)
        self.child.wait(timeout=max(0.001, deadline - time.monotonic()))

    def stop(self):
        cleanup_errors = []
        if self.child is not None and self.child.poll() is None:
            try:
                os.write(self.master, b"\x03")
                self._wait_for_exit_draining_pty(8)
            except (OSError, subprocess.TimeoutExpired) as error:
                cleanup_errors.append("native CLI did not exit cleanly: " + str(error))
                if self.child.poll() is None:
                    try: os.killpg(self.child.pid, signal.SIGTERM)
                    except ProcessLookupError: pass
                    try: self.child.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        try: os.killpg(self.child.pid, signal.SIGKILL)
                        except ProcessLookupError: pass
                        self.child.wait(timeout=2)
        if self.child is not None and self.child.returncode not in (None, 0):
            cleanup_errors.append("native CLI exited nonzero: %s" % self.child.returncode)
        if self.saved_termios is not None and self.master is not None:
            try:
                restored = termios.tcgetattr(self.master) == self.saved_termios
                self.assertions.append({"journey": self.label, "name": "termios-restored", "pass": bool(restored)})
                if not restored: cleanup_errors.append("terminal attributes were not restored")
            except OSError as error:
                cleanup_errors.append("unable to verify termios restoration: " + str(error))
        if self.cleanup_daemon and self.daemon_descriptor is not None:
            pid = self.daemon_descriptor.get("_validated_pid")
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                pass
            else:
                try: stop_owned_daemon(self.daemon_descriptor)
                except (OSError, RuntimeError) as error: cleanup_errors.append("owned daemon cleanup failed: " + str(error))
                try: os.kill(pid, 0)
                except ProcessLookupError: pass
                else: cleanup_errors.append("validated daemon remains after cleanup")
        # The CLI owns this newly created session/process group. Reap any
        # remaining daemon members after the CLI has exited; this is never a
        # group ID read from disk or inherited from the caller.
        if self.cleanup_daemon and self.child is not None:
            try:
                os.killpg(self.child.pid, 0)
            except ProcessLookupError:
                pass
            else:
                try: os.killpg(self.child.pid, signal.SIGTERM)
                except ProcessLookupError: pass
                deadline = time.monotonic() + 2
                while time.monotonic() < deadline:
                    try: os.killpg(self.child.pid, 0)
                    except ProcessLookupError: break
                    time.sleep(.03)
                else:
                    try: os.killpg(self.child.pid, signal.SIGKILL)
                    except ProcessLookupError: pass
                    try: os.killpg(self.child.pid, 0)
                    except ProcessLookupError: pass
                    else: cleanup_errors.append("owned process group remained after SIGKILL")
        for fd in (self.slave, self.master):
            if fd is not None:
                try: os.close(fd)
                except OSError: pass
        self.slave = self.master = None
        if cleanup_errors:
            raise AssertionError("; ".join(cleanup_errors))

    def await_initial(self, expect_home):
        def ready(screen):
            if expect_home:
                return "Ask anything" in screen.text()
            return self.explicit_session in screen.text() and screen.composer() and screen.home_absent()
        wait_frame(self.master, self.child, self.screen, self.captured, ready, 0, timeout=20)
        attrs = termios.tcgetattr(self.master)
        raw_mask = termios.ICANON | termios.ECHO | termios.ISIG
        self.check("native-terminal-raw-mode-active", attrs[3] & raw_mask == 0)

    def session_api(self, path, method="GET", body=None):
        return api(self.daemon_descriptor["http_origin"], self.daemon_descriptor["auth_token"], path, method, body)


def make_environment(root, daemon_port, provider_port):
    # A daemon Unix socket path must stay below sockaddr_un.sun_path's limit.
    # The runner's TMPDIR itself is long, so keep the data root as a short,
    # direct child rather than nesting it under install/fixtures/journey.
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    home, project = root / "home", root / "project"
    data = pathlib.Path(tempfile.mkdtemp(prefix="e", dir=os.environ.get("TMPDIR", tempfile.gettempdir())))
    for path in (home, data, project, data / "runtime", data / "catalog", home / ".config", home / ".local" / "share"):
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
    if len(os.fsencode(data / "runtime" / "opencode-rk.sock")) >= 104:
        raise RuntimeError("disposable runtime socket path exceeds sockaddr_un limit")
    # Fixture-only model/auth data lives under fresh temporary roots. No caller
    # environment (and therefore no ambient credential) is inherited.
    catalog = data / "catalog" / "models.dev.api.json"
    catalog.write_text(json.dumps({"openai": {"name": "fixture", "models": {
        "gpt-5.6": {"name": "readiness fixture", "limit": {"context": 200000}},
        "gpt-5.6-mini": {"name": "descriptor readiness fixture", "limit": {"context": 200000}},
    }}}), encoding="utf-8")
    auth = data / "opencode" / "auth.json"
    auth.parent.mkdir(mode=0o700)
    fd = os.open(auth, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as stream:
        json.dump({"openai": {"type": "api", "key": KEY}}, stream)
    xdg_config, xdg_runtime = root / "xdg-config", root / "xdg-runtime"
    for path in (xdg_config, xdg_runtime): path.mkdir(mode=0o700)
    env = {
        "HOME": str(home), "PATH": "/usr/bin:/bin", "TERM": "xterm-256color",
        "TMPDIR": str(root), "XDG_CONFIG_HOME": str(xdg_config),
        "XDG_DATA_HOME": str(data), "XDG_RUNTIME_DIR": str(xdg_runtime),
        "OPENCODE_RK_HOME": str(data), "OPENCODE_PROJECT_DIR": str(project),
        "OPENCODE_RK_DAEMON_ADDR": "127.0.0.1:%d" % daemon_port,
        "OPENAI_BASE_URL": "http://127.0.0.1:%d/v1" % provider_port,
    }
    return env, data


def run_new_session(binary, provider, root, assertions):
    port = free_loopback_port()
    journey = Journey(binary, root, provider, all_assertions=assertions, label="slash-new")
    journey.env, journey.data = make_environment(root, port, provider.server_port)
    try:
        journey.start()
        journey.await_initial(expect_home=True)
        rows = get_sessions(journey.daemon_descriptor["http_origin"], journey.daemon_descriptor["auth_token"])
        journey.check("initial-auto-session-exists-and-is-empty", len(rows) == 1 and api(
            journey.daemon_descriptor["http_origin"], journey.daemon_descriptor["auth_token"],
            "/api/sessions/%s/messages" % rows[0]["id"]) == {"messages": []})
        old_id = rows[0]["id"]
        start = len(journey.captured)
        start_frame = journey.screen.frame_counter
        os.write(journey.master, b"/new\r")
        wait_frame(journey.master, journey.child, journey.screen, journey.captured,
                   lambda screen: screen.home_absent() and screen.composer(), start,
                   start_frame=start_frame)
        rows = get_sessions(journey.daemon_descriptor["http_origin"], journey.daemon_descriptor["auth_token"])
        created = [row for row in rows if row.get("id") != old_id]
        journey.check("exactly-one-new-session", len(rows) == 2 and len(created) == 1)
        target = created[0]["id"]
        journey.check("new-session-id-is-canonical-uuid", UUID_RE.fullmatch(target) is not None)
        history = api(journey.daemon_descriptor["http_origin"], journey.daemon_descriptor["auth_token"],
                      "/api/sessions/%s/messages" % target)
        journey.check("new-session-remains-empty", history == {"messages": []})
        wait_frame(journey.master, journey.child, journey.screen, journey.captured,
                   lambda screen: screen.home_absent() and screen.composer() and
                   screen.selected_sidebar_identity(target, (old_id,)), start,
                   start_frame=start_frame)
        journey.check("home-logo-absent", journey.screen.home_absent())
        journey.check("session-composer-visible", journey.screen.composer())
        journey.check("sidebar-identifies-new-empty-session", journey.screen.selected_sidebar_identity(target, (old_id,)))
        journey.check("fixture-key-absent-from-raw-output", KEY.encode() not in bytes(journey.captured))
        journey.check("new-command-did-not-contact-provider", provider.requests == 0 and provider.error is None)
        return journey.assertions
    finally:
        cleanup_error = None
        data_error = None
        try:
            journey.stop()
        except BaseException as error:
            cleanup_error = error
        finally:
            try:
                shutil.rmtree(journey.data, ignore_errors=False)
            except BaseException as error:
                data_error = error
        if cleanup_error is not None:
            raise cleanup_error
        if data_error is not None:
            raise data_error


def run_existing_empty_session(binary, provider, root, assertions):
    port = free_loopback_port()
    bootstrap = Journey(binary, root, provider, all_assertions=assertions,
                        label="explicit-existing-bootstrap", cleanup_daemon=True)
    bootstrap.env, bootstrap.data = make_environment(root, port, provider.server_port)
    target = None
    journey = None
    try:
        bootstrap.start()
        bootstrap.await_initial(expect_home=True)
        origin, token = bootstrap.daemon_descriptor["http_origin"], bootstrap.daemon_descriptor["auth_token"]
        rows = get_sessions(origin, token)
        bootstrap.check("bootstrap-auto-session-exists", len(rows) == 1)
        auto_session_id = rows[0].get("id") if rows else None
        created = api(origin, token, "/api/sessions", method="POST", body={"title": "Explicit Empty Target"})
        target_obj = created.get("session", created)
        target = target_obj.get("id") if isinstance(target_obj, dict) else None
        bootstrap.check("existing-empty-target-created", isinstance(target, str) and UUID_RE.fullmatch(target) is not None)
        bootstrap.check("explicit-target-differs-from-auto-session", isinstance(target, str) and target != auto_session_id)
        bootstrap.check("existing-target-has-no-messages", api(origin, token, "/api/sessions/%s/messages" % target) == {"messages": []})
        decoy = api(origin, token, "/api/sessions", method="POST", body={"title": "More Recent Decoy"})
        decoy_obj = decoy.get("session", decoy)
        decoy_id = decoy_obj.get("id") if isinstance(decoy_obj, dict) else None
        bootstrap.check("newer-decoy-is-distinct-from-explicit-target",
                        isinstance(decoy_id, str) and UUID_RE.fullmatch(decoy_id) is not None and decoy_id != target)
        ordered = get_sessions(origin, token)
        bootstrap.check("explicit-target-is-not-most-recent-session",
                        len(ordered) >= 2 and ordered[0].get("id") == decoy_id and
                        any(row.get("id") == target for row in ordered[1:]))
        # Keep the daemon owner alive while a second independent TUI attaches.
        # The test separates the explicit route from most-recent selection by
        # creating a newer decoy before this launch.
        journey = Journey(binary, root, provider, explicit_session=target,
                          all_assertions=assertions, label="explicit-existing-empty",
                          cleanup_daemon=False)
        journey.env = bootstrap.env
        journey.data = bootstrap.data
        journey.start(existing_descriptor=bootstrap.daemon_descriptor)
        start_frame = journey.screen.frame_counter
        wait_frame(journey.master, journey.child, journey.screen, journey.captured,
                   lambda screen: target in screen.text() and screen.composer() and
                    screen.home_absent() and screen.selected_sidebar_identity(target, (auto_session_id, decoy_id)),
                    0, timeout=20, start_frame=start_frame)
        attrs = termios.tcgetattr(journey.master)
        raw_mask = termios.ICANON | termios.ECHO | termios.ISIG
        journey.check("native-terminal-raw-mode-active", attrs[3] & raw_mask == 0)
        origin, token = journey.daemon_descriptor["http_origin"], journey.daemon_descriptor["auth_token"]
        rows = get_sessions(origin, token)
        journey.check("explicit-target-still-in-session-list", any(row.get("id") == target for row in rows))
        journey.check("explicit-target-history-empty", api(origin, token, "/api/sessions/%s/messages" % target) == {"messages": []})
        journey.check("explicit-target-identity-visible-not-auto-session", target in journey.screen.text())
        journey.check("explicit-session-home-absent", journey.screen.home_absent())
        journey.check("explicit-session-composer-visible", journey.screen.composer())
        journey.check("explicit-session-sidebar-42-cells",
                      journey.screen.selected_sidebar_identity(target, (auto_session_id, decoy_id)))
        journey.check("fixture-key-absent-from-raw-output", KEY.encode() not in bytes(journey.captured))
        journey.check("existing-empty-bind-did-not-contact-provider", provider.requests == 0 and provider.error is None)
    finally:
        cleanup_errors = []
        try:
            if journey is not None:
                journey.stop()
        except BaseException as error:
            cleanup_errors.append(error)
        finally:
            try:
                bootstrap.stop()
            except BaseException as error:
                cleanup_errors.append(error)
        try:
            shutil.rmtree(bootstrap.data, ignore_errors=False)
        except BaseException as error:
            cleanup_errors.append(error)
        if cleanup_errors:
            raise AssertionError("; ".join(str(error) for error in cleanup_errors))
    return assertions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--native-library", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--output", required=True, help="new output directory for exact-SHA receipt")
    args = parser.parse_args()
    output = pathlib.Path(args.output)
    if not output.is_absolute(): parser.error("--output must be absolute")
    if output.exists() and any(output.iterdir()): parser.error("--output must be new or empty")
    output.mkdir(parents=True, exist_ok=True)
    binary, library, source_sha, binary_sha, library_sha = checked_artifact(
        args.binary, args.native_library, args.build_json)
    if source_sha.lower() != args.candidate_sha.lower():
        parser.error("--candidate-sha does not match build attestation source SHA")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", args.candidate_sha): parser.error("candidate SHA must be 40 hex characters")
    stage = pathlib.Path(tempfile.mkdtemp(prefix="oc2-empty-installed-", dir=os.environ.get("TMPDIR", tempfile.gettempdir())))
    provider = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    provider.requests = 0
    provider.error = None
    provider.daemon_threads = True
    thread = threading.Thread(target=provider.serve_forever, daemon=True)
    thread.start()
    assertions = []
    result = {"status": "FAIL", "candidate_sha": args.candidate_sha,
              "build_json_sha256": sha256(pathlib.Path(args.build_json)), "binary_sha256": binary_sha,
              "native_library_sha256": library_sha, "test_sha256": sha256(pathlib.Path(__file__)),
              "assertions": assertions, "provider_requests": 0, "reason": None}
    failure = None
    try:
        install = stage / "install"
        (install / "bin").mkdir(parents=True)
        (install / "lib").mkdir()
        installed_bin = install / "bin" / "oc2"
        installed_lib = install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX)
        shutil.copyfile(binary, installed_bin)
        shutil.copyfile(library, installed_lib)
        os.chmod(installed_bin, 0o755)
        if sha256(installed_bin) != binary_sha or sha256(installed_lib) != library_sha:
            raise RuntimeError("installed copy digest differs from build attestation")
        journeys_root = stage / "fixtures"
        journeys_root.mkdir(mode=0o700)
        run_new_session(installed_bin, provider, journeys_root / "new", assertions)
        run_existing_empty_session(installed_bin, provider, journeys_root / "existing", assertions)
        result["assertions"] = assertions
        if provider.requests or provider.error:
            raise AssertionError(provider.error or "provider received forbidden request")
        result["status"] = "PASS"
    except BaseException as error:
        failure = error
        result["reason"] = str(error)[:2000]
    finally:
        result["provider_requests"] = provider.requests
        try:
            provider.shutdown()
            provider.server_close()
            thread.join(timeout=3)
            if thread.is_alive(): raise RuntimeError("provider fixture thread failed bounded shutdown")
        except BaseException as error:
            result["status"] = "FAIL"
            result["reason"] = (result["reason"] + "; " if result["reason"] else "") + "cleanup: " + str(error)
        result["installed_binary_sha256"] = sha256(installed_bin) if 'installed_bin' in locals() and installed_bin.exists() else None
        result["installed_native_library_sha256"] = sha256(installed_lib) if 'installed_lib' in locals() and installed_lib.exists() else None
        result["fixture_root"] = str(stage)
        receipt = output / "native-empty-session-result.json"
        receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"status": result["status"], "candidate_sha": args.candidate_sha,
                          "receipt": str(receipt), "reason": result["reason"]}))
        shutil.rmtree(stage, ignore_errors=False)
    if failure is not None or result["status"] != "PASS":
        raise SystemExit(1)


if __name__ == "__main__":
    main()

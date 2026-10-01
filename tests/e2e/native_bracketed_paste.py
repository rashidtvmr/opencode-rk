#!/usr/bin/env python3
"""G5 RED contract: native bracketed paste is inert until explicit Enter.

Installed-binary PTY fixture only. The loopback provider records the exact
canonical user input sent by the real native turn path; it never calls a
decoder or inspects renderer internals.

Phases:
1. Fragmented ESC[200~ + Unicode/CRLF/CR multiline body + fragmented
   ESC[201~ lands in the draft with NO provider request and NO durable user
   message. The body carries quit-like and slash-command text to prove paste
   is inert (no auto submit, no tool, no provider call from contents).
2. Explicit Enter submits once; the provider must receive the exact
   upstream-normalized (CRLF/CR -> LF) multiline prompt.
3. A second plain turn completes; durable history must equal the exact two
   turns (user/assistant x2).
4. Negatives: an unterminated paste must not mutate the sentinel draft, and
   an oversized framed paste (> 32 KiB) must not mutate the draft nor trigger
   any provider request or durable message.

Protocol negotiation is observed, not parsed: the PTY byte stream must show
the real bracketed-paste enable (ESC[?2004h) before pasting and the restore
(ESC[?2004l) on normal exit.

Current product is RED here by design: native_loop never enables
bracketed-paste, has no Paste event and no Composer::apply_paste caller, so
the embedded CR bytes submit early and no 2004h is emitted.
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
    HOST_LIBRARY_SUFFIX, MAX_PTY, TerminalScreen, checked_artifact, descriptor,
    free_loopback_port, read_until, stop_owned_daemon,
)

MAX_REQUEST, MAX_RESPONSE, TIMEOUT, TOTAL_TIMEOUT = 128 * 1024, 256 * 1024, 20, 90
KEY, MODEL = "fixture-native-paste-9c41", "gpt-5.6"
PASTE_BEGIN, PASTE_END = b"\x1b[200~", b"\x1b[201~"
ENABLE_PASTE, DISABLE_PASTE = b"\x1b[?2004h", b"\x1b[?2004l"
PASTE_RAW = "paste contract alpha caf\u00e9\r\n\u65e5\u672c\u8a9e line \U0001f980\r:quit /connect stays inert".encode("utf-8")
SECOND = "paste contract second turn plain"
SENTINEL = "sentinel keep"
MALFORMED_TOKEN = "MALFORMED-INJECT-NEG-7f2a"
OVERSIZED_BYTES = 32 * 1024 + 1
FIRST_REPLY, SECOND_REPLY = "PASTE_FIRST_RESPONSE", "PASTE_SECOND_RESPONSE"


def normalize_paste(raw: bytes) -> str:
    """Upstream boundary rule: decode, CRLF first, then residual CR."""
    return raw.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")


FIRST_EXPECTED = normalize_paste(PASTE_RAW)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sse(text):
    return ("event: response.output_text.delta\ndata: " +
            json.dumps({"type": "response.output_text.delta", "delta": text}) +
            "\n\nevent: response.completed\ndata: "
            "{\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n").encode()


class State:
    def __init__(self):
        self.requests, self.error, self.semantic_errors = [], None, []
        self.lock = threading.Lock()

    def body(self, headers, raw):
        if len(raw) > MAX_REQUEST:
            raise RuntimeError("request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY:
            raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw.decode("utf-8"))
        if value.get("model") != MODEL or value.get("stream") is not True:
            raise RuntimeError("invalid model or stream mode")
        inputs = value.get("input")
        if not isinstance(inputs, list):
            raise RuntimeError("input is not a list")
        with self.lock:
            if len(self.requests) >= 2:
                raise RuntimeError("provider request count exceeded two")
            ordinal = len(self.requests)
            self.requests.append(value)
        users = [item.get("content") for item in inputs
                 if isinstance(item, dict) and item.get("role") == "user"]
        assistants = [item.get("content") for item in inputs
                      if isinstance(item, dict) and item.get("role") == "assistant"]
        expected_users = [FIRST_EXPECTED] if ordinal == 0 else [FIRST_EXPECTED, SECOND]
        expected_assistants = [] if ordinal == 0 else [FIRST_REPLY]
        if users != expected_users or assistants != expected_assistants:
            self.semantic_errors.append({"request": ordinal + 1, "users": users,
                                         "assistants": assistants,
                                         "expected_users": expected_users,
                                         "expected_assistants": expected_assistants})
        return sse(FIRST_REPLY if ordinal == 0 else SECOND_REPLY)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):  # noqa: N802
        try:
            if self.path != "/v1/responses":
                raise RuntimeError("unexpected provider route")
            length = int(self.headers.get("Content-Length", "-1"))
            if length < 0 or length > MAX_REQUEST:
                raise RuntimeError("invalid content length")
            deadline = time.monotonic() + 5
            raw = bytearray()
            while len(raw) < length:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError("provider request deadline")
                self.connection.settimeout(remaining)
                chunk = self.rfile.read(min(65536, length - len(raw)))
                if not chunk:
                    raise RuntimeError("truncated provider request")
                raw.extend(chunk)
            if len(raw) != length:
                raise RuntimeError("truncated provider request")
            payload = self.server.state.body(self.headers, raw)
            if len(payload) > MAX_RESPONSE:
                raise RuntimeError("response exceeded 256 KiB")
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(payload)
            self.wfile.flush()
        except Exception as exc:  # fixture errors are reported, never hidden
            self.server.state.error = str(exc)
            try:
                self.send_error(500, str(exc))
            except OSError:
                pass

    def log_message(self, *_):
        pass


class Fixture(http.server.HTTPServer):
    allow_reuse_address = True

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(5)
        return connection, address


def send_fragments(master, data, delay=0.012):
    """Write one-byte fragments; delay stays below the decoder carry window."""
    deadline = time.monotonic() + 5
    for byte in data:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("PTY input write deadline")
            _, writable, _ = select.select([], [master], [], min(remaining, .1))
            if not writable:
                continue
            try:
                if os.write(master, bytes((byte,))) != 1:
                    raise RuntimeError("PTY input short write")
                break
            except BlockingIOError:
                continue
        time.sleep(delay)


def send_bulk(master, data, chunk=4096, captures=None, screen=None):
    """Bounded bulk write while concurrently draining the PTY output.

    The oversized negative deliberately writes more than the terminal input
    buffer can hold.  Draining here is part of the fixture contract: a native
    renderer is allowed to redraw/reject the frame while the producer is
    still writing, and a writer which does not service output can deadlock
    both sides of the PTY.
    """
    if captures is None:
        captures = bytearray()
    deadline = time.monotonic() + 20
    offset = 0
    while offset < len(data):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("PTY bulk write deadline")
        readable, writable, _ = select.select([master], [master], [], min(remaining, .1))
        if readable:
            if len(captures) >= MAX_PTY:
                raise RuntimeError("PTY capture bound reached during oversized paste")
            try:
                output = os.read(master, min(65536, MAX_PTY - len(captures)))
            except OSError as exc:
                raise RuntimeError("PTY read failed during oversized paste: %s" % exc)
            if not output:
                raise RuntimeError("PTY EOF during oversized paste")
            captures.extend(output)
            if screen is not None:
                screen.feed(output)
        if not writable:
            continue
        try:
            written = os.write(master, data[offset:offset + chunk])
        except BlockingIOError:
            continue
        if written <= 0:
            raise RuntimeError("PTY bulk short write")
        offset += written


def wait_history(descriptor_value, expected, deadline):
    """Require canonical full-history persistence, not merely a streamed delta."""
    last = None
    while time.monotonic() < deadline:
        try:
            last = history_raw(descriptor_value)
        except (OSError, ValueError, urllib.error.URLError):
            last = None
        if last == expected:
            return last
        time.sleep(.05)
    raise AssertionError("durable full history did not settle: %r" % (last,))


def drain_exit_output(master, captures, screen, deadline):
    """Drain bounded post-exit bytes; macOS PTYs report terminal EOF as EIO."""
    while time.monotonic() < deadline:
        if len(captures) >= MAX_PTY:
            raise RuntimeError("PTY capture bound reached while draining CLI exit")
        ready, _, _ = select.select([master], [], [], min(.1, max(0, deadline - time.monotonic())))
        if not ready:
            continue
        try:
            output = os.read(master, min(65536, MAX_PTY - len(captures)))
        except OSError as exc:
            if exc.errno == 5:  # EIO is terminal EOF for a closed macOS PTY slave.
                return
            raise RuntimeError("PTY read failed while draining CLI exit: %s" % exc)
        if not output:
            return
        captures.extend(output)
        screen.feed(output)
    raise RuntimeError("timed out draining CLI exit output")


def settle(master, captures, screen, seconds):
    """Drain PTY output for a bounded window; feeds screen, respects MAX_PTY."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if len(captures) >= MAX_PTY:
            raise RuntimeError("PTY capture bound reached while settling")
        ready, _, _ = select.select([master], [], [],
                                    min(.1, max(0, deadline - time.monotonic())))
        if not ready:
            continue
        try:
            chunk = os.read(master, min(65536, MAX_PTY - len(captures)))
        except OSError as exc:
            raise RuntimeError("PTY read failed while settling: %s" % exc)
        if not chunk:
            raise RuntimeError("PTY EOF while settling")
        captures.extend(chunk)
        screen.feed(chunk)


def history_raw(descriptor_value):
    def get(path):
        request = urllib.request.Request(
            descriptor_value["http_origin"] + path,
            headers={"Authorization": "Bearer " + descriptor_value["auth_token"]})
        with urllib.request.urlopen(request, timeout=3) as response:
            raw = response.read(MAX_RESPONSE + 1)
            if response.status != 200 or len(raw) > MAX_RESPONSE:
                raise RuntimeError("bounded authenticated history response failed")
            return json.loads(raw)
    sessions = get("/api/sessions").get("sessions")
    if not isinstance(sessions, list) or len(sessions) != 1:
        raise RuntimeError("expected exactly one fixture session")
    messages = get("/api/sessions/%s/messages?limit=20" % sessions[0]["id"]).get("messages")
    if not isinstance(messages, list):
        raise RuntimeError("canonical messages are absent")
    return [(item.get("role"), item.get("body", {}).get("text")) for item in messages]


def history(descriptor_value):
    observed = history_raw(descriptor_value)
    expected = [("user", FIRST_EXPECTED), ("assistant", FIRST_REPLY),
                ("user", SECOND), ("assistant", SECOND_REPLY)]
    if observed != expected:
        raise AssertionError("durable history differs from the exact two completed turns")
    return observed


def run(binary, library, manifest, artifacts):
    started = time.monotonic()
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    if not artifacts.is_absolute():
        raise RuntimeError("artifact directory must be absolute")
    build = json.loads(manifest.read_text())
    if build.get("native") is not True or build.get("profile") != "release":
        raise RuntimeError("the contract requires an attested native release")
    tmpdir = os.environ.get("TMPDIR") or "/tmp"
    root = pathlib.Path(tempfile.mkdtemp(prefix="npaste-", dir=tmpdir))
    for name in ("home", "project", "data"):
        (root / name).mkdir()
    (root / "data" / "runtime").mkdir()
    if len(os.fsencode(root / "data" / "runtime" / "opencode-rk.sock")) > 100:
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
        MODEL: {"name": "Paste fixture", "limit": {"context": 200000}},
        "gpt-5.6-mini": {"name": "Readiness fixture", "limit": {"context": 200000}}
    }}}) + "\n")
    state = State()
    server = Fixture(("127.0.0.1", 0), Handler)
    server.state = state
    provider_thread = threading.Thread(target=server.serve_forever, daemon=False)
    provider_thread.start()
    master = slave = child = None
    descriptor_value = None
    captures = bytearray()
    screen = TerminalScreen()
    saved_attrs = None
    semantic_error = None
    termios_restored = False
    child_reaped = False
    daemon_gone = False
    raw_seen = False
    controlling_terminal_seen = False
    enable_observed = False
    disable_observed = False
    forced_kill = False
    pre_enter_clean = False
    negatives_clean = False
    messages = []
    try:
        port = free_loopback_port()
        env = {
            "HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""),
            "TERM": "xterm-256color", "TMPDIR": str(root),
            "XDG_CONFIG_HOME": str(root / "home" / ".config"),
            "XDG_DATA_HOME": str(root / "data"),
            "XDG_STATE_HOME": str(root / "home" / "state"),
            "XDG_CACHE_HOME": str(root / "home" / "cache"),
            "XDG_RUNTIME_DIR": str(root / "data" / "runtime"),
            "OPENCODE_RK_HOME": str(root / "data"),
            "OPENCODE_PROJECT_DIR": str(root / "project"),
            "OPENCODE_RK_DAEMON_ADDR": "127.0.0.1:%d" % port,
            "OPENAI_BASE_URL": "http://127.0.0.1:%d/v1" % server.server_port,
        }
        (root / "home" / ".config").mkdir()
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        os.set_blocking(master, False)
        saved_attrs = termios.tcgetattr(master)
        # Own controlling PTY via fresh child exec (no Python-thread preexec);
        # exec preserves the PID/process group used by ownership checks.
        terminal_exec = (
            "import fcntl, os, sys, termios; "
            "fcntl.ioctl(0, termios.TIOCSCTTY, 0); "
            "os.execve(sys.argv[1], sys.argv[1:], dict(os.environ))"
        )
        child = subprocess.Popen(
            [sys.executable, "-c", terminal_exec, str(install / "bin" / "oc2"),
             "--data-dir", str(root / "data")],
            cwd=root / "project", env=env, stdin=slave, stdout=slave,
            stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        descriptor_value = descriptor(root / "data", port, child, time.monotonic() + TIMEOUT)
        if descriptor_value.get("schema_version") != 1:
            raise RuntimeError("unexpected daemon descriptor schema")
        token = descriptor_value.get("auth_token", "")
        if not isinstance(token, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", token):
            raise RuntimeError("descriptor auth token is not a 64-hex capability")
        read_until(master, time.monotonic() + TIMEOUT, captures, b"OpenCode", 0,
                    screen, lambda: screen.contains("model: openai/" + MODEL))
        raw_seen = not bool(termios.tcgetattr(master)[3] & termios.ICANON)
        controlling_terminal_seen = os.tcgetpgrp(master) == child.pid
        if not controlling_terminal_seen:
            raise AssertionError("CLI does not own the foreground controlling-terminal group")
        enable_observed = bytes(ENABLE_PASTE) in bytes(captures)
        if not enable_observed:
            raise AssertionError("native feature did not enable bracketed-paste (no ESC[?2004h)")
        # Fragmented bracketed paste: split begin/end framing and the UTF-8
        # body itself; CRLF + lone CR + quit-like text must stay inert draft.
        send_fragments(master, b"\x1b[")
        send_fragments(master, b"20")
        send_fragments(master, b"0~")
        send_fragments(master, PASTE_RAW)
        send_fragments(master, b"\x1b[20")
        send_fragments(master, b"1~")
        settle(master, captures, screen, 1.5)
        if child.poll() is not None:
            raise AssertionError("CLI exited from paste contents; paste was not inert")
        if state.error or state.semantic_errors or len(state.requests) != 0:
            raise AssertionError(state.error or state.semantic_errors
                                 or "paste submitted without explicit Enter")
        if history_raw(descriptor_value) != []:
            raise AssertionError("paste persisted a durable user message before Enter")
        if not screen.contains("paste contract alpha") or not screen.contains("\u65e5\u672c\u8a9e"):
            raise AssertionError("normalized paste draft is not visible after framing")
        pre_enter_clean = True
        # Explicit Enter submits exactly once with the normalized multiline text.
        send_fragments(master, b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captures, FIRST_REPLY.encode(),
                   len(captures), screen, lambda: screen.contains(FIRST_REPLY))
        wait_history(descriptor_value,
                     [("user", FIRST_EXPECTED), ("assistant", FIRST_REPLY)],
                     time.monotonic() + TIMEOUT)
        if state.error or state.semantic_errors or len(state.requests) != 1:
            raise AssertionError(state.error or state.semantic_errors or "first request missing")
        # Second plain turn, then the exact durable two-turn history.
        send_fragments(master, SECOND.encode("utf-8") + b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captures, SECOND_REPLY.encode(),
                   len(captures), screen, lambda: screen.contains(SECOND_REPLY))
        wait_history(descriptor_value,
                     [("user", FIRST_EXPECTED), ("assistant", FIRST_REPLY),
                      ("user", SECOND), ("assistant", SECOND_REPLY)],
                     time.monotonic() + TIMEOUT)
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError(state.error or state.semantic_errors or "second request missing")
        messages = history(descriptor_value)
        # Negatives: sentinel draft, then an unterminated paste (no 201~).
        send_fragments(master, SENTINEL.encode("utf-8"))
        settle(master, captures, screen, 0.8)
        if not screen.contains(SENTINEL):
            raise AssertionError("sentinel draft did not land before negatives")
        send_fragments(master, PASTE_BEGIN)
        send_fragments(master, MALFORMED_TOKEN.encode("utf-8"))
        settle(master, captures, screen, 1.5)
        if child.poll() is not None:
            raise AssertionError("CLI exited during malformed paste")
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError(state.error or state.semantic_errors
                                 or "malformed paste triggered a provider request")
        if MALFORMED_TOKEN.lower() in screen.text().lower():
            raise AssertionError("unterminated paste mutated the visible draft")
        if not screen.contains(SENTINEL):
            raise AssertionError("unterminated paste clobbered the sentinel draft")
        if history_raw(descriptor_value) != [("user", FIRST_EXPECTED),
                                             ("assistant", FIRST_REPLY),
                                             ("user", SECOND),
                                             ("assistant", SECOND_REPLY)]:
            raise AssertionError("malformed paste changed durable history")
        send_fragments(master, b"\x1b")  # standalone Escape clears any draft
        settle(master, captures, screen, 0.5)
        # Oversized framed paste: rejected, draft untouched, still 2 requests.
        send_bulk(master, PASTE_BEGIN + b"x" * OVERSIZED_BYTES + PASTE_END,
                  captures=captures, screen=screen)
        settle(master, captures, screen, 2.0)
        if child.poll() is not None:
            raise AssertionError("CLI exited during oversized paste")
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError(state.error or state.semantic_errors
                                 or "oversized paste triggered a provider request")
        if "xxxxxxxxxx" in screen.text():
            raise AssertionError("oversized paste mutated the visible draft")
        if history_raw(descriptor_value) != [("user", FIRST_EXPECTED),
                                             ("assistant", FIRST_REPLY),
                                             ("user", SECOND),
                                             ("assistant", SECOND_REPLY)]:
            raise AssertionError("oversized paste changed durable history")
        negatives_clean = True
    except BaseException as exc:
        semantic_error = str(exc)
    finally:
        if child is not None and child.poll() is None:
            try:
                send_fragments(master, b"\x03", delay=0)
                child.wait(timeout=10)
            except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
                semantic_error = semantic_error or "normal CLI reap failed: %s" % exc
                forced_kill = True
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=3)
                except (OSError, subprocess.TimeoutExpired) as kill_error:
                    semantic_error = semantic_error or "forced CLI cleanup failed: %s" % kill_error
        child_reaped = child is not None and child.poll() is not None
        if master is not None and child_reaped and not forced_kill:
            try:
                drain_exit_output(master, captures, screen, time.monotonic() + 5)
            except (OSError, RuntimeError) as exc:
                semantic_error = semantic_error or "normal CLI output drain failed: %s" % exc
        disable_observed = bytes(DISABLE_PASTE) in bytes(captures)
        if master is not None and saved_attrs is not None:
            try:
                termios_restored = termios.tcgetattr(master) == saved_attrs
            except OSError as exc:
                semantic_error = semantic_error or "terminal restoration probe failed: %s" % exc
        if descriptor_value is not None:
            try:
                stop_owned_daemon(descriptor_value)
            except Exception as exc:
                semantic_error = semantic_error or "owned daemon stop failed: %s" % exc
            try:
                os.kill(descriptor_value["_validated_pid"], 0)
            except ProcessLookupError:
                daemon_gone = True
            except OSError as exc:
                semantic_error = semantic_error or "owned daemon liveness probe failed: %s" % exc
        for fd in (slave, master):
            if fd is not None:
                try:
                    os.close(fd)
                except OSError as exc:
                    semantic_error = semantic_error or "PTY close failed: %s" % exc
        server.shutdown(); server.server_close(); provider_thread.join(timeout=10)
        provider_joined = not provider_thread.is_alive()
        if not provider_joined:
            semantic_error = semantic_error or "fixture provider thread did not join"
        secret_echo = KEY.encode() in captures or KEY in screen.text()
        if secret_echo:
            semantic_error = semantic_error or "fixture API key appeared in terminal output"
        capture_bound = len(captures) <= MAX_PTY
        if not capture_bound:
            semantic_error = semantic_error or "PTY capture bound exceeded"
        if not disable_observed:
            semantic_error = semantic_error or "native exit did not restore bracketed-paste (no ESC[?2004l)"
        time_ok = time.monotonic() - started <= TOTAL_TIMEOUT
        if not time_ok:
            semantic_error = semantic_error or "fixture exceeded total timeout"
        success = (semantic_error is None and state.error is None and
                   not state.semantic_errors and len(state.requests) == 2 and
                   pre_enter_clean and negatives_clean and len(messages) == 4 and
                   enable_observed and disable_observed and
                   child_reaped and child.returncode == 0 and not forced_kill and
                   daemon_gone and termios_restored and raw_seen and
                   controlling_terminal_seen and provider_joined and
                   capture_bound and not secret_echo and time_ok)
        evidence = {"phase": "success" if success else "failed",
                    "error": semantic_error or state.error or "",
                    "source_sha": source, "binary_sha256": binary_sha,
                    "native_library_sha256": library_sha,
                    "test_sha256": sha256(pathlib.Path(__file__)),
                    "requests": state.requests, "semantic_errors": state.semantic_errors,
                    "paste_expected": FIRST_EXPECTED,
                    "pre_enter_clean": pre_enter_clean,
                    "negatives_clean": negatives_clean,
                    "bracketed_paste_enabled": enable_observed,
                    "bracketed_paste_restored": disable_observed,
                    "termios_restored": termios_restored,
                    "child_reaped": child_reaped, "daemon_gone": daemon_gone,
                    "cli_exit_code": child.returncode if child is not None else None,
                    "forced_kill": forced_kill, "raw_mode_seen": raw_seen,
                    "controlling_terminal_seen": controlling_terminal_seen,
                    "secret_echo": secret_echo, "messages": messages,
                    "fixture_root": str(root),
                    "provider_thread_joined": provider_joined,
                    "captured_bytes": len(captures), "max_capture": MAX_PTY}
        output = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
        if len(output.encode()) > 512 * 1024:
            raise RuntimeError("evidence exceeded 512 KiB")
        artifacts.mkdir(parents=True, exist_ok=True)
        (artifacts / ("native-paste-result.json" if success else "native-paste-failure.json")).write_text(output)
        print("preserved forensic fixture root: %s" % root)
        if not success:
            raise AssertionError(evidence["error"] or "native paste contract failed")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary"); parser.add_argument("--native-library")
    parser.add_argument("--build-json"); parser.add_argument("--artifact-dir")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        assert FIRST_EXPECTED == "paste contract alpha caf\u00e9\n\u65e5\u672c\u8a9e line \U0001f980\n:quit /connect stays inert"
        assert OVERSIZED_BYTES == 32 * 1024 + 1
        state = State()
        headers = {"Authorization": "Bearer " + KEY}
        first = json.dumps({"model": MODEL, "stream": True,
                            "input": [{"role": "user", "content": FIRST_EXPECTED}]}).encode()
        assert b"response.completed" in state.body(headers, first)
        second = json.dumps({"model": MODEL, "stream": True,
                             "input": [{"role": "user", "content": FIRST_EXPECTED},
                                       {"role": "assistant", "content": FIRST_REPLY},
                                       {"role": "user", "content": SECOND}]}).encode()
        assert b"response.completed" in state.body(headers, second)
        assert not state.semantic_errors and len(state.requests) == 2
        try:
            state.body(headers, second)
            raise AssertionError("third request accepted")
        except RuntimeError:
            pass
        mismatch = State()
        mismatch.body(headers, json.dumps({"model": MODEL, "stream": True,
                                           "input": [{"role": "user",
                                                      "content": "un-normalized\r\npaste"}]}).encode())
        assert len(mismatch.requests) == 1 and mismatch.semantic_errors
        try:
            State().body({"Authorization": "Bearer wrong"}, first)
            raise AssertionError("wrong auth accepted")
        except RuntimeError:
            pass
        print("self-check: CRLF/CR normalize, exact two-turn history, request cap, auth, mismatch passed")
        return
    if not all((args.binary, args.native_library, args.build_json, args.artifact_dir)):
        parser.error("actual run requires --binary --native-library --build-json --artifact-dir")
    run(pathlib.Path(args.binary), pathlib.Path(args.native_library),
        pathlib.Path(args.build_json), pathlib.Path(args.artifact_dir))


if __name__ == "__main__":
    main()

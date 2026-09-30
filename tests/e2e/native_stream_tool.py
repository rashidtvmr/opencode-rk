#!/usr/bin/env python3
"""Native G3/G4/G5 contract: real streamed delta, tool round, and resume.

This is deliberately an installed-binary contract.  It does not mock the
daemon, native renderer, broker, or provider protocol.  The loopback provider
only controls deterministic Responses frames and records the sanitized
request history.  The test is intentionally not runnable without explicit
artifact arguments.
"""

import argparse
import hashlib
import http.server
import json
import os
import pathlib
import platform
import pty
import select
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import urllib.error
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX,
    MAX_PTY,
    TerminalScreen,
    checked_artifact,
    descriptor,
    free_loopback_port,
    read_until,
    stop_owned_daemon,
)

MAX_REQUEST = 128 * 1024
MAX_RESPONSE = 256 * 1024
MAX_DESCRIPTOR = 8 * 1024
SOCKET_LIMIT = 100
PROVIDER_TIMEOUT = 5
FIXTURE_TIMEOUT = 10
KEY = "fixture-stream-generated-5e7c"
MODEL = "gpt-5.6"
FIRST = "native stream first"
SECOND = "native stream second"
RESUMED = "native stream resumed"
CALL_ID = "native_stream_call_1"
FILE = "native-stream-tool-marker.txt"
PARTIAL = "STREAM_PARTIAL_7d91"
FINAL = "STREAM_FINAL_3a42"
RESUME = "STREAM_RESUMED_9b28"
SECOND_FINAL = "STREAM_SECOND_FINAL_6c18"
ASSISTANT_FIRST = PARTIAL + " " + FINAL
MARKER = "native stream broker marker\n"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def redact(value):
    return value.replace(KEY, "[REDACTED]")


def sse(event, data):
    return (f"event: {event}\ndata: {json.dumps(data, separators=(',', ':'))}\n\n").encode()


class ProviderState:
    def __init__(self, project):
        self.project = project
        self.requests = []
        self.error = None
        self.phase = "idle"
        self.stream_request_seen = threading.Event()
        self.partial_visible = threading.Event()
        self.release_partial = threading.Event()
        self.active_send = None
        self.restart_generation = 0
        self.lock = threading.Lock()

    def fail(self, message):
        with self.lock:
            if self.error is None:
                self.error = message
        self.release_partial.set()

    def body(self, headers, raw):
        if len(raw) > MAX_REQUEST:
            raise RuntimeError("provider request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY:
            raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw.decode())
        if value.get("model") != MODEL or not isinstance(value.get("input"), list):
            raise RuntimeError("invalid provider request schema")
        stream = value.get("stream", False)
        if not isinstance(stream, bool):
            raise RuntimeError("stream must be boolean when present")
        with self.lock:
            ordinal = len(self.requests)
            if ordinal >= 4:
                raise RuntimeError("provider request count exceeded four")
            self.requests.append(value)
        expected_call = {"type": "function_call", "call_id": CALL_ID, "name": "write", "arguments": json.dumps({"path": FILE, "content": MARKER, "append": False}, separators=(",", ":"))}
        expected_output = {"type": "function_call_output", "call_id": CALL_ID, "output": "write success"}
        users = [item.get("content") for item in value["input"] if item.get("role") == "user"]
        calls = [item for item in value["input"] if item.get("type") == "function_call"]
        outputs = [item for item in value["input"] if item.get("type") == "function_call_output"]
        if not stream:
            self.phase = "completed_before_client_observation"
            return json.dumps({"id": "fixture", "object": "response", "status": "completed", "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "NONSTREAM_COMPLETED"}]}]}).encode()
        if ordinal == 0:
            if len(value["input"]) != 1 or users != [FIRST] or calls or outputs:
                raise RuntimeError("first request history/tool pair is wrong")
            if not stream:
                raise RuntimeError("native stream request did not set stream=true")
            self.phase = "waiting_partial_observation"
            self.stream_request_seen.set()
            payload = sse("response.output_text.delta", {"type": "response.output_text.delta", "delta": PARTIAL})
            if self.active_send is None:
                raise RuntimeError("stream sender was not bound")
            self.active_send(payload)
            if not self.partial_visible.wait(FIXTURE_TIMEOUT):
                self.fail("native screen never exposed the first provider delta")
            self.release_partial.wait(FIXTURE_TIMEOUT)
            self.phase = "released_final_completion"
            return sse(
                "response.output_item.done",
                {"type": "response.output_item.done", "item": {"type": "function_call", "id": CALL_ID, "call_id": CALL_ID, "name": "write", "arguments": json.dumps({"path": FILE, "content": MARKER, "append": False}, separators=(",", ":"))}},
            ) + sse("response.completed", {"type": "response.completed", "response": {"status": "completed"}})
        if ordinal == 1:
            if len(value["input"]) != 3 or calls != [expected_call] or outputs != [expected_output] or users != [FIRST]:
                raise RuntimeError("continuation lacks exactly one ordered typed call/output")
            if value["input"].index(expected_call) >= value["input"].index(expected_output):
                raise RuntimeError("continuation output preceded its call")
            if not stream:
                raise RuntimeError("continuation request did not stream")
            if (self.project / FILE).read_text() != MARKER:
                raise RuntimeError("broker write marker bytes are wrong")
            return sse("response.output_text.delta", {"type": "response.output_text.delta", "delta": " " + FINAL}) + sse("response.completed", {"type": "response.completed", "response": {"status": "completed"}})
        if ordinal == 2 and (len(value["input"]) != 5 or users != [FIRST, SECOND]):
            raise RuntimeError("second-turn durable user history is wrong")
        if ordinal == 3 and (len(value["input"]) != 7 or users != [FIRST, SECOND, RESUMED] or self.restart_generation != 1):
            raise RuntimeError("restart durable user history is wrong")
        expected_assistants = [ASSISTANT_FIRST] if ordinal == 2 else [ASSISTANT_FIRST, SECOND_FINAL]
        if [x.get("content") for x in value["input"] if x.get("role") == "assistant"] != expected_assistants:
            raise RuntimeError("assistant history is not durable across continuation/restart")
        if calls != [expected_call] or outputs != [expected_output] or len(calls) != 1 or len(outputs) != 1:
            raise RuntimeError("typed pair was not preserved across turn/restart")
        if value["input"].index(expected_call) >= value["input"].index(expected_output):
            raise RuntimeError("typed output preceded its function call")
        text = SECOND_FINAL if ordinal == 2 else RESUME
        return sse("response.output_text.delta", {"type": "response.output_text.delta", "delta": text}) + sse("response.completed", {"type": "response.completed", "response": {"status": "completed"}})


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path != "/v1/responses":
                raise RuntimeError("unexpected provider route")
            length = int(self.headers.get("Content-Length", "-1"))
            if length < 0 or length > MAX_REQUEST:
                raise RuntimeError("invalid request length")
            raw = self.rfile.read(length)
            if len(raw) != length:
                raise RuntimeError("truncated provider request")
            value = json.loads(raw.decode())
            stream = value.get("stream", False)
            if not isinstance(stream, bool):
                self.server.state.fail("stream must be boolean when present")
                self.send_error(400, "invalid stream type")
                return
            # Validate the ordinary JSON protocol before sending headers.  A
            # non-stream request must never be misframed as SSE.
            if self.headers.get("Authorization") != "Bearer " + KEY or value.get("model") != MODEL or not isinstance(value.get("input"), list):
                self.server.state.fail("invalid auth/model/input")
                self.send_error(400, "invalid request")
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream" if stream else "application/json")
            self.send_header("Connection", "close")
            self.end_headers()
            self.server.state.active_send = lambda payload: self._send_chunk(payload)
            payload = self.server.state.body(self.headers, raw)
            if len(payload) > MAX_RESPONSE:
                raise RuntimeError("response exceeded 256 KiB")
            self.wfile.write(payload)
            self.wfile.flush()
        except Exception as exc:
            self.server.state.fail(str(exc))
            # The response may already have started for a gated stream;
            # closing the bounded provider connection is the safe failure.
            try:
                self.connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

    def log_message(self, *_):
        return

    def _send_chunk(self, payload):
        self.wfile.write(payload)
        self.wfile.flush()


class Fixture(http.server.HTTPServer):
    allow_reuse_address = True

    def get_request(self):
        conn, address = super().get_request()
        conn.settimeout(PROVIDER_TIMEOUT)
        return conn, address

def run(binary, library, manifest, artifacts):
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    root = pathlib.Path(tempfile.mkdtemp(prefix="ns-", dir=os.environ.get("TMPDIR", "/tmp")))
    install = root / "install"; (install / "bin").mkdir(parents=True); (install / "lib").mkdir()
    shutil.copyfile(binary, install / "bin" / "oc2"); shutil.copyfile(library, install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX)); os.chmod(install / "bin" / "oc2", 0o755)
    for name in ("home", "project", "data"):
        (root / name).mkdir()
    (root / "data" / "runtime").mkdir()
    if len(os.fsencode(root / "data" / "runtime" / "opencode-rk.sock")) > SOCKET_LIMIT:
        raise RuntimeError("Unix socket path exceeds 100 bytes")
    auth = root / "data" / "opencode" / "auth.json"; auth.parent.mkdir()
    auth.write_text(json.dumps({"openai": {"type": "api", "key": KEY}}) + "\n"); os.chmod(auth, 0o600)
    catalog = root / "data" / "catalog" / "models.dev.api.json"; catalog.parent.mkdir()
    catalog.write_text(json.dumps({"openai": {"name": "fixture", "models": {MODEL: {"name": "stream fixture", "limit": {"context": 200000}}, "gpt-5.6-mini": {"name": "mini fixture", "limit": {"context": 200000}}}}}) + "\n")
    state = ProviderState(root / "project")
    server = Fixture(("127.0.0.1", 0), Handler); server.state = state
    provider_thread = threading.Thread(target=server.serve_forever, daemon=False); provider_thread.start()
    daemon = None; descriptors = []; captures = bytearray(); screen = TerminalScreen(80, 24); master = slave = None
    try:
        port = free_loopback_port()
        env = {"HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""), "TERM": "xterm-256color", "TMPDIR": str(root), "XDG_CONFIG_HOME": str(root / "home" / ".config"), "XDG_DATA_HOME": str(root / "data"), "XDG_RUNTIME_DIR": str(root / "data" / "runtime"), "OPENCODE_RK_HOME": str(root / "data"), "OPENCODE_PROJECT_DIR": str(root / "project"), "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{port}", "OPENAI_BASE_URL": f"http://127.0.0.1:{server.server_port}/v1", "OPENCODE_RK_TURN_TOOLS": "write"}
        (root / "home" / ".config").mkdir(); (root / "home" / "xdg").mkdir()
        master, slave = pty.openpty(); import fcntl; fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        daemon = subprocess.Popen([str(install / "bin" / "oc2"), "--data-dir", str(root / "data")], cwd=root / "project", env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        d = descriptor(root / "data", port, daemon, time.monotonic() + FIXTURE_TIMEOUT); descriptors.append(d)
        start = len(captures); read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, b"OpenCode", start, screen)
        os.write(master, (FIRST + "\r").encode())
        read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, PARTIAL.encode(), start, screen, lambda: screen.contains(PARTIAL))
        if state.phase != "waiting_partial_observation" or not state.stream_request_seen.is_set():
            raise AssertionError("partial screen was not gated against an actual stream request")
        if screen.contains(FINAL):
            raise AssertionError("final completion was visible before fixture barrier release")
        state.partial_visible.set(); state.release_partial.set()
        read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, FINAL.encode(), start, screen, lambda: screen.contains(FINAL))
        if not (root / "project" / FILE).is_file() or (root / "project" / FILE).read_text() != MARKER:
            raise AssertionError("write tool did not create exact marker bytes")
        if not screen.contains("write"):
            raise AssertionError("native screen did not visibly identify the write tool call")
        if not screen.contains("success") and not screen.contains("completed"):
            raise AssertionError("native screen did not visibly identify tool completion")
        second_start = len(captures); os.write(master, (SECOND + "\r").encode()); read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, SECOND_FINAL.encode(), second_start, screen, lambda: screen.contains(SECOND_FINAL))
        old_daemon = daemon
        stop_owned_daemon(d)
        try:
            old_daemon.wait(timeout=3)
        except subprocess.TimeoutExpired:
            raise AssertionError("first native CLI was not reaped after daemon shutdown")
        daemon = None
        # Restart the installed native CLI and its owned daemon through a new
        # PTY.  The prior PTY is closed only after the validated process group
        # has terminated, preserving terminal ownership boundaries.
        os.close(master); master = None
        master, slave = pty.openpty(); fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        daemon = subprocess.Popen([str(install / "bin" / "oc2"), "--data-dir", str(root / "data")], cwd=root / "project", env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        d2 = descriptor(root / "data", port, daemon, time.monotonic() + FIXTURE_TIMEOUT); descriptors.append(d2)
        state.restart_generation = 1
        screen = TerminalScreen(80, 24)
        start = len(captures)
        read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, b"OpenCode", start, screen, lambda: screen.contains(FIRST) and screen.contains(MODEL))
        os.write(master, (RESUMED + "\r").encode())
        read_until(master, time.monotonic() + FIXTURE_TIMEOUT, captures, RESUME.encode(), start, screen, lambda: screen.contains(RESUME))
        if state.error or len(state.requests) != 4 or state.restart_generation != 1:
            raise AssertionError(state.error or f"expected four provider requests, got {len(state.requests)}")
        state.phase = "success"
        result = {"phase": "success", "source_sha": source, "binary_sha256": binary_sha, "native_library_sha256": library_sha, "provider_requests": [{"model": x.get("model"), "stream": x.get("stream", False), "input": x.get("input")} for x in state.requests], "screen": redact(screen.text()), "file_sha256": sha256(root / "project" / FILE)}
        (artifacts / "native-stream-tool-result.json").write_text(json.dumps(result, indent=2) + "\n")
    except BaseException as error:
        state.fail(str(error))
        raise
    finally:
        state.partial_visible.set(); state.release_partial.set()
        if daemon is not None:
            try:
                if daemon.poll() is None:
                    os.write(master, b"\x03") if master is not None else None
                    daemon.wait(timeout=3)
            except (OSError, subprocess.TimeoutExpired):
                pass
            if descriptors:
                try: stop_owned_daemon(descriptors[-1])
                except (OSError, RuntimeError): pass
            try: daemon.wait(timeout=3)
            except subprocess.TimeoutExpired: pass
        if slave is not None: os.close(slave)
        if master is not None: os.close(master)
        server.shutdown(); server.server_close(); provider_thread.join(timeout=10)
        if provider_thread.is_alive(): raise RuntimeError("fixture server failed bounded shutdown")
        secret_echo = KEY.encode() in captures or KEY in screen.text()
        if secret_echo:
            state.fail("fixture API key appeared in terminal output")
        evidence = {"phase": state.phase, "error": redact(state.error or ""), "source_sha": source, "binary_sha256": binary_sha, "native_library_sha256": library_sha, "test_sha256": sha256(pathlib.Path(__file__)), "fixture_root": str(root), "provider_requests": len(state.requests), "requests": [{"model": x.get("model"), "stream": x.get("stream", False), "input": x.get("input")} for x in state.requests], "screen": redact(screen.text()), "captured_output": redact(bytes(captures[:MAX_PTY]).decode("utf-8", "replace")), "owned_process_cleanup": bool(daemon is None or daemon.poll() is not None), "owned_descriptors": descriptors}
        raw_evidence = json.dumps(evidence, indent=2) + "\n"
        if len(raw_evidence.encode()) > 512 * 1024: raise RuntimeError("evidence exceeded 512 KiB")
        evidence_name = "native-stream-tool-result.json" if state.phase == "success" and not state.error else "native-stream-tool-failure.json"
        (artifacts / evidence_name).write_text(raw_evidence)
        if secret_echo:
            raise AssertionError("fixture API key appeared in terminal output")


def self_check():
    """Exercise the actual bounded HTTP handler without launching product code."""
    root = pathlib.Path(tempfile.mkdtemp(prefix="ns-check-"))
    server = Fixture(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=False)
    thread.start()
    import http.client

    def request(state, value, auth=KEY):
        server.state = state
        connection = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=PROVIDER_TIMEOUT)
        raw = json.dumps(value, separators=(",", ":")).encode()
        connection.request("POST", "/v1/responses", raw, {"Authorization": "Bearer " + auth, "Content-Length": str(len(raw))})
        response = connection.getresponse()
        body = response.read(MAX_RESPONSE + 1)
        connection.close()
        return response.status, response.getheader("Content-Type"), body

    try:
        base = {"model": MODEL, "input": [{"role": "user", "content": FIRST}]}
        for variant in ({}, {"stream": False}):
            state = ProviderState(root)
            status, content_type, body = request(state, {**base, **variant})
            assert status == 200 and content_type == "application/json"
            assert json.loads(body)["status"] == "completed" and state.phase == "completed_before_client_observation"

        state = ProviderState(root)
        server.state = state
        connection = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=PROVIDER_TIMEOUT)
        raw = json.dumps({**base, "stream": True}, separators=(",", ":")).encode()
        connection.request("POST", "/v1/responses", raw, {"Authorization": "Bearer " + KEY, "Content-Length": str(len(raw))})
        response = connection.getresponse()
        assert response.status == 200 and response.getheader("Content-Type") == "text/event-stream"
        first = bytearray()
        deadline = time.monotonic() + FIXTURE_TIMEOUT
        while PARTIAL.encode() not in first and time.monotonic() < deadline:
            first.extend(response.read(1))
        assert PARTIAL.encode() in first and b"response.completed" not in first
        assert state.phase == "waiting_partial_observation"
        state.partial_visible.set(); state.release_partial.set()
        body = bytes(first) + response.read(MAX_RESPONSE + 1)
        connection.close()
        assert b"response.completed" in body

        bad = ProviderState(root)
        status, _, _ = request(bad, base, auth="wrong")
        assert status == 400 and bad.error
        bad = ProviderState(root)
        status, _, _ = request(bad, {**base, "stream": "false"})
        assert status == 400 and bad.error

        bad = ProviderState(root)
        try:
            bad.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": [{"role": "user", "content": "wrong history"}], "stream": True}).encode())
            raise AssertionError("wrong history accepted")
        except RuntimeError:
            pass
        marker = root / FILE
        marker.write_text(MARKER)
        valid = ProviderState(root)
        valid.active_send = lambda payload: None
        valid.partial_visible.set()
        valid.release_partial.set()
        valid.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": [{"role": "user", "content": FIRST}], "stream": True}).encode())
        call_item = {"type": "function_call", "call_id": CALL_ID, "name": "write", "arguments": json.dumps({"path": FILE, "content": MARKER, "append": False}, separators=(",", ":"))}
        output_item = {"type": "function_call_output", "call_id": CALL_ID, "output": "write success"}
        continuation = valid.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": [{"role": "user", "content": FIRST}, call_item, output_item], "stream": True}).encode())
        assert FINAL.encode() in continuation and SECOND_FINAL.encode() not in continuation
        second = valid.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": [{"role": "user", "content": FIRST}, call_item, output_item, {"role": "assistant", "content": ASSISTANT_FIRST}, {"role": "user", "content": SECOND}], "stream": True}).encode())
        assert SECOND_FINAL.encode() in second
        valid.restart_generation = 1
        resumed = valid.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": [{"role": "user", "content": FIRST}, call_item, output_item, {"role": "assistant", "content": ASSISTANT_FIRST}, {"role": "user", "content": SECOND}, {"role": "assistant", "content": SECOND_FINAL}, {"role": "user", "content": RESUMED}], "stream": True}).encode())
        assert RESUME.encode() in resumed
        first_user = {"role": "user", "content": FIRST}
        invalid_pairs = [
            ([first_user, output_item, call_item], "output preceded its call"),
            ([first_user, call_item], "exactly one ordered typed call/output"),
            ([first_user, call_item, output_item, output_item], "exactly one ordered typed call/output"),
        ]
        for items, expected_error in invalid_pairs:
            invalid = ProviderState(root)
            invalid.requests.append({"input": [first_user], "stream": True})
            try:
                invalid.body({"Authorization": "Bearer " + KEY}, json.dumps({"model": MODEL, "input": items, "stream": True}).encode())
                raise AssertionError("invalid typed pair accepted")
            except RuntimeError as error:
                assert expected_error in str(error)
        print("self-check: HTTP JSON defaults, actual gated early SSE, auth, invalid stream, four-round state history and typed-pair negatives passed")
    finally:
        server.state.release_partial.set()
        server.shutdown(); server.server_close(); thread.join(timeout=5)
        shutil.rmtree(root, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary"); parser.add_argument("--native-library"); parser.add_argument("--build-json"); parser.add_argument("--artifact-dir"); parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return
    if not all((args.binary, args.native_library, args.build_json, args.artifact_dir)):
        parser.error("actual run requires --binary, --native-library, --build-json, and --artifact-dir")
    raw_artifacts = pathlib.Path(args.artifact_dir)
    if not raw_artifacts.is_absolute():
        parser.error("--artifact-dir must be absolute")
    artifacts = raw_artifacts.resolve()
    if artifacts.exists() and any(artifacts.iterdir()):
        parser.error("artifact directory must be new or empty")
    artifacts.mkdir(parents=True, exist_ok=True)
    run(pathlib.Path(args.binary), pathlib.Path(args.native_library), pathlib.Path(args.build_json), artifacts)


if __name__ == "__main__":
    main()

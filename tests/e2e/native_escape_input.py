#!/usr/bin/env python3
"""G5 frozen contract: native PTY control sequences are not draft text.

This is intentionally an installed-binary test.  The fixture provider records
the canonical user input sent by the real native turn path; it does not call a
decoder or inspect renderer internals.  The first turn exercises fragmented
CSI/SS3/cursor-report/kitty input, Unicode scalar backspace, and the second
turn proves that Escape closes a model dialog without submitting its filter.
"""
import argparse, hashlib, http.server, json, os, pathlib, pty, shutil, signal
import subprocess, sys, tempfile, termios, threading, time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX, MAX_PTY, TerminalScreen, checked_artifact, descriptor,
    free_loopback_port, read_until, stop_owned_daemon,
)

MAX_REQUEST, MAX_RESPONSE, TIMEOUT = 128 * 1024, 256 * 1024, 20
KEY, MODEL = "fixture-native-escape-2f31", "gpt-5.6"
FIRST = "native escape seed c café"
SECOND = "native escape after dialog"
FIRST_REPLY, SECOND_REPLY = "ESCAPE_FIRST_RESPONSE", "ESCAPE_SECOND_RESPONSE"


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
        expected_users = [FIRST] if ordinal == 0 else [FIRST, SECOND]
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
            raw = self.rfile.read(length)
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
    for byte in data:
        os.write(master, bytes((byte,)))
        time.sleep(delay)


def launch(exe, root, env, slave):
    return subprocess.Popen([str(exe), "--data-dir", str(root / "data")],
                            cwd=root / "project", env=env, stdin=slave,
                            stdout=slave, stderr=slave, start_new_session=True,
                            close_fds=True)


def run(binary, library, manifest, artifacts):
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    root = pathlib.Path(tempfile.mkdtemp(prefix="nescape-"))
    for name in ("home", "project", "data"):
        (root / name).mkdir()
    (root / "data" / "runtime").mkdir()
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
        MODEL: {"name": "Escape fixture", "limit": {"context": 200000}},
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
    try:
        port = free_loopback_port()
        env = {
            "HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""),
            "TERM": "xterm-256color", "TMPDIR": str(root),
            "XDG_CONFIG_HOME": str(root / "home" / ".config"),
            "XDG_DATA_HOME": str(root / "data"),
            "XDG_RUNTIME_DIR": str(root / "data" / "runtime"),
            "OPENCODE_RK_HOME": str(root / "data"),
            "OPENCODE_PROJECT_DIR": str(root / "project"),
            "OPENCODE_RK_DAEMON_ADDR": "127.0.0.1:%d" % port,
            "OPENAI_BASE_URL": "http://127.0.0.1:%d/v1" % server.server_port,
        }
        (root / "home" / ".config").mkdir()
        master, slave = pty.openpty()
        saved_attrs = termios.tcgetattr(master)
        child = launch(install / "bin" / "oc2", root, env, slave)
        os.close(slave); slave = None
        descriptor_value = descriptor(root / "data", port, child, time.monotonic() + TIMEOUT)
        read_until(master, time.monotonic() + TIMEOUT, captures, b"OpenCode", 0,
                   screen, lambda: screen.contains("model: openai/" + MODEL))
        # Seed, fragmented CSI/SS3 arrows, cursor report, Kitty Unicode 'c',
        # then a scalar backspace that removes the temporary emoji.
        send_fragments(master, b"native escape seed ")
        for sequence in (b"\x1b[", b"C", b"\x1bO", b"D", b"\x1b[6n"):
            send_fragments(master, sequence)
        send_fragments(master, b"\x1b[99;1u")
        send_fragments(master, " café😀".encode("utf-8"))
        send_fragments(master, b"\x7f")
        send_fragments(master, b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captures, FIRST_REPLY.encode(),
                   len(captures), screen, lambda: screen.contains(FIRST_REPLY))
        if state.error or state.semantic_errors or len(state.requests) != 1:
            raise AssertionError(state.error or state.semantic_errors or "first request missing")
        # Dialog Escape is a standalone control: its filter must not submit.
        send_fragments(master, b"/models\r")
        read_until(master, time.monotonic() + TIMEOUT, captures, b"Select model",
                   len(captures), screen, lambda: screen.contains("Select model"))
        send_fragments(master, b"fixture-filter")
        send_fragments(master, b"\x1b")
        send_fragments(master, SECOND.encode("utf-8") + b"\r")
        read_until(master, time.monotonic() + TIMEOUT, captures, SECOND_REPLY.encode(),
                   len(captures), screen, lambda: screen.contains(SECOND_REPLY))
        if state.error or state.semantic_errors or len(state.requests) != 2:
            raise AssertionError(state.error or state.semantic_errors or "second request missing")
    except BaseException as exc:
        semantic_error = str(exc)
    finally:
        if child is not None and child.poll() is None:
            try:
                os.write(master, b"\x03")
                child.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired):
                try: child.kill(); child.wait(timeout=3)
                except (OSError, subprocess.TimeoutExpired): pass
        child_reaped = child is not None and child.poll() is not None
        if master is not None and saved_attrs is not None:
            try: termios_restored = termios.tcgetattr(master) == saved_attrs
            except OSError: termios_restored = False
        if descriptor_value is not None:
            try: stop_owned_daemon(descriptor_value)
            except Exception as exc: semantic_error = semantic_error or str(exc)
            try: os.kill(descriptor_value["_validated_pid"], 0)
            except ProcessLookupError: daemon_gone = True
            except OSError: daemon_gone = False
        for fd in (slave, master):
            if fd is not None:
                try: os.close(fd)
                except OSError: pass
        server.shutdown(); server.server_close(); provider_thread.join(timeout=10)
        provider_joined = not provider_thread.is_alive()
        capture_bound = len(captures) <= MAX_PTY
        success = (semantic_error is None and state.error is None and
                   not state.semantic_errors and len(state.requests) == 2 and
                   child_reaped and daemon_gone and termios_restored and
                   provider_joined and capture_bound)
        evidence = {"phase": "success" if success else "failed",
                    "error": semantic_error or state.error or "",
                    "source_sha": source, "binary_sha256": binary_sha,
                    "native_library_sha256": library_sha,
                    "test_sha256": sha256(pathlib.Path(__file__)),
                    "requests": state.requests, "semantic_errors": state.semantic_errors,
                    "termios_restored": termios_restored,
                    "child_reaped": child_reaped, "daemon_gone": daemon_gone,
                    "provider_thread_joined": provider_joined,
                    "captured_bytes": len(captures), "max_capture": MAX_PTY}
        output = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
        if len(output.encode()) > 512 * 1024: raise RuntimeError("evidence exceeded 512 KiB")
        artifacts.mkdir(parents=True, exist_ok=True)
        (artifacts / ("native-escape-result.json" if success else "native-escape-failure.json")).write_text(output)
        if not success: raise AssertionError(evidence["error"] or "native escape contract failed")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary"); parser.add_argument("--native-library")
    parser.add_argument("--build-json"); parser.add_argument("--artifact-dir")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        state = State()
        headers = {"Authorization": "Bearer " + KEY}
        first = json.dumps({"model": MODEL, "stream": True,
                            "input": [{"role": "user", "content": FIRST}]}).encode()
        assert b"response.completed" in state.body(headers, first)
        second = json.dumps({"model": MODEL, "stream": True,
                             "input": [{"role": "user", "content": FIRST},
                                       {"role": "assistant", "content": FIRST_REPLY},
                                       {"role": "user", "content": SECOND}]}).encode()
        assert b"response.completed" in state.body(headers, second)
        assert not state.semantic_errors and len(state.requests) == 2
        print("self-check: exact two-turn history, bounded SSE, auth, and request cap passed")
        return
    if not all((args.binary, args.native_library, args.build_json, args.artifact_dir)):
        parser.error("actual run requires --binary --native-library --build-json --artifact-dir")
    run(pathlib.Path(args.binary), pathlib.Path(args.native_library),
        pathlib.Path(args.build_json), pathlib.Path(args.artifact_dir))


if __name__ == "__main__":
    main()

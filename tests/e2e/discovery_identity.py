#!/usr/bin/env python3
"""DISC-101 native discovery composite gate.

This fixture intentionally runs the supplied artifact, not a parser model.  A
live loopback endpoint publishes its own health identity while backend.json
publishes a different owned live PID.  The native caller must authenticate the
identity before making an API request.  Current Rust discovery does not yet do
that, so this is an executable product RED.
"""
import hashlib, http.server, json, os, pathlib, pty, select, signal, socket, subprocess
import tempfile, termios, time, urllib.request

MAX_PTY = 256 * 1024
MAX_WIRE = 64 * 1024
TIMEOUT = 8.0

def artifact(name, executable=False):
    value = os.environ.get(name)
    if not value or not os.path.isabs(value) or not os.path.isfile(value):
        raise AssertionError(f"required attested artifact {name} is missing: {value!r}")
    if executable and not os.access(value, os.X_OK):
        raise AssertionError(f"artifact is not executable: {value}")
    return value

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""): h.update(chunk)
    return h.hexdigest()

def checked_artifacts():
    binary = artifact("OC2_TEST_BINARY", True)
    library = artifact("OC2_TEST_NATIVE_LIBRARY")
    manifest = artifact("OC2_TEST_BUILD_JSON")
    data = json.loads(pathlib.Path(manifest).read_text())
    expected = (data.get("binary_sha256") or data.get("binarySha256") or "").lower()
    if len(expected) != 64 or sha256(binary) != expected:
        raise AssertionError("OC2_TEST_BINARY does not match OC2_TEST_BUILD_JSON")
    if pathlib.Path(library).suffix not in (".dylib", ".so"):
        raise AssertionError("OC2_TEST_NATIVE_LIBRARY must be .dylib or .so")
    return binary, library

class Endpoint(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True
    def __init__(self, address):
        super().__init__(address, Handler)
        self.api_requests = 0
        self.health_requests = 0

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            self.server.health_requests += 1
            body = json.dumps({"schema_version": 1, "status": "ok", "pid": os.getpid()}).encode()
            self.send_response(200)
        elif self.path.startswith("/api/"):
            self.server.api_requests += 1
            body = b'{"sessions":[]}'
            self.send_response(200)
        else:
            body = b"not found"
            self.send_response(404)
        if len(body) > MAX_WIRE: raise AssertionError("fixture response exceeded wire bound")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers(); self.wfile.write(body)
    def log_message(self, *_): pass

def read_pty(fd, deadline, buf):
    while time.monotonic() < deadline and len(buf) < MAX_PTY:
        ready, _, _ = select.select([fd], [], [], .1)
        if not ready: continue
        chunk = os.read(fd, min(65536, MAX_PTY - len(buf)))
        if not chunk: break
        buf.extend(chunk)

def run():
    binary, library = checked_artifacts()
    with tempfile.TemporaryDirectory(prefix="oc2-disc101-") as root:
        root = pathlib.Path(root); home = root / "home"; data = root / "data"; project = root / "project"
        for path in (home, data, project, root / "config", root / "cache", root / "runtime"): path.mkdir()
        endpoint = Endpoint(("127.0.0.1", 0)); endpoint.timeout = 1
        origin = f"http://127.0.0.1:{endpoint.server_address[1]}"
        import threading
        thread = threading.Thread(target=endpoint.serve_forever, daemon=True); thread.start()
        try:
            sleeper = subprocess.Popen(["/bin/sleep", "30"], start_new_session=True,
                                       stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                token = "a" * 64
                runtime = data / "runtime"; runtime.mkdir()
                (runtime / "backend.json").write_text(json.dumps({
                    "pid": sleeper.pid, "http_origin": origin, "schema_version": 1, "auth_token": token
                }))
                master, slave = pty.openpty(); saved = termios.tcgetattr(master); buf = bytearray()
                env = {"HOME": str(home), "PATH": os.environ.get("PATH", ""), "TERM": "xterm-256color",
                       "XDG_CONFIG_HOME": str(root / "config"), "XDG_DATA_HOME": str(root / "cache"),
                       "XDG_RUNTIME_DIR": str(root / "runtime"), "OPENCODE_RK_HOME": str(data),
                       "OPENCODE_PROJECT_DIR": str(project), "OC2_NATIVE_LIBRARY": library, "OC2_OFFLINE": "1"}
                child = subprocess.Popen([binary, "--native", "--data-dir", str(data)], cwd=project,
                                         env=env, stdin=slave, stdout=slave, stderr=slave,
                                         start_new_session=True, close_fds=True)
                os.close(slave)
                try:
                    read_pty(master, time.monotonic() + TIMEOUT, buf)
                    if child.poll() is not None: raise AssertionError("native caller exited before discovery")
                    # Health is the only permitted foreign-case contact. Any
                    # API contact proves attach occurred before identity denial.
                    assert endpoint.health_requests >= 1, "native caller did not authenticate/probe health"
                    assert endpoint.api_requests == 0, "native caller trusted forged descriptor before identity denial"
                finally:
                    if child.poll() is None:
                        os.killpg(child.pid, signal.SIGTERM)
                        try: child.wait(timeout=2)
                        except subprocess.TimeoutExpired:
                            os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=2)
                    termios.tcsetattr(master, termios.TCSANOW, saved); os.close(master)
            finally:
                if sleeper.poll() is None:
                    sleeper.kill()
                sleeper.wait(timeout=2)
        finally:
            endpoint.shutdown(); endpoint.server_close(); thread.join(timeout=2)

if __name__ == "__main__": run()

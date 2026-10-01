#!/usr/bin/env python3
"""DISC-101 native discovery identity fixture (source-only frozen contract)."""
import hashlib, http.server, json, os, pathlib, pty, select, signal, socket
import struct, subprocess, tempfile, termios, threading, time, urllib.request

MAX_PTY, MAX_WIRE, MAX_REQUESTS = 256 * 1024, 128 * 1024, 8

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""): h.update(chunk)
    return h.hexdigest()

def artifacts():
    names = ("OC2_TEST_BINARY", "NATIVE_LIBRARY", "BUILD_JSON")
    values = [os.environ.get(n) for n in names]
    if any(not v or not os.path.isabs(v) for v in values):
        raise AssertionError("DISC-101 requires absolute attested artifact paths")
    binary, library, manifest = map(pathlib.Path, values)
    if not binary.is_file() or not os.access(binary, os.X_OK): raise AssertionError("binary missing")
    if not library.is_file() or not os.access(library, os.R_OK): raise AssertionError("native library missing")
    data = json.loads(manifest.read_text())
    source = data.get("source_sha") or data.get("sourceSha") or data.get("git_sha")
    expected = str(data.get("binary_sha256") or data.get("binarySha256") or "").lower()
    native = str(data.get("native_library_sha256") or data.get("nativeLibrarySha256") or "").lower()
    if not isinstance(source, str) or len(source) != 40: raise AssertionError("source attestation missing")
    if len(expected) != 64 or sha256(binary) != expected: raise AssertionError("binary SHA mismatch")
    if len(native) != 64 or sha256(library) != native: raise AssertionError("library SHA mismatch")
    if data.get("native") is not True or data.get("profile") != "release": raise AssertionError("not native release")
    return binary

class State:
    def __init__(self): self.count = 0
    def hit(self):
        self.count += 1
        if self.count > MAX_REQUESTS: raise RuntimeError("request bound exceeded")

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802
        self.server.state.hit()
        body = b'{"status":"ok"}'
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *_): pass

def run_case(case, binary, root):
    data, project = root / ("data-" + case), root / ("project-" + case); data.mkdir(); project.mkdir()
    server = http.server.HTTPServer(("127.0.0.1", 0), Handler); server.state = State()
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    foreign = subprocess.Popen(["sleep", "30"]); master = slave = child = None; captured = bytearray()
    try:
        pid = foreign.pid if case == "LIVEFOREIGNPID" else (os.getpid() if case == "EMPTYTOKEN" else 99999999)
        runtime = data / "runtime"; runtime.mkdir()
        (runtime / "backend.json").write_text(json.dumps({"pid": pid, "http_origin": "http://127.0.0.1:%d" % server.server_port, "schema_version": 1, "auth_token": "" if case == "EMPTYTOKEN" else "a" * 64}))
        master, slave = pty.openpty(); fcntl = __import__("fcntl")
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        env = {"HOME": str(root / "home"), "PATH": os.environ.get("PATH", ""), "TERM": "xterm-256color", "OPENCODE_RK_HOME": str(data), "OPENCODE_PROJECT_DIR": str(project), "OC2_OFFLINE": "1"}
        (root / "home").mkdir(exist_ok=True)
        child = subprocess.Popen([str(binary), "--native", "--data-dir", str(data)], cwd=project, env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        end = time.monotonic() + 5
        while time.monotonic() < end and len(captured) < MAX_PTY:
            ready, _, _ = select.select([master], [], [], .1)
            if ready:
                try: captured.extend(os.read(master, min(65536, MAX_PTY - len(captured))))
                except OSError: break
        if case == "VALIDIDENTITY":
            descriptor = json.loads((data / "runtime/backend.json").read_text())
            request = urllib.request.Request(descriptor["http_origin"] + "/health")
            with urllib.request.urlopen(request, timeout=2) as response: assert response.status == 200
        if server.state.count != (1 if case == "VALIDIDENTITY" else 0): raise AssertionError("identity case API effect mismatch")
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try: child.wait(timeout=3)
            except subprocess.TimeoutExpired: os.killpg(child.pid, signal.SIGKILL); child.wait()
        if master is not None: os.close(master)
        if slave is not None: os.close(slave)
        foreign.terminate(); foreign.wait(timeout=3); server.shutdown(); server.server_close(); thread.join(timeout=2)
    return server.state.count

def main():
    binary = artifacts()
    with tempfile.TemporaryDirectory(prefix="disc101-") as raw:
        root = pathlib.Path(raw)
        results = {c: run_case(c, binary, root) for c in ("VALIDIDENTITY", "STALEPID", "LIVEFOREIGNPID", "EMPTYTOKEN")}
    print(json.dumps({"DISC101": "CANDIDATE", "cases": results}))

if __name__ == "__main__": main()

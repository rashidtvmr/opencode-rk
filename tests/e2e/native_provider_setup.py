#!/usr/bin/env python3
"""G2 contract: fresh native app, in-app API-key setup, model choice, and resume.

This is intentionally a contract harness, not a product implementation.  It
uses only stdlib and a loopback Responses fixture; credentials are generated in
the fixture and are never written by this test before the UI enters them.
"""
import argparse, hashlib, http.server, json, os, pathlib, pty, re, select
import shutil, signal, socket, subprocess, tempfile, termios, time, urllib.parse

MAX_PTY = 256 * 1024
MAX_REQUEST = 128 * 1024
MAX_RESPONSE = 256 * 1024
KEY = "fixture-generated-key-7f3a"
MODEL = "gpt-5.6-mini"
PROMPTS = ["native provider contract first", "native provider contract second"]

def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def checked_artifact(binary, build_json):
    p = pathlib.Path(binary).resolve()
    if not p.is_absolute() or not p.is_file(): raise RuntimeError("--binary must be an absolute file")
    m = json.loads(pathlib.Path(build_json).read_text())
    source = m.get("source_sha") or m.get("sourceSha") or m.get("git_sha")
    expected = (m.get("binary_sha256") or m.get("binarySha256") or "").lower()
    if not isinstance(source, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", source):
        raise RuntimeError("build manifest lacks an attested 40-hex source SHA")
    if not re.fullmatch(r"[0-9a-f]{64}", expected) or digest(p) != expected:
        raise RuntimeError("installed binary does not match build manifest")
    return p, source, expected

class State:
    def __init__(self): self.requests = []; self.error = None
    def body(self, headers, raw):
        if len(raw) > MAX_REQUEST: raise RuntimeError("provider request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY: raise RuntimeError("wrong provider authorization")
        value = json.loads(raw)
        if value.get("model") != MODEL: raise RuntimeError("chosen non-default model did not reach provider")
        users = [x.get("content") for x in value.get("input", []) if x.get("role") == "user"]
        expected = PROMPTS[:len(self.requests) + 1]
        if users != expected: raise RuntimeError("request history did not survive selection/restart")
        self.requests.append(value)
        text = "G2 first response" if len(self.requests) == 1 else "G2 resumed response"
        out = ('event: response.output_text.delta\ndata: ' + json.dumps({'delta': text}) +
               '\n\nevent: response.completed\ndata: {"type":"response.completed"}\n\n').encode()
        if len(out) > MAX_RESPONSE: raise RuntimeError("provider response exceeded 256 KiB")
        return out

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path != "/v1/responses": raise RuntimeError("unexpected provider endpoint")
            n = int(self.headers.get("Content-Length", "-1"))
            if n < 0 or n > MAX_REQUEST: raise RuntimeError("invalid request length")
            raw = self.rfile.read(n)
            if len(raw) != n: raise RuntimeError("truncated provider request")
            out = self.server.state.body(self.headers, raw)
            self.send_response(200); self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
        except Exception as e:
            self.server.state.error = str(e); self.send_error(500, str(e))
    def log_message(self, *_): pass

def frame(server, models):
    return {"openai": {"name": "OpenAI loopback", "models": {
        "gpt-5.6": {"name": "Default fixture", "limit": {"context": 200000}},
        MODEL: {"name": "Non-default fixture", "limit": {"context": 200000}}}}}

def read_until(fd, deadline, buf, needle=None):
    while time.monotonic() < deadline and len(buf) < MAX_PTY:
        ready, _, _ = select.select([fd], [], [], min(.1, deadline-time.monotonic()))
        if not ready: continue
        try: chunk = os.read(fd, min(65536, MAX_PTY-len(buf)))
        except OSError: return
        if not chunk: return
        buf.extend(chunk)
        if needle and needle in bytes(buf): return
    if needle and needle not in bytes(buf): raise AssertionError("native frame timeout: %r" % needle)

def run_ui(exe, env, root, first):
    master, slave = pty.openpty(); out = bytearray()
    try:
        child = subprocess.Popen([str(exe)], cwd=root / "project", env=env, stdin=slave,
                                 stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        def cmd(value, marker, timeout=15):
            os.write(master, value.encode() + b"\r"); read_until(master, time.monotonic()+timeout, out, marker)
        read_until(master, time.monotonic()+15, out, b"OpenCode")
        if first:
            cmd("/connect", b"provider")
            cmd("openai", b"API")
            before = len(out); cmd(KEY, b"model")
            recent = bytes(out)[before:]
            if KEY.encode() in recent: raise AssertionError("API key was echoed in PTY")
            cmd(MODEL, MODEL.encode())
        cmd(PROMPTS[0 if first else 1], b"G2 ")
        time.sleep(.2)
        if child.poll() is not None: raise AssertionError("native process exited during turn")
        if not first and PROMPTS[0].encode() not in bytes(out): raise AssertionError("history absent after restart")
        os.write(master, b"\x03"); child.wait(timeout=10)
        if child.returncode != 0: raise AssertionError("native exit failed: %s" % child.returncode)
        return bytes(out)
    finally:
        try: os.close(slave)
        except (OSError, TypeError): pass
        try:
            if 'child' in locals() and child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM); child.wait(timeout=3)
        except (OSError, subprocess.TimeoutExpired): pass
        try: os.close(master)
        except OSError: pass

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True); ap.add_argument("--native-library", required=True)
    ap.add_argument("--build-json", required=True); ap.add_argument("--artifact-dir", required=True)
    a = ap.parse_args(); binary, source, binary_sha = checked_artifact(a.binary, a.build_json)
    lib = pathlib.Path(a.native_library).resolve()
    if not lib.is_file() or lib.suffix not in (".dylib", ".so"): raise RuntimeError("invalid native library")
    artifacts = pathlib.Path(a.artifact_dir).resolve(); artifacts.mkdir(parents=True, exist_ok=True)
    root = pathlib.Path(tempfile.mkdtemp(prefix="g2-native-provider-")); install = root/"install"; (install/"bin").mkdir(parents=True); (install/"lib").mkdir()
    exe = install/"bin"/"oc2"; shutil.copyfile(binary, exe); shutil.copyfile(lib, install/"lib"/lib.name); os.chmod(exe, 0o755)
    for name in ("home", "data", "project", "models"): (root/name).mkdir()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler); server.state = State()
    import threading; thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    models = root/"models"/"models.json"; models.write_text(json.dumps(frame(server, models)))
    env = {"HOME": str(root/"home"), "XDG_CONFIG_HOME": str(root/"home"/".config"), "XDG_DATA_HOME": str(root/"data"),
           "XDG_RUNTIME_DIR": str(root/"data"/"runtime"), "OPENCODE_PROJECT_DIR": str(root/"project"), "PATH": "/usr/bin:/bin", "TERM": "xterm-256color",
           "OPENCODE_MODELS_PATH": str(models), "OPENAI_BASE_URL": "http://127.0.0.1:%d/v1" % server.server_port}
    (root/"home"/".config").mkdir(); (root/"data"/"runtime").mkdir()
    try:
        run_ui(exe, env, root, True)
        auth = root/"data"/"opencode"/"auth.json"
        if not auth.is_file() or (auth.stat().st_mode & 0o777) != 0o600: raise AssertionError("UI did not create 0600 auth.json")
        saved = json.loads(auth.read_text()).get("openai", {})
        if saved.get("type") != "api" or saved.get("key") != KEY: raise AssertionError("wrong persisted API auth")
        run_ui(exe, env, root, False)
        if server.state.error: raise AssertionError(server.state.error)
        if len(server.state.requests) != 2: raise AssertionError("expected two actual provider requests")
        (artifacts/"native-provider-evidence.json").write_text(json.dumps({"source_sha":source,"binary_sha256":binary_sha,"model":MODEL,"requests":[{"model":x.get("model"),"input":x.get("input")} for x in server.state.requests]}, indent=2)+"\n")
    finally:
        server.shutdown(); server.server_close(); thread.join(timeout=2); shutil.rmtree(root, ignore_errors=True)

if __name__ == "__main__": main()

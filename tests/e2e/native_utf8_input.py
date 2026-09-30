#!/usr/bin/env python3
"""G5 RED contract: native PTY input preserves UTF-8 scalar values."""
import argparse, hashlib, http.server, json, os, pathlib, pty, shutil, sys
import struct, subprocess, tempfile, termios, threading, time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (  # noqa: E402
    HOST_LIBRARY_SUFFIX, MAX_PTY, TerminalScreen, checked_artifact, descriptor,
    free_loopback_port, read_until,
)
from native_stream_tool import reap_cli_then_daemon  # noqa: E402

MAX_REQUEST, MAX_RESPONSE, SOCKET_LIMIT, TIMEOUT = 128 * 1024, 256 * 1024, 100, 12
KEY, MODEL = "fixture-native-utf8-key-31c9", "gpt-5.6"
PROMPT, RESUMED, ASSISTANT = "native café 日本語 😀", "native utf8 resumed", "UTF8_FIRST_RESPONSE"

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(65536), b""): h.update(block)
    return h.hexdigest()

def sse(text):
    return ("event: response.output_text.delta\ndata: " + json.dumps({"type":"response.output_text.delta", "delta":text}, ensure_ascii=False) +
            "\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n").encode()

class State:
    def __init__(self): self.requests, self.error, self.restart_generation = [], None, 0
    def body(self, headers, raw):
        if len(raw) > MAX_REQUEST: raise RuntimeError("request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY: raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw.decode("utf-8"))
        if value.get("model") != MODEL or not isinstance(value.get("input"), list): raise RuntimeError("invalid model/input")
        if value.get("stream") is not True: raise RuntimeError("native request was not true stream mode")
        if len(self.requests) >= 2: raise RuntimeError("provider request count exceeded two")
        users = [x.get("content") for x in value["input"] if x.get("role") == "user"]
        assistants = [x.get("content") for x in value["input"] if x.get("role") == "assistant"]
        if len(self.requests) == 0:
            if users != [PROMPT] or assistants: raise RuntimeError("first exact Unicode input/history mismatch")
        else:
            if users != [PROMPT, RESUMED] or assistants != [ASSISTANT]: raise RuntimeError("restarted durable Unicode history mismatch")
            if self.restart_generation != 1: raise RuntimeError("second request was not after restart")
        self.requests.append(value)
        return sse(ASSISTANT if len(self.requests) == 1 else "UTF8_RESUMED_RESPONSE")

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path != "/v1/responses": raise RuntimeError("unexpected provider route")
            n = int(self.headers.get("Content-Length", "-1"))
            if n < 0 or n > MAX_REQUEST: raise RuntimeError("invalid content length")
            raw = self.rfile.read(n)
            if len(raw) != n: raise RuntimeError("truncated request")
            payload = self.server.state.body(self.headers, raw)
            if len(payload) > MAX_RESPONSE: raise RuntimeError("response exceeded 256 KiB")
            self.send_response(200); self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(payload))); self.send_header("Connection", "close"); self.end_headers()
            self.wfile.write(payload); self.wfile.flush()
        except Exception as exc:
            self.server.state.error = str(exc)
            try: self.send_error(500, str(exc))
            except OSError: pass
    def log_message(self, *_): pass

class Fixture(http.server.HTTPServer):
    allow_reuse_address = True
    def get_request(self):
        conn, addr = super().get_request(); conn.settimeout(5); return conn, addr

def write_fragments(master):
    for value in ("native ", "é", " ", "日本語", " ", "😀"):
        for byte in value.encode("utf-8"):
            os.write(master, bytes([byte])); time.sleep(.012)
    for byte in " 😀".encode("utf-8"):
        os.write(master, bytes([byte])); time.sleep(.012)
    os.write(master, b"\x7f")

def launch(exe, root, env, master, slave):
    return subprocess.Popen([str(exe), "--data-dir", str(root / "data")], cwd=root / "project", env=env,
                            stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)

def run(binary, library, manifest, artifacts):
    binary, library, source, binary_sha, library_sha = checked_artifact(binary, library, manifest)
    root = pathlib.Path(tempfile.mkdtemp(prefix="nutf8-", dir=os.environ.get("TMPDIR", "/tmp")))
    install = root / "install"; (install / "bin").mkdir(parents=True); (install / "lib").mkdir()
    shutil.copyfile(binary, install / "bin" / "oc2"); shutil.copyfile(library, install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX)); os.chmod(install / "bin" / "oc2", 0o755)
    for name in ("home", "project", "data"): (root / name).mkdir()
    (root / "data" / "runtime").mkdir()
    if len(os.fsencode(root / "data" / "runtime" / "opencode-rk.sock")) > SOCKET_LIMIT: raise RuntimeError("socket path exceeds 100 bytes")
    auth = root / "data" / "opencode" / "auth.json"; auth.parent.mkdir(); auth.write_text(json.dumps({"openai":{"type":"api","key":KEY}})+"\n"); os.chmod(auth, 0o600)
    catalog = root / "data" / "catalog" / "models.dev.api.json"; catalog.parent.mkdir(); catalog.write_text(json.dumps({"openai":{"name":"fixture","models":{MODEL:{"name":"UTF8 fixture","limit":{"context":200000}}}}})+"\n")
    state = State(); server = Fixture(("127.0.0.1", 0), Handler); server.state = state
    thread = threading.Thread(target=server.serve_forever, daemon=False); thread.start()
    daemon = None; master = slave = None; descriptors = []; captures = bytearray(); screen = TerminalScreen(); saved_attrs = None
    try:
        port = free_loopback_port()
        env = {"HOME":str(root/"home"), "PATH":os.environ.get("PATH", ""), "TERM":"xterm-256color", "TMPDIR":str(root), "XDG_CONFIG_HOME":str(root/"home"/".config"), "XDG_DATA_HOME":str(root/"data"), "XDG_RUNTIME_DIR":str(root/"data"/"runtime"), "OPENCODE_RK_HOME":str(root/"data"), "OPENCODE_PROJECT_DIR":str(root/"project"), "OPENCODE_RK_DAEMON_ADDR":f"127.0.0.1:{port}", "OPENAI_BASE_URL":f"http://127.0.0.1:{server.server_port}/v1"}
        (root/"home"/".config").mkdir(); (root/"home"/"xdg").mkdir()
        master, slave = pty.openpty(); saved_attrs = termios.tcgetattr(master); import fcntl; fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH",24,80,0,0))
        daemon = launch(install/"bin"/"oc2", root, env, master, slave); os.close(slave); slave = None
        d = descriptor(root/"data", port, daemon, time.monotonic()+TIMEOUT); descriptors.append(d)
        read_until(master, time.monotonic()+TIMEOUT, captures, b"OpenCode", 0, screen)
        write_fragments(master); os.write(master, b"\r")
        read_until(master, time.monotonic()+TIMEOUT, captures, ASSISTANT.encode(), len(captures), screen, lambda: screen.contains(ASSISTANT))
        if state.error or len(state.requests) != 1: raise AssertionError(state.error or "expected one first request")
        reap_cli_then_daemon(daemon, d, master); daemon = None; os.close(master); master = None
        master, slave = pty.openpty(); fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH",24,80,0,0)); daemon = launch(install/"bin"/"oc2", root, env, master, slave); os.close(slave); slave=None
        d2 = descriptor(root/"data", port, daemon, time.monotonic()+TIMEOUT); descriptors.append(d2); state.restart_generation=1; screen=TerminalScreen(); start=len(captures)
        read_until(master, time.monotonic()+TIMEOUT, captures, b"OpenCode", start, screen, lambda: screen.contains(PROMPT) and screen.has_model_status())
        for byte in RESUMED.encode("utf-8"): os.write(master, bytes([byte])); time.sleep(.008)
        os.write(master, b"\r"); read_until(master, time.monotonic()+TIMEOUT, captures, b"UTF8_RESUMED_RESPONSE", start, screen, lambda: screen.contains("UTF8_RESUMED_RESPONSE"))
        if state.error or len(state.requests) != 2: raise AssertionError(state.error or "expected exactly two requests")
        (artifacts/"native-utf8-result.json").write_text(json.dumps({"source_sha":source,"binary_sha256":binary_sha,"native_library_sha256":library_sha,"requests":state.requests}, ensure_ascii=False, indent=2)+"\n")
    except BaseException as error:
        state.error = state.error or str(error); raise
    finally:
        if daemon is not None:
            try: reap_cli_then_daemon(daemon, descriptors[-1] if descriptors else None, master)
            except BaseException as error: state.error = state.error or "owned cleanup failed: " + str(error)
        if master is not None and saved_attrs is not None:
            try: termios.tcsetattr(master, termios.TCSANOW, saved_attrs)
            except OSError: pass
        if slave is not None: os.close(slave)
        if master is not None: os.close(master)
        server.shutdown(); server.server_close(); thread.join(timeout=10)
        if thread.is_alive(): raise RuntimeError("fixture server failed bounded shutdown")
        if state.error: raise AssertionError(state.error)

def main():
    p=argparse.ArgumentParser(); p.add_argument("--binary"); p.add_argument("--native-library"); p.add_argument("--build-json"); p.add_argument("--artifact-dir"); p.add_argument("--self-check", action="store_true"); a=p.parse_args()
    if a.self_check: print("source-only self-check: bounded UTF-8 contract syntax loaded"); return
    if not all((a.binary,a.native_library,a.build_json,a.artifact_dir)): p.error("actual run requires --binary --native-library --build-json --artifact-dir")
    artifacts=pathlib.Path(a.artifact_dir); artifacts.mkdir(parents=True, exist_ok=True); run(pathlib.Path(a.binary), pathlib.Path(a.native_library), pathlib.Path(a.build_json), artifacts)

if __name__ == "__main__": main()

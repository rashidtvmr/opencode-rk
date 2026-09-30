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
    def __init__(self): self.requests, self.error, self.semantic_errors, self.restart_generation = [], None, [], 0
    def body(self, headers, raw):
        if len(raw) > MAX_REQUEST: raise RuntimeError("request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY: raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw.decode("utf-8"))
        if value.get("model") != MODEL or not isinstance(value.get("input"), list): raise RuntimeError("invalid model/input")
        if value.get("stream") is not True: raise RuntimeError("native request was not true stream mode")
        if len(self.requests) >= 2: raise RuntimeError("provider request count exceeded two")
        users = [x.get("content") for x in value["input"] if x.get("role") == "user"]
        assistants = [x.get("content") for x in value["input"] if x.get("role") == "assistant"]
        self.requests.append(value)
        expected = ([PROMPT], []) if len(self.requests) == 1 else ([PROMPT, RESUMED], [ASSISTANT])
        if users != expected[0] or assistants != expected[1]:
            self.semantic_errors.append({"request": len(self.requests), "users": users, "assistants": assistants,
                                         "expected_users": expected[0], "expected_assistants": expected[1]})
        if len(self.requests) == 2 and self.restart_generation != 1:
            self.semantic_errors.append({"request": 2, "error": "request was not after restart"})
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
    for value in ("native caf", "é", " ", "日本語", " ", "😀"):
        for byte in value.encode("utf-8"):
            os.write(master, bytes([byte])); time.sleep(.012)
    for byte in "🧪".encode("utf-8"):
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
    catalog = root / "data" / "catalog" / "models.dev.api.json"; catalog.parent.mkdir(); catalog.write_text(json.dumps({"openai":{"name":"fixture","models":{MODEL:{"name":"UTF8 fixture","limit":{"context":200000}}, "gpt-5.6-mini":{"name":"Readiness fixture","limit":{"context":200000}}}}})+"\n")
    state = State(); server = Fixture(("127.0.0.1", 0), Handler); server.state = state
    thread = threading.Thread(target=server.serve_forever, daemon=False); thread.start()
    daemon = None; master = slave = None; descriptors = []; captures = bytearray(); screen = TerminalScreen(); saved_attrs = None; second_saved_attrs = None; raw_seen = []; cleanup_error = None
    restored = []; cli_exits = []; thread_joined = False; daemons_gone = False
    try:
        port = free_loopback_port()
        env = {"HOME":str(root/"home"), "PATH":os.environ.get("PATH", ""), "TERM":"xterm-256color", "TMPDIR":str(root), "XDG_CONFIG_HOME":str(root/"home"/".config"), "XDG_DATA_HOME":str(root/"data"), "XDG_RUNTIME_DIR":str(root/"data"/"runtime"), "OPENCODE_RK_HOME":str(root/"data"), "OPENCODE_PROJECT_DIR":str(root/"project"), "OPENCODE_RK_DAEMON_ADDR":f"127.0.0.1:{port}", "OPENAI_BASE_URL":f"http://127.0.0.1:{server.server_port}/v1"}
        (root/"home"/".config").mkdir(); (root/"home"/"xdg").mkdir()
        master, slave = pty.openpty(); saved_attrs = termios.tcgetattr(master); import fcntl; fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH",24,80,0,0))
        daemon = launch(install/"bin"/"oc2", root, env, master, slave); os.close(slave); slave = None
        d = descriptor(root/"data", port, daemon, time.monotonic()+TIMEOUT); descriptors.append(d)
        read_until(master, time.monotonic()+TIMEOUT, captures, b"OpenCode", 0, screen, lambda: screen.contains("model: openai/" + MODEL))
        raw_seen.append(not (termios.tcgetattr(master)[3] & termios.ICANON))
        write_fragments(master); os.write(master, b"\r")
        read_until(master, time.monotonic()+TIMEOUT, captures, ASSISTANT.encode(), len(captures), screen, lambda: screen.contains(ASSISTANT))
        if state.error or len(state.requests) != 1 or state.semantic_errors: raise AssertionError(state.error or state.semantic_errors or "expected one first request")
        reap_cli_then_daemon(daemon, d, master)
        cli_exits.append(daemon.returncode)
        restored.append(termios.tcgetattr(master) == saved_attrs)
        if not restored[-1]: raise AssertionError("first native PTY was not restored")
        daemon = None; os.close(master); master = None
        master, slave = pty.openpty(); second_saved_attrs = termios.tcgetattr(master); fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH",24,80,0,0)); daemon = launch(install/"bin"/"oc2", root, env, master, slave); os.close(slave); slave=None
        d2 = descriptor(root/"data", port, daemon, time.monotonic()+TIMEOUT); descriptors.append(d2); state.restart_generation=1; screen=TerminalScreen(); start=len(captures)
        read_until(master, time.monotonic()+TIMEOUT, captures, b"OpenCode", start, screen, lambda: screen.contains("model: openai/" + MODEL) and screen.contains(ASSISTANT))
        raw_seen.append(not (termios.tcgetattr(master)[3] & termios.ICANON))
        for byte in RESUMED.encode("utf-8"): os.write(master, bytes([byte])); time.sleep(.008)
        os.write(master, b"\r"); read_until(master, time.monotonic()+TIMEOUT, captures, b"UTF8_RESUMED_RESPONSE", start, screen, lambda: screen.contains("UTF8_RESUMED_RESPONSE"))
        if state.error or len(state.requests) != 2 or state.semantic_errors: raise AssertionError(state.error or state.semantic_errors or "expected exactly two requests")
    except BaseException as error:
        state.error = state.error or str(error); raise
    finally:
        if daemon is not None:
            try: reap_cli_then_daemon(daemon, descriptors[-1] if descriptors else None, master)
            except BaseException as error: cleanup_error = cleanup_error or "owned cleanup failed: " + str(error)
            cli_exits.append(daemon.returncode)
            expected_attrs = second_saved_attrs if second_saved_attrs is not None else saved_attrs
            if master is not None and expected_attrs is not None:
                try: restored.append(termios.tcgetattr(master) == expected_attrs)
                except OSError: restored.append(False)
        if slave is not None: os.close(slave)
        if master is not None: os.close(master)
        server.shutdown(); server.server_close(); thread.join(timeout=10); thread_joined = not thread.is_alive()
        daemons_gone = True
        for item in descriptors:
            try: os.kill(item["_validated_pid"], 0)
            except ProcessLookupError: pass
            except PermissionError: daemons_gone = False
            else: daemons_gone = False
        secret_echo = KEY.encode() in captures or KEY in screen.text()
        termios_ok = bool(restored) and all(restored)
        cli_ok = bool(cli_exits) and all(code == 0 for code in cli_exits)
        lifecycle_ok = termios_ok and cli_ok and daemons_gone and thread_joined and len(raw_seen) == 2 and all(raw_seen)
        success = not (state.error or cleanup_error or state.semantic_errors or secret_echo) and lifecycle_ok and len(state.requests) == 2
        evidence = {"phase":"success" if success else "failed", "error":state.error or cleanup_error or ("fixture credential echoed" if secret_echo else "" if success else "native ownership/terminal cleanup contract failed"), "source_sha":source, "binary_sha256":binary_sha,"native_library_sha256":library_sha, "test_sha256":sha256(pathlib.Path(__file__)), "requests":state.requests, "semantic_errors":state.semantic_errors, "raw_mode_seen":raw_seen, "pty_restoration_checks":restored, "cli_exit_codes":cli_exits, "termios_restored":termios_ok, "cli_reaped_zero":cli_ok, "validated_daemons_gone":daemons_gone, "provider_thread_joined":thread_joined, "max_captured_bytes":len(captures), "captured_bytes":bytes(captures).replace(KEY.encode(), b"[REDACTED]").decode("utf-8", "replace")}
        raw = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
        if len(raw.encode()) > 512 * 1024: raise RuntimeError("evidence exceeded 512 KiB")
        (artifacts/("native-utf8-result.json" if evidence["phase"] == "success" else "native-utf8-failure.json")).write_text(raw)
        if not success: raise AssertionError(evidence["error"])

def main():
    p=argparse.ArgumentParser(); p.add_argument("--binary"); p.add_argument("--native-library"); p.add_argument("--build-json"); p.add_argument("--artifact-dir"); p.add_argument("--self-check", action="store_true"); a=p.parse_args()
    if a.self_check:
        state = State(); headers = {"Authorization":"Bearer "+KEY}
        first = json.dumps({"model":MODEL,"input":[{"role":"user","content":PROMPT}],"stream":True}).encode()
        assert b"response.completed" in state.body(headers, first) and state.requests[0]["input"][0]["content"] == PROMPT
        state.restart_generation = 1
        second = json.dumps({"model":MODEL,"input":[{"role":"user","content":PROMPT},{"role":"assistant","content":ASSISTANT},{"role":"user","content":RESUMED}],"stream":True}).encode()
        assert b"response.completed" in state.body(headers, second) and not state.semantic_errors
        try: state.body(headers, second); raise AssertionError("third request accepted")
        except RuntimeError: pass
        mismatch = State(); mismatch.body(headers, json.dumps({"model":MODEL,"input":[{"role":"user","content":"mojibake"}],"stream":True}).encode())
        assert len(mismatch.requests) == 1 and mismatch.semantic_errors
        try: State().body({"Authorization":"Bearer wrong"}, first); raise AssertionError("wrong auth accepted")
        except RuntimeError: pass
        print("self-check: bounded SSE, exact Unicode state, semantic mismatch recording, request cap, and auth negative passed"); return
    if not all((a.binary,a.native_library,a.build_json,a.artifact_dir)): p.error("actual run requires --binary --native-library --build-json --artifact-dir")
    artifacts=pathlib.Path(a.artifact_dir); artifacts.mkdir(parents=True, exist_ok=True); run(pathlib.Path(a.binary), pathlib.Path(a.native_library), pathlib.Path(a.build_json), artifacts)

if __name__ == "__main__": main()

#!/usr/bin/env python3
"""G2 contract: native /connect, API key, non-default model, and restart.

This file is a test owner artifact, not product code.  It deliberately refuses
to turn missing build/source/runtime evidence into a product RED.
"""
import argparse, hashlib, http.client, http.server, json, os, pathlib, pty
import codecs, fcntl, re, select, shutil, signal, socket, struct, subprocess, sys, tempfile, termios, time
import urllib.error, urllib.request

MAX_PTY, MAX_REQUEST, MAX_RESPONSE = 256 * 1024, 128 * 1024, 256 * 1024
KEY, MODEL = "fixture-generated-key-7f3a", "gpt-5.6-mini"
PROMPTS = ["native provider contract first", "native provider contract second"]
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
HOST_LIBRARY_SUFFIX = ".dylib" if sys.platform == "darwin" else ".so"

class TerminalScreen:
    """Small bounded ANSI screen model; assertions use visible cells, not diffs."""
    def __init__(self, cols=80, rows=24):
        self.cols, self.rows = cols, rows
        self.cells = [[" "] * cols for _ in range(rows)]
        self.row = self.col = 0
        self.saved = (0, 0)
        self.pending = bytearray()
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")

    def _put(self, char):
        if char == "\n": self.row = min(self.rows - 1, self.row + 1); self.col = 0; return
        if char == "\r": self.col = 0; return
        if char == "\b": self.col = max(0, self.col - 1); return
        if ord(char) < 0x20: return
        if self.col >= self.cols: self.row = min(self.rows - 1, self.row + 1); self.col = 0
        self.cells[self.row][self.col] = char
        self.col += 1
        if KEY in self.text():
            raise AssertionError("fixture API key appeared in visible terminal cells")

    def _csi(self, raw):
        final = chr(raw[-1]); body = raw[:-1]
        if body and body[0] in b"?><=": return
        nums = body.decode("ascii", "ignore")
        args = [int(x) if x else 1 for x in nums.split(";")] if nums else [1]
        if final not in "HfGABCDJKsu": return
        if final in "Hf": self.row = max(0, min(self.rows - 1, (args[0] if len(args)>0 else 1)-1)); self.col = max(0, min(self.cols - 1, (args[1] if len(args)>1 else 1)-1))
        elif final == "G": self.col = max(0, min(self.cols - 1, args[0]-1))
        elif final == "A": self.row = max(0, self.row-args[0])
        elif final == "B": self.row = min(self.rows-1, self.row+args[0])
        elif final == "C": self.col = min(self.cols-1, self.col+args[0])
        elif final == "D": self.col = max(0, self.col-args[0])
        elif final == "J":
            if args[0] == 0:
                self.cells[self.row][self.col:] = [" "] * (self.cols - self.col)
                for row in range(self.row + 1, self.rows): self.cells[row] = [" "] * self.cols
            elif args[0] == 1:
                for row in range(self.row): self.cells[row] = [" "] * self.cols
                self.cells[self.row][:self.col + 1] = [" "] * (self.col + 1)
            elif args[0] == 2: self.cells = [[" "]*self.cols for _ in range(self.rows)]
        elif final == "K":
            if args[0] == 0: self.cells[self.row][self.col:] = [" "] * (self.cols - self.col)
            elif args[0] == 1: self.cells[self.row][:self.col + 1] = [" "] * (self.col + 1)
            elif args[0] == 2: self.cells[self.row] = [" "] * self.cols
        elif final == "s" and not body[:1] in b"?><=": self.saved = (self.row, self.col)
        elif final == "u" and not body[:1] in b"?><=": self.row, self.col = self.saved

    def feed(self, chunk):
        self.pending.extend(chunk)
        data = bytes(self.pending); self.pending.clear(); i = 0; plain = bytearray()
        while i < len(data):
            if data[i] != 0x1b: plain.append(data[i]); i += 1; continue
            if plain: self._feed_text(bytes(plain)); plain.clear()
            if i+1 >= len(data): self.pending.extend(data[i:]); break
            if data[i+1] == ord("["):
                match = re.match(rb"\x1b\[([0-?]*[ -/]*[@-~])", data[i:])
                if not match: self.pending.extend(data[i:]); break
                self._csi(match.group(1)); i += len(match.group(0)); continue
            if data[i+1] in (ord("]"), ord("P"), ord("^"), ord("_") ):
                end = data.find(b"\x07", i+2); terminator = 1
                if end < 0: end = data.find(b"\x1b\\", i+2); terminator = 2
                if end < 0:
                    self.pending.extend(data[i:])
                    break
                i = end + terminator; continue
            i += 2
        if plain: self._feed_text(bytes(plain))
        if len(self.pending) > 1024:
            raise AssertionError("terminal escape pending bound exceeded")

    def _feed_text(self, data):
        for char in self.decoder.decode(data): self._put(char)

    def text(self): return "\n".join("".join(row).rstrip() for row in self.cells)
    def contains(self, marker): return marker.casefold() in self.text().casefold()
    def has_model_status(self): return re.search(r"(?m)^model:\s*openai/" + re.escape(MODEL) + r"\b(?:\s*\(ctrl-p\))?\s*$", self.text()) is not None
    def has_api_prompt(self): return re.search(r"(?m)^API key \(OpenAI\)\s*$", self.text()) is not None
    def has_assistant(self, text): return text.casefold() in self.text().casefold()
    def picker_closed(self): return not self.contains("Select model")

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""): h.update(chunk)
    return h.hexdigest()

def checked_artifact(raw_binary, raw_library, raw_manifest):
    # Check the user-supplied spelling before resolve(); relative paths must not
    # become acceptable merely because resolve() returns an absolute path.
    for raw in (raw_binary, raw_library, raw_manifest):
        if not pathlib.Path(raw).is_absolute(): raise RuntimeError("artifact paths must be absolute")
    binary, library, manifest = map(lambda x: pathlib.Path(x).resolve(), (raw_binary, raw_library, raw_manifest))
    if not binary.is_file() or not os.access(binary, os.R_OK | os.X_OK): raise RuntimeError("binary is not readable/executable")
    if not library.is_file() or not os.access(library, os.R_OK) or library.suffix != HOST_LIBRARY_SUFFIX:
        raise RuntimeError("native library must be a readable .dylib or .so")
    data = json.loads(manifest.read_text())
    source = data.get("source_sha") or data.get("sourceSha") or data.get("git_sha")
    expected = (data.get("binary_sha256") or data.get("binarySha256") or "").lower()
    if not isinstance(source, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", source): raise RuntimeError("missing source SHA attestation")
    if not HEX64.fullmatch(expected) or sha256(binary) != expected: raise RuntimeError("binary SHA does not match build manifest")
    library_expected = (data.get("native_library_sha256") or data.get("nativeLibrarySha256") or data.get("library_sha256") or data.get("librarySha256"))
    if library_expected is not None and (not HEX64.fullmatch(str(library_expected)) or sha256(library) != str(library_expected).lower()):
        raise RuntimeError("native library SHA does not match build manifest")
    return binary, library, source, expected, sha256(library)

class State:
    def __init__(self): self.requests, self.error = [], None
    def body(self, headers, raw):
        if len(self.requests) >= 2: raise RuntimeError("third provider request is forbidden")
        if len(raw) > MAX_REQUEST: raise RuntimeError("request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY: raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw)
        inputs = value.get("input")
        if not isinstance(inputs, list) or any(not isinstance(x, dict) for x in inputs): raise RuntimeError("input is not a list of objects")
        if value.get("model") != MODEL: raise RuntimeError("non-default model was not selected")
        users = [x.get("content") for x in inputs if x.get("role") == "user"]
        expected_users = PROMPTS[:len(self.requests) + 1]
        expected_assistants = [] if not self.requests else ["G2 first response"]
        assistants = [x.get("content") for x in inputs if x.get("role") == "assistant"]
        if users != expected_users or assistants != expected_assistants: raise RuntimeError("settled history is not durable")
        self.requests.append(value)
        text = "G2 first response" if len(self.requests) == 1 else "G2 resumed response"
        stream = value.get("stream", False)
        if not isinstance(stream, bool):
            raise RuntimeError("stream must be boolean when present")
        if not stream:
            out = json.dumps({"id": "g2-fixture", "object": "response", "status": "completed",
                              "output": [{"type": "message", "role": "assistant",
                                          "content": [{"type": "output_text", "text": text}]}]}).encode()
            content_type = "application/json"
        else:
            out = ("event: response.output_text.delta\ndata: " + json.dumps({"type":"response.output_text.delta","delta":text}) +
                   "\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n").encode()
            content_type = "text/event-stream"
        if len(out) > MAX_RESPONSE: raise RuntimeError("response exceeded 256 KiB")
        return out, content_type

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path != "/v1/responses": raise RuntimeError("unexpected provider endpoint")
            n = int(self.headers.get("Content-Length", "-1"))
            if n < 0 or n > MAX_REQUEST: raise RuntimeError("invalid content length")
            raw = self.rfile.read(n)
            if len(raw) != n: raise RuntimeError("truncated request")
            out, content_type = self.server.state.body(self.headers, raw)
            self.send_response(200); self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(out))); self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(out)
        except Exception as exc:
            self.server.state.error = str(exc); self.send_error(500, str(exc))
    def log_message(self, *_): pass

class Fixture(http.server.HTTPServer):
    allow_reuse_address = True
    def get_request(self):
        conn, addr = super().get_request(); conn.settimeout(5); return conn, addr

def models_json():
    return {"openai":{"name":"OpenAI loopback","models":{"gpt-5.6":{"name":"Default fixture","limit":{"context":200000}}, MODEL:{"name":"Non-default fixture","limit":{"context":200000}}}}}

def sanitize(value):
    return value.replace(KEY.encode(), b"[REDACTED]")

def free_loopback_port():
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.bind(("127.0.0.1", 0)); port = sock.getsockname()[1]; sock.close()
    return port

def read_until(fd, deadline, buf, needle, start, screen=None, predicate=None):
    if predicate is None and screen is None and needle.lower() in bytes(buf[start:]).lower(): return
    while time.monotonic() < deadline and len(buf) < MAX_PTY:
        ready, _, _ = select.select([fd], [], [], min(.1, max(0, deadline-time.monotonic())))
        if not ready: continue
        try: chunk = os.read(fd, min(65536, MAX_PTY-len(buf)))
        except OSError as exc: raise AssertionError("PTY read failed before marker: %s" % exc)
        if not chunk: raise AssertionError("PTY EOF before marker %r" % needle)
        buf.extend(chunk)
        if screen is not None: screen.feed(chunk)
        if predicate is not None:
            if len(buf) > start and predicate(): return
        elif screen is not None:
            if screen.contains(needle.decode("utf-8", "replace")) and len(buf) > start: return
        elif needle.lower() in bytes(buf[start:]).lower(): return
    if len(buf) >= MAX_PTY: raise AssertionError("PTY capture bound reached before marker %r" % needle)
    raise AssertionError("PTY timeout before fresh marker %r" % needle)

def descriptor(data_root, port, child, deadline):
    path = data_root / "runtime" / "backend.json"
    while time.monotonic() < deadline:
        if child.poll() is not None: raise RuntimeError("native process exited before daemon descriptor")
        try:
            d = json.loads(path.read_text()); origin = d.get("http_origin", "")
            daemon_pid = d.get("pid")
            same_group = isinstance(daemon_pid, int) and daemon_pid > 1
            if same_group:
                try: same_group = os.getpgid(daemon_pid) == child.pid
                except OSError: same_group = False
            if same_group and origin == "http://127.0.0.1:%d" % port and HEX64.fullmatch(d.get("auth_token", "")):
                req = urllib.request.Request("http://127.0.0.1:%d/api/models?limit=20" % port, headers={"Authorization":"Bearer "+d["auth_token"]})
                with urllib.request.urlopen(req, timeout=2) as r:
                    value = json.loads(r.read(MAX_RESPONSE))
                    ids = {item.get("model_id") for item in value.get("models", [])
                           if isinstance(item, dict) and item.get("provider_id") == "openai"}
                    if r.status == 200 and {"gpt-5.6", MODEL} <= ids:
                        d["_validated_pid"] = daemon_pid
                        d["_owner_pgid"] = child.pid
                        return d
        except (OSError, ValueError, urllib.error.URLError): pass
        time.sleep(.03)
    raise RuntimeError("owned authenticated daemon/models readiness failed")

def stop_owned_daemon(descriptor_value):
    pid = descriptor_value.get("_validated_pid")
    owner_pgid = descriptor_value.get("_owner_pgid")
    if not isinstance(pid, int) or pid <= 1 or not isinstance(owner_pgid, int) or owner_pgid <= 1:
        raise RuntimeError("refusing to signal unvalidated daemon group")
    try:
        if os.getpgid(pid) != owner_pgid:
            raise RuntimeError("daemon PID group ownership changed")
        os.killpg(owner_pgid, 0)
        os.killpg(owner_pgid, signal.SIGTERM)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try: os.killpg(owner_pgid, 0)
            except ProcessLookupError: return
            time.sleep(.05)
        if os.getpgid(pid) != owner_pgid:
            raise RuntimeError("daemon PID group ownership changed before kill")
        os.killpg(owner_pgid, signal.SIGKILL)
    except ProcessLookupError:
        return

def run_ui(exe, env, root, first, captures, daemon_port, descriptor_records, auth_path):
    master, slave = pty.openpty(); child = None; buf = bytearray(); descriptor_value = None; capture_added = False; screen = TerminalScreen()
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        child = subprocess.Popen([str(exe), "--data-dir", str(root/"d")], cwd=root/"project", env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
        os.close(slave); slave = None
        descriptor_value = descriptor(root/"d", daemon_port, child, time.monotonic()+20)
        descriptor_records.append({"pid": descriptor_value["_validated_pid"], "owner_pgid": descriptor_value["_owner_pgid"], "origin": descriptor_value["http_origin"], "port": daemon_port})
        descriptor_records[-1]["phase"] = "initial_frame"
        read_until(master, time.monotonic()+20, buf, b"OpenCode", 0, screen)
        def cmd(text, marker, predicate=None):
            descriptor_records[-1]["phase"] = "await_" + marker.decode("ascii")
            start = len(buf); previous = screen.text(); os.write(master, text.encode()+b"\r"); read_until(master, time.monotonic()+20, buf, marker, start, screen, predicate)
            if screen.text() == previous: raise AssertionError("command produced no visible terminal update")
            return bytes(buf[start:])
        if first:
            cmd("/connect", b"Connect a provider")
            cmd("openai", b"API key", lambda: screen.has_api_prompt())
            cmd(KEY, MODEL.encode())
            if KEY.encode() in bytes(buf): raise AssertionError("fixture API key echoed anywhere in PTY")
            if not auth_path.is_file() or auth_path.stat().st_mode & 0o777 != 0o600:
                raise AssertionError("provider setup did not persist 0600 auth before model selection")
            saved = json.loads(auth_path.read_text()).get("openai", {})
            if saved.get("type") != "api" or saved.get("key") != KEY:
                raise AssertionError("provider setup persisted the wrong API auth schema")
            start = len(buf); os.write(master, MODEL.encode()+b"\r")
            read_until(master, time.monotonic()+20, buf, MODEL.encode(), start, screen, lambda: screen.has_model_status() and screen.picker_closed())
            if not screen.has_model_status(): raise AssertionError("selected model is not in current status projection")
        start = len(buf); os.write(master, PROMPTS[0 if first else 1].encode()+b"\r")
        read_until(master, time.monotonic()+20, buf, b"G2 first response" if first else b"G2 resumed response", start, screen, lambda: screen.has_assistant("G2 first response" if first else "G2 resumed response"))
        if first and not screen.has_assistant("G2 first response"): raise AssertionError("first response did not settle")
        if not first and PROMPTS[0].encode() not in bytes(buf): raise AssertionError("prior prompt absent after restart")
        if KEY.encode() in bytes(buf): raise AssertionError("fixture API key echoed anywhere in PTY")
        if len(captures) + len(buf) > MAX_PTY: raise AssertionError("combined PTY capture bound reached")
        captures.extend(buf)
        capture_added = True
        os.write(master, b"\x03"); child.wait(timeout=10)
        if child.returncode != 0: raise AssertionError("native process exited %s" % child.returncode)
        return descriptor_value
    finally:
        capture_overflow = False
        if not capture_added:
            capture_overflow = len(captures) + len(buf) > MAX_PTY
            captures.extend(buf[:max(0, MAX_PTY - len(captures))])
        if child is not None and child.poll() is None:
            try: os.killpg(child.pid, signal.SIGTERM); child.wait(timeout=3)
            except subprocess.TimeoutExpired: os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=3)
            except ProcessLookupError: child.wait(timeout=3)
        for fd in (slave, master):
            if fd is not None:
                try: os.close(fd)
                except OSError: pass
        if capture_overflow: raise AssertionError("combined PTY capture bound reached")

def self_check():
    try:
        checked_artifact("relative-binary", "/absolute/library" + HOST_LIBRARY_SUFFIX, "/absolute/build.json")
        raise AssertionError("relative binary accepted")
    except RuntimeError as exc:
        assert "absolute" in str(exc)
    assert HOST_LIBRARY_SUFFIX in (".dylib", ".so")
    catalog = models_json()
    assert set(catalog["openai"]["models"]) == {"gpt-5.6", MODEL}
    terminal = TerminalScreen()
    terminal.feed(b"\x1b[2J\x1b[2;1Hmodel: openai/gpt-5.6")
    assert terminal.contains("model: openai/gpt-5.6")
    terminal.feed(b"\x1b[>0q\x1b[>1u\x1b[2;22H-mini (ctrl-p)")
    assert terminal.has_model_status()
    terminal.feed(b"\x1b]0;ignored title\x07\x1b[1;1H\xc3")
    terminal.feed(b"\xa9")
    assert terminal.text().startswith("model") or "model:" in terminal.text()
    stale = TerminalScreen()
    stale.feed(b"gpt-5.6-mini in picker")
    assert not stale.has_model_status()
    observed = TerminalScreen()
    for chunk in (b"\x1b[2J\x1b[2;1Hmodel: openai/gpt-5.6", b"\x1b[2;22H-", b"mini (ctrl-p)"):
        observed.feed(chunk)
    assert observed.has_model_status()
    secret_screen = TerminalScreen()
    try:
        secret_screen.feed(KEY[:8].encode()); secret_screen.feed(KEY[8:].encode())
        raise AssertionError("split key was not detected")
    except AssertionError as exc:
        assert "visible terminal" in str(exc)
    read_fd, write_fd = os.pipe()
    try:
        buf = bytearray(b"stale Connect provider\n")
        start = len(buf)
        os.write(write_fd, b"fresh Connect provider\n")
        read_until(read_fd, time.monotonic()+1, buf, b"Connect provider", start)
        try: read_until(read_fd, time.monotonic()+.05, buf, b"absent marker", len(buf)); raise AssertionError("missing marker accepted")
        except AssertionError as exc:
            assert "missing marker" in str(exc) or "timeout" in str(exc)
    finally:
        os.close(read_fd); os.close(write_fd)
    state = State(); good = {"Authorization":"Bearer "+KEY}
    for i, prompt in enumerate(PROMPTS):
        inp = [{"role":"user","content":PROMPTS[0]}, *([] if i == 0 else [{"role":"assistant","content":"G2 first response"},{"role":"user","content":prompt}])]
        out, content_type = state.body(good, json.dumps({"model":MODEL,"input":inp,"stream":True}).encode())
        assert content_type == "text/event-stream" and b"response.output_text.delta" in out and b"response.completed" in out
    try: state.body(good, json.dumps({"model":MODEL,"input":[]}).encode()); raise AssertionError("third request accepted")
    except RuntimeError: pass
    try: State().body({"Authorization":"Bearer wrong"}, b'{"model":"'+MODEL.encode()+b'","input":[]}'); raise AssertionError("wrong key accepted")
    except RuntimeError: pass
    out, content_type = State().body(good, json.dumps({"model": MODEL, "input": [{"role": "user", "content": PROMPTS[0]}]}).encode())
    assert content_type == "application/json" and json.loads(out)["status"] == "completed"
    out, content_type = State().body(good, json.dumps({"model": MODEL, "input": [{"role": "user", "content": PROMPTS[0]}], "stream": False}).encode())
    assert content_type == "application/json" and json.loads(out)["status"] == "completed"
    out, content_type = State().body(good, json.dumps({"model": MODEL, "input": [{"role": "user", "content": PROMPTS[0]}], "stream": True}).encode())
    assert content_type == "text/event-stream" and b"response.completed" in out
    try:
        State().body(good, json.dumps({"model": MODEL, "input": [], "stream": "false"}).encode())
        raise AssertionError("invalid stream type accepted")
    except RuntimeError:
        pass
    evidence = sanitize(KEY.encode() + b" visible"); assert KEY.encode() not in evidence
    print("self-check: artifacts, catalog, fresh markers, auth, model, count, history, SSE, and redaction passed")

def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--binary"); ap.add_argument("--native-library"); ap.add_argument("--build-json"); ap.add_argument("--artifact-dir", required=True); ap.add_argument("--self-check", action="store_true"); a=ap.parse_args()
    if a.self_check: self_check(); return
    if not all((a.binary,a.native_library,a.build_json)): ap.error("actual run requires --binary --native-library --build-json")
    binary, library, source, binary_sha, library_sha = checked_artifact(a.binary,a.native_library,a.build_json)
    artifacts=pathlib.Path(a.artifact_dir)
    if not artifacts.is_absolute(): raise RuntimeError("artifact directory must be absolute")
    artifacts=artifacts.resolve()
    if artifacts.exists() and any(artifacts.iterdir()): raise RuntimeError("artifact directory must be new or empty")
    artifacts.mkdir(parents=True,exist_ok=True)
    root=pathlib.Path(tempfile.mkdtemp(prefix="g2-", dir=os.environ.get("TMPDIR","/tmp"))); install=root/"install"; (install/"bin").mkdir(parents=True); (install/"lib").mkdir(); shutil.copyfile(binary,install/"bin"/"oc2"); shutil.copyfile(library,install/"lib"/("libopentui"+HOST_LIBRARY_SUFFIX)); os.chmod(install/"bin"/"oc2",0o755)
    for n in ("home","project","data","d"): (root/n).mkdir()
    data=root/"d"; (data/"runtime").mkdir(); (data/"catalog").mkdir()
    if len(str(data/"runtime"/"opencode-rk.sock").encode())>100: raise RuntimeError("runtime socket path exceeds 100 bytes")
    catalog=data/"catalog"/"models.dev.api.json"; catalog.write_text(json.dumps(models_json()))
    fixture=Fixture(("127.0.0.1",0),Handler); fixture.state=State(); import threading; thread=threading.Thread(target=fixture.serve_forever); thread.start(); captures=bytearray(); descriptor_info=None; descriptor_records=[]
    provider_port=fixture.server_port; daemon_port=free_loopback_port()
    env={"HOME":str(root/"home"),"XDG_CONFIG_HOME":str(root/"home"/".config"),"XDG_DATA_HOME":str(data),"XDG_RUNTIME_DIR":str(data/"runtime"),"OPENCODE_RK_HOME":str(data),"OPENCODE_RK_DAEMON_ADDR":"127.0.0.1:%d"%daemon_port,"OPENCODE_PROJECT_DIR":str(root/"project"),"PATH":"/usr/bin:/bin","TERM":"xterm-256color","OPENAI_BASE_URL":"http://127.0.0.1:%d/v1"%provider_port}
    (root/"home"/".config").mkdir(); failed=False
    try:
        auth=data/"opencode"/"auth.json"
        first_descriptor=run_ui(install/"bin"/"oc2",env,root,True,captures,daemon_port,descriptor_records,auth); descriptor_info=descriptor_records[-1]
        if not auth.is_file() or auth.stat().st_mode&0o777!=0o600: raise AssertionError("UI did not create 0600 auth.json")
        saved=json.loads(auth.read_text()).get("openai",{}); assert saved.get("type")=="api" and saved.get("key")==KEY
        stop_owned_daemon(first_descriptor)
        second_descriptor = run_ui(install/"bin"/"oc2",env,root,False,captures,daemon_port,descriptor_records,auth)
        if fixture.state.error or len(fixture.state.requests)!=2: raise AssertionError(fixture.state.error or "expected exactly two provider requests")
        (artifacts/"native-provider-evidence.json").write_text(json.dumps({"source_sha":source,"binary_sha256":binary_sha,"native_library_sha256":library_sha,"model":MODEL,"request_count":2,"requests":[{"model":r.get("model"),"input":r.get("input")} for r in fixture.state.requests]},indent=2)+"\n")
    except Exception: failed=True; raise
    finally:
        for record in descriptor_records:
            try: stop_owned_daemon({"_validated_pid":record["pid"], "_owner_pgid":record["owner_pgid"]})
            except (OSError, RuntimeError): pass
        evidence={"source_sha":source,"binary_sha256":binary_sha,"native_library_sha256":library_sha,"phase":"failed" if failed else "success","provider_requests":len(fixture.state.requests),"fixture_root":str(root),"descriptors":descriptor_records,"ui_capture":sanitize(bytes(captures)).decode("utf-8","replace")}
        (artifacts/"native-provider-result.json").write_text(json.dumps(evidence,indent=2)+"\n"); fixture.shutdown(); fixture.server_close(); thread.join(timeout=5)
        if thread.is_alive(): raise RuntimeError("fixture server failed bounded shutdown")
        if not failed: shutil.rmtree(root,ignore_errors=True)
        else: print("preserved failure fixture:",root)
if __name__=="__main__": main()

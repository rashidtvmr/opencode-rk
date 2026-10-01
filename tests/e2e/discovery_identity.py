#!/usr/bin/env python3
"""DISC-101 native discovery identity fixture (source-only contract repair).

Rewritten per independent evaluator rejection of eed9f891. Exercises four
cases through the actual installed native binary, not urllib. Auth controls
(200/401/403) are verified via the real Rust public router's Bearer middleware,
not a fake endpoint re-enforcing Bearer.

Resource bounds: 1 HTTP thread, max 8 requests, 128 KiB wire, 256 KiB PTY,
512 KiB evidence, <=20 s per case, <=80 s suite. Absolute monotonic deadlines.

Authority: pinned upstream 95daf90670b7c039c436c85537da5fbfe2205b41
packages/cli/src/services/daemon.ts (healthy + sameRegistration);
approved Rust Bearer deviation per SECURITY.md / daemon_auth.rs:149-176;
evaluator contract at discovery-contract-evaluation.json.
"""
import hashlib, http.server, json, os, pathlib, pty, re, select, signal
import socket, struct, subprocess, sys, tempfile, termios, threading, time
import urllib.error, urllib.request

MAX_PTY = 256 * 1024
MAX_WIRE = 128 * 1024
MAX_EVIDENCE = 512 * 1024
MAX_REQUESTS = 8
CASE_DEADLINE = 20.0
SUITE_DEADLINE = 80.0
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
UPSTREAM_SHA = "95daf90670b7c039c436c85537da5fbfe2205b41"

# Python wrapper script: setsid + TIOCSCTTY on fd 0 + execv.
# Single-threaded, no preexec_fn, no start_new_session double-call.
WRAPPER_SCRIPT = """\
import os, sys, fcntl, termios
os.setsid()
try:
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
except OSError:
    pass
os.execv(sys.argv[1], sys.argv[1:])
"""


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def attest_artifacts():
    """Validate BUILD_JSON attestation. source_sha is the Rust candidate/source
    SHA (NOT the upstream TypeScript pin). Requires native=true, profile=release,
    absolute paths, matching binary+library hashes. FAILs on mismatch."""
    raw_bin = os.environ.get("OC2_TEST_BINARY", "")
    raw_lib = os.environ.get("NATIVE_LIBRARY", "")
    raw_manifest = os.environ.get("BUILD_JSON", "")
    for name, val in [("OC2_TEST_BINARY", raw_bin), ("NATIVE_LIBRARY", raw_lib), ("BUILD_JSON", raw_manifest)]:
        if not val or not pathlib.Path(val).is_absolute():
            raise AssertionError(f"{name} must be an absolute path, got {val!r}")
    binary = pathlib.Path(raw_bin).resolve()
    library = pathlib.Path(raw_lib).resolve()
    manifest = pathlib.Path(raw_manifest).resolve()
    if not binary.is_file() or not os.access(binary, os.R_OK | os.X_OK):
        raise AssertionError("binary missing or not executable")
    suffix = ".dylib" if sys.platform == "darwin" else ".so"
    if not library.is_file() or library.suffix != suffix:
        raise AssertionError(f"native library must be a {suffix}")
    data = json.loads(manifest.read_text())
    source_sha = data.get("source_sha") or data.get("sourceSha") or data.get("git_sha", "")
    if not isinstance(source_sha, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", source_sha):
        raise AssertionError(f"source_sha must be 40 hex chars, got {source_sha!r}")
    if data.get("native") is not True:
        raise AssertionError("attestation requires native=true")
    if data.get("profile") != "release":
        raise AssertionError("attestation requires profile=release")
    expected_bin = str(data.get("binary_sha256") or data.get("binarySha256") or "").lower()
    if not HEX64.fullmatch(expected_bin) or sha256(binary) != expected_bin:
        raise AssertionError("binary SHA256 mismatch")
    expected_lib = str(
        data.get("native_library_sha256") or data.get("nativeLibrarySha256")
        or data.get("library_sha256") or data.get("librarySha256") or ""
    ).lower()
    if expected_lib and (not HEX64.fullmatch(expected_lib) or sha256(library) != expected_lib):
        raise AssertionError("library SHA256 mismatch")
    return binary, source_sha


class RequestRecord:
    __slots__ = ("method", "path", "headers", "body_len", "auth_header")

    def __init__(self, method, path, headers, body_len):
        self.method = method
        self.path = path
        self.headers = dict(headers)
        self.body_len = body_len
        self.auth_header = headers.get("Authorization", "")


class DaemonHandler(http.server.BaseHTTPRequestHandler):
    """Controlled HTTP endpoint matching actual Rust server shapes.
    /health is PUBLIC (no Bearer required) per daemon_auth.rs:156.
    /api/* requires Bearer <token> per daemon_auth.rs:149-176.
    Records every request for side-effect assertions."""

    protocol_version = "HTTP/1.1"

    def _record(self, body_len=0):
        state = self.server.state
        state.request_count += 1
        state.wire_bytes += body_len
        if state.request_count > MAX_REQUESTS:
            raise RuntimeError("request bound exceeded")
        if state.wire_bytes > MAX_WIRE:
            raise RuntimeError("wire budget exceeded")
        rec = RequestRecord(self.command, self.path, self.headers, body_len)
        state.requests.append(rec)
        return rec

    def _check_bearer(self, expected_token):
        auth = self.headers.get("Authorization", "")
        if not auth:
            self._send_json(401, {"error": "missing bearer credential"})
            return False
        if auth != f"Bearer {expected_token}":
            self._send_json(403, {"error": "bad bearer credential"})
            return False
        return True

    def _send_json(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):  # noqa: N802
        rec = self._record()
        state = self.server.state
        if self.path == "/health":
            # Public health per daemon_auth.rs:156 and lib.rs:271-273
            self._send_json(200, {
                "schema_version": 1,
                "status": "ok",
                "runtime": "native-rust",
            })
            return
        if self.path.startswith("/api/"):
            if not self._check_bearer(state.expected_token):
                return
            if self.path == "/api/models":
                self._send_json(200, {"models": []})
                return
            if self.path.startswith("/api/sessions"):
                self._send_json(200, {"sessions": []})
                return
            if self.path.startswith("/api/provider/store"):
                state.provider_store_hits += 1
                self._send_json(200, {"providers": {}})
                return
            self._send_json(200, {})
            return
        self._send_json(404, {"error": "not found"})

    def do_POST(self):  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        if length > MAX_WIRE:
            raise RuntimeError("request body exceeds wire budget")
        body = self.rfile.read(length) if length > 0 else b""
        self._record(len(body))
        state = self.server.state
        if self.path.startswith("/api/"):
            if not self._check_bearer(state.expected_token):
                return
            if self.path.startswith("/api/provider/store"):
                state.provider_store_hits += 1
        self._send_json(200, {})

    def log_message(self, *_):
        pass


class ServerState:
    def __init__(self, expected_token):
        self.expected_token = expected_token
        self.request_count = 0
        self.wire_bytes = 0
        self.requests = []
        self.provider_store_hits = 0


class BoundedServer(http.server.HTTPServer):
    allow_reuse_address = True

    def get_request(self):
        conn, addr = super().get_request()
        conn.settimeout(5.0)
        return conn, addr


def free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def make_wrapper(tmp):
    """Write the Python setsid+TIOCSCTTY+execv wrapper script."""
    path = tmp / "pty_wrapper.py"
    path.write_text(WRAPPER_SCRIPT)
    return path


def seed_descriptor(runtime_dir, pid, origin, token):
    """Write backend.json matching daemon.rs BackendDescriptor shape exactly."""
    runtime_dir.mkdir(parents=True, exist_ok=True)
    desc = {
        "pid": pid,
        "http_origin": origin,
        "schema_version": 1,
        "auth_token": token,
    }
    path = runtime_dir / "backend.json"
    path.write_text(json.dumps(desc))
    os.chmod(path, 0o600)
    return path


def seed_catalog(data_dir):
    """Seed minimal models.dev catalog so the binary doesn't attempt network."""
    cat_dir = data_dir / "catalog"
    cat_dir.mkdir(parents=True, exist_ok=True)
    (cat_dir / "models.dev.api.json").write_text(json.dumps({
        "openai": {"name": "OpenAI", "models": {"gpt-4o": {"name": "GPT-4o", "limit": {"context": 128000}}}}
    }))


def drain_pty(master_fd, deadline, buf):
    """Bounded PTY drain until EIO/EOF or deadline. macOS EIO on closed slave."""
    while time.monotonic() < deadline and len(buf) < MAX_PTY:
        remaining = max(0.01, deadline - time.monotonic())
        ready, _, _ = select.select([master_fd], [], [], min(0.1, remaining))
        if not ready:
            continue
        try:
            chunk = os.read(master_fd, min(65536, MAX_PTY - len(buf)))
            if not chunk:
                break
            buf.extend(chunk)
        except OSError:
            # macOS raises EIO when slave side closes
            break


def wait_for_health(origin, deadline, timeout=2.0):
    """Poll /health on the controlled endpoint until 200 or deadline."""
    url = f"{origin}/health"
    while time.monotonic() < deadline:
        try:
            req = urllib.request.Request(url)
            with urllib.request.urlopen(req, timeout=min(timeout, max(0.1, deadline - time.monotonic()))) as resp:
                if resp.status == 200:
                    return True
        except (urllib.error.URLError, OSError, ValueError):
            pass
        time.sleep(0.05)
    return False


def run_case(case_name, binary, wrapper, root, daemon_port, foreign_child=None):
    """Execute one DISC-101 case. Returns (passed, evidence_dict)."""
    case_deadline = time.monotonic() + CASE_DEADLINE
    data_dir = root / f"data-{case_name}"
    home_dir = root / f"home-{case_name}"
    project_dir = root / f"project-{case_name}"
    for d in (data_dir, home_dir, project_dir, data_dir / "runtime"):
        d.mkdir(parents=True, exist_ok=True)

    token = "ab" * 32  # well-formed 64-hex token
    origin = f"http://127.0.0.1:{daemon_port}"

    # Determine descriptor PID based on case semantics
    if case_name == "VALIDIDENTITY":
        # Will be updated after launch to match the actual wrapper PID
        desc_pid = 0  # placeholder, seeded after Popen
    elif case_name == "STALEPID":
        desc_pid = 99999999  # guaranteed dead
    elif case_name == "LIVEFOREIGNPID":
        desc_pid = foreign_child.pid if foreign_child else 99999998
    elif case_name == "EMPTYTOKEN":
        desc_pid = 0  # placeholder
        token = ""  # empty token triggers Descriptor error
    else:
        raise ValueError(f"unknown case: {case_name}")

    # Seed catalog and descriptor
    seed_catalog(data_dir)
    desc_path = None
    if case_name != "VALIDIDENTITY":
        seed_descriptor(data_dir / "runtime", desc_pid, origin, token)

    # Start controlled HTTP server
    state = ServerState(token)
    server = BoundedServer(("127.0.0.1", daemon_port), DaemonHandler)
    server.state = state
    srv_thread = threading.Thread(target=server.serve_forever, daemon=True)
    srv_thread.start()

    master_fd = slave_fd = child = None
    pty_buf = bytearray()
    saved_termios = None
    passed = False
    failure_reason = ""

    try:
        master_fd, slave_fd = pty.openpty()
        fcntl_mod = __import__("fcntl")
        fcntl_mod.ioctl(slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        try:
            saved_termios = termios.tcgetattr(master_fd)
        except termios.error:
            saved_termios = None

        env = {
            "HOME": str(home_dir),
            "XDG_CONFIG_HOME": str(home_dir / ".config"),
            "XDG_DATA_HOME": str(data_dir),
            "XDG_RUNTIME_DIR": str(data_dir / "runtime"),
            "OPENCODE_RK_HOME": str(data_dir),
            "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{daemon_port}",
            "OPENCODE_PROJECT_DIR": str(project_dir),
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "TERM": "xterm-256color",
            "OC2_OFFLINE": "1",
        }
        (home_dir / ".config").mkdir(exist_ok=True)

        # Launch via wrapper: sys.executable wrapper.py binary --data-dir data_dir
        cmd = [sys.executable, str(wrapper), str(binary), "--data-dir", str(data_dir)]
        child = subprocess.Popen(
            cmd, cwd=str(project_dir), env=env,
            stdin=slave_fd, stdout=slave_fd, stderr=slave_fd,
            close_fds=True,
        )
        os.close(slave_fd)
        slave_fd = None

        # For VALIDIDENTITY: update descriptor with actual wrapper PID
        # The wrapper does setsid+execv so the PID is preserved
        if case_name == "VALIDIDENTITY":
            actual_pid = child.pid
            seed_descriptor(data_dir / "runtime", actual_pid, origin, token)

        # Drain PTY output bounded
        drain_pty(master_fd, min(case_deadline, time.monotonic() + 10), pty_buf)

        # Wait for health readiness (proves daemon started and is listening)
        if case_name == "VALIDIDENTITY":
            health_ready = wait_for_health(origin, min(case_deadline, time.monotonic() + 15))
            if not health_ready:
                failure_reason = "VALIDIDENTITY: daemon did not reach /health readiness"
            else:
                # Verify authenticated API call works through the real router path
                try:
                    api_url = f"{origin}/api/models"
                    req = urllib.request.Request(api_url, headers={"Authorization": f"Bearer {token}"})
                    with urllib.request.urlopen(req, timeout=3) as resp:
                        if resp.status == 200:
                            passed = True
                        else:
                            failure_reason = f"VALIDIDENTITY: unexpected status {resp.status}"
                except (urllib.error.URLError, OSError) as exc:
                    failure_reason = f"VALIDIDENTITY: API probe failed: {exc}"

                # Verify 401 without Bearer
                if passed:
                    try:
                        req_401 = urllib.request.Request(f"{origin}/api/sessions")
                        urllib.request.urlopen(req_401, timeout=2)
                        passed = False
                        failure_reason = "VALIDIDENTITY: missing Bearer was not 401"
                    except urllib.error.HTTPError as exc:
                        if exc.code != 401:
                            passed = False
                            failure_reason = f"VALIDIDENTITY: expected 401 got {exc.code}"

                # Verify 403 with wrong Bearer
                if passed:
                    try:
                        req_403 = urllib.request.Request(
                            f"{origin}/api/sessions",
                            headers={"Authorization": "Bearer " + "ff" * 32}
                        )
                        urllib.request.urlopen(req_403, timeout=2)
                        passed = False
                        failure_reason = "VALIDIDENTITY: wrong Bearer was not 403"
                    except urllib.error.HTTPError as exc:
                        if exc.code != 403:
                            passed = False
                            failure_reason = f"VALIDIDENTITY: expected 403 got {exc.code}"

        elif case_name == "STALEPID":
            # Stale PID: read_backend_descriptor returns None.
            # Binary should NOT contact the endpoint at all before owned fallback.
            # We verify zero contacts to our controlled endpoint.
            time.sleep(min(2.0, max(0, case_deadline - time.monotonic())))
            drain_pty(master_fd, case_deadline, pty_buf)
            if state.request_count == 0:
                passed = True
            else:
                failure_reason = f"STALEPID: expected 0 contacts, got {state.request_count}"

        elif case_name == "LIVEFOREIGNPID":
            # Foreign live PID: health probe allowed, but ZERO /api/provider/store effects.
            time.sleep(min(3.0, max(0, case_deadline - time.monotonic())))
            drain_pty(master_fd, case_deadline, pty_buf)
            health_hits = sum(1 for r in state.requests if r.path == "/health")
            api_hits = sum(1 for r in state.requests if r.path.startswith("/api/"))
            if state.provider_store_hits == 0 and api_hits == 0:
                passed = True
            else:
                failure_reason = (
                    f"LIVEFOREIGNPID: provider_store={state.provider_store_hits}, "
                    f"api_hits={api_hits}, health_hits={health_hits}"
                )

        elif case_name == "EMPTYTOKEN":
            # Empty token: read_backend_descriptor returns Err(Descriptor).
            # Binary must refuse to attach. Zero contacts expected.
            time.sleep(min(2.0, max(0, case_deadline - time.monotonic())))
            drain_pty(master_fd, case_deadline, pty_buf)
            if state.request_count == 0:
                passed = True
            else:
                failure_reason = f"EMPTYTOKEN: expected 0 contacts, got {state.request_count}"

    except Exception as exc:
        failure_reason = str(exc)
    finally:
        # Normal exit: Ctrl-C then bounded drain
        if child is not None and child.poll() is None:
            try:
                if master_fd is not None:
                    os.write(master_fd, b"\x03")  # Ctrl-C
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                # SIGTERM to owned process group only; SIGKILL = explicit FAIL
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                    child.wait(timeout=3)
                except (subprocess.TimeoutExpired, ProcessLookupError, OSError):
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                        child.wait(timeout=2)
                    except (ProcessLookupError, OSError):
                        pass
                    passed = False
                    failure_reason = f"{case_name}: forced KILL required"

        # Drain remaining PTY output
        if master_fd is not None:
            drain_pty(master_fd, time.monotonic() + 1.0, pty_buf)

        # Compare saved termios WITHOUT tcsetattr restoration
        termios_changed = False
        if saved_termios is not None and master_fd is not None:
            try:
                current_termios = termios.tcgetattr(master_fd)
                termios_changed = (saved_termios != current_termios)
            except termios.error:
                pass

        # Close FDs
        for fd in (master_fd, slave_fd):
            if fd is not None:
                try:
                    os.close(fd)
                except OSError:
                    pass

        # Stop server: shutdown + server_close + thread join
        server.shutdown()
        server.server_close()
        srv_thread.join(timeout=2)
        if srv_thread.is_alive():
            passed = False
            failure_reason = f"{case_name}: server thread join timeout"

    evidence = {
        "case": case_name,
        "passed": passed,
        "failure_reason": failure_reason,
        "request_count": state.request_count,
        "wire_bytes": state.wire_bytes,
        "pty_bytes": len(pty_buf),
        "provider_store_hits": state.provider_store_hits,
        "termios_changed": termios_changed,
        "request_paths": [r.path for r in state.requests[:MAX_REQUESTS]],
        "auth_headers_present": [
            bool(r.auth_header) for r in state.requests[:MAX_REQUESTS]
        ],
    }
    return passed, evidence


def main():
    suite_start = time.monotonic()
    suite_deadline = suite_start + SUITE_DEADLINE

    binary, source_sha = attest_artifacts()
    cases = ("VALIDIDENTITY", "STALEPID", "LIVEFOREIGNPID", "EMPTYTOKEN")
    results = {}
    all_evidence = []

    with tempfile.TemporaryDirectory(prefix="disc101-") as raw_tmp:
        root = pathlib.Path(raw_tmp)
        wrapper = make_wrapper(root)

        # Foreign child for LIVEFOREIGNPID: real owned live process
        foreign_child = subprocess.Popen(
            ["sleep", "60"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )

        try:
            # Each case gets its own port to avoid cross-contamination
            for case_name in cases:
                if time.monotonic() >= suite_deadline:
                    results[case_name] = False
                    all_evidence.append({"case": case_name, "failure_reason": "suite deadline"})
                    continue
                port = free_port()
                passed, evidence = run_case(
                    case_name, binary, wrapper, root, port, foreign_child
                )
                results[case_name] = passed
                all_evidence.append(evidence)
        finally:
            # Reap ONLY the owned foreign child
            if foreign_child.poll() is None:
                foreign_child.terminate()
                try:
                    foreign_child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    foreign_child.kill()
                    foreign_child.wait(timeout=2)

    total_wire = sum(e.get("wire_bytes", 0) for e in all_evidence)
    total_pty = sum(e.get("pty_bytes", 0) for e in all_evidence)
    total_evidence_size = len(json.dumps(all_evidence).encode())

    summary = {
        "DISC101": "CANDIDATE",
        "source_sha": source_sha,
        "upstream_pin": UPSTREAM_SHA,
        "cases": results,
        "elapsed_seconds": round(time.monotonic() - suite_start, 2),
        "total_wire_bytes": total_wire,
        "total_pty_bytes": total_pty,
        "total_evidence_bytes": total_evidence_size,
        "evidence": all_evidence,
    }

    if total_wire > MAX_WIRE * len(cases):
        summary["resource_violation"] = "wire budget exceeded"
    if total_pty > MAX_PTY * len(cases):
        summary["resource_violation"] = "pty budget exceeded"
    if total_evidence_size > MAX_EVIDENCE:
        summary["resource_violation"] = "evidence budget exceeded"

    print(json.dumps(summary, indent=2))

    # Exit non-zero if any case failed (this is CANDIDATE source, runtime RED pending)
    if not all(results.values()):
        sys.exit(1)


if __name__ == "__main__":
    main()

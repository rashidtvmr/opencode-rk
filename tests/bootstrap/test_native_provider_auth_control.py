"""G2 native provider-auth HTTP security contract.

The contract deliberately uses the installed daemon, a loopback bearer, and a
disposable data root.  It does not use ambient provider credentials or the
user's OpenCode state.
"""

import hashlib
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request


MAX_REQUEST = 16 * 1024
MAX_RESPONSE = 32 * 1024
MAX_AUTH_FILE = 1024 * 1024
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
FAKE_KEY = "fixture-api-key-never-print-7f3a"
CATALOG_IDS = {"gpt-5.6", "gpt-5.6-mini"}
NO_AUTH_HEADER = object()
DAEMON_AUTH = object()


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def required_absolute_file(name: str, suffixes=()):
    value = os.environ.get(name)
    if not value:
        raise AssertionError(f"{name} is required")
    path = pathlib.Path(value)
    if not path.is_absolute() or not path.is_file() or not os.access(path, os.R_OK):
        raise AssertionError(f"{name} must be an absolute readable file")
    if suffixes and path.suffix not in suffixes:
        raise AssertionError(f"{name} has an unsupported suffix")
    return path


def checked_artifacts():
    binary = required_absolute_file("OC2_TEST_BINARY")
    library = required_absolute_file("OC2_TEST_NATIVE_LIBRARY", {".dylib", ".so"})
    if not os.access(binary, os.X_OK):
        raise AssertionError("installed binary must be executable")
    suffix = ".dylib" if sys.platform == "darwin" else ".so"
    if library.suffix != suffix:
        raise AssertionError("native library does not match the host platform")
    manifest = required_absolute_file("OC2_TEST_BUILD_JSON", {".json"})
    receipt = json.loads(manifest.read_text(encoding="utf-8"))
    expected_source = receipt.get("source_sha")
    expected_binary = receipt.get("binary_sha256")
    expected_library = receipt.get("native_library_sha256")
    if not isinstance(expected_source, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", expected_source):
        raise AssertionError("build receipt lacks a 40-hex source_sha")
    if expected_binary != sha256(binary) or not HEX64.fullmatch(str(expected_binary)):
        raise AssertionError("binary does not match build receipt")
    if not isinstance(expected_library, str) or not HEX64.fullmatch(expected_library):
        raise AssertionError("build receipt lacks a valid native library SHA")
    if expected_library != sha256(library):
        raise AssertionError("native library does not match build receipt")
    profile = receipt.get("profile")
    native = receipt.get("native")
    if profile != "release" or native is not True:
        raise AssertionError("build receipt is not a native release receipt")
    return binary, library, receipt


def bounded_json(response):
    try:
        raw = response.read(MAX_RESPONSE + 1)
        if len(raw) > MAX_RESPONSE:
            raise AssertionError("HTTP response exceeded bounded contract")
        if FAKE_KEY.encode() in raw:
            raise AssertionError("HTTP response leaked fixture API key")
        return json.loads(raw.decode("utf-8"))
    finally:
        response.close()


def bounded_discard(response):
    try:
        raw = response.read(MAX_RESPONSE + 1)
        if len(raw) > MAX_RESPONSE:
            raise AssertionError("HTTP response exceeded bounded contract")
        if FAKE_KEY.encode() in raw:
            raise AssertionError("HTTP response leaked fixture API key")
        return raw
    finally:
        response.close()


class NativeAuthService:
    def __init__(self, test_case):
        self.temp = None
        try:
            self._initialize(test_case)
        except BaseException:
            log_stream = getattr(self, "log_stream", None)
            if log_stream is not None:
                log_stream.close()
            if self.temp is not None:
                self.temp.cleanup()
            raise

    def _initialize(self, test_case):
        self.test_case = test_case
        self.temp = tempfile.TemporaryDirectory(prefix="pa-", dir=os.environ.get("TMPDIR"))
        self.root = pathlib.Path(self.temp.name)
        self.home = self.root / "home"
        self.project = self.root / "project"
        self.data = self.root / "d"
        if len(str(self.data / "runtime" / "opencode-rk.sock").encode()) > 100:
            raise AssertionError("fixture Unix socket path exceeds 100 bytes")
        self.catalog = self.data / "catalog" / "models.dev.api.json"
        self.runtime = self.data / "runtime"
        for path in (self.home, self.project, self.data, self.runtime):
            path.mkdir(parents=True, exist_ok=True)
        self.binary, self.library, self.receipt = checked_artifacts()
        install = self.root / "install"
        (install / "bin").mkdir(parents=True)
        (install / "lib").mkdir()
        self.executable = install / "bin" / "oc2"
        shutil.copy2(self.binary, self.executable)
        shutil.copy2(self.library, install / "lib" / f"libopentui{self.library.suffix}")
        self.catalog.parent.mkdir(parents=True)
        self.catalog.write_text(
            json.dumps({"openai": {"name": "fixture", "models": {
                model: {"name": model, "limit": {"context": 200000}}
                for model in sorted(CATALOG_IDS)
            }}}) + "\n",
            encoding="utf-8",
        )
        self.log_path = self.root / "daemon.log"
        self.log_stream = self.log_path.open("wb")
        self.process = None
        self._stopped = False
        self.origin = None
        self.token = None
        self.artifacts = None
        artifact_root = os.environ.get("OC2_AUTH_CONTROL_ARTIFACT_ROOT")
        if artifact_root:
            artifact_root = pathlib.Path(artifact_root)
            if not artifact_root.is_absolute():
                raise AssertionError("OC2_AUTH_CONTROL_ARTIFACT_ROOT must be absolute")
            artifact_root.mkdir(parents=True, exist_ok=True)
            self.artifacts = pathlib.Path(tempfile.mkdtemp(
                prefix=test_case.id().rsplit(".", 1)[-1] + "-", dir=artifact_root))

    def start(self):
        env = {
            "HOME": str(self.home),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "LANG": "C",
            "LC_ALL": "C",
            "TMPDIR": str(self.root / "tmp"),
            "XDG_CONFIG_HOME": str(self.root / "config"),
            "XDG_DATA_HOME": str(self.data),
            "XDG_RUNTIME_DIR": str(self.root / "runtime-sock"),
            "OPENCODE_RK_HOME": str(self.data),
            "OPENCODE_PROJECT_DIR": str(self.project),
            "OC2_OFFLINE": "1",
        }
        for key in ("TMPDIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR"):
            pathlib.Path(env[key]).mkdir(parents=True, exist_ok=True)
        self.process = subprocess.Popen(
            [str(self.executable), "serve", "--listen", "127.0.0.1:0"],
            cwd=self.project,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=self.log_stream,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            close_fds=True,
            umask=0o022,
        )
        self.log_stream.close()
        self.log_stream = None
        descriptor = self.runtime / "backend.json"
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            self._check_log_limit()
            if self.process.poll() is not None:
                raise AssertionError("owned daemon exited before descriptor")
            try:
                if not descriptor.is_file():
                    raise ValueError("descriptor is not a regular file")
                raw_descriptor = descriptor.read_bytes()
                if len(raw_descriptor) > 8192:
                    raise ValueError("descriptor too large")
                value = json.loads(raw_descriptor)
                origin = value["http_origin"]
                token = value["auth_token"]
                pid = value["pid"]
                port = int(origin.rsplit(":", 1)[1])
                if (pid == self.process.pid and os.getpgid(pid) == pid
                        and re.fullmatch(r"http://127\.0\.0\.1:[0-9]+", origin)
                        and 0 < port <= 65535 and HEX64.fullmatch(token)):
                    self.origin, self.token = origin, token
                    response = self.request("GET", "/api/models")
                    if response[0] == 200:
                        models = bounded_json(response[1])
                        ids = {
                            item.get("model_id")
                            for item in models.get("models", [])
                            if item.get("provider_id") == "openai"
                        }
                        if ids == CATALOG_IDS:
                            return
                    else:
                        bounded_discard(response[1])
            except (OSError, KeyError, ValueError, urllib.error.URLError):
                pass
            time.sleep(0.03)
        raise AssertionError("owned authenticated daemon/models readiness failed")

    def request(self, method, path, body=None, bearer=DAEMON_AUTH):
        raw = None if body is None else json.dumps(body, separators=(",", ":")).encode()
        if raw is not None and len(raw) > MAX_REQUEST:
            raise AssertionError("test request exceeded contract bound")
        request = urllib.request.Request(self.origin + path, data=raw, method=method)
        if bearer is DAEMON_AUTH:
            bearer = self.token
        if bearer is not NO_AUTH_HEADER:
            request.add_header("Authorization", "Bearer " + bearer)
        if raw is not None:
            request.add_header("Content-Type", "application/json")
        try:
            response = urllib.request.urlopen(request, timeout=2)
            self._check_log_limit()
            return response.status, response
        except urllib.error.HTTPError as error:
            self._check_log_limit()
            return error.code, error

    def stop(self):
        if self._stopped:
            return
        self._stopped = True
        if self.process is None:
            self.temp.cleanup()
            return
        try:
            if self.process.poll() is None:
                os.killpg(self.process.pid, signal.SIGTERM)
                try:
                    self.process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(self.process.pid, signal.SIGKILL)
                    self.process.wait(timeout=3)
            self._check_log_limit(final=True)
            with self.log_path.open("rb") as stream:
                log_bytes = stream.read(MAX_RESPONSE + 1)
            if FAKE_KEY.encode() in log_bytes:
                raise AssertionError("bounded daemon output leaked fixture API key")
            if self.artifacts is not None:
                shutil.copy2(self.log_path, self.artifacts / "daemon.log")
                (self.artifacts / "receipt.json").write_text(json.dumps({
                    "source_sha": self.receipt["source_sha"],
                    "test": self.test_case.id(),
                    "pid": self.process.pid,
                    "owned_process_reaped": self.process.poll() is not None,
                    "fixture_root": str(self.root),
                    "log_sha256": sha256(self.log_path),
                }, indent=2) + "\n")
        finally:
            if self.log_stream is not None:
                self.log_stream.close()
            self.temp.cleanup()
            self.process = None

    def _check_log_limit(self, final=False):
        if self.log_path.stat().st_size > MAX_RESPONSE:
            raise AssertionError("daemon output exceeded bounded contract")
        if final and self.log_path.stat().st_size > MAX_RESPONSE:
            raise AssertionError("daemon output exceeded bounded contract")


def api_info(key=FAKE_KEY, metadata=None):
    value = {"type": "api", "key": key}
    if metadata is not None:
        value["metadata"] = metadata
    return value


def tree_snapshot(root):
    snapshot = {}
    if not root.exists():
        return snapshot
    for path in sorted(root.rglob("*")):
        if path.is_file():
            snapshot[str(path.relative_to(root))] = path.read_bytes()
        elif path.is_dir():
            snapshot[str(path.relative_to(root)) + "/"] = None
    return snapshot


class NativeProviderAuthControlTests(unittest.TestCase):
    def run_service(self, initial=None):
        service = NativeAuthService(self)
        auth = service.data / "opencode" / "auth.json"
        service.test_case.addCleanup(service.stop)
        if initial is not None:
            auth.parent.mkdir(parents=True, exist_ok=True)
            auth.write_text(json.dumps(initial, separators=(",", ":")) + "\n", encoding="utf-8")
            os.chmod(auth, 0o600)
        service.start()
        return service, auth

    def test_daemon_bearer_descriptor_is_owner_only(self):
        service, _ = self.run_service()
        try:
            descriptor = service.runtime / "backend.json"
            self.assertEqual(descriptor.stat().st_mode & 0o777, 0o600)
        finally:
            service.stop()

    def test_unauthorized_valid_put_has_no_side_effect(self):
        service, auth = self.run_service()
        try:
            before = tree_snapshot(service.data)
            for bearer, expected_status in ((NO_AUTH_HEADER, 401), ("wrong-token", 403)):
                status, response = service.request("PUT", "/auth/openai", api_info(), bearer)
                self.assertEqual(status, expected_status)
                self.assertNotIn(FAKE_KEY.encode(), bounded_discard(response))
                self.assertFalse(auth.exists())
                self.assertEqual(tree_snapshot(service.data), before)
        finally:
            service.stop()

    def test_authorized_api_put_persists0600_without_secret_echo(self):
        service, auth = self.run_service()
        try:
            status, response = service.request("PUT", "/auth/openai", api_info(metadata={"account": "fixture"}))
            self.assertEqual(status, 200)
            self.assertIs(bounded_json(response), True)
            self.assertTrue(auth.is_file())
            self.assertEqual(auth.stat().st_mode & 0o777, 0o600)
            saved = json.loads(auth.read_text())
            self.assertEqual(saved["openai"], api_info(metadata={"account": "fixture"}))
        finally:
            service.stop()

    def test_existing_oauth_and_api_entries_are_preserved(self):
        initial = {"other": {"type": "oauth", "refresh": "refresh-fixture", "access": "access-fixture", "expires": 7}, "legacy": api_info("old-fixture", {"account": "old"})}
        service, auth = self.run_service(initial)
        try:
            before = json.loads(auth.read_text())
            status, response = service.request("PUT", "/auth/openai%2F", api_info(metadata={"account": "fixture"}))
            self.assertEqual(status, 200)
            self.assertNotIn(FAKE_KEY.encode(), bounded_discard(response))
            saved = json.loads(auth.read_text())
            self.assertEqual(saved["other"], before["other"])
            self.assertEqual(saved["legacy"], before["legacy"])
            self.assertEqual(saved["openai"]["type"], "api")
        finally:
            service.stop()

    def test_invalid_api_payload_is_rejected_without_mutation_or_echo(self):
        initial = {"legacy": api_info("old-fixture", {"account": "old"})}
        service, auth = self.run_service(initial)
        try:
            before = auth.read_bytes()
            for body in ({"type": "api", "key": 3}, {"type": "api", "key": FAKE_KEY, "metadata": {"bad": 3}}, {"type": "unknown", "key": FAKE_KEY}):
                status, response = service.request("PUT", "/auth/openai", body)
                self.assertGreaterEqual(status, 400)
                self.assertLess(status, 500)
                self.assertNotIn(FAKE_KEY.encode(), bounded_discard(response))
                self.assertEqual(auth.read_bytes(), before)
        finally:
            service.stop()

    def test_auth_directory_is_not_overwritten(self):
        service, auth = self.run_service()
        try:
            auth.parent.mkdir(parents=True, exist_ok=True)
            auth.mkdir()
            marker = auth / "marker"
            marker.write_text("owned-marker", encoding="utf-8")
            status, response = service.request("PUT", "/auth/openai", api_info())
            self.assertGreaterEqual(status, 400)
            self.assertLess(status, 600)
            bounded_discard(response)
            self.assertTrue(auth.is_dir())
            self.assertEqual(marker.read_text(), "owned-marker")
        finally:
            service.stop()

    def test_oversized_existing_auth_file_is_rejected_unchanged(self):
        initial = {"legacy": api_info("old-fixture", {"account": "old"}), "padding": "x" * MAX_AUTH_FILE}
        service, auth = self.run_service(initial)
        try:
            before = auth.read_bytes()
            self.assertGreater(len(before), MAX_AUTH_FILE)
            status, response = service.request("PUT", "/auth/openai", api_info())
            self.assertGreaterEqual(status, 400)
            self.assertLess(status, 600)
            bounded_discard(response)
            self.assertEqual(auth.read_bytes(), before)
        finally:
            service.stop()


if __name__ == "__main__":
    unittest.main()

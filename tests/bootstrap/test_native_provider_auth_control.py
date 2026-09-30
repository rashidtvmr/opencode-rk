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
    manifest = required_absolute_file("OC2_TEST_BUILD_JSON", {".json"})
    receipt = json.loads(manifest.read_text(encoding="utf-8"))
    expected_source = receipt.get("source_sha")
    expected_binary = receipt.get("binary_sha256")
    expected_library = receipt.get("native_library_sha256")
    if not isinstance(expected_source, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", expected_source):
        raise AssertionError("build receipt lacks a 40-hex source_sha")
    if expected_binary != sha256(binary) or not HEX64.fullmatch(str(expected_binary)):
        raise AssertionError("binary does not match build receipt")
    if expected_library and expected_library != sha256(library):
        raise AssertionError("native library does not match build receipt")
    return binary, library, receipt


def bounded_json(response):
    raw = response.read(MAX_RESPONSE + 1)
    if len(raw) > MAX_RESPONSE:
        raise AssertionError("HTTP response exceeded bounded contract")
    return json.loads(raw.decode("utf-8"))


class NativeAuthService:
    def __init__(self, test_case):
        self.test_case = test_case
        self.temp = tempfile.TemporaryDirectory(prefix="pa-")
        self.root = pathlib.Path(self.temp.name)
        self.home = self.root / "home"
        self.project = self.root / "project"
        self.data = self.root / "d"
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
        self.catalog.write_text(
            json.dumps({"openai": {"name": "fixture", "models": {
                model: {"name": model, "limit": {"context": 200000}}
                for model in sorted(CATALOG_IDS)
            }}}) + "\n",
            encoding="utf-8",
        )
        self.logs = bytearray()
        self.process = None
        self.origin = None
        self.token = None

    def start(self):
        env = {
            "HOME": str(self.home),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "LANG": "C",
            "LC_ALL": "C",
            "TMPDIR": str(self.root / "tmp"),
            "XDG_CONFIG_HOME": str(self.root / "config"),
            "XDG_DATA_HOME": str(self.root / "xdg-data"),
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
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            close_fds=True,
        )
        descriptor = self.runtime / "backend.json"
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AssertionError("owned daemon exited before descriptor")
            try:
                value = json.loads(descriptor.read_text(encoding="utf-8"))
                origin = value["http_origin"]
                token = value["auth_token"]
                if (re.fullmatch(r"http://127\.0\.0\.1:[0-9]+", origin)
                        and HEX64.fullmatch(token)):
                    self.origin, self.token = origin, token
                    response = self.request("GET", "/api/models")
                    if response[0] == 200:
                        models = bounded_json(response[1])
                        ids = {item.get("model_id") for item in models.get("models", [])}
                        if CATALOG_IDS <= ids:
                            return
            except (OSError, KeyError, ValueError, urllib.error.URLError):
                pass
            time.sleep(0.03)
        raise AssertionError("owned authenticated daemon/models readiness failed")

    def request(self, method, path, body=None, bearer=None):
        raw = None if body is None else json.dumps(body, separators=(",", ":")).encode()
        if raw is not None and len(raw) > MAX_REQUEST:
            raise AssertionError("test request exceeded contract bound")
        request = urllib.request.Request(self.origin + path, data=raw, method=method)
        request.add_header("Authorization", "Bearer " + (self.token if bearer is None else bearer))
        if raw is not None:
            request.add_header("Content-Type", "application/json")
        try:
            response = urllib.request.urlopen(request, timeout=2)
            return response.status, response
        except urllib.error.HTTPError as error:
            return error.code, error

    def stop(self):
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
            if self.process.stdout is not None:
                self.logs.extend(self.process.stdout.read(MAX_RESPONSE + 1))
        finally:
            self.temp.cleanup()
        if len(self.logs) > MAX_RESPONSE or FAKE_KEY.encode() in self.logs:
            raise AssertionError("bounded daemon output leaked fixture API key")


def api_info(key=FAKE_KEY, metadata=None):
    value = {"type": "api", "key": key}
    if metadata is not None:
        value["metadata"] = metadata
    return value


class NativeProviderAuthControlTests(unittest.TestCase):
    def run_service(self, initial=None):
        service = NativeAuthService(self)
        auth = service.data / "opencode" / "auth.json"
        if initial is not None:
            auth.parent.mkdir(parents=True, exist_ok=True)
            auth.write_text(json.dumps(initial, separators=(",", ":")) + "\n", encoding="utf-8")
            os.chmod(auth, 0o600)
        service.start()
        return service, auth

    def test_unauthorized_valid_put_has_no_side_effect(self):
        service, auth = self.run_service()
        try:
            for bearer in ("", "wrong-token"):
                status, response = service.request("PUT", "/auth/openai", api_info(), bearer)
                self.assertIn(status, (401, 403))
                response.read(MAX_RESPONSE)
                self.assertFalse(auth.exists())
        finally:
            service.stop()

    def test_authorized_api_put_persists0600_without_secret_echo(self):
        service, auth = self.run_service()
        try:
            status, response = service.request("PUT", "/auth/openai", api_info(metadata={"account": "fixture"}))
            self.assertEqual(status, 200)
            self.assertTrue(bounded_json(response))
            self.assertTrue(auth.is_file())
            self.assertEqual(auth.stat().st_mode & 0o777, 0o600)
            saved = json.loads(auth.read_text())
            self.assertEqual(saved["openai"], api_info(metadata={"account": "fixture"}))
            self.assertNotIn(FAKE_KEY.encode(), bytes(service.logs))
        finally:
            service.stop()

    def test_existing_oauth_and_api_entries_are_preserved(self):
        initial = {"other": {"type": "oauth", "refresh": "refresh-fixture", "access": "access-fixture", "expires": 7}, "legacy": api_info("old-fixture", {"account": "old"})}
        service, auth = self.run_service(initial)
        try:
            before = json.loads(auth.read_text())
            status, response = service.request("PUT", "/auth/openai/", api_info(metadata={"account": "fixture"}))
            self.assertEqual(status, 200)
            response.read(MAX_RESPONSE)
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
                response.read(MAX_RESPONSE)
                self.assertEqual(auth.read_bytes(), before)
        finally:
            service.stop()

    def test_auth_directory_is_not_overwritten(self):
        service, auth = self.run_service()
        try:
            auth.mkdir(parents=True)
            marker = auth / "marker"
            marker.write_text("owned-marker", encoding="utf-8")
            status, response = service.request("PUT", "/auth/openai", api_info())
            self.assertGreaterEqual(status, 400)
            self.assertLess(status, 600)
            response.read(MAX_RESPONSE)
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
            response.read(MAX_RESPONSE)
            self.assertEqual(auth.read_bytes(), before)
        finally:
            service.stop()


if __name__ == "__main__":
    unittest.main()

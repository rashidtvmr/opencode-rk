#!/usr/bin/env python3
"""Bounded two-process runner for the frozen G6 actual-browser journey.

This is test orchestration only. It does not build/install the product, modify
the frozen fixture/driver, synthesize UI actions, or claim product acceptance.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import threading
import time
from typing import Any


BASE = pathlib.Path(__file__).resolve().parent
FIXTURE = BASE / "web_tool_journey_fixture.py"
DRIVER = BASE / "web_browser_journey.mjs"
APPROVED_ROOT = pathlib.Path(
    "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode"
)
FIXTURE_SHA256 = "d950994a50e9edd9b251c310c0c3a2f176774a509271158cc1343ffb8926a73e"
FIXTURE_BYTES = 17322
DRIVER_SHA256 = "b8e4efa6c00cd269c8a95d802eddad1ba88b4091670ff1327e65057d93eef000"
DRIVER_BYTES = 9647
LOG_BUDGET = 8 * 1024 * 1024
METADATA_SECONDS = 15
FIXTURE_GRACE_SECONDS = 8
OVERALL_SECONDS = 180
SHA40 = re.compile(r"^[0-9a-fA-F]{40}$")
SHA64 = re.compile(r"^[0-9a-fA-F]{64}$")


class GateError(Exception):
    """Invalid inputs or orchestration failure, distinct from semantic RED."""


class LogBudget:
    def __init__(self):
        self.lock = threading.Lock()
        self.retained = 0
        self.truncated = False


class CappedLogs:
    """Drain both pipes continuously while retaining at most 8 MiB total."""

    def __init__(self, stdout_path: pathlib.Path, stderr_path: pathlib.Path, budget: LogBudget):
        self.paths = (stdout_path, stderr_path)
        self.files = tuple(path.open("xb") for path in self.paths)
        self.budget = budget
        self.threads: list[threading.Thread] = []

    def attach(self, process: subprocess.Popen[bytes]) -> None:
        assert process.stdout is not None and process.stderr is not None
        for pipe, output in zip((process.stdout, process.stderr), self.files):
            thread = threading.Thread(target=self._drain, args=(pipe, output), daemon=True)
            thread.start()
            self.threads.append(thread)

    def _drain(self, pipe: Any, output: Any) -> None:
        try:
            while True:
                chunk = pipe.read1(65536)
                if not chunk:
                    return
                with self.budget.lock:
                    remaining = LOG_BUDGET - self.budget.retained
                    if remaining > 0:
                        output.write(chunk[:remaining])
                        output.flush()
                        self.budget.retained += min(len(chunk), remaining)
                    if len(chunk) > max(remaining, 0):
                        self.budget.truncated = True
        finally:
            pipe.close()

    def finish(self) -> None:
        for thread in self.threads:
            thread.join(timeout=2)
        for output in self.files:
            output.close()


class OwnedProcess:
    def __init__(self, command: list[str], cwd: pathlib.Path, env: dict[str, str],
                 stdout: pathlib.Path, stderr: pathlib.Path, budget: LogBudget):
        self.command = command
        self.logs = CappedLogs(stdout, stderr, budget)
        self.process = subprocess.Popen(
            command,
            cwd=str(cwd),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
            close_fds=True,
        )
        self.logs.attach(self.process)

    @property
    def pid(self) -> int:
        return self.process.pid

    def poll(self) -> int | None:
        return self.process.poll()

    def wait(self, timeout: float) -> int | None:
        try:
            return self.process.wait(timeout=max(timeout, 0))
        except subprocess.TimeoutExpired:
            return None

    def signal_group(self, sig: int) -> None:
        # This process created the session/group; never signal by a metadata PID.
        try:
            os.killpg(self.pid, sig)
        except ProcessLookupError:
            pass

    def group_exists(self) -> bool:
        try:
            os.killpg(self.pid, 0)
            return True
        except ProcessLookupError:
            return False
        except PermissionError:
            return True

    def stop_group(self, grace: float = 2.0) -> int | None:
        self.signal_group(signal.SIGTERM)
        if self.wait(grace) is None or self.group_exists():
            self.signal_group(signal.SIGKILL)
        result = self.wait(2.0)
        self.logs.finish()
        return result


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def check_no_symlink(path: pathlib.Path, *, must_exist: bool = True) -> pathlib.Path:
    """Require an absolute path with no symlink in any existing component."""
    if not path.is_absolute():
        raise GateError(f"path must be absolute: {path}")
    current = pathlib.Path(path.anchor)
    parts = path.parts[1:]
    for index, part in enumerate(parts):
        current = current / part
        try:
            info = current.lstat()
        except FileNotFoundError:
            if must_exist or index != len(parts) - 1:
                raise GateError(f"path component does not exist: {current}")
            continue
        if pathlib.Path(current).is_symlink():
            raise GateError(f"symlink paths are not allowed: {current}")
    return path


def regular_file(value: str, label: str, executable: bool = False) -> pathlib.Path:
    path = check_no_symlink(pathlib.Path(value))
    if not path.is_file():
        raise GateError(f"{label} must be a regular file: {path}")
    if executable and not os.access(path, os.X_OK):
        raise GateError(f"{label} is not executable: {path}")
    if not os.access(path, os.R_OK):
        raise GateError(f"{label} is not readable: {path}")
    return path


def under(path: pathlib.Path, root: pathlib.Path) -> bool:
    try:
        path.relative_to(root)
        return path != root
    except ValueError:
        return False


def sha_arg(value: str, label: str) -> str:
    if not SHA40.fullmatch(value):
        raise GateError(f"{label} must be exactly 40 hexadecimal characters")
    return value.lower()


def parse_build(build_path: pathlib.Path, binary: pathlib.Path, expected_source: str,
                expected_tree: str | None) -> dict[str, str]:
    try:
        with build_path.open("rb") as stream:
            raw = stream.read(1024 * 1024 + 1)
        if len(raw) > 1024 * 1024:
            raise GateError("build record exceeds 1 MiB")
        record = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read build record: {error}") from error
    if not isinstance(record, dict):
        raise GateError("build record must be a JSON object")
    source = record.get("source_sha", record.get("sourceSha", record.get("git_sha")))
    if not isinstance(source, str) or not SHA40.fullmatch(source):
        raise GateError("build record has no valid full source SHA")
    if source.lower() != expected_source:
        raise GateError("build source SHA differs from --source-sha")
    recorded_binary = record.get("binary_sha256", record.get("binarySha256"))
    actual_binary = sha256_file(binary)
    if not isinstance(recorded_binary, str) or not SHA64.fullmatch(recorded_binary):
        raise GateError("build record has no valid binary SHA-256")
    if recorded_binary.lower() != actual_binary:
        raise GateError("installed binary bytes differ from producer build record")
    tree = record.get("source_tree", record.get("source_tree_sha", record.get("sourceTreeSha")))
    if expected_tree is not None:
        if not isinstance(tree, str) or not SHA40.fullmatch(tree):
            raise GateError("--source-tree supplied but build record has no full source-tree SHA")
        if tree.lower() != expected_tree:
            raise GateError("build source-tree SHA differs from --source-tree")
    result = {"source_sha": source.lower(), "binary_sha256": actual_binary}
    if isinstance(tree, str) and SHA40.fullmatch(tree):
        result["source_tree"] = tree.lower()
    return result


def read_json_bounded(path: pathlib.Path, maximum: int = 512 * 1024) -> dict[str, Any] | None:
    try:
        if path.is_symlink() or not path.is_file():
            return None
        with path.open("rb") as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            return None
        value = json.loads(raw.decode("utf-8"))
        return value if isinstance(value, dict) else None
    except (OSError, UnicodeError, json.JSONDecodeError):
        return None


def wait_metadata(path: pathlib.Path, fixture: OwnedProcess, deadline: float) -> dict[str, Any]:
    while time.monotonic() < deadline:
        metadata = read_json_bounded(path)
        if metadata is not None:
            root = pathlib.Path(metadata.get("fixture_root", ""))
            control = pathlib.Path(metadata.get("control_dir", ""))
            if (root.is_absolute() and control.is_absolute()
                    and control.parent == path.parent and control.is_dir()
                    and isinstance(metadata.get("pid"), int)
                    and metadata.get("pid") > 1
                    and isinstance(metadata.get("launch_fragment"), str)
                    and isinstance(metadata.get("origin"), str)):
                return metadata
        if fixture.poll() is not None:
            raise GateError("fixture exited before publishing valid browser metadata")
        time.sleep(0.05)
    raise GateError("fixture metadata availability exceeded 15 seconds")


def verify_metadata(metadata: dict[str, Any], artifacts: pathlib.Path,
                    build: dict[str, str]) -> None:
    source = metadata.get("source_sha")
    binary = metadata.get("binary_sha256")
    if not isinstance(source, str) or source.lower() != build["source_sha"]:
        raise GateError("fixture metadata source SHA differs from verified producer build")
    if not isinstance(binary, str) or binary.lower() != build["binary_sha256"]:
        raise GateError("fixture metadata binary SHA differs from installed production binary")
    control = pathlib.Path(metadata["control_dir"])
    evidence = pathlib.Path(metadata.get("request_evidence", ""))
    if control.parent != artifacts or evidence != artifacts / "provider-requests.json":
        raise GateError("fixture control/evidence path is not in the fresh artifact directory")
    root = pathlib.Path(metadata["fixture_root"])
    if root.parent != pathlib.Path("/private/tmp") and sys.platform == "darwin":
        raise GateError("fixture temporary root is not the approved Darwin /private/tmp location")
    if not isinstance(metadata.get("launch_fragment"), str):
        raise GateError("fixture metadata has no browser launch fragment")


def sanitized_env(home: pathlib.Path, tmp: pathlib.Path, path_value: str) -> dict[str, str]:
    return {
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_DATA_HOME": str(home / ".local" / "share"),
        "TMPDIR": str(tmp),
        "PATH": path_value,
        "LANG": "C",
        "LC_ALL": "C",
    }


def command_path(command: list[str]) -> list[str]:
    # Preserve argv exactly as arrays in receipts; never pass through a shell.
    return [str(item) for item in command]


def discover_playwright_pin(preload: pathlib.Path) -> str | None:
    """Report package version only when found beside the explicitly owned hook."""
    for parent in preload.parents:
        package = parent / "node_modules" / "playwright" / "package.json"
        if package.exists():
            check_no_symlink(package)
            if not package.is_file() or package.stat().st_size > 64 * 1024:
                return None
            try:
                value = json.loads(package.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError):
                return None
            if value.get("name") == "playwright" and isinstance(value.get("version"), str):
                return value["version"]
    return None


def classify(node_exit: int | None, fixture_exit: int | None, overall_timed_out: bool,
             fixture_result: dict[str, Any] | None, node_stderr: pathlib.Path) -> str:
    if overall_timed_out:
        return "harness_overall_timeout_not_product_red"
    if node_exit is not None and node_exit != 0:
        try:
            content = node_stderr.read_bytes()
        except OSError:
            content = b""
        if b"G6 permission-ui-red:" in content:
            return "actual_browser_driver_semantic_permission_ui_red"
        if b"G6 backend-pending-red:" in content:
            return "actual_browser_driver_semantic_backend_pending_red"
        return "actual_browser_driver_nonzero_unclassified_not_product_red"
    if fixture_exit not in (None, 0):
        return "fixture_process_nonzero_harness_failure"
    if fixture_result and fixture_result.get("provider_failure"):
        return "fixture_provider_contract_failure_not_product_red"
    if node_exit == 0 and fixture_exit == 0:
        return "browser_driver_and_fixture_completed"
    return "harness_setup_or_cleanup_failure"


def write_receipt(path: pathlib.Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(path.name + ".tmp")
    with temporary.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def write_artifact_manifest(root: pathlib.Path) -> None:
    """Hash retained evidence after receipt; deliberately omit this manifest."""
    rows = []
    manifest_path = root / "artifact-manifest.json"
    for path in sorted(root.rglob("*")):
        if path == manifest_path:
            continue
        if path.is_symlink():
            raise GateError(f"refusing symlink in evidence artifact directory: {path}")
        if not path.is_file():
            continue
        rows.append({"path": str(path.relative_to(root)), "bytes": path.stat().st_size,
                     "sha256": sha256_file(path)})
    if not rows:
        raise GateError("cannot create an empty G6 evidence manifest")
    write_receipt(manifest_path, {
        "manifest_scope": "files present after runner-receipt; manifest file excluded",
        "files": rows,
    })


def main() -> int:
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, help="absolute installed production binary")
    parser.add_argument("--build-json", required=True, help="absolute producer build record")
    parser.add_argument("--source-sha", required=True, help="expected full 40-character source SHA")
    parser.add_argument("--source-tree", help="optional expected full 40-character source-tree SHA")
    parser.add_argument("--artifact-dir", required=True, help="fresh absolute directory under approved evidence root")
    parser.add_argument("--node", required=True, help="absolute existing Node executable")
    parser.add_argument("--playwright-preload", required=True, help="absolute owned Node resolve-hook module")
    parser.add_argument("--browser-cache", required=True, help="absolute owned pinned Playwright browser cache")
    args = parser.parse_args()

    fixture_proc: OwnedProcess | None = None
    node_proc: OwnedProcess | None = None
    artifacts: pathlib.Path | None = None
    metadata_path: pathlib.Path | None = None
    metadata: dict[str, Any] | None = None
    fixture_exit: int | None = None
    node_exit: int | None = None
    timed_out = False
    failure: str | None = None
    commands: dict[str, list[str]] = {}
    common_path: str | None = None
    logs_capped = False
    started = time.monotonic()
    build: dict[str, str] = {}
    pin: str | None = None
    log_budget = LogBudget()

    try:
        source = sha_arg(args.source_sha, "--source-sha")
        tree = sha_arg(args.source_tree, "--source-tree") if args.source_tree else None
        binary = regular_file(args.binary, "--binary", executable=True)
        build_path = regular_file(args.build_json, "--build-json")
        node = regular_file(args.node, "--node", executable=True)
        preload = regular_file(args.playwright_preload, "--playwright-preload")
        if not under(preload, APPROVED_ROOT):
            raise GateError("--playwright-preload must be inside the approved owned temporary root")
        cache = check_no_symlink(pathlib.Path(args.browser_cache))
        if not cache.is_dir():
            raise GateError("--browser-cache must be an existing directory")
        if not os.access(cache, os.R_OK | os.X_OK):
            raise GateError("--browser-cache is not readable")
        if not under(cache, APPROVED_ROOT):
            raise GateError("--browser-cache must be inside the approved owned temporary root")
        build = parse_build(build_path, binary, source, tree)
        pin = discover_playwright_pin(preload)

        for file, expected_digest, expected_size, label in (
            (FIXTURE, FIXTURE_SHA256, FIXTURE_BYTES, "fixture"),
            (DRIVER, DRIVER_SHA256, DRIVER_BYTES, "browser driver"),
        ):
            check_no_symlink(file)
            if not file.is_file() or file.stat().st_size != expected_size or sha256_file(file) != expected_digest:
                raise GateError(f"frozen {label} bytes do not match the controller-verified manifest")

        artifacts = check_no_symlink(pathlib.Path(args.artifact_dir), must_exist=False)
        if not under(artifacts, APPROVED_ROOT):
            raise GateError(f"--artifact-dir must be a fresh child of {APPROVED_ROOT}")
        if artifacts.exists():
            raise GateError("--artifact-dir already exists; preserve evidence and use a fresh directory")
        artifacts.mkdir(mode=0o700, parents=True, exist_ok=False)
        artifacts = artifacts.resolve(strict=True)
        if not under(artifacts, APPROVED_ROOT):
            raise GateError("artifact directory resolved outside approved evidence root")
        metadata_path = artifacts / "fixture-metadata.json"
        temp = artifacts / "tmp"
        home = artifacts / "runner-home"
        temp.mkdir(mode=0o700)
        home.mkdir(mode=0o700)
        fixture_logs = (artifacts / "fixture.stdout.log", artifacts / "fixture.stderr.log")
        node_logs = (artifacts / "browser.stdout.log", artifacts / "browser.stderr.log")
        common_path = ":".join((str(pathlib.Path(node).parent), "/usr/bin", "/bin", "/usr/sbin", "/sbin"))
        fixture_env = sanitized_env(home, temp, common_path)
        fixture_cmd = [
            sys.executable, str(FIXTURE), "--binary", str(binary),
            "--build-json", str(build_path), "--artifact-dir", str(artifacts),
        ]
        commands["fixture"] = command_path(fixture_cmd)
        fixture_proc = OwnedProcess(fixture_cmd, artifacts, fixture_env, *fixture_logs, log_budget)

        metadata = wait_metadata(metadata_path, fixture_proc, time.monotonic() + METADATA_SECONDS)
        verify_metadata(metadata, artifacts, build)
        node_env = sanitized_env(home, temp, common_path)
        node_env["PLAYWRIGHT_BROWSERS_PATH"] = str(cache)
        node_cmd = [str(node), "--import", str(preload), str(DRIVER), "--metadata", str(metadata_path)]
        commands["browser"] = command_path(node_cmd)
        node_proc = OwnedProcess(node_cmd, artifacts, node_env, *node_logs, log_budget)
        overall_deadline = started + OVERALL_SECONDS
        while node_proc.poll() is None:
            if time.monotonic() >= overall_deadline:
                timed_out = True
                node_proc.signal_group(signal.SIGTERM)
                if node_proc.wait(2.0) is None:
                    node_proc.signal_group(signal.SIGKILL)
                break
            if fixture_proc.poll() is not None:
                # Keep the actual driver alive long enough to report the frozen
                # semantic failure and retain its real exit status. The fixture
                # can terminate immediately after its provider contract catches
                # an erroneous side effect, before Node prints the matching RED.
                fixture_exit = fixture_proc.poll()
            time.sleep(0.05)
        node_exit = node_proc.wait(2.0)
        if node_exit is None:
            node_proc.stop_group(0.1)
            node_exit = node_proc.poll()
        # The frozen browser driver normally requests this itself; repeat safely
        # after exit so early browser failures still release the owned daemon.
        control_raw = metadata.get("control_dir") if metadata else None
        if isinstance(control_raw, str):
            control = pathlib.Path(control_raw)
            if control.parent == artifacts and control.is_dir() and not control.is_symlink():
                stop_file = control / "stop.request"
                if not stop_file.exists():
                    try:
                        fd = os.open(stop_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                        with os.fdopen(fd, "w", encoding="utf-8") as stream:
                            stream.write("stop\n")
                    except FileExistsError:
                        pass
        fixture_exit = fixture_proc.wait(FIXTURE_GRACE_SECONDS)
        if fixture_exit is None:
            fixture_proc.stop_group(1.0)
            fixture_exit = fixture_proc.poll()

    except (GateError, OSError, ValueError) as error:
        failure = str(error)
    finally:
        if node_proc is not None:
            if node_proc.poll() is None:
                node_proc.stop_group(1.0)
            else:
                node_exit = node_proc.poll()
                node_proc.logs.finish()
        if fixture_proc is not None:
            # Ask the fixture's own control loop to terminate first. Its
            # process-group cleanup is safe because it owns that new session.
            if metadata and isinstance(metadata.get("control_dir"), str):
                control = pathlib.Path(metadata["control_dir"])
                if control.parent == artifacts and control.is_dir() and not control.is_symlink():
                    stop_file = control / "stop.request"
                    if not stop_file.exists():
                        try:
                            fd = os.open(stop_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                            with os.fdopen(fd, "w", encoding="utf-8") as stream:
                                stream.write("stop\n")
                        except FileExistsError:
                            pass
            if fixture_proc.poll() is None:
                fixture_exit = fixture_proc.wait(FIXTURE_GRACE_SECONDS)
            # Also sweep the runner-owned group if the fixture exited early and
            # its daemon was orphaned into that same group.
            fixture_proc.stop_group(1.0)
            fixture_exit = fixture_proc.poll() if fixture_exit is None else fixture_exit
            fixture_proc.logs.finish()
        if node_proc is not None:
            logs_capped = logs_capped or log_budget.truncated
        if fixture_proc is not None:
            logs_capped = logs_capped or log_budget.truncated

        if artifacts is not None and artifacts.is_dir():
            fixture_result = read_json_bounded(artifacts / "fixture-result.json")
            outcome = classify(node_exit, fixture_exit, timed_out, fixture_result,
                               artifacts / "browser.stderr.log")
            semantic_red = outcome in (
                "actual_browser_driver_semantic_permission_ui_red",
                "actual_browser_driver_semantic_backend_pending_red",
            )
            receipt = {
                "gate": "G6 actual-browser journey runner",
                "status": (
                    "FROZEN_G6_OBSERVED_RED"
                    if semantic_red
                    else "FROZEN_EXECUTABLE_TEST_RUNNER_PREP_ONLY_NOT_RED"
                ),
                "classification": outcome,
                "runner_invocation": [sys.executable, str(pathlib.Path(__file__).resolve()), *sys.argv[1:]],
                "commands": commands,
                "execution_envelope": {
                    "overall_deadline_seconds": OVERALL_SECONDS,
                    "fixture_metadata_deadline_seconds": METADATA_SECONDS,
                    "fixture_grace_seconds": FIXTURE_GRACE_SECONDS,
                    "environment": {
                        "HOME": str(artifacts / "runner-home"),
                        "TMPDIR": str(artifacts / "tmp"),
                        "sanitized_path": common_path if artifacts.exists() else None,
                        "playwright_browsers_path": str(cache) if "cache" in locals() else None,
                        "inherited_user_credentials_or_proxies": False,
                    },
                },
                "source": build,
                "installed_binary": str(args.binary),
                "frozen_files": {
                    "fixture": {"path": str(FIXTURE), "sha256": FIXTURE_SHA256, "bytes": FIXTURE_BYTES},
                    "driver": {"path": str(DRIVER), "sha256": DRIVER_SHA256, "bytes": DRIVER_BYTES},
                },
                "node_exit": node_exit,
                "fixture_exit": fixture_exit,
                "browser_package_pin": pin,
                "browser_package_pin_source": str(pathlib.Path(args.playwright_preload)) if pin else None,
                "log_budget_bytes_total": LOG_BUDGET,
                "logs_truncated": logs_capped,
                "elapsed_seconds": round(time.monotonic() - started, 3),
                "overall_timeout": timed_out,
                "harness_failure": failure,
                "fixture_metadata": str(metadata_path) if metadata_path and metadata_path.exists() else None,
                "fixture_result": fixture_result,
                "observations": [
                    (
                        "The frozen actual browser driver reported a semantic G6 RED; see preserved driver stderr and fixture/provider evidence."
                        if semantic_red
                        else "No browser semantic RED is asserted by this receipt."
                    ),
                    "A nonzero/unclassified driver result and all harness failures are not product semantic RED.",
                    "G6 product acceptance requires a fresh train binary and an independent run on the exact integrated SHA.",
                ],
            }
            try:
                write_receipt(artifacts / "runner-receipt.json", receipt)
                write_artifact_manifest(artifacts)
            except OSError as error:
                print(f"could not write runner receipt: {error}", file=sys.stderr)
            except GateError as error:
                print(f"could not write evidence manifest: {error}", file=sys.stderr)

    if timed_out:
        print("G6 overall harness timeout (not product RED)", file=sys.stderr)
        return 124
    if node_exit not in (None, 0):
        return node_exit if node_exit > 0 else 128 - node_exit
    if fixture_exit not in (None, 0):
        return fixture_exit if fixture_exit > 0 else 128 - fixture_exit
    if failure:
        print(f"G6 harness failure (not product RED): {failure}", file=sys.stderr)
        return 70
    if node_exit == 0 and fixture_exit == 0:
        return 0
    return 70


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"G6 harness input error (not product RED): {error}", file=sys.stderr)
        raise SystemExit(70)

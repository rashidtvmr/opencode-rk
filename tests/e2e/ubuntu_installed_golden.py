#!/usr/bin/env python3
"""G7 installed Ubuntu release contract for the native G3/G4 golden journey."""

import argparse
import fcntl
import hashlib
import json
import os
import pathlib
import platform
import pty
import re
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import urllib.parse
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import native_stream_tool as golden  # noqa: E402

MAX_ARCHIVE_BYTES = 256 * 1024 * 1024
MAX_HTTP_BODY = 256 * 1024
MAX_CAPTURE = golden.MAX_PTY
SOCKET_LIMIT = 100
HEX40 = re.compile(r"^[0-9a-fA-F]{40}$")
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
TARGET = "aarch64-unknown-linux-gnu"
BASE_PATH = "/usr/bin:/bin"
HELPER_SHA256 = "c6d8d561a96136061c4d6e87e183bad144e762df17847fcfca4a347d0d30633e"
SETUP_SHA256 = "32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc"
INSTALLER_SHA256 = "9b7f259dc02bb38e86cac52482e0ac01c2c09074c648533f3e6ec6d255c07f16"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def checked_file(raw, label):
    path = pathlib.Path(raw)
    if not path.is_absolute():
        raise ValueError(f"{label} path must be absolute")
    if path.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    path = path.resolve(strict=True)
    if not path.is_file():
        raise ValueError(f"{label} must be a regular file")
    return path


def git_command(*arguments):
    env = {
        "HOME": "/",
        "PATH": BASE_PATH,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_OPTIONAL_LOCKS": "0",
    }
    try:
        return subprocess.run(
            ["/usr/bin/git", "-C", str(ROOT), *arguments],
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=5,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError("could not verify source checkout with Git") from error


def git_head_sha():
    """Read this checkout's real HEAD (works for linked Git worktrees too)."""
    result = git_command("rev-parse", "--verify", "HEAD^{commit}")
    value = result.stdout.decode("ascii", "replace").strip().lower()
    if result.returncode != 0 or not HEX40.fullmatch(value):
        raise RuntimeError("could not attest the current checkout's Git HEAD")
    return value


def changed_source_paths():
    result = git_command("status", "--porcelain=v1", "-z", "--untracked-files=all")
    if result.returncode != 0:
        raise RuntimeError("could not verify source checkout cleanliness")
    fields = result.stdout.split(b"\0")
    paths = set()
    index = 0
    while index < len(fields):
        field = fields[index]
        index += 1
        if not field:
            continue
        if len(field) < 4:
            raise RuntimeError("Git returned malformed porcelain status")
        name = field[3:]
        if field[:2] in (b" R", b"R ", b" C", b"C "):
            if index >= len(fields) or not fields[index]:
                raise RuntimeError("Git returned malformed renamed-path status")
            index += 1
        paths.add(name.decode("utf-8", "surrogateescape"))
    return paths

def check_source_and_helpers(expected_source_sha):
    if not isinstance(expected_source_sha, str) or not HEX40.fullmatch(expected_source_sha):
        raise ValueError("--expected-source-sha must be 40 hexadecimal characters")
    expected_source_sha = expected_source_sha.lower()
    actual_head = git_head_sha()
    if actual_head != expected_source_sha:
        raise ValueError(
            f"checkout HEAD mismatch: actual={actual_head} expected={expected_source_sha}"
        )
    allowed = {str((HERE / "ubuntu_installed_golden.py").relative_to(ROOT))}
    changed = changed_source_paths()
    if changed - allowed:
        raise RuntimeError("G7 source checkout has unrelated changes: " + ", ".join(sorted(changed - allowed)))
    helper = checked_file(HERE / "native_stream_tool.py", "native G3/G4 helper")
    setup = checked_file(HERE / "native_provider_setup.py", "native G3/G4 helper dependency")
    installer = checked_file(ROOT / "scripts" / "install-oc2.sh", "source installer")
    if sha256(helper) != HELPER_SHA256:
        raise ValueError("native G3/G4 helper hash differs from the reviewed pin")
    if sha256(setup) != SETUP_SHA256:
        raise ValueError("native G3/G4 helper dependency hash differs from the reviewed pin")
    if sha256(installer) != INSTALLER_SHA256:
        raise ValueError("source installer hash differs from the reviewed pin")
    return actual_head, sha256(helper), sha256(setup), sha256(installer)


def validate_prerequisites(archive, checksum, manifest_path, expected_source_sha):
    if not isinstance(expected_source_sha, str) or not HEX40.fullmatch(expected_source_sha):
        raise ValueError("--expected-source-sha must be 40 hexadecimal characters")
    expected_source_sha = expected_source_sha.lower()
    if not isinstance(checksum, str) or not HEX64.fullmatch(checksum):
        raise ValueError("--checksum must be 64 hexadecimal characters")
    if archive.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError("release archive exceeds 256 MiB")
    if manifest_path.stat().st_size > 64 * 1024:
        raise ValueError("build.json exceeds 64 KiB")
    raw_manifest = manifest_path.read_bytes()
    if len(raw_manifest) > 64 * 1024:
        raise ValueError("build.json exceeds 64 KiB")
    build = json.loads(raw_manifest)
    if not isinstance(build, dict):
        raise ValueError("build.json must contain an object")
    source = build.get("source_sha") or build.get("sourceSha") or build.get("git_sha")
    binary_hash_raw = build.get("binary_sha256") or build.get("binarySha256") or ""
    library_hash_raw = (
        build.get("native_library_sha256")
        or build.get("nativeLibrarySha256")
        or build.get("library_sha256")
        or build.get("librarySha256")
        or ""
    )
    declared_archive_raw = build.get("archive_sha256") or ""
    if not all(isinstance(value, str) for value in (binary_hash_raw, library_hash_raw, declared_archive_raw)):
        raise ValueError("build.json digest fields must be strings")
    binary_hash = binary_hash_raw.lower()
    library_hash = library_hash_raw.lower()
    declared_archive = declared_archive_raw.lower()
    if not isinstance(source, str) or not HEX40.fullmatch(source):
        raise ValueError("build.json lacks a 40-character source SHA")
    if source.lower() != expected_source_sha:
        raise ValueError(
            f"release source SHA mismatch: build={source.lower()} expected={expected_source_sha}"
        )
    if build.get("profile") != "release" or build.get("native") is not True:
        raise ValueError("build.json must attest a native release profile")
    if build.get("target") != TARGET:
        raise ValueError(f"build.json target must be {TARGET}")
    if not HEX64.fullmatch(binary_hash) or not HEX64.fullmatch(library_hash):
        raise ValueError("build.json lacks binary/native-library SHA-256 attestations")
    actual_archive = sha256(archive)
    if actual_archive != checksum.lower() or declared_archive != actual_archive:
        raise ValueError("archive SHA-256 differs from checksum/build.json")
    return build, actual_archive, binary_hash, library_hash


def ubuntu_arm64():
    if platform.system() != "Linux" or platform.machine().lower() not in ("aarch64", "arm64"):
        raise RuntimeError("G7 runtime requires Ubuntu Linux arm64")
    try:
        os_id = platform.freedesktop_os_release().get("ID", "").lower()
    except (AttributeError, OSError):
        os_id = ""
    if os_id != "ubuntu":
        raise RuntimeError("G7 runtime requires Ubuntu (ID=ubuntu)")


def write_failure_evidence(artifacts, reason):
    path = artifacts / "ubuntu-installed-golden-failure.json"
    try:
        path.write_text(
            json.dumps(
                {
                    "phase": "failed",
                    "owned_process_cleanup": False,
                    "reason": reason,
                },
                indent=2,
            )
            + "\n"
        )
    except OSError as error:
        print(f"could not write G7 failure ownership evidence: {error}", file=sys.stderr)


def backend_descriptor(data_dir):
    path = data_dir / "runtime" / "backend.json"
    value = json.loads(path.read_text())
    origin = value.get("http_origin")
    token = value.get("auth_token")
    pid = value.get("pid")
    parsed = urllib.parse.urlsplit(origin or "")
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or parsed.port is None
        or parsed.username is not None
        or parsed.password is not None
        or not HEX64.fullmatch(token or "")
        or not isinstance(pid, int)
        or pid <= 1
    ):
        raise RuntimeError("owned daemon descriptor is invalid")
    return {"origin": origin, "token": token, "pid": pid, "port": parsed.port}


def authenticated_sessions(descriptor):
    request = urllib.request.Request(
        descriptor["origin"] + "/api/sessions",
        headers={"Authorization": "Bearer " + descriptor["token"]},
    )
    with urllib.request.urlopen(request, timeout=2) as response:
        raw = response.read(MAX_HTTP_BODY + 1)
        if response.status != 200 or len(raw) > MAX_HTTP_BODY:
            raise RuntimeError("bounded authenticated session-list request failed")
    value = json.loads(raw)
    sessions = value.get("sessions") if isinstance(value, dict) else None
    if not isinstance(sessions, list) or len(sessions) != 1:
        raise RuntimeError("second client did not observe the single active session")
    session_id = sessions[0].get("id") if isinstance(sessions[0], dict) else None
    if not isinstance(session_id, str) or not session_id:
        raise RuntimeError("active session has no ID")
    return session_id


def request_count(state):
    with state.lock:
        return len(state.requests)


def run_second_client(state, binary):
    project = state.project
    root = project.parent
    data = root / "data"
    before = backend_descriptor(data)
    if request_count(state) != 1 or state.phase != "waiting_partial_observation":
        raise RuntimeError("second-client probe did not overlap the pending first turn")
    try:
        os.kill(before["pid"], 0)
    except (ProcessLookupError, PermissionError) as error:
        raise RuntimeError("first client's validated daemon is not live") from error
    owner = getattr(state, "primary_process", None)
    if owner is None or owner.poll() is not None:
        raise RuntimeError("cannot establish the live first-client daemon owner")
    try:
        if os.getpgid(before["pid"]) != owner.pid:
            raise RuntimeError("daemon process group is not owned by the active first client")
    except ProcessLookupError as error:
        raise RuntimeError("first client's daemon exited before second-client attachment") from error
    first_session = authenticated_sessions(before)

    master = slave = child = None
    captured = bytearray()
    screen = golden.TerminalScreen(80, 24)
    saved_attrs = None
    cleanup_errors = []
    primary_error = None
    try:
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        saved_attrs = termios.tcgetattr(master)
        env = {
            "HOME": str(root / "home"),
            "PATH": BASE_PATH,
            "TERM": "xterm-256color",
            "TMPDIR": str(root),
            "XDG_CONFIG_HOME": str(root / "home" / ".config"),
            "XDG_DATA_HOME": str(data),
            "XDG_RUNTIME_DIR": str(data / "runtime"),
            "OPENCODE_RK_HOME": str(data),
            "OPENCODE_PROJECT_DIR": str(project),
            "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{before['port']}",
        }
        terminal_exec = (
            "import fcntl, os, sys, termios; "
            "fcntl.ioctl(0, termios.TIOCSCTTY, 0); "
            "os.execve(sys.argv[1], sys.argv[1:], dict(os.environ))"
        )
        child = subprocess.Popen(
            [sys.executable, "-c", terminal_exec, str(binary), "--data-dir", str(data)],
            cwd=project,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            start_new_session=True,
            close_fds=True,
        )
        os.close(slave)
        slave = None
        golden.read_until(
            master,
            time.monotonic() + golden.FIXTURE_TIMEOUT,
            captured,
            b"OpenCode",
            0,
            screen,
        )
        if child.poll() is not None:
            raise RuntimeError("second client exited instead of attaching")
        if os.tcgetpgrp(master) != child.pid:
            raise RuntimeError("second client does not own its foreground controlling terminal")
        raw_attrs = termios.tcgetattr(master)
        if raw_attrs[3] & (termios.ICANON | termios.ECHO | termios.ISIG):
            raise RuntimeError("second client did not enter native raw terminal mode")
        after = backend_descriptor(data)
        if (after["pid"], after["origin"], after["token"]) != (
            before["pid"], before["origin"], before["token"]
        ):
            raise RuntimeError("second client changed the validated daemon descriptor")
        second_session = authenticated_sessions(after)
        if second_session != first_session:
            raise RuntimeError("second client observed a different session")
        if request_count(state) != 1:
            raise RuntimeError("second client duplicated provider work")
        if golden.KEY.encode() in captured or golden.KEY in screen.text():
            raise RuntimeError("fixture credential appeared in second-client terminal output")
        os.write(master, b"\x03")
        child.wait(timeout=10)
        if child.returncode != 0:
            raise RuntimeError(f"second client exited with status {child.returncode}")
        if termios.tcgetattr(master) != saved_attrs:
            raise RuntimeError("second client did not restore its PTY terminal settings")
        if request_count(state) != 1:
            raise RuntimeError("second-client exit duplicated provider work")
        return {
            "attached_to_existing_daemon": True,
            "daemon_pid": before["pid"],
            "active_session_count": 1,
            "provider_requests_during_attachment": 1,
            "client_exit_status": child.returncode,
            "foreground_controlling_terminal": True,
            "raw_terminal_observed": True,
            "terminal_restored": True,
            "child_reaped": child.poll() is not None,
            "credential_echo": False,
        }
    except BaseException as error:
        primary_error = error
        raise
    finally:
        if child is not None and child.poll() is None:
            if master is not None:
                try:
                    os.write(master, b"\x03")
                except OSError:
                    pass
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                try:
                    if os.getpgid(child.pid) != child.pid:
                        raise RuntimeError("refusing to signal a changed second-client process group")
                    os.killpg(child.pid, signal.SIGTERM)
                except (OSError, RuntimeError) as error:
                    cleanup_errors.append(f"could not safely terminate second client: {error}")
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    try:
                        if os.getpgid(child.pid) != child.pid:
                            raise RuntimeError("refusing to kill a changed second-client process group")
                        os.killpg(child.pid, signal.SIGKILL)
                    except (OSError, RuntimeError) as error:
                        cleanup_errors.append(f"could not safely kill second client: {error}")
                    try:
                        child.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        cleanup_errors.append("second-client process could not be reaped")
        if child is not None and child.poll() is None:
            cleanup_errors.append("second-client process remains live; runtime must be preserved")
            state.g7_unsafe_cleanup = True
            write_failure_evidence(
                state.g7_artifacts,
                f"second-client pid {child.pid} remains live after bounded cleanup",
            )
        if slave is not None:
            try:
                os.close(slave)
            except OSError as error:
                cleanup_errors.append(f"could not close second-client slave PTY: {error}")
        if child is not None and child.poll() is not None and saved_attrs is not None and master is not None:
            try:
                if termios.tcgetattr(master) != saved_attrs:
                    cleanup_errors.append("second-client PTY settings were not restored")
            except OSError as error:
                cleanup_errors.append(f"could not verify second-client terminal restoration: {error}")
        if master is not None:
            try:
                os.close(master)
            except OSError as error:
                cleanup_errors.append(f"could not close second-client master PTY: {error}")
        if cleanup_errors:
            if primary_error is None:
                raise RuntimeError("; ".join(cleanup_errors))
            if hasattr(primary_error, "add_note"):
                primary_error.add_note("second-client cleanup: " + "; ".join(cleanup_errors))


class CallbackEvent(threading.Event):
    def __init__(self, callback):
        super().__init__()
        self.callback = callback
        self.callback_used = False
        self.callback_lock = threading.Lock()

    def set(self):
        with self.callback_lock:
            invoke = not self.callback_used
            self.callback_used = True
        try:
            if invoke:
                self.callback()
        finally:
            super().set()


def run_golden(binary, library, manifest, artifacts):
    original_state = golden.ProviderState
    original_openpty = golden.pty.openpty
    original_reaper = golden.reap_cli_then_daemon
    observations = []
    pty_records = {}
    primary_clients = []
    primary_terminal_checks = []
    lifecycle_checks = []

    class SecondClientState(original_state):
        def __init__(self, project):
            super().__init__(project)
            self.g7_artifacts = artifacts
            self.g7_unsafe_cleanup = False
            self.primary_process = primary_clients[-1] if primary_clients else None
            nonlocal active_state
            active_state = self
            self.partial_visible = CallbackEvent(
                lambda: observations.append(run_second_client(self, binary))
            )

    def tracked_openpty(*args, **kwargs):
        master, slave = original_openpty(*args, **kwargs)
        pty_records[master] = termios.tcgetattr(master)
        return master, slave

    def tracked_reaper(child, descriptor_value, master):
        original_reaper(child, descriptor_value, master)
        saved = pty_records.get(master)
        if saved is None:
            raise RuntimeError("primary client reaped without an owned PTY snapshot")
        try:
            restored = termios.tcgetattr(master) == saved
        except OSError:
            restored = False
        reaped = child.poll() is not None
        lifecycle_checks.append(
            {"child_reaped": reaped, "terminal_restored": restored}
        )
        if not reaped or not restored:
            raise RuntimeError("primary client did not reap and restore its PTY")

    original_popen = subprocess.Popen
    binary_sha = sha256(pathlib.Path(binary))
    active_state = None

    def controlling_terminal_popen(argv, *args, **kwargs):
        argv = [str(part) for part in argv]
        if (
            argv
            and pathlib.Path(argv[0]).is_file()
            and sha256(pathlib.Path(argv[0])) == binary_sha
        ):
            terminal_exec = (
                "import fcntl, os, sys, termios; "
                "fcntl.ioctl(0, termios.TIOCSCTTY, 0); "
                "os.execve(sys.argv[1], sys.argv[1:], dict(os.environ))"
            )
            argv = [sys.executable, "-c", terminal_exec, *argv]
        child = original_popen(argv, *args, **kwargs)
        if argv and argv[0] == sys.executable and len(argv) > 1 and "TIOCSCTTY" in argv[1]:
            master = next(reversed(pty_records))
            deadline = time.monotonic() + 3
            controlling_terminal = False
            raw_terminal = False
            while time.monotonic() < deadline and child.poll() is None:
                try:
                    controlling_terminal = os.tcgetpgrp(master) == child.pid
                    attrs = termios.tcgetattr(master)
                    raw_terminal = not bool(attrs[3] & (termios.ICANON | termios.ECHO | termios.ISIG))
                except OSError:
                    pass
                if controlling_terminal and raw_terminal:
                    break
                time.sleep(0.01)
            primary_terminal_checks.append(
                {
                    "foreground_controlling_terminal": controlling_terminal,
                    "raw_terminal_observed": raw_terminal,
                }
            )
            if not controlling_terminal or not raw_terminal:
                raise RuntimeError("primary client did not acquire controlling terminal and enter raw mode")
            primary_clients.append(child)
        return child

    golden.ProviderState = SecondClientState
    golden.pty.openpty = tracked_openpty
    golden.reap_cli_then_daemon = tracked_reaper
    old_tmpdir = os.environ.get("TMPDIR")
    old_path = os.environ.get("PATH")
    subprocess.Popen = controlling_terminal_popen
    try:
        os.environ["TMPDIR"] = str(artifacts / "runtime-tmp")
        os.environ["PATH"] = BASE_PATH
        (artifacts / "runtime-tmp").mkdir()
        try:
            golden.run(binary, library, manifest, artifacts)
        except BaseException:
            if active_state is not None and active_state.g7_unsafe_cleanup:
                write_failure_evidence(
                    artifacts,
                    "second-client process was not reaped; preserve fixture runtime",
                )
            raise
    finally:
        golden.ProviderState = original_state
        golden.pty.openpty = original_openpty
        golden.reap_cli_then_daemon = original_reaper
        subprocess.Popen = original_popen
        if old_tmpdir is None:
            os.environ.pop("TMPDIR", None)
        else:
            os.environ["TMPDIR"] = old_tmpdir
        if old_path is None:
            os.environ.pop("PATH", None)
        else:
            os.environ["PATH"] = old_path
    if len(observations) != 1:
        raise RuntimeError("second-client overlap probe did not run exactly once")
    if len(lifecycle_checks) != 2 or not all(
        item["child_reaped"] and item["terminal_restored"] for item in lifecycle_checks
    ):
        raise RuntimeError("both primary golden clients must be reaped and restore terminal state")
    if len(primary_terminal_checks) != 2 or not all(
        item["foreground_controlling_terminal"] and item["raw_terminal_observed"]
        for item in primary_terminal_checks
    ):
        raise RuntimeError("both primary clients must own a real raw-mode controlling terminal")
    if (artifacts / "ubuntu-installed-golden-failure.json").is_file():
        raise RuntimeError("second-client cleanup was not proven; preserve the fixture runtime")
    if request_count_from_result(artifacts) != 4:
        raise RuntimeError("golden journey did not settle exactly four provider turns")
    result_path = artifacts / "native-stream-tool-result.json"
    result = json.loads(result_path.read_text())
    if result.get("phase") != "success":
        raise RuntimeError("installed golden journey did not complete successfully")
    evidence = {
        "phase": "success",
        "source_sha": result.get("source_sha"),
        "binary_sha256": result.get("binary_sha256"),
        "native_library_sha256": result.get("native_library_sha256"),
        "test_sha256": sha256(pathlib.Path(__file__).resolve()),
        "second_client": observations[0],
        "primary_client_lifecycle": lifecycle_checks,
        "primary_client_terminal": primary_terminal_checks,
        "helper_sha256": HELPER_SHA256,
        "helper_dependency_sha256": SETUP_SHA256,
        "installer_sha256": INSTALLER_SHA256,
        "provider_requests": 4,
        "typed_history_and_restart_resume": True,
    }
    (artifacts / "ubuntu-installed-golden-result.json").write_text(
        json.dumps(evidence, indent=2) + "\n"
    )


def request_count_from_result(artifacts):
    result_path = artifacts / "native-stream-tool-result.json"
    if not result_path.is_file():
        return 0
    value = json.loads(result_path.read_text())
    requests = value.get("provider_requests")
    return len(requests) if isinstance(requests, list) else 0


def install_and_run(args):
    source_sha, helper_hash, setup_hash, installer_hash = check_source_and_helpers(
        args.expected_source_sha
    )
    checksum = args.checksum.lower()
    if not HEX64.fullmatch(checksum):
        raise ValueError("--checksum must be 64 hexadecimal characters")
    artifacts = pathlib.Path(args.artifact_dir)
    if not artifacts.is_absolute():
        raise ValueError("--artifact-dir must be absolute")
    if artifacts.is_symlink():
        raise ValueError("--artifact-dir must not be a symlink")
    artifacts = artifacts.resolve()
    if artifacts.is_relative_to(ROOT):
        raise ValueError("artifact directory must be outside the repository")
    if artifacts.exists() and any(artifacts.iterdir()):
        raise ValueError("artifact directory must be new or empty")
    artifacts.mkdir(parents=True, exist_ok=True)
    ubuntu_arm64()
    archive = checked_file(args.archive, "archive")
    manifest = checked_file(args.build_json, "build manifest")
    installer = checked_file(args.installer, "installer")
    if installer != (ROOT / "scripts" / "install-oc2.sh").resolve(strict=True):
        raise ValueError("installer must be the pinned scripts/install-oc2.sh from this source")
    build, archive_hash, binary_hash, library_hash = validate_prerequisites(
        archive, checksum, manifest, source_sha
    )

    temp_root = pathlib.Path(tempfile.mkdtemp(prefix="g7-install-", dir=artifacts))
    safe_cleanup = False
    install_bin = temp_root / "install" / "bin"
    install_bin.mkdir(parents=True)
    env = {
        "HOME": str(temp_root / "home"),
        "PATH": BASE_PATH,
        "TMPDIR": str(temp_root),
        "XDG_CONFIG_HOME": str(temp_root / "home" / ".config"),
        "XDG_DATA_HOME": str(temp_root / "home" / ".local" / "share"),
        "XDG_STATE_HOME": str(temp_root / "home" / ".local" / "state"),
        "XDG_CACHE_HOME": str(temp_root / "home" / ".cache"),
    }
    for key in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
        pathlib.Path(env[key]).mkdir(parents=True, exist_ok=True)
    command = [
        "/bin/sh",
        str(installer),
        "--archive",
        str(archive),
        "--checksum",
        checksum,
        "--install-dir",
        str(install_bin),
    ]
    try:
        installed = subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            capture_output=True,
            timeout=60,
            check=False,
        )
        if installed.returncode != 0:
            output = (installed.stdout + installed.stderr)[:8192].decode(
                "utf-8", "replace"
            )
            raise RuntimeError(
                f"Ubuntu archive installer failed ({installed.returncode}): {output}"
            )
        binary = install_bin / "oc2"
        library = install_bin.parent / "lib" / "libopentui.so"
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise RuntimeError("installer did not produce executable install/bin/oc2")
        if not library.is_file():
            raise RuntimeError("installer did not produce install/lib/libopentui.so")
        if sha256(binary) != binary_hash or sha256(library) != library_hash:
            raise RuntimeError("installed payload hashes differ from build.json")
        if len(os.fsencode(temp_root / "data" / "runtime" / "opencode-rk.sock")) > SOCKET_LIMIT:
            raise RuntimeError("disposable daemon socket path exceeds 100 bytes")
        installed_manifest = artifacts / "build.json"
        shutil.copyfile(manifest, installed_manifest)
        try:
            result = subprocess.run(
                [str(binary), "--version"],
                cwd=temp_root,
                env=env,
                capture_output=True,
                timeout=10,
                check=False,
            )
        except subprocess.TimeoutExpired:
            safe_cleanup = False
            raise
        if result.returncode != 0 or len(result.stdout) + len(result.stderr) > 8192:
            raise RuntimeError("installed oc2 --version failed or exceeded output bound")
        version = (result.stdout + result.stderr).decode("utf-8", "replace").strip()
        if "oc2" not in version or "opencode-rk" in version:
            raise RuntimeError("installed binary identity is not oc2")
        run_golden(binary, library, installed_manifest, artifacts)
        record = {
            "phase": "success",
            "source_sha": source_sha,
            "archive_sha256": archive_hash,
            "binary_sha256": sha256(binary),
            "native_library_sha256": sha256(library),
            "target": TARGET,
            "profile": "release",
            "version": version,
            "test_sha256": sha256(pathlib.Path(__file__).resolve()),
            "native_helper_sha256": helper_hash,
            "native_helper_dependency_sha256": setup_hash,
            "installer_sha256": installer_hash,
        }
        (artifacts / "ubuntu-install-metadata.json").write_text(
            json.dumps(record, indent=2) + "\n"
        )
        shutil.rmtree(temp_root)
    except subprocess.TimeoutExpired:
        safe_cleanup = False
        write_failure_evidence(artifacts, "installer or installed binary timed out; preserve fixture runtime")
        print(f"preserved timed-out fixture root: {temp_root}", file=sys.stderr)
        raise
    except BaseException:
        g7_failure = artifacts / "ubuntu-installed-golden-failure.json"
        failure = artifacts / "native-stream-tool-failure.json"
        if g7_failure.is_file():
            try:
                safe_cleanup = safe_cleanup and (
                    json.loads(g7_failure.read_text()).get("owned_process_cleanup") is True
                )
            except (OSError, ValueError):
                safe_cleanup = False
        if not safe_cleanup:
            write_failure_evidence(
                artifacts,
                "owned child or daemon cleanup was not proven; preserve fixture runtime",
            )
        elif failure.is_file():
            try:
                safe_cleanup = safe_cleanup and (
                    json.loads(failure.read_text()).get("owned_process_cleanup") is True
                )
            except (OSError, ValueError):
                safe_cleanup = False
        if safe_cleanup:
            shutil.rmtree(temp_root, ignore_errors=True)
        else:
            print(f"preserved failed fixture root: {temp_root}", file=sys.stderr)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True)
    parser.add_argument("--checksum", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--expected-source-sha", required=True)
    parser.add_argument("--installer", default=str(ROOT / "scripts" / "install-oc2.sh"))
    args = parser.parse_args()
    install_and_run(args)


if __name__ == "__main__":
    main()

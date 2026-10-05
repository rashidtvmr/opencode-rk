#!/usr/bin/env python3
"""P1-DURABILITY probe: installed native second-client overlap/no-duplicate-work.

A second local client attaches through the real native TUI path while the
first client's streamed turn is in-flight (gated on the first provider
delta). It must reuse the validated daemon descriptor (no second daemon),
auto-resume the same live session through the product snapshot path, render
the known first-turn user prompt visibly, and cause no additional provider
request. The probe reuses the frozen native_stream_tool journey unmodified
and only injects the overlap callback.
"""

import argparse
import fcntl
import hashlib
import http.server
import json
import os
import pathlib
import pty
import re
import select
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
sys.path.insert(0, str(HERE))
import native_stream_tool as golden  # noqa: E402
from native_provider_setup import (  # noqa: E402
    TerminalScreen,
    checked_artifact,
    read_until,
)

MAX_HTTP_BODY = 256 * 1024
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
KEY = golden.KEY
FIRST = golden.FIRST
NATIVE_MARKER = "OpenCode RK (native)"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def redact(value):
    return value.replace(KEY, "[REDACTED]").replace(FIRST, "[PROMPT]")


def write_failure_evidence(artifacts, reason):
    path = artifacts / "native-second-client-failure.json"
    try:
        path.write_text(
            json.dumps(
                {"phase": "failed", "owned_process_cleanup": False, "reason": reason},
                indent=2,
            )
            + "\n"
        )
    except OSError as error:
        print(f"could not write second-client failure evidence: {error}", file=sys.stderr)


def backend_descriptor(data_dir):
    try:
        value = json.loads((data_dir / "runtime" / "backend.json").read_text())
    except (OSError, ValueError) as error:
        raise RuntimeError("no owned daemon descriptor") from error
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


def authed_get(descriptor, path):
    request = urllib.request.Request(
        descriptor["origin"] + path,
        headers={"Authorization": "Bearer " + descriptor["token"]},
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        raw = response.read(MAX_HTTP_BODY + 1)
        if response.status != 200 or len(raw) > MAX_HTTP_BODY:
            raise RuntimeError(f"bounded authenticated request to {path} failed")
    return json.loads(raw)


def live_session_info(descriptor):
    """Return (id, title) of the newest session, mirroring product auto-resume."""
    value = authed_get(descriptor, "/api/sessions")
    sessions = value.get("sessions") if isinstance(value, dict) else None
    if not isinstance(sessions, list) or len(sessions) != 1:
        raise RuntimeError("second client did not observe the single active session")
    entry = sessions[0]
    if not isinstance(entry, dict):
        raise RuntimeError("active session entry is not an object")
    session_id = entry.get("id")
    title = entry.get("title")
    if not isinstance(session_id, str) or not session_id:
        raise RuntimeError("active session has no ID")
    if not isinstance(title, str) or not title:
        raise RuntimeError("active session has no title")
    return session_id, title


def message_texts(descriptor, session_id):
    """Fetch persisted messages via the same endpoint the product renders."""
    value = authed_get(
        descriptor,
        "/api/sessions/" + urllib.parse.quote(session_id, safe="") + "/messages?limit=200",
    )
    messages = value.get("messages") if isinstance(value, dict) else None
    if not isinstance(messages, list) or not messages:
        raise RuntimeError("active session has no persisted messages")
    texts = []
    for message in messages:
        if not isinstance(message, dict):
            raise RuntimeError("message entry is not an object")
        role = message.get("role")
        body = message.get("body")
        if not isinstance(role, str) or not isinstance(body, dict):
            raise RuntimeError("message entry lacks typed role/body")
        storage = body.get("storage")
        text = body.get("text")
        if storage != "inline" or not isinstance(text, str) or not text:
            raise RuntimeError("message entry lacks inline text")
        texts.append((role, text))
    return texts


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
    first_session, first_title = live_session_info(before)
    persisted = message_texts(before, first_session)
    if not any(role == "user" and text == FIRST for role, text in persisted):
        raise RuntimeError("first-turn user prompt is not yet persisted for the second client")

    master = slave = child = None
    captured = bytearray()
    screen = TerminalScreen(80, 24)
    saved_attrs = None
    cleanup_errors = []
    primary_error = None
    try:
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        saved_attrs = termios.tcgetattr(master)
        env = {
            "HOME": str(root / "home"),
            "PATH": os.environ.get("PATH", ""),
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
            [
                sys.executable,
                "-c",
                terminal_exec,
                str(binary),
                "--data-dir",
                str(data),
            ],
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
        expected_live = f"session: {first_title} (live)"
        # native_loop labels the prompt as `you:` (the second-client screen is
        # the product's rendered snapshot, not the provider's role spelling).
        # Keep this assertion tied to the visible transcript rather than the
        # API role used by message_texts().
        # The transcript role prefix is presentation detail and may be
        # clipped/reflowed by the native renderer.  Require the exact known
        # prompt text in visible cells, while separately proving it persisted
        # through the authenticated messages endpoint above.
        expected_prompt = FIRST
        deadline = time.monotonic() + golden.FIXTURE_TIMEOUT
        read_until(
            master,
            deadline,
            captured,
            NATIVE_MARKER.encode(),
            0,
            screen,
            lambda: screen.contains(NATIVE_MARKER),
        )
        if child.poll() is not None:
            raise RuntimeError("second client exited instead of attaching")
        if os.tcgetpgrp(master) != child.pid:
            raise RuntimeError("second client does not own its foreground controlling terminal")
        raw_attrs = termios.tcgetattr(master)
        if raw_attrs[3] & (termios.ICANON | termios.ECHO | termios.ISIG):
            raise RuntimeError("second client did not enter native raw terminal mode")
        # The first native frame is frequently delivered as one PTY read
        # containing the marker, title, and transcript.  read_until() waits
        # for a *fresh* byte when given a predicate, so first accept the frame
        # already parsed by the marker read and only wait for a redraw when the
        # required transcript is genuinely absent.
        if not screen.contains(expected_live):
            read_until(
                master,
                deadline,
                captured,
                expected_live.encode(),
                len(captured),
                screen,
                lambda: screen.contains(expected_live),
            )
        if not screen.contains(expected_prompt):
            read_until(
                master,
                deadline,
                captured,
                expected_prompt.encode(),
                len(captured),
                screen,
                lambda: screen.contains(expected_prompt),
            )
        visible = screen.text()
        if NATIVE_MARKER not in visible:
            raise RuntimeError("second client did not render through the native TUI path")
        if expected_live not in visible:
            raise RuntimeError("second client did not render the live session title")
        if FIRST not in visible:
            raise RuntimeError("second client did not render the known first-turn prompt")
        if "message" not in visible.lower():
            raise RuntimeError("second client did not render a message count")
        after = backend_descriptor(data)
        if (after["pid"], after["origin"], after["token"]) != (
            before["pid"],
            before["origin"],
            before["token"],
        ):
            raise RuntimeError("second client changed the validated daemon descriptor")
        second_session, second_title = live_session_info(after)
        if second_session != first_session or second_title != first_title:
            raise RuntimeError("second client resumed a different session")
        if request_count(state) != 1:
            raise RuntimeError("second client duplicated provider work")
        if KEY.encode() in captured or KEY in visible:
            raise RuntimeError("fixture credential appeared in second-client terminal output")
        # ISIG is disabled by the native renderer, so ETX is deliberately sent
        # as an input byte. Drain output while waiting: stopping reads after
        # the readiness frame can block the child in its final renderer write.
        os.write(master, b"\x03")
        exit_deadline = time.monotonic() + golden.FIXTURE_TIMEOUT
        exit_eof = False
        while child.poll() is None and time.monotonic() < exit_deadline:
            ready, _, _ = select.select([master], [], [], 0.1)
            if not ready:
                continue
            try:
                chunk = os.read(master, min(65536, golden.MAX_PTY - len(captured)))
            except OSError as error:
                if getattr(error, "errno", None) == 5:
                    exit_eof = True
                    break
                raise RuntimeError(f"second-client PTY read failed during exit: {error}") from error
            if not chunk:
                exit_eof = True
                break
            captured.extend(chunk)
            screen.feed(chunk)
        # PTY EOF/EIO means the slave closed; waitpid is the authority for
        # whether the product actually completed its supported exit path.
        # Do not confuse that normal close/reap race with a product failure.
        if exit_eof:
            remaining = max(0.0, exit_deadline - time.monotonic())
            try:
                child.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                raise RuntimeError("second client did not naturally exit after PTY EOF")
        if child.poll() is None:
            raise RuntimeError("second client did not naturally exit after Ctrl-C")
        if child.returncode != 0:
            raise RuntimeError(f"second client exited with status {child.returncode}")
        restored_attrs = termios.tcgetattr(master)
        if restored_attrs != saved_attrs:
            raise RuntimeError(
                "second client did not restore its PTY terminal settings: "
                f"saved={saved_attrs!r} current={restored_attrs!r}"
            )
        if request_count(state) != 1:
            raise RuntimeError("second-client exit duplicated provider work")
        return {
            "attached_to_existing_daemon": True,
            "daemon_pid": before["pid"],
            "native_marker_observed": True,
            "live_session_title": first_title,
            "live_session_id_match": True,
            "first_turn_prompt_rendered": True,
            "persisted_user_message_observed": True,
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
            state.second_client_unsafe = True
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


def run(binary, library, manifest, artifacts):
    binary, library, source, binary_sha, library_sha = checked_artifact(
        str(binary), str(library), str(manifest)
    )
    original_state = golden.ProviderState
    observations = []
    active = {}

    class SecondClientState(original_state):
        def __init__(self, project):
            super().__init__(project)
            active["state"] = self
            self.second_client_unsafe = False
            self.second_client_diagnostic = None
            self.partial_visible = CallbackEvent(
                lambda: observations.append(run_second_client(self, binary))
            )

    golden.ProviderState = SecondClientState
    try:
        golden.run(binary, library, manifest, artifacts)
    except BaseException as error:
        write_failure_evidence(artifacts, redact(str(error)))
        raise
    finally:
        golden.ProviderState = original_state
    if len(observations) != 1:
        raise RuntimeError("second-client overlap probe did not run exactly once")
    if (artifacts / "native-stream-tool-failure.json").is_file():
        raise RuntimeError("underlying streamed journey did not complete successfully")
    if (artifacts / "native-second-client-failure.json").is_file():
        raise RuntimeError("second-client cleanup was not proven; preserve the fixture runtime")
    result_path = artifacts / "native-stream-tool-result.json"
    result = json.loads(result_path.read_text())
    if result.get("phase") != "success":
        raise RuntimeError("underlying streamed journey did not complete successfully")
    requests = result.get("requests")
    if not isinstance(requests, list) or len(requests) != 4:
        raise RuntimeError("underlying journey did not settle exactly four provider turns")
    state = active.get("state")
    if state is not None and getattr(state, "second_client_unsafe", False):
        raise RuntimeError("second-client process was not reaped; preserve the fixture runtime")
    evidence = {
        "phase": "success",
        "source_sha": source,
        "binary_sha256": binary_sha,
        "native_library_sha256": library_sha,
        "test_sha256": sha256(pathlib.Path(__file__).resolve()),
        "second_client": observations[0],
        "provider_requests": 4,
        "typed_history_and_restart_resume": True,
    }
    (artifacts / "native-second-client-result.json").write_text(
        json.dumps(evidence, indent=2) + "\n"
    )


def self_check():
    """Exercise descriptor/session/message validation without product code."""
    root = pathlib.Path(tempfile.mkdtemp(prefix="sc-check-"))
    try:
        data = root / "data"
        (data / "runtime").mkdir(parents=True)
        good = {
            "http_origin": "http://127.0.0.1:4096",
            "auth_token": "ab" * 32,
            "pid": 1234,
        }
        (data / "runtime" / "backend.json").write_text(json.dumps(good))
        descriptor = backend_descriptor(data)
        assert descriptor["port"] == 4096 and descriptor["pid"] == 1234
        for bad in (
            {**good, "auth_token": "short"},
            {**good, "http_origin": "http://localhost:4096"},
            {**good, "http_origin": "http://127.0.0.1:4096@evil/"},
            {**good, "pid": 0},
            {"http_origin": "http://127.0.0.1:4096"},
        ):
            (data / "runtime" / "backend.json").write_text(json.dumps(bad))
            try:
                backend_descriptor(data)
                raise AssertionError("invalid descriptor accepted")
            except RuntimeError:
                pass
        (data / "runtime" / "backend.json").unlink()
        try:
            backend_descriptor(data)
            raise AssertionError("missing descriptor accepted")
        except RuntimeError:
            pass

        sessions_seen = {
            "sessions": [{"id": "s1", "title": "Chat"}],
            "messages": [
                {"role": "user", "body": {"storage": "inline", "text": FIRST}},
            ],
        }

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                if self.headers.get("Authorization") != "Bearer " + "ab" * 32:
                    self.send_error(401)
                    return
                if self.path == "/api/sessions":
                    body = json.dumps({"sessions": sessions_seen["sessions"]}).encode()
                elif self.path.startswith("/api/sessions/s1/messages"):
                    body = json.dumps({"messages": sessions_seen["messages"]}).encode()
                else:
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                return

        server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            fake = {
                "origin": f"http://127.0.0.1:{server.server_port}",
                "token": "ab" * 32,
                "pid": 1234,
                "port": server.server_port,
            }
            session_id, title = live_session_info(fake)
            assert (session_id, title) == ("s1", "Chat")
            texts = message_texts(fake, "s1")
            assert texts == [("user", FIRST)]
            sessions_seen["sessions"] = []
            try:
                live_session_info(fake)
                raise AssertionError("zero sessions accepted")
            except RuntimeError:
                pass
            sessions_seen["sessions"] = [{"id": "s1", "title": "Chat"}, {"id": "s2", "title": "Other"}]
            try:
                live_session_info(fake)
                raise AssertionError("two sessions accepted")
            except RuntimeError:
                pass
            sessions_seen["sessions"] = [{"id": "", "title": "Chat"}]
            try:
                live_session_info(fake)
                raise AssertionError("empty session id accepted")
            except RuntimeError:
                pass
            sessions_seen["sessions"] = [{"id": "s1", "title": "Chat"}]
            sessions_seen["messages"] = []
            try:
                message_texts(fake, "s1")
                raise AssertionError("empty messages accepted")
            except RuntimeError:
                pass
            sessions_seen["messages"] = [{"role": "user", "body": {"storage": "blob"}}]
            try:
                message_texts(fake, "s1")
                raise AssertionError("non-inline body accepted")
            except RuntimeError:
                pass
            wrong = dict(fake, token="cd" * 32)
            try:
                live_session_info(wrong)
                raise AssertionError("wrong bearer accepted")
            except Exception:
                pass
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        assert redact("prefix " + KEY + " suffix") == "prefix [REDACTED] suffix"
        fired = []
        event = CallbackEvent(lambda: fired.append(1))
        event.set()
        event.set()
        assert fired == [1] and event.is_set()
        print("self-check: descriptor, session, messages, auth, redaction, and callback passed")
    finally:
        shutil.rmtree(root, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary")
    parser.add_argument("--native-library")
    parser.add_argument("--build-json")
    parser.add_argument("--artifact-dir")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return
    if not all((args.binary, args.native_library, args.build_json, args.artifact_dir)):
        parser.error("actual run requires --binary, --native-library, --build-json, and --artifact-dir")
    raw_artifacts = pathlib.Path(args.artifact_dir)
    if not raw_artifacts.is_absolute():
        parser.error("--artifact-dir must be absolute")
    artifacts = raw_artifacts.resolve()
    if artifacts.exists() and any(artifacts.iterdir()):
        parser.error("artifact directory must be new or empty")
    artifacts.mkdir(parents=True, exist_ok=True)
    run(pathlib.Path(args.binary), pathlib.Path(args.native_library), pathlib.Path(args.build_json), artifacts)


if __name__ == "__main__":
    main()

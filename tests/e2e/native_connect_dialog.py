#!/usr/bin/env python3
"""Installed native PTY contract for the provider connection dialog.

Uses a disposable authenticated daemon, a loopback provider sink, and the
bounded ANSI screen/artifact helpers from ``native_empty_session.py``. The
journey deliberately cancels before submitting the fake key, so it proves the
rendered title/masking/cancel behavior without persisting credentials or
contacting a provider.
"""

import argparse
import hashlib
import http.server
import json
import os
import pathlib
import re
import shutil
import sys
import tempfile
import threading

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_empty_session import (  # noqa: E402
    HOST_LIBRARY_SUFFIX,
    KEY,
    Journey,
    api,
    checked_artifact,
    free_loopback_port,
    get_sessions,
    make_environment,
    sha256,
    wait_frame,
)

DEADLINE = 20
EXPECTED_API_TITLE = "API key"
EXPECTED_MODEL_NAME = "readiness fixture"
EXPECTED_PROVIDER_NAME = "OpenAI"
UNSENT_DRAFT = "keep this draft"


class ProviderSink(http.server.BaseHTTPRequestHandler):
    """A fail-closed loopback endpoint; any provider HTTP request is a failure."""

    def _reject(self):
        self.server.requests += 1
        self.server.error = "unexpected outbound provider request: " + self.command
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if 0 <= length <= 128 * 1024:
            self.rfile.read(length)
        self.send_error(503, "provider traffic is forbidden in this test")

    do_GET = _reject
    do_POST = _reject
    do_PUT = _reject
    do_PATCH = _reject
    do_DELETE = _reject
    do_OPTIONS = _reject

    def log_message(self, *_):
        pass


def sha_bytes(data):
    return hashlib.sha256(data).hexdigest()


def visible_lines(screen):
    return [row.strip() for row in screen.text().splitlines()]


def has_exact_line(screen, expected):
    return expected in visible_lines(screen)


def send_and_wait(journey, data, predicate, name):
    start = len(journey.captured)
    start_frame = journey.screen.frame_counter
    os.write(journey.master, data)
    try:
        wait_frame(
            journey.master,
            journey.child,
            journey.screen,
            journey.captured,
            predicate,
            start,
            timeout=DEADLINE,
            start_frame=start_frame,
        )
    except AssertionError as error:
        # A bounded, credential-safe snapshot makes a missing expected
        # transition actionable without recording raw PTY bytes.
        visible = journey.screen.text().replace(KEY, "[REDACTED]")[:4096]
        raise AssertionError("%s: %s; visible screen=%r" % (name, error, visible)) from error
    journey.check(name, True)


def run_journey(binary, provider, root, assertions):
    port = free_loopback_port()
    journey = Journey(binary, root, provider, all_assertions=assertions,
                      label="provider-connect-cancel")
    journey.env, journey.data = make_environment(root, port, provider.server_port)
    auth_path = journey.data / "opencode" / "auth.json"
    catalogue_path = journey.data / "catalog" / "models.dev.api.json"
    # The shared empty-session fixture uses a generic provider name. For this
    # targeted journey, give the same OpenAI catalogue entry its real display
    # name while preserving the tested selected model ID/name and fake auth.
    try:
        catalogue = json.loads(catalogue_path.read_text(encoding="utf-8"))
        openai = catalogue.get("openai", {})
        if openai.get("models", {}).get("gpt-5.6", {}).get("name") != EXPECTED_MODEL_NAME:
            raise AssertionError("connect journey catalogue model fixture changed")
        openai["name"] = EXPECTED_PROVIDER_NAME
        catalogue_path.write_text(json.dumps(catalogue), encoding="utf-8")
        journey.start()
        journey.await_initial(expect_home=True)
        # await_initial has already accepted a fresh settled frame. Home's
        # composer is the visible placeholder, not the session composer `>`.
        home_placeholder_visible = any(re.fullmatch(
            r'Ask anything… "(?:Fix a TODO in the codebase|'
            r'What is the tech stack of this project\?|Fix broken tests)"', line
        ) for line in visible_lines(journey.screen))
        if not home_placeholder_visible:
            safe_screen = journey.screen.text().replace(KEY, "[REDACTED]")[:4096]
            raise AssertionError("initial Home placeholder absent from fresh settled frame; visible screen=%r" % safe_screen)
        journey.check("initial-home-placeholder-visible", True)

        origin = journey.daemon_descriptor["http_origin"]
        token = journey.daemon_descriptor["auth_token"]
        sessions = get_sessions(origin, token)
        journey.check("initial-session-is-empty", len(sessions) == 1 and api(
            origin, token, "/api/sessions/%s/messages" % sessions[0]["id"]
        ) == {"messages": []})

        # This fixture is intentionally seeded with a disposable fake key so
        # /connect is available from the ordinary Home composer. Keep its
        # exact bytes/mode as the no-write baseline for cancellation.
        original_auth = auth_path.read_bytes()
        original_auth_mode = auth_path.stat().st_mode & 0o777
        journey.check("disposable-auth-fixture-is-private",
                      original_auth_mode == 0o600 and json.loads(original_auth).get(
                          "openai", {}).get("key") == KEY)

        send_and_wait(
            journey,
            UNSENT_DRAFT.encode("ascii"),
            lambda screen: has_exact_line(screen, UNSENT_DRAFT),
            "ascii-unsent-composer-draft-visible",
        )
        send_and_wait(
            journey,
            b"\x0b",
            lambda screen: has_exact_line(screen, "== Commands =="),
            "local-command-page-opens-with-draft-preserved",
        )
        send_and_wait(
            journey,
            b"/connect\r",
            lambda screen: has_exact_line(screen, "Connect a provider")
            and has_exact_line(screen, "OpenAI (API key)"),
            "commands-route-opens-provider-picker",
        )
        journey.check("draft-preserved-under-provider-picker",
                      has_exact_line(journey.screen, UNSENT_DRAFT))

        send_and_wait(
            journey,
            b"openai\r",
            lambda screen: has_exact_line(screen, EXPECTED_API_TITLE)
            or has_exact_line(screen, "API key (OpenAI)"),
            "api-key-method-dialog-is-visible",
        )
        # The wrong current title is not mistaken for the placeholder or
        # accepted as the pinned API method label.
        journey.check("api-method-title-is-exact-pinned-label",
                      has_exact_line(journey.screen, EXPECTED_API_TITLE))

        # Under the modal, verify the existing Home composer status row is
        # derived from the selected fixture catalogue entry. This later parity
        # assertion intentionally follows the API-heading RED above.
        status_suffix = EXPECTED_MODEL_NAME + "  " + EXPECTED_PROVIDER_NAME
        status_rows = [line for line in visible_lines(journey.screen)
                       if line.endswith(status_suffix)]
        journey.check("composer-status-shows-catalogue-model-and-provider-names",
                      len(status_rows) == 1 and "openai/gpt-5.6" not in status_rows[0])

        # Check display-name parity only after the primary title assertion, so
        # a RED candidate identifies the first independently observed gap.
        # This is the current settled frame: it contains the native Home
        # composer status behind the API-key overlay.
        initial_status_rows = [
            line for line in visible_lines(journey.screen)
            if line.endswith(EXPECTED_MODEL_NAME + "  " + EXPECTED_PROVIDER_NAME)
        ]
        journey.check("composer-status-shows-catalogue-model-and-provider-names",
                      len(initial_status_rows) == 1
                      and "openai/gpt-5.6" not in initial_status_rows[0])

        send_and_wait(
            journey,
            KEY.encode("utf-8"),
            lambda screen: any(re.fullmatch(
                r"key: \*{%d} \(enter / escape\)" % len(KEY), line
            ) for line in visible_lines(screen)),
            "fake-key-renders-as-mask",
        )
        journey.check("fake-key-absent-from-visible-cells",
                      KEY not in journey.screen.text())
        journey.check("fake-key-absent-from-raw-pty-output",
                      KEY.encode("utf-8") not in bytes(journey.captured))

        send_and_wait(
            journey,
            b"\x1b",
            lambda screen: has_exact_line(screen, UNSENT_DRAFT)
            and not has_exact_line(screen, "Connect a provider")
            and not has_exact_line(screen, EXPECTED_API_TITLE),
            "escape-restores-unsent-draft-to-home-composer",
        )
        journey.check("cancelled-key-not-visible-after-return",
                      KEY not in journey.screen.text())

        current_auth = auth_path.read_bytes()
        journey.check("cancel-does-not-update-auth-file",
                      current_auth == original_auth
                      and sha_bytes(current_auth) == sha_bytes(original_auth)
                      and (auth_path.stat().st_mode & 0o777) == original_auth_mode)
        end_sessions = get_sessions(origin, token)
        journey.check("cancel-does-not-add-session-history",
                      len(end_sessions) == 1 and api(
                          origin, token,
                          "/api/sessions/%s/messages" % sessions[0]["id"]
                      ) == {"messages": []})
        journey.check("connect-cancel-does-not-contact-provider",
                      provider.requests == 0 and provider.error is None)

        # Exercise Ctrl-D explicitly; Journey.stop() then independently checks
        # terminal restoration and owned-daemon/process-group cleanup.
        os.write(journey.master, b"\x04")
        journey._wait_for_exit_draining_pty(8)
        journey.check("ctrl-d-exits-normally", journey.child.returncode == 0)
    finally:
        cleanup_error = None
        data_error = None
        try:
            journey.stop()
        except BaseException as error:
            cleanup_error = error
        finally:
            try:
                shutil.rmtree(journey.data, ignore_errors=False)
            except BaseException as error:
                data_error = error
        if cleanup_error is not None:
            raise cleanup_error
        if data_error is not None:
            raise data_error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--native-library", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--output", required=True,
                        help="new output directory for exact-SHA receipt")
    args = parser.parse_args()
    output = pathlib.Path(args.output)
    if not output.is_absolute():
        parser.error("--output must be absolute")
    if output.exists() and any(output.iterdir()):
        parser.error("--output must be new or empty")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", args.candidate_sha):
        parser.error("candidate SHA must be 40 hex characters")
    output.mkdir(parents=True, exist_ok=True)
    binary, library, source_sha, binary_sha, library_sha = checked_artifact(
        args.binary, args.native_library, args.build_json)
    if source_sha.lower() != args.candidate_sha.lower():
        parser.error("--candidate-sha does not match build attestation source SHA")

    stage = pathlib.Path(tempfile.mkdtemp(
        prefix="oc2-connect-installed-",
        dir=os.environ.get("TMPDIR", tempfile.gettempdir()),
    ))
    provider = http.server.ThreadingHTTPServer(("127.0.0.1", 0), ProviderSink)
    provider.requests = 0
    provider.error = None
    provider.daemon_threads = True
    thread = threading.Thread(target=provider.serve_forever, daemon=True)
    thread.start()
    assertions = []
    result = {
        "status": "FAIL",
        "candidate_sha": args.candidate_sha,
        "build_json_sha256": sha256(pathlib.Path(args.build_json)),
        "binary_sha256": binary_sha,
        "native_library_sha256": library_sha,
        "test_sha256": sha256(pathlib.Path(__file__)),
        "assertions": assertions,
        "provider_requests": 0,
        "reason": None,
    }
    failure = None
    installed_bin = installed_lib = None
    try:
        install = stage / "install"
        (install / "bin").mkdir(parents=True)
        (install / "lib").mkdir()
        installed_bin = install / "bin" / "oc2"
        installed_lib = install / "lib" / ("libopentui" + HOST_LIBRARY_SUFFIX)
        shutil.copyfile(binary, installed_bin)
        shutil.copyfile(library, installed_lib)
        os.chmod(installed_bin, 0o755)
        if sha256(installed_bin) != binary_sha or sha256(installed_lib) != library_sha:
            raise RuntimeError("installed copy digest differs from build attestation")
        journeys_root = stage / "fixtures"
        journeys_root.mkdir(mode=0o700)
        run_journey(installed_bin, provider, journeys_root / "connect", assertions)
        if provider.requests or provider.error:
            raise AssertionError(provider.error or "provider received forbidden request")
        result["status"] = "PASS"
    except BaseException as error:
        failure = error
        # Never put captured PTY bytes, fixture auth, or a key into the receipt.
        result["reason"] = str(error)[:2000]
    finally:
        result["assertions"] = assertions
        result["provider_requests"] = provider.requests
        try:
            provider.shutdown()
            provider.server_close()
            thread.join(timeout=3)
            if thread.is_alive():
                raise RuntimeError("provider fixture thread failed bounded shutdown")
        except BaseException as error:
            result["status"] = "FAIL"
            result["reason"] = (result["reason"] + "; " if result["reason"] else "") + "cleanup: " + str(error)
        result["installed_binary_sha256"] = (
            sha256(installed_bin) if installed_bin is not None and installed_bin.exists() else None)
        result["installed_native_library_sha256"] = (
            sha256(installed_lib) if installed_lib is not None and installed_lib.exists() else None)
        result["fixture_root"] = str(stage)
        receipt = output / "native-connect-dialog-result.json"
        receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"status": result["status"],
                          "candidate_sha": args.candidate_sha,
                          "receipt": str(receipt), "reason": result["reason"]}))
        shutil.rmtree(stage, ignore_errors=False)
    if failure is not None or result["status"] != "PASS":
        raise SystemExit(1)


if __name__ == "__main__":
    main()

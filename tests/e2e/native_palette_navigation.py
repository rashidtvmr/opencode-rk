#!/usr/bin/env python3
"""Installed native regression for command-palette arrow navigation.

Runs one bounded PTY journey in a disposable authenticated daemon fixture.
The palette's selected route is identified from visible cells (``> /route``),
not terminal escape bytes or a symmetric color difference. The existing
native_empty_session fixture provides the completed-frame observer, isolated
environment, artifact attestation, daemon ownership checks, and cleanup.
"""

import argparse
import hashlib
import http.server
import importlib.util
import json
import os
import pathlib
import re
import shutil
import sys
import tempfile
import threading


SOURCE_TEST = pathlib.Path(__file__).with_name("native_empty_session.py")
HEX40 = re.compile(r"^[0-9a-fA-F]{40}$")
DRAFT = "café世界"
MAX_RECEIPT_REASON = 1600


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            h.update(block)
    return h.hexdigest()


def load_module(name, path):
    if not path.is_file():
        raise RuntimeError("required native PTY fixture is unavailable: " + path.name)
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load native PTY fixture: " + path.name)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def unicode_screen_type(fixture, paste_screen):
    """Use bounded synchronized frames plus established OpenTUI cell widths."""
    class UnicodeScreen(fixture.Screen):
        CONTINUATION = paste_screen.CONTINUATION
        cols, rows = fixture.COLS, fixture.ROWS
        _width = staticmethod(paste_screen._width)
        put = paste_screen._put
        text = paste_screen.text

    return UnicodeScreen


def selected_route(screen, selected, other):
    """Require `> ` on exactly the selected route and no marker on the other."""
    def rows_for(route):
        matches = []
        for row in screen.cells:
            visible = "".join(row).rstrip().lstrip()
            if visible.startswith("> " + route):
                suffix = visible[2 + len(route):]
                if not suffix or suffix[0].isspace():
                    matches.append(True)
            elif visible.startswith(route):
                suffix = visible[len(route):]
                if not suffix or suffix[0].isspace():
                    matches.append(False)
        return matches

    return rows_for(selected) == [True] and rows_for(other) == [False]


def visible_draft_lines(screen):
    return [line for line in screen.text().splitlines() if DRAFT in line]


def home_placeholder(screen):
    # Native Home uses U+2026 (horizontal ellipsis), not three ASCII dots.
    return any("Ask anything\u2026" in "".join(row) for row in screen.cells)


class NoProvider(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.server.requests += 1
        self.server.error = "unexpected provider request during palette-only journey"
        length = int(self.headers.get("Content-Length", "0"))
        if 0 <= length <= 128 * 1024:
            self.rfile.read(length)
        self.send_error(500, "provider requests are forbidden")

    def log_message(self, *_):
        pass


def run(args):
    fixture = load_module("palette_native_empty_fixture", SOURCE_TEST)
    paste = load_module("palette_native_unicode_fixture", SOURCE_TEST.with_name("native_bracketed_paste.py"))
    screen_type = unicode_screen_type(fixture, paste.PasteTerminalScreen)

    for value in (args.binary, args.native_library, args.build_json, args.output):
        if not pathlib.Path(value).is_absolute():
            raise ValueError("binary, native-library, build-json, and output paths must be absolute")
    binary, library, source_sha, binary_sha, library_sha = fixture.checked_artifact(
        args.binary, args.native_library, args.build_json
    )
    if not HEX40.fullmatch(args.candidate_sha):
        raise ValueError("candidate SHA must be exactly 40 hexadecimal characters")
    if source_sha.lower() != args.candidate_sha.lower():
        raise ValueError("candidate SHA does not match build attestation source SHA")
    output = pathlib.Path(args.output)
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise ValueError("output must be new or empty; existing receipts are preserved")
    output.mkdir(parents=True, exist_ok=True, mode=0o700)

    stage = pathlib.Path(tempfile.mkdtemp(prefix="oc2-palette-", dir=os.environ.get("TMPDIR", tempfile.gettempdir())))
    provider = http.server.ThreadingHTTPServer(("127.0.0.1", 0), NoProvider)
    provider.requests = 0
    provider.error = None
    provider.daemon_threads = True
    provider_thread = threading.Thread(target=provider.serve_forever, daemon=True)
    provider_thread.start()
    journey = None
    result = {
        "status": "FAIL",
        "candidate_sha": args.candidate_sha.lower(),
        "source_test": str(SOURCE_TEST),
        "test_sha256": digest(pathlib.Path(__file__)),
        "build_json_sha256": digest(pathlib.Path(args.build_json)),
        "binary_sha256": binary_sha,
        "native_library_sha256": library_sha,
        "provider_requests": 0,
        "assertions": [],
        "failure_stage": None,
        "reason": None,
    }

    def check(name, predicate):
        passed = bool(predicate)
        result["assertions"].append({"name": name, "pass": passed})
        if not passed:
            raise AssertionError("assertion failed: " + name)

    failure = None
    try:
        install = stage / "install"
        (install / "bin").mkdir(parents=True)
        (install / "lib").mkdir()
        installed_binary = install / "bin" / "oc2"
        installed_library = install / "lib" / ("libopentui" + fixture.HOST_LIBRARY_SUFFIX)
        shutil.copyfile(binary, installed_binary)
        shutil.copyfile(library, installed_library)
        os.chmod(installed_binary, 0o755)
        check("installed-binary-hash-matches-attestation", digest(installed_binary) == binary_sha)
        check("installed-native-library-hash-matches-attestation", digest(installed_library) == library_sha)

        root = stage / "fixture"
        daemon_port = fixture.free_loopback_port()
        env, data = fixture.make_environment(root, daemon_port, provider.server_port)
        journey = fixture.Journey(installed_binary, root, provider, label="command-palette")
        journey.screen = screen_type()
        journey.env, journey.data = env, data
        journey.start()

        result["failure_stage"] = "initial-home"
        journey.await_initial(expect_home=True)
        check("explicit-native-tui-home-placeholder-visible", home_placeholder(journey.screen))

        import os as _os
        _os.write(journey.master, DRAFT.encode("utf-8"))
        result["failure_stage"] = "unicode-draft-visible"
        start_frame = journey.screen.frame_counter
        fixture.wait_frame(
            journey.master, journey.child, journey.screen, journey.captured,
            lambda screen: DRAFT in screen.text(), 0, start_frame=start_frame,
        )
        check("unicode-draft-visible-before-palette", DRAFT in journey.screen.text())
        draft_before = visible_draft_lines(journey.screen)
        check("unicode-draft-has-one-visible-composer-line", len(draft_before) == 1)

        start_frame = journey.screen.frame_counter
        _os.write(journey.master, b"\x0b")
        result["failure_stage"] = "ctrl-k-command-palette-sessions-selected"
        fixture.wait_frame(
            journey.master, journey.child, journey.screen, journey.captured,
            lambda screen: "Commands" in screen.text() and
            selected_route(screen, "/sessions", "/models"),
            0, start_frame=start_frame,
        )
        check("ctrl-k-opens-command-palette-with-sessions-selected",
              selected_route(journey.screen, "/sessions", "/models"))

        start_frame = journey.screen.frame_counter
        _os.write(journey.master, b"\x1b[B")
        result["failure_stage"] = "down-selects-models"
        fixture.wait_frame(
            journey.master, journey.child, journey.screen, journey.captured,
            lambda screen: selected_route(screen, "/models", "/sessions"),
            0, start_frame=start_frame,
        )
        check("down-selects-models-entry", selected_route(journey.screen, "/models", "/sessions"))

        start_frame = journey.screen.frame_counter
        _os.write(journey.master, b"\x1b[A")
        result["failure_stage"] = "up-restores-sessions"
        fixture.wait_frame(
            journey.master, journey.child, journey.screen, journey.captured,
            lambda screen: selected_route(screen, "/sessions", "/models"),
            0, start_frame=start_frame,
        )
        check("up-restores-sessions-selection", selected_route(journey.screen, "/sessions", "/models"))

        start_frame = journey.screen.frame_counter
        _os.write(journey.master, b"\x1b")
        result["failure_stage"] = "escape-preserves-draft"
        fixture.wait_frame(
            journey.master, journey.child, journey.screen, journey.captured,
            lambda screen: DRAFT in screen.text() and "Commands" not in screen.text(),
            0, start_frame=start_frame,
        )
        check("escape-closes-palette-and-preserves-exact-unicode-draft",
              visible_draft_lines(journey.screen) == draft_before and
              "Commands" not in journey.screen.text())
        check("palette-journey-made-no-provider-request",
              provider.requests == 0 and provider.error is None)

        _os.write(journey.master, b"\x04")
        result["failure_stage"] = "ctrl-d-terminal-restoration"
        journey._wait_for_exit_draining_pty(8)
        check("ctrl-d-exits-successfully", journey.child.returncode == 0)
        check("terminal-attributes-restored-after-ctrl-d",
              journey.saved_termios is not None and
              fixture.termios.tcgetattr(journey.master) == journey.saved_termios)
        journey.stop()
        journey = None
        result["failure_stage"] = None
        check("provider-still-unused-after-exit", provider.requests == 0 and provider.error is None)
        result["status"] = "PASS"
    except BaseException as error:
        failure = error
        result["reason"] = str(error)[:MAX_RECEIPT_REASON]
        if journey is not None:
            try:
                result["last_visible_grid"] = journey.screen.text().replace(
                    fixture.KEY, "[REDACTED]"
                )[:8192]
            except BaseException:
                result["last_visible_grid"] = "<unavailable>"
    finally:
        if journey is not None:
            try:
                journey.stop()
            except BaseException as cleanup_error:
                result["reason"] = ((result["reason"] + "; ") if result["reason"] else "") + (
                    "journey cleanup: " + str(cleanup_error)[:600]
                )
                result["status"] = "FAIL"
        result["provider_requests"] = provider.requests
        if provider.error:
            result["reason"] = ((result["reason"] + "; ") if result["reason"] else "") + provider.error
            result["status"] = "FAIL"
        try:
            provider.shutdown()
            provider.server_close()
            provider_thread.join(timeout=3)
            if provider_thread.is_alive():
                raise RuntimeError("local no-provider server failed bounded shutdown")
        except BaseException as cleanup_error:
            result["reason"] = ((result["reason"] + "; ") if result["reason"] else "") + (
                "provider cleanup: " + str(cleanup_error)[:600]
            )
            result["status"] = "FAIL"
        result["fixture_root_removed"] = False
        try:
            shutil.rmtree(stage)
            result["fixture_root_removed"] = True
        except BaseException as cleanup_error:
            result["reason"] = ((result["reason"] + "; ") if result["reason"] else "") + (
                "fixture cleanup: " + str(cleanup_error)[:600]
            )
            result["status"] = "FAIL"
        receipt = output / "native-palette-navigation-result.json"
        receipt.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"status": result["status"], "candidate_sha": args.candidate_sha.lower(),
                          "receipt": str(receipt), "reason": result["reason"]}))
    if failure is not None or result["status"] != "PASS":
        raise SystemExit(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--native-library", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--output", required=True, help="new/empty directory for exact-SHA result receipt")
    run(parser.parse_args())


if __name__ == "__main__":
    main()

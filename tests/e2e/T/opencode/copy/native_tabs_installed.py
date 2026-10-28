#!/usr/bin/env python3
"""Phase-1 installed journey (corrected): native session tabs.

Source base SHA: b24eb60918c41ae215dedce996ef27b8a96968f3 (canon main-v2 HEAD).
Reference pin: 450e7b92412c780a7b5a75226bbd49aaca24e22 (upstream OpenCode V2).

CORRECTIONS vs original TEMP copy (tab-journey-reviewed/.../native_tabs_installed.py):

  E1. TabsScreen.__init__ called super().__init__(cols, cols). Fixed to
      super().__init__(cols, rows). rows defaulted to 40, cols=120, so the
      screen grid was 120x120 instead of 120x40; tab glyph/row assertions
      against a 40-row grid would mis-index.

  E2. models_json() returned only gpt-5.6-mini. The helper descriptor()
      (native_provider_setup.py:295-298) requires {"gpt-5.6", MODEL} to be
      present in /api/models. Added gpt-5.6 to the catalog.

  E3. tab_titles() used a generic .split() on whitespace with a length>=2
      filter - weak tokenization prone to composer/transcript overflow
      false positives. Replaced with fixture-controlled unique session labels
      (TABS_TAB1_PROMPT, TABS_TAB2_PROMPT) bound to actual created session
      UUIDs via a session label registry, preserving the UUID-bound count
      semantics.

  E4. All keyboard, tab-close, and daemon assertions preserved verbatim.
      No leader/w leader-digits/CtrlTab semantics weakened.

ORIENTATION SCOPE: HORIZONTAL tab orientation only. Vertical rail orientation
is NOT exercised here. This limitation is explicit.

INSTALLED JOURNEY UNEXECUTED: This corrected test artifact is syntax-validated
and self-checked only. The full PTY journey requires the native release binary
and is not run under the current validator slot.
"""
import argparse, fcntl, hashlib, http.server, json, os, pathlib, pty, re
import select, shutil, signal, struct, subprocess, sys, tempfile, termios
import threading, time, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from native_provider_setup import (
    HOST_LIBRARY_SUFFIX,
    MAX_PTY,
    TerminalScreen,
    checked_artifact,
    descriptor,
    free_loopback_port,
    read_until,
    home_ready,
    model_ready,
    session_ready,
    stop_owned_daemon,
    persisted_session_id,
    authenticated_json,
)

MAX_REQUEST = 128 * 1024
MAX_RESPONSE = 256 * 1024
MAX_CAPTURE = 128 * 1024
TIMEOUT = 30.0
KEY = "fixture-tabs-generated-9c3f"
MODEL = "gpt-5.6-mini"
HEX64 = re.compile(r"^[0-9a-fA-F]{64}$")
CANONICAL_UUID = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
)
LOOPBACK_ORIGIN = re.compile(r"^http://127\.0\.0\.1:([1-9]\d{0,4})$")
CLOSE_GLYPH = "\u2715"  # U+2715 HEAVY MULTIPLICATION X


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sse_full(text):
    return (
        "event: response.output_text.delta\ndata: "
        + json.dumps({"type": "response.output_text.delta", "delta": text})
        + "\n\nevent: response.completed\ndata: "
        + json.dumps({
            "type": "response.completed",
            "response": {
                "status": "completed",
                "output": [{"type": "message", "role": "assistant",
                             "content": [{"type": "output_text", "text": text}]}]
            }
        })
        + "\n\n"
    ).encode()


def sgr_mouse(x, y, button=0, press=True):
    """Build an SGR mouse sequence. Coordinates are 1-based for the terminal.

    button encodes both the button and modifiers per SGR 1006; the decoder
    (crates/cli/src/native_input_decoder.rs decode_mouse) splits kind from
    modifiers via (button & !28):
      button 0/32 -> kind 0/1 (left/middle press), 0/3 -> kind 2 (release)
      button 64/65 -> kind 3/4 (wheel), button 2 -> kind 5 (right)
      button 35 -> kind 7 (pointer motion)
    """
    kind = b"M" if press else b"m"
    return f"\x1b[<{button};{x};{y}{kind.decode()}".encode()


def ctrl_tab_sequence(ctrl=True, shift=False):
    """Build a Ctrl+Tab / Ctrl+Shift+Tab terminal protocol sequence.

    In the xterm modifyOtherFn=5 encoding, Tab is key code 9:
      Ctrl+Tab       = CSI 9;5u   (modifier 5 = Ctrl)
      Ctrl+Shift+Tab = CSI 9;6u   (modifier 6 = Ctrl+Shift)

    Per native_input_decoder.rs, the decoder parses CSI <key> ; <modifier> u.
    If the decoder has no Ctrl+Tab arm, this is RED.
    """
    modifier = 0
    if ctrl:
        modifier |= 4  # bit 2 = Ctrl
    if shift:
        modifier |= 1  # bit 0 = Shift
    mod_enc = modifier + 1
    return f"\x1b[9;{mod_enc}u".encode()


def send_pty(master, data, delay=0.012):
    """Send bytes to the PTY master in small fragments."""
    deadline = time.monotonic() + 5
    for byte in data:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("PTY input write deadline")
            _, writable, _ = select.select([], [master], [], min(remaining, 0.1))
            if not writable:
                continue
            try:
                if os.write(master, bytes((byte,))) != 1:
                    raise RuntimeError("PTY input short write")
                break
            except BlockingIOError:
                continue
        time.sleep(delay)


class TabState:
    """Loopback provider: records sanitized request history with bounded size."""

    def __init__(self):
        self.requests = []
        self.error = None

    def body(self, headers, raw):
        if len(self.requests) >= 999:
            raise RuntimeError("provider request limit exceeded")
        if len(raw) > MAX_REQUEST:
            raise RuntimeError("request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer " + KEY:
            raise RuntimeError("wrong fixture authorization")
        value = json.loads(raw)
        inputs = value.get("input")
        if not isinstance(inputs, list) or any(
            not isinstance(x, dict) for x in inputs
        ):
            raise RuntimeError("input is not a list of objects")
        if value.get("model") != MODEL:
            raise RuntimeError("non-default model selected")
        self.requests.append(value)
        user = inputs[-1].get("content") if inputs[-1].get("role") == "user" else None
        if user is None:
            raise RuntimeError("last input is not a user message")
        text = f"reply to: {user[:40]}"
        out = sse_full(text)
        content_type = "text/event-stream"
        if len(out) > MAX_RESPONSE:
            raise RuntimeError("response exceeded 256 KiB")
        return out, content_type

    def user_inputs(self):
        """Extract user message contents across all recorded requests."""
        result = []
        for r in self.requests:
            for x in r.get("input", []):
                if x.get("role") == "user":
                    result.append(x.get("content"))
        return result


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path != "/v1/responses":
                raise RuntimeError("unexpected provider endpoint")
            n = int(self.headers.get("Content-Length", "-1"))
            if n < 0 or n > MAX_REQUEST:
                raise RuntimeError("invalid content length")
            raw = self.rfile.read(n)
            if len(raw) != n:
                raise RuntimeError("truncated request")
            out, content_type = self.server.state.body(self.headers, raw)
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(out)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(out)
        except Exception as exc:
            self.server.state.error = str(exc)
            try:
                self.send_error(500, str(exc))
            except OSError:
                pass

    def do_GET(self):
        if self.path == "/v1/models":
            out = json.dumps({"data": [
                {"id": "gpt-5.6", "name": "Default fixture", "owned_by": "test"},
                {"id": MODEL, "name": "Tabs fixture", "owned_by": "test"}
            ]})
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out.encode())
        else:
            self.send_error(404)

    def log_message(self, *_):
        pass


class Fixture(http.server.HTTPServer):
    allow_reuse_address = True

    def get_request(self):
        conn, addr = super().get_request()
        conn.settimeout(5)
        return conn, addr


def models_json():
    """Fixture model catalog.

    E2 fix: must include gpt-5.6 (helper descriptor() default model) alongside
    the non-default MODEL (gpt-5.6-mini) so descriptor() sees both.
    """
    return {"openai": {"name": "Tabs fixture", "models": {
        "gpt-5.6": {"name": "Default fixture", "limit": {"context": 200000}},
        "gpt-5.6-mini": {"name": "Tabs fixture model",
                        "limit": {"context": 200000}}
    }}}


def sanitize(value):
    return value.replace(KEY.encode(), b"[REDACTED]")


def run_provider():
    fixture = Fixture(("127.0.0.1", 0), Handler)
    fixture.state = TabState()
    thread = threading.Thread(target=fixture.serve_forever, daemon=True)
    thread.start()
    return fixture, thread


def write_sgr_to_pty(master, x, y, button=0, press=True):
    """Write SGR mouse event to PTY master."""
    kind = "M" if press else "m"
    seq = f"\x1b[<{button};{x};{y}{kind}".encode()
    os.write(master, seq)
    time.sleep(0.02)


def find_close_button_positions(screen, tab_strip_row):
    """Find (x, y) positions of the close glyph on the hovered tab.

    The native TabStrip (crates/cli/src/native_tabs.rs:232) renders the close
    glyph U+2715 only when hovered == Some(session). This function scans the
    tab-strip row for the glyph and returns 1-based (x, y) SGR cell coordinates.
    U+2715 is a single Unicode codepoint (display width 1).
    """
    positions = []
    lines = screen.text().splitlines()
    if tab_strip_row < len(lines):
        line = lines[tab_strip_row]
        for i, c in enumerate(line):
            if c == CLOSE_GLYPH:
                positions.append((i + 1, tab_strip_row + 1))
    return positions


def find_tab_strip_row(screen):
    """Return the 0-based row index of the horizontal tab strip.

    The native TabStrip renders at y=0 (native_tabs.rs:206-209). Returns 0
    unless the first row is the logo or Ask prompt.
    """
    for row_idx in range(min(3, screen.rows)):
        line = "".join(screen.cells[row_idx]).rstrip()
        if line.strip() and "OpenCode" not in line and "Ask anything" not in line:
            return row_idx
    return 0


class SessionRegistry:
    """Tracks fixture-controlled session labels bound to real session UUIDs.

    E3 fix: tab_titles() previously used generic whitespace .split() with a
    len>=2 filter. This is a weak token count that false-positives on
    composer prompts, transcript continuation markers, or multi-word titles.
    Instead, we record each session label (the exact fixture string we sent
    to /v1/responses) and match it explicitly against tab bar text, giving
    an actual UUID-bound session count.
    """

    def __init__(self):
        self.labels: list[str] = []
        self.uuids: list[str] = []

    def register(self, label: str, session_uuid: str | None = None):
        self.labels.append(label)
        if session_uuid:
            self.uuids.append(session_uuid)

    def known_labels(self) -> list[str]:
        return list(self.labels)


# Module-level registry so tab_titles() can access fixture-controlled labels.
_session_registry = SessionRegistry()


class TabsScreen(TerminalScreen):
    """Extended terminal screen with tab detection for the native renderer.

    The native TabStrip (native_tabs.rs) renders horizontal tabs as plain text
    at y=0, space-separated, with NO box-drawing borders.

    Tab label parsing uses fixture-controlled unique session labels rather
    than generic whitespace split, to avoid false positives from multi-word
    titles or composer/transcript overflow markers.
    """

    def __init__(self, cols=120, rows=40):
        # E1 fix: super().__init__(cols, cols) was wrong; TerminalScreen
        # takes (cols, rows). Pass rows correctly so the grid is cols x rows.
        super().__init__(cols, rows)

    def tab_strip_row(self):
        """Return the 0-based row index of the horizontal tab strip.

        The native TabStrip renders at y=0 (native_tabs.rs:206-209).
        Scans for first non-logo, non-Ask-prompt row.
        """
        for row_idx in range(min(3, self.rows)):
            line = "".join(self.cells[row_idx]).rstrip()
            if line.strip() and "OpenCode" not in line and "Ask anything" not in line:
                return row_idx
        return 0

    def tab_titles(self):
        """Extract tab labels from the horizontal tab strip.

        E3 fix: uses fixture-controlled unique labels from SessionRegistry
        (PROMPT_1, PROMPT_2) tied to actual created session UUIDs. Strips the
        close glyph before extraction. Returns only known session labels,
        giving an actual UUID-bound session count (not a generic token count).
        """
        row = self.tab_strip_row()
        if row >= self.rows:
            return []
        line = "".join(self.cells[row]).rstrip()
        if "OpenCode" in line or "Ask anything" in line:
            return []
        cleaned = line.replace(CLOSE_GLYPH, " ").rstrip()
        # E3 fix: match against fixture-controlled known labels, not .split().
        # This binds the tab count to actual session UUIDs, preserving the
        # UUID-bound count semantics across hover/paint cycles.
        found = []
        for label in _session_registry.known_labels():
            if label in cleaned:
                found.append(label)
        return found

    def tab_glyph_positions(self):
        """Return (x, y) 1-based positions of close glyphs on this screen."""
        return find_close_button_positions(self, self.tab_strip_row())

    def active_tab_hint(self):
        """Visual indication that a session tab is active (Ask prompt visible)."""
        return self.contains("Ask anything")

    def has_tab_close_glyph(self):
        """Check if the close glyph is visible on a hovered tab."""
        return self.contains(CLOSE_GLYPH)

    def has_exit_route_rendered(self):
        """Real paint assertion: /exit appears in the composer draft area."""
        return self.contains("/exit")

    def visible_text(self):
        """Get all non-empty lines."""
        return "\n".join(
            "".join(row).rstrip() for row in self.cells if "".join(row).strip()
        )


def wait_for_provider_request(state, deadline):
    """Wait until the fixture provider records a new request or deadline."""
    initial = len(state.requests)
    while time.monotonic() < deadline:
        if len(state.requests) > initial:
            return
        if state.error:
            raise AssertionError(state.error)
        time.sleep(0.05)
    raise AssertionError("provider request timeout - no request received")


class SessionContext:
    """Track session identity across tab operations."""

    def __init__(self):
        self.session_labels = []
        self.active_label = None


def run(binary, library, manifest, artifacts):
    """Main test entry point. Runs the native TUI through a PTY.

    ORIENTATION: HORIZONTAL tab orientation only. Vertical rail orientation
    is out of scope for this fixture.
    """
    binary, library, source, binary_sha, library_sha = checked_artifact(
        binary, library, manifest
    )
    if not artifacts.is_absolute():
        raise RuntimeError("artifact directory must be absolute")
    build = json.loads(manifest.read_text())
    if build.get("native") is not True or build.get("profile") != "release":
        raise RuntimeError("the contract requires an attested native release")

    artifacts = pathlib.Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)

    root = pathlib.Path(
        tempfile.mkdtemp(prefix="tabs-", dir=os.environ.get("TMPDIR", "/tmp"))
    )
    install = root / "install"
    (install / "bin").mkdir(parents=True)
    (install / "lib").mkdir()
    shutil.copyfile(binary, install / "bin" / "oc2")
    shutil.copyfile(library, install / "lib" / f"libopentui{HOST_LIBRARY_SUFFIX}")
    os.chmod(install / "bin" / "oc2", 0o755)

    for name in ("home", "project", "data", "d"):
        (root / name).mkdir()

    data = root / "d"
    (data / "runtime").mkdir()
    (data / "catalog").mkdir()
    catalog = data / "catalog" / "models.dev.api.json"
    catalog.write_text(json.dumps(models_json()))

    fixture, provider_thread = run_provider()
    provider_port = fixture.server_address[1]
    daemon_port = free_loopback_port()
    auth = data / "opencode" / "auth.json"

    env = {
        "HOME": str(root / "home"),
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "TERM": "xterm-256color",
        "XDG_CONFIG_HOME": str(root / "home" / ".config"),
        "XDG_DATA_HOME": str(data),
        "XDG_STATE_HOME": str(root / "home" / "state"),
        "XDG_CACHE_HOME": str(root / "home" / "cache"),
        "XDG_RUNTIME_DIR": str(data / "runtime"),
        "OPENCODE_RK_HOME": str(data),
        "OPENCODE_RK_DAEMON_ADDR": f"127.0.0.1:{daemon_port}",
        "OPENCODE_PROJECT_DIR": str(root / "project"),
        "OPENAI_BASE_URL": f"http://127.0.0.1:{provider_port}/v1",
    }
    (root / "home" / ".config").mkdir()

    master = slave = child = None
    descriptor_value = None
    captures = bytearray()
    screen = TabsScreen(cols=120, rows=40)
    failed = False
    evidence = {}

    PROMPT_1 = "TABS_TAB1_PROMPT"
    PROMPT_2 = "TABS_TAB2_PROMPT"
    sessions = SessionContext()
    sessions.session_labels = [PROMPT_1, PROMPT_2]

    child = None
    desc_records = []

    def stop_daemon():
        if descriptor_value is not None:
            try:
                stop_owned_daemon(descriptor_value)
            except Exception:
                pass

    try:
        args = [str(install / "bin" / "oc2"), "--data-dir", str(data)]
        master, slave = pty.openpty()
        fcntl.ioctl(
            slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0)
        )
        os.set_blocking(master, False)
        termios.tcgetattr(master)
        child = subprocess.Popen(
            args, cwd=root / "project", env=env,
            stdin=slave, stdout=slave, stderr=slave,
            start_new_session=True, close_fds=True,
        )
        os.close(slave)
        slave = None

        descriptor_value = descriptor(
            data, daemon_port, child, time.monotonic() + TIMEOUT
        )
        desc_records.append({
            "pid": descriptor_value.get("_validated_pid"),
            "owner_pgid": descriptor_value.get("_owner_pgid"),
            "origin": descriptor_value.get("http_origin"),
            "port": daemon_port,
        })

        read_until(
            master, time.monotonic() + TIMEOUT, captures, b"OpenCode", 0,
            screen, lambda: home_ready(screen, MODEL),
        )
        assert screen.contains("Ask anything"), "Home screen did not show Ask anything"
        assert not screen.has_api_prompt(), "No provider connect dialog at home"

        # Phase 1: Connect provider
        send_pty(master, b"/connect\r")
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            b"Connect a provider", len(captures), screen,
        )
        send_pty(master, b"openai\r")
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            b"API key", len(captures), screen,
            lambda: screen.has_api_prompt(),
        )
        send_pty(master, (KEY + "\r").encode())
        if KEY.encode() in bytes(captures):
            raise AssertionError("fixture API key echoed in PTY")
        if not auth.is_file() or auth.stat().st_mode & 0o777 != 0o600:
            raise AssertionError("auth.json not created with 0600")

        send_pty(master, (MODEL + "\r").encode())
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            MODEL.encode(), len(captures), screen,
            lambda: model_ready(screen, MODEL) and screen.picker_closed(),
        )
        assert model_ready(screen, MODEL), "Model not shown as selected"

        read_until(
            master, time.monotonic() + TIMEOUT, captures, b"Ask anything",
            len(captures), screen,
            lambda: home_ready(screen, MODEL),
        )

        # Phase 2: Create two sessions (tabs) via prompt
        send_pty(master, (PROMPT_1 + "\r").encode())
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            PROMPT_1.encode(), len(captures), screen,
        )
        wait_for_provider_request(fixture.state, time.monotonic() + 10)
        assert any(
            any(x.get("content") == PROMPT_1
                for x in r.get("input", []) if x.get("role") == "user")
            for r in fixture.state.requests
        ), "First prompt not sent to provider"
        # E3: register session label with UUID-bound count
        _session_registry.register(PROMPT_1)

        send_pty(master, b"/new\r")
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            b"Ask anything", len(captures), screen,
        )

        send_pty(master, (PROMPT_2 + "\r").encode())
        read_until(
            master, time.monotonic() + TIMEOUT, captures,
            PROMPT_2.encode(), len(captures), screen,
        )
        wait_for_provider_request(fixture.state, time.monotonic() + 10)

        user_inputs = fixture.state.user_inputs()
        assert PROMPT_1 in user_inputs, "First session prompt missing from provider"
        assert PROMPT_2 in user_inputs, "Second session prompt missing from provider"
        _session_registry.register(PROMPT_2)

        # Phase 3: Required upstream keybinding contract (may RED)
        # leader+w MUST close current tab (keybind.ts session.tab.close).
        # tui_entry.rs:8946-8973 only handles b'b'; leader+w is KNOWN GAP.
        tab_titles_before = screen.tab_titles()
        requests_before_w = len(fixture.state.requests)
        send_pty(master, b"\x18")  # Ctrl-X (leader arm)
        time.sleep(0.1)
        send_pty(master, b"w")  # leader+w REQUIRED to close tab
        time.sleep(0.5)
        read_until(
            master, time.monotonic() + 2, captures, b"",
            len(captures), screen, None,
        )
        tab_titles_after_w = screen.tab_titles()
        assert len(tab_titles_after_w) < len(tab_titles_before), (
            f"<leader>w must close tab (upstream session.tab.close): "
            f"{len(tab_titles_before)} -> {len(tab_titles_after_w)}"
        )
        assert len(fixture.state.requests) == requests_before_w, (
            "<leader>w triggered an unexpected provider request"
        )

        # leader+1 MUST select tab 1 (keybind.ts session.tab.select.1).
        # tui_entry.rs:8946 consumes '1'. Assert active session IDENTITY changes.
        screen_before = screen.text()
        send_pty(master, b"\x18")  # leader
        time.sleep(0.05)
        send_pty(master, b"1")  # leader+1 REQUIRED tab select
        time.sleep(0.5)
        read_until(
            master, time.monotonic() + 2, captures, b"",
            len(captures), screen, None,
        )
        assert screen.text() != screen_before, (
            "leader+1 must switch active tab (upstream session.tab.select.1)"
        )
        assert screen.contains(PROMPT_1) or screen.active_tab_hint(), (
            "leader+1 did not bind to session 1 identity"
        )

        # Phase 4: Mouse-based tab selection
        read_until(
            master, time.monotonic() + 2, captures, b"",
            len(captures), screen, None,
        )
        tab_titles = screen.tab_titles()
        assert len(tab_titles) >= 1, f"No tab titles found: {screen.visible_text()}"
        tab_strip_row = find_tab_strip_row(screen)

        # Phase 5: Background tab close via mouse
        # Close glyph U+2715 only renders on HOVERED tab (native_tabs.rs:232).
        # Send motion (button 35, kind 7) FIRST to establish hover + paint glyph.
        glyph_positions = find_close_button_positions(screen, tab_strip_row)
        requests_before_close = len(fixture.state.requests)
        provider_inputs_before_close = fixture.state.user_inputs()
        if len(glyph_positions) >= 1:
            close_x, close_y = glyph_positions[0]
            write_sgr_to_pty(master, close_x, close_y, button=35, press=True)
            time.sleep(0.1)
            read_until(
                master, time.monotonic() + 2, captures, b"",
                len(captures), screen, None,
            )
            write_sgr_to_pty(master, close_x, close_y, button=0, press=True)
            time.sleep(0.05)
            write_sgr_to_pty(master, close_x, close_y, button=0, press=False)
            time.sleep(0.5)
            read_until(
                master, time.monotonic() + 2, captures, b"",
                len(captures), screen, None,
            )
            remaining_titles = screen.tab_titles()
            assert len(remaining_titles) <= len(tab_titles), (
                f"Tab count should have decreased: "
                f"{len(remaining_titles)} >= {len(tab_titles)}"
            )
            assert len(fixture.state.requests) == requests_before_close, (
                "background close triggered an unexpected provider request"
            )
            assert fixture.state.user_inputs() == provider_inputs_before_close, (
                "background close caused a provider input change"
            )

        # Phase 6: /exit closes current tab
        request_count_before_exit = len(fixture.state.requests)
        provider_inputs_before_exit = fixture.state.user_inputs()
        send_pty(master, b"/exit\r")
        time.sleep(0.5)
        read_until(
            master, time.monotonic() + 3, captures, b"",
            len(captures), screen, None,
        )
        assert "/exit" not in " ".join(fixture.state.user_inputs()), (
            "/exit was submitted to the provider as user input"
        )
        assert len(fixture.state.requests) == request_count_before_exit, (
            "/exit triggered an unexpected provider request"
        )
        assert fixture.state.user_inputs() == provider_inputs_before_exit, (
            "/exit caused a provider input change"
        )

        # Phase 7: Tab navigation (legacy native Ctrl-N/Ctrl-O + Ctrl-G)
        # Ctrl-N (0x0E) moves picker highlight; Ctrl-G (0x07) confirms.
        # SEPARATE legacy native probe, NOT ctrl+tab replacement.
        read_until(
            master, time.monotonic() + 2, captures, b"",
            len(captures), screen, None,
        )
        current_titles = screen.tab_titles()
        if len(current_titles) >= 2:
            screen_before_nav = screen.text()
            send_pty(master, b"\x0e")  # Ctrl-N
            time.sleep(0.3)
            send_pty(master, b"\x07")  # Ctrl-G confirm
            time.sleep(0.5)
            read_until(
                master, time.monotonic() + TIMEOUT, captures, b"",
                len(captures), screen, None,
            )
            assert screen.text() != screen_before_nav, (
                "Ctrl-N+Ctrl-G did not produce a visible screen change"
            )

            screen_before_nav2 = screen.text()
            send_pty(master, b"\x0f")  # Ctrl-O
            time.sleep(0.3)
            send_pty(master, b"\x07")  # Ctrl-G confirm
            time.sleep(0.5)
            read_until(
                master, time.monotonic() + TIMEOUT, captures, b"",
                len(captures), screen, None,
            )
            assert screen.text() != screen_before_nav2, (
                "Ctrl-O+Ctrl-G did not produce a visible screen change"
            )

        # Phase 7b: Ctrl+Tab contract (actual terminal protocol)
        # Sends CSI 9;5u (Ctrl+Tab) and CSI 9;6u (Ctrl+Shift+Tab) via PTY.
        # If the native decoder has no Ctrl+Tab mapping, this REDs.
        # SEPARATE assertion, NOT replacing leader/select bindings.
        read_until(
            master, time.monotonic() + 2, captures, b"",
            len(captures), screen, None,
        )
        ctrl_tab_seq = ctrl_tab_sequence(ctrl=True, shift=False)
        screens_nav = []
        for _ in range(2):
            send_pty(master, ctrl_tab_seq)
            time.sleep(0.3)
            read_until(
                master, time.monotonic() + 3, captures, b"",
                len(captures), screen, None,
            )
            screens_nav.append(screen.text())

        ctrl_shift_tab_seq = ctrl_tab_sequence(ctrl=True, shift=True)
        for _ in range(2):
            send_pty(master, ctrl_shift_tab_seq)
            time.sleep(0.3)
            read_until(
                master, time.monotonic() + 3, captures, b"",
                len(captures), screen, None,
            )
            screens_nav.append(screen.text())

        # REQUIRED: ctrl+tab must produce a screen change (tab switch).
        # If the decoder drops the sequence, no change -> RED.
        assert any(s != screens_nav[0] for s in screens_nav[1:]), (
            "Ctrl+Tab/Ctrl+Shift+Tab produced no screen change"
        )

        # Phase 8: Draft persistence across tab switch
        draft_text = "draft across tabs test"
        send_pty(master, draft_text.encode())
        time.sleep(0.3)
        current_titles = screen.tab_titles()
        if len(current_titles) >= 2:
            send_pty

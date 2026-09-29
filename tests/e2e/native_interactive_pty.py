#!/usr/bin/env python3
"""TUI-012 RED: exercise the installed native CLI through a real PTY.

This deliberately does not use ``--once``: that option is a snapshot escape
path and cannot prove interactive input or terminal ownership.  The fixture
paths are explicit so a missing build is an infrastructure error, not a
silently skipped product test.
"""

import atexit
import os
import pty
import select
import shutil
import signal
import subprocess
import tempfile
import termios
import time
import unittest


MAX_CAPTURE = 1024 * 1024
DEADLINE = 8.0
DEFAULT_BINARY = (
    "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/"
    "tui-integrated-verify-d7bc401/target-cli/debug/oc2"
)
DEFAULT_DYLIB = (
    "/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/"
    "native-build-attest/wt/packages/native/lib/aarch64-macos/libopentui.dylib"
)


def fixture(name, default):
    value = os.environ.get(name, default)
    if not os.path.isfile(value) or not os.access(value, os.R_OK):
        raise AssertionError(
            f"fixture error: {name} must name an existing readable file: {value}"
        )
    return value


def read_until(fd, deadline, captured):
    """Read a bounded amount without blocking the test or retaining unbounded output."""
    while time.monotonic() < deadline and len(captured) < MAX_CAPTURE:
        remaining = max(0.0, deadline - time.monotonic())
        ready, _, _ = select.select([fd], [], [], min(0.1, remaining))
        if not ready:
            continue
        try:
            chunk = os.read(fd, min(65536, MAX_CAPTURE - len(captured)))
        except OSError:
            break
        if not chunk:
            break
        captured.extend(chunk)


class NativeInteractivePTY(unittest.TestCase):
    def setUp(self):
        self.master = None
        self.child = None
        self.saved_attrs = None
        self.cleaned = False
        self.temp = tempfile.TemporaryDirectory(prefix="oc2-tui012-")
        self.root = self.temp.name
        self.home = os.path.join(self.root, "home")
        self.project = os.path.join(self.root, "project")
        self.data = os.path.join(self.root, "data")
        for path in (self.home, self.project, self.data):
            os.makedirs(path)
        self.bin = fixture("OC2_NATIVE_BINARY", DEFAULT_BINARY)
        self.dylib = fixture("MAC_OPENTUI_FIXTURE", DEFAULT_DYLIB)
        # Stage an owned installed layout, never mutate the retained fixtures.
        self.install = os.path.join(self.root, "install")
        os.makedirs(os.path.join(self.install, "bin"))
        os.makedirs(os.path.join(self.install, "lib"))
        self.executable = os.path.join(self.install, "bin", "oc2")
        shutil.copyfile(self.bin, self.executable)
        shutil.copyfile(
            self.dylib, os.path.join(self.install, "lib", "libopentui.dylib")
        )
        os.chmod(self.executable, 0o755)
        atexit.register(self._cleanup)

    def _cleanup(self):
        if self.cleaned:
            return
        self.cleaned = True
        child = self.child
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=1.0)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=1.0)
                except (ProcessLookupError, subprocess.TimeoutExpired):
                    pass
        if self.master is not None:
            try:
                if self.saved_attrs is not None:
                    termios.tcsetattr(self.master, termios.TCSANOW, self.saved_attrs)
            except OSError:
                pass
            try:
                os.close(self.master)
            except OSError:
                pass
            self.master = None
        self.temp.cleanup()

    def test_native_character_input_changes_frame_without_newline(self):
        self.master, slave = pty.openpty()
        self.saved_attrs = termios.tcgetattr(self.master)
        # Fixed geometry makes frame behavior reproducible and prevents an
        # implementation from depending on the host terminal dimensions.
        import struct
        import fcntl

        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        env = {
            "HOME": self.home,
            "PATH": os.environ.get("PATH", ""),
            "TERM": "xterm-256color",
            "XDG_CONFIG_HOME": os.path.join(self.root, "config"),
            "XDG_DATA_HOME": os.path.join(self.root, "xdg-data"),
            "XDG_RUNTIME_DIR": os.path.join(self.root, "runtime"),
            "OPENCODE_RK_HOME": self.data,
            "OPENCODE_PROJECT_DIR": self.project,
            "OC2_OFFLINE": "1",
        }
        for path in (env["XDG_CONFIG_HOME"], env["XDG_DATA_HOME"], env["XDG_RUNTIME_DIR"]):
            os.makedirs(path)
        captured = bytearray()
        try:
            try:
                self.child = subprocess.Popen(
                    [self.executable, "--native", "--data-dir", self.data],
                    stdin=slave,
                    stdout=slave,
                    stderr=slave,
                    env=env,
                    cwd=self.project,
                    start_new_session=True,
                    close_fds=True,
                )
            finally:
                os.close(slave)
            read_until(self.master, time.monotonic() + DEADLINE, captured)
            self.assertIsNone(self.child.poll(), "native interactive child exited before input")

            attrs = termios.tcgetattr(self.master)
            self.assertFalse(attrs[3] & termios.ICANON, "native TUI must enter raw/noncanonical mode")
            self.assertFalse(attrs[3] & termios.ECHO, "native TUI must disable terminal echo")
            self.assertFalse(attrs[3] & termios.ISIG, "raw native input must deliver Ctrl-C as a byte")
            before = bytes(captured)
            os.write(self.master, b"x")
            read_until(self.master, time.monotonic() + DEADLINE, captured)
            update = bytes(captured)[len(before) :]
            self.assertTrue(update, "one byte without newline must drive a frame/event")
            self.assertIn(b"x", update, "the frame update must represent the submitted character")
            os.write(self.master, b"\x03")
            self.child.wait(timeout=DEADLINE)
            self.assertEqual(self.child.returncode, 0, "Ctrl-C must terminate the owned interactive session")
        finally:
            # Restore the PTY before closing it; this assertion must not be
            # masked by cleanup of the descriptor or by a forced kill.
            if self.master is not None:
                termios.tcsetattr(self.master, termios.TCSANOW, self.saved_attrs)
                restored = termios.tcgetattr(self.master)
                self.assertEqual(
                    restored[3] & (termios.ICANON | termios.ECHO),
                    termios.ICANON | termios.ECHO,
                )
            self._cleanup()
            self.child = None


if __name__ == "__main__":
    unittest.main()

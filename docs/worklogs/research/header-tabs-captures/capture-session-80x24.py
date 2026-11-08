#!/usr/bin/env python3
"""Accurate session capture at 140x40 using pyte (real VT emulator)."""
import os, pty, select, struct, termios, time, fcntl, subprocess, sys, json, importlib.util, socket, pathlib
import pyte

sys.path.insert(0, "/Users/mymac/Projects/opencode-rk-recovery-20261005/tests/e2e")
spec = importlib.util.spec_from_file_location(
    "probe", "/Users/mymac/Projects/opencode-rk-recovery-20261005/tests/e2e/native_provider_setup.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

BINARY = "/Users/mymac/Projects/opencode-rk-recovery-lanes/ui/target/debug/oc2"
DYLIB = "/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib"
KEY = probe.KEY
MODEL = probe.MODEL
COLS, ROWS = 80, 24

root = "/tmp/p1-ui-tabs-s3-80"
subprocess.run(["rm", "-rf", root])
for n in ("home/.config", "project", "d/runtime", "d/catalog"):
    os.makedirs(os.path.join(root, n), exist_ok=True)
pathlib.Path(root + "/d/catalog/models.dev.api.json").write_text(json.dumps(probe.models_json()))
fixture = probe.Fixture(("127.0.0.1", 0), probe.Handler)
fixture.state = probe.State()
import threading
threading.Thread(target=fixture.serve_forever, daemon=True).start()
s = socket.socket(); s.bind(("127.0.0.1", 0)); daemon_port = s.getsockname()[1]; s.close()

env = {
    "HOME": root + "/home", "XDG_CONFIG_HOME": root + "/home/.config",
    "XDG_DATA_HOME": root + "/d", "XDG_RUNTIME_DIR": root + "/d/runtime",
    "OPENCODE_RK_HOME": root + "/d",
    "OPENCODE_RK_DAEMON_ADDR": "127.0.0.1:%d" % daemon_port,
    "OPENCODE_PROJECT_DIR": root + "/project",
    "PATH": "/usr/bin:/bin", "TERM": "xterm-256color",
    "OPENAI_BASE_URL": "http://127.0.0.1:%d/v1" % fixture.server_port,
    "DYLD_LIBRARY_PATH": "/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin",
}
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
child = subprocess.Popen([BINARY, "--data-dir", root + "/d"], cwd=root + "/project", env=env,
                         stdin=slave, stdout=slave, stderr=slave, start_new_session=True, close_fds=True)
os.close(slave)
stream = pyte.ByteStream()
screen = pyte.Screen(COLS, ROWS)
stream.attach(screen)

def pump(d):
    end = time.monotonic() + d
    while time.monotonic() < end:
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                out = os.read(master, 65536)
            except OSError:
                return
            if not out:
                return
            stream.feed(out)

def render(label, path=None):
    lines = screen.display
    print(f"===== {label} =====")
    for i, line in enumerate(lines):
        if line.strip():
            print(f"{i:2d} |{line}|")
    if path:
        with open(path, "w") as f:
            for i, line in enumerate(lines):
                f.write(f"{i:2d} |{line}|\n")

def wait_for(predicate, timeout=20):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        r, _, _ = select.select([master], [], [], 0.1)
        if r:
            try:
                out = os.read(master, 65536)
            except OSError:
                return False
            if not out:
                return False
            stream.feed(out)
        if predicate():
            return True
    return False

t0 = time.monotonic()
while time.monotonic() - t0 < 20:
    r, _, _ = select.select([master], [], [], 0.1)
    if r:
        out = os.read(master, 65536)
        if not out:
            break
        stream.feed(out)
        if sum(1 for ch in screen.display[0] + screen.display[12] if ch != " ") > 20:
            break
pump(2.0)
render("initial home", "/tmp/p1-tabs3-80-initial.txt")

os.write(master, b"/connect\r"); wait_for(lambda: any("Connect a provider" in r for r in screen.display))
os.write(master, b"openai\r"); wait_for(lambda: any("Connect openai" in r for r in screen.display))
os.write(master, b"\x1b[B\r"); wait_for(lambda: any("Key:" in r for r in screen.display))
os.write(master, KEY.encode() + b"\r"); wait_for(lambda: any("Select model" in r for r in screen.display))
os.write(master, MODEL.encode() + b"\r"); wait_for(lambda: any("Ask anything" in r for r in screen.display) and not any("Connect openai" in r for r in screen.display), 15)
pump(1.5)
render("after connect (session/home?)", "/tmp/p1-tabs3-80-afterconnect.txt")

os.write(master, probe.PROMPTS[0].encode() + b"\r")
pump(0.8)
render("thinking state", "/tmp/p1-tabs3-80-thinking.txt")
pump(10.0)
render("settled session", "/tmp/p1-tabs3-80-session.txt")

os.write(master, b"\x03")
try:
    child.wait(timeout=5)
except subprocess.TimeoutExpired:
    os.killpg(child.pid, 9)
print("requests:", len(fixture.state.requests))

#!/usr/bin/env python3
"""Installed configured command: discovery, expansion, real provider and ACK."""
import argparse
import fcntl
import hashlib
import json
import os
import pathlib
import pty
import struct
import subprocess
import tempfile
import termios
import threading
import time

from native_empty_session import Journey, ROWS, COLS, KEY, make_environment, wait_frame
from native_provider_setup import checked_artifact, descriptor, free_loopback_port
from native_escape_input import Fixture, Handler, sse

ARGUMENTS = '"first value"\n  second 世界'
EXPANDED = 'Inspect <' + ARGUMENTS + '>\nfirst=first value tail=second 世界'
REPLY = "CONFIGURED_COMMAND_EXECUTED"


class Provider:
    def __init__(self):
        self.requests = []
        self.error = None

    def body(self, headers, raw):
        assert headers.get("Authorization") == "Bearer " + KEY
        value = json.loads(raw)
        assert len(self.requests) == 0, "command duplicated provider work"
        self.requests.append(value)
        assert value["model"] == "gpt-5.6-mini", "command model did not override active model"
        users = [item["content"] for item in value["input"] if item.get("role") == "user"]
        assert users == [EXPANDED], "provider received literal slash command or lossy arguments"
        assert any(item.get("role") == "system" and "Agent: auditor" in item.get("content", "")
                   and "Audit configured inputs." in item.get("content", "") for item in value["input"])
        return sse(REPLY)


class CommandJourney(Journey):
    def start(self):
        self.master, self.slave = pty.openpty()
        self.saved_termios = termios.tcgetattr(self.master)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.child = subprocess.Popen([str(self.binary), "--data-dir", str(self.data)],
                                     cwd=self.project, env=self.env, stdin=self.slave,
                                     stdout=self.slave, stderr=self.slave, start_new_session=True,
                                     close_fds=True)
        os.close(self.slave)
        self.slave = None
        port = int(self.env["OPENCODE_RK_DAEMON_ADDR"].rsplit(":", 1)[1])
        self.daemon_descriptor = descriptor(self.data, port, self.child, time.monotonic() + 20)

    def wait(self, predicate):
        wait_frame(self.master, self.child, self.screen, self.captured, predicate,
                   len(self.captured), timeout=20)


def run(binary, library, manifest, artifacts):
    binary, library, source, binary_hash, library_hash = checked_artifact(binary, library, manifest)
    artifacts.mkdir(parents=True, exist_ok=False)
    parent = pathlib.Path('/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode')
    root = pathlib.Path(tempfile.mkdtemp(prefix="command-", dir=parent))
    provider = Provider()
    server = Fixture(("127.0.0.1", 0), Handler)
    server.state = provider
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    journey = CommandJourney(binary, root, server)
    result = {"pass": False, "source_sha": source, "binary_sha256": binary_hash,
              "native_library_sha256": library_hash,
              "test_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()}
    try:
        journey.env, journey.data = make_environment(root, free_loopback_port(), server.server_port)
        journey.env["DYLD_LIBRARY_PATH"] = str(library.parent)
        journey.env["LD_LIBRARY_PATH"] = str(library.parent)
        (journey.project / "opencode.json").write_text(json.dumps({
            "commands": {"inspect": {"template": "Inspect <$ARGUMENTS>\nfirst=$1 tail=$2",
                                     "description": "Inspect configured inputs", "agent": "auditor",
                                     "model": "openai/gpt-5.6-mini", "variant": "high"}},
            "agents": {"auditor": {"mode": "primary", "prompt": "Audit configured inputs."}}
        }))
        markdown = journey.project / ".opencode/commands/team"
        markdown.mkdir(parents=True)
        (markdown / "simple.md").write_text("---\ndescription: Markdown discovery\n---\nExplain $ARGUMENTS.")
        journey.start()
        journey.wait(lambda screen: "Ask anything" in screen.text())
        listing = journey.session_api("/api/commands")["commands"]
        assert {item["name"] for item in listing} == {"inspect", "team/simple"}
        os.write(journey.master, b"/commands\r")
        journey.wait(lambda screen: "/inspect" in screen.text() and "Inspect configured inputs" in screen.text())
        os.write(journey.master, b"\x1b")
        journey.wait(lambda screen: "Ask anything" in screen.text())
        invocation = "/inspect " + ARGUMENTS
        os.write(journey.master, b"\x1b[200~" + invocation.encode() + b"\x1b[201~\r")
        journey.wait(lambda screen: REPLY in screen.text())
        assert not provider.error, provider.error
        sessions = journey.session_api("/api/sessions")["sessions"]
        assert len(sessions) == 1
        session = sessions[0]["id"]
        messages = journey.session_api(f"/api/sessions/{session}/messages")["messages"]
        assert [(item["role"], item["body"]["text"]) for item in messages] == [("user", EXPANDED), ("assistant", REPLY)]
        assert all(item["session_id"] == session for item in messages)
        # A rejected invocation must preserve its exact composer draft and may
        # never become an ordinary model prompt or a durable user message.
        os.write(journey.master, b"/missing-command untouched\r")
        journey.wait(lambda screen: "configured command not found" in screen.text().lower())
        assert "missing-command untouched" in journey.screen.text()
        assert len(provider.requests) == 1
        assert journey.session_api(f"/api/sessions/{session}/messages")["messages"] == messages
        result.update({"pass": True, "provider_requests": 1, "expanded_prompt": EXPANDED,
                       "agent_and_model_applied": True, "unknown_command_side_effect_free": True,
                       "assertions": journey.assertions})
    except Exception as error:
        result["error"] = str(error)
    finally:
        try:
            journey.stop()
        except Exception as error:
            result.update({"pass": False, "cleanup_error": str(error)})
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        if thread.is_alive(): result.update({"pass": False, "provider_thread_alive": True})
        (artifacts / "terminal.ansi").write_bytes(journey.captured)
        (artifacts / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(result, indent=2, ensure_ascii=False))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=pathlib.Path)
    parser.add_argument("--native-library", required=True, type=pathlib.Path)
    parser.add_argument("--build-json", required=True, type=pathlib.Path)
    parser.add_argument("--artifact-dir", required=True, type=pathlib.Path)
    args = parser.parse_args()
    raise SystemExit(run(args.binary, args.native_library, args.build_json, args.artifact_dir))

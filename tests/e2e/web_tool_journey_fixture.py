#!/usr/bin/env python3
"""Bounded loopback provider and owned-daemon harness for G6 browser proof."""

import argparse
import hashlib
import http.server
import json
import os
import pathlib
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request
import urllib.error

ROOT = pathlib.Path("/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp")
APPROVED_ARTIFACT_ROOT = pathlib.Path("/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode")
MARKER = "G6 browser fixture marker\n"
CALL_ID = "g6_write_1"
FIRST_PROMPT = "write the browser marker"
SECOND_PROMPT = "confirm the browser marker"
RESUMED_PROMPT = "resume after restart"
MAX_REQUESTS = 4
MAX_REQUEST_BYTES = 128 * 1024
MAX_EVIDENCE_BYTES = 512 * 1024
TOKEN_RE = re.compile(r"^[0-9a-fA-F]{64}$")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_json(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    os.replace(temporary, path)


def check_build(binary, manifest):
    if not binary.is_absolute() or not binary.is_file():
        raise ValueError("--binary must be an absolute regular file")
    build = json.loads(manifest.read_text())
    source = build.get("source_sha") or build.get("sourceSha") or build.get("git_sha")
    expected = (build.get("binary_sha256") or build.get("binarySha256") or "").lower()
    if not isinstance(source, str) or not re.fullmatch(r"[0-9a-fA-F]{40}", source):
        raise ValueError("build.json source SHA must be 40 hexadecimal characters")
    if not re.fullmatch(r"[0-9a-f]{64}", expected) or sha256(binary) != expected:
        raise ValueError("installed binary hash does not match build.json")
    return source, expected


class ProviderState:
    def __init__(self, project):
        self.project = project
        self.requests = []
        self.restart_generation = 0
        self.failed = None

    def set_restart_generation(self, generation):
        self.restart_generation = generation

    def response(self, headers, body):
        if len(body) > MAX_REQUEST_BYTES:
            raise ValueError("provider request exceeded 128 KiB")
        if headers.get("Authorization") != "Bearer fixture-key":
            raise ValueError("provider did not receive exact fixture credential")
        value = json.loads(body.decode("utf-8"))
        self.requests.append(value)
        number = len(self.requests)
        if number > MAX_REQUESTS:
            raise ValueError("provider request count exceeded four")
        inputs = value.get("input")
        if not isinstance(inputs, list):
            raise ValueError("provider input is not an array")
        if any(not isinstance(item, dict) for item in inputs):
            raise ValueError("provider input contains a non-object")
        if value.get("model") != "gpt-5.6":
            raise ValueError("the selected fixture model did not reach the provider")
        users = [item.get("content") for item in inputs if item.get("role") == "user"]
        expected_users = {
            1: [FIRST_PROMPT],
            2: [FIRST_PROMPT],
            3: [FIRST_PROMPT, SECOND_PROMPT],
            4: [FIRST_PROMPT, SECOND_PROMPT, RESUMED_PROMPT],
        }[number]
        if users != expected_users:
            raise ValueError(f"unexpected durable user history on request {number}: {users!r}")
        calls = [x for x in inputs if x.get("type") == "function_call"]
        outputs = [x for x in inputs if x.get("type") == "function_call_output"]
        if number == 1:
            if calls or outputs:
                raise ValueError("first request contains an unexpected tool pair")
            arguments = json.dumps({"path": "g6-browser-marker.txt", "content": MARKER, "append": False}, separators=(",", ":"))
            return sse_function_call(arguments)
        if not (pathlib.Path(self.project, "g6-browser-marker.txt").is_file() and pathlib.Path(self.project, "g6-browser-marker.txt").read_text() == MARKER):
            raise ValueError("write success was reported without the exact marker bytes")
        assistants = [item.get("content") for item in inputs if item.get("role") == "assistant"]
        expected_assistants = {
            2: [],
            3: ["G6 first turn settled"],
            4: ["G6 first turn settled", "G6 second turn settled"],
        }[number]
        if assistants != expected_assistants:
            raise ValueError("settled assistant history changed before continuation or resume")
        if len(calls) != 1 or len(outputs) != 1 or len(inputs) != len(users) + len(assistants) + 2:
            raise ValueError("request does not contain exactly one typed call and output")
        call, output = calls[0], outputs[0]
        expected_args = json.dumps({"path": "g6-browser-marker.txt", "content": MARKER, "append": False}, separators=(",", ":"))
        if call != {"type": "function_call", "call_id": CALL_ID, "name": "write", "arguments": expected_args}:
            raise ValueError("typed call identity or arguments changed")
        if output != {"type": "function_call_output", "call_id": CALL_ID, "output": "write success"}:
            raise ValueError("typed output identity or result changed")
        if inputs.index(call) >= inputs.index(output):
            raise ValueError("typed output precedes its function call")
        if number == 4 and self.restart_generation < 1:
            raise ValueError("resumed request arrived before a fresh daemon restart")
        return sse_text({2: "G6 first turn settled", 3: "G6 second turn settled", 4: "G6 resumed turn settled"}[number])


def sse_function_call(arguments):
    event = {"type": "response.output_item.done", "item": {"type": "function_call", "id": "g6-item-1", "call_id": CALL_ID, "name": "write", "arguments": arguments}}
    return ("event: response.output_item.done\ndata: " + json.dumps(event) + "\n\n" + completed()).encode()


def sse_text(text):
    return ("event: response.output_text.delta\ndata: " + json.dumps({"type": "response.output_text.delta", "delta": text}) + "\n\n" + completed()).encode()


def completed():
    return 'event: response.completed\ndata: {"type":"response.completed","response":{"id":"g6-fixture","status":"completed"}}\n\n'


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            self.send_error(400)
            return
        if length < 0 or length > MAX_REQUEST_BYTES:
            self.send_error(413)
            return
        try:
            if self.path != "/v1/responses":
                raise ValueError("unexpected provider route")
            body = self.rfile.read(length)
            if len(body) != length:
                raise ValueError("truncated provider request body")
            response = self.server.state.response(self.headers, body)
            self.server.persist_evidence()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(response)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(response)
        except Exception as error:
            self.server.state.failed = str(error)
            self.server.persist_evidence()
            self.send_error(500, str(error))

    def log_message(self, *_):
        return


class Provider(http.server.HTTPServer):
    allow_reuse_address = True

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(5)
        return connection, address

    def persist_evidence(self):
        if getattr(self, "evidence_path", None) is None:
            return
        evidence = json.dumps(self.state.requests, indent=2).encode()
        if len(evidence) > MAX_EVIDENCE_BYTES:
            raise ValueError("provider evidence exceeded 512 KiB")
        atomic_json(self.evidence_path, self.state.requests)


def stop(child):
    if child and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=2)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=2)


def descriptor(path, deadline, child):
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise RuntimeError("owned daemon exited before publishing a ready descriptor")
        try:
            value = json.loads(path.read_text())
            if (value.get("pid") == child.pid
                    and isinstance(value.get("http_origin"), str)
                    and re.fullmatch(r"http://127\.0\.0\.1:[0-9]+", value["http_origin"])
                    and TOKEN_RE.fullmatch(value.get("auth_token", ""))
                    and healthy(value)):
                return value
        except (OSError, ValueError):
            pass
        time.sleep(0.02)
    raise RuntimeError("authenticated daemon descriptor readiness timeout")


def healthy(value):
    host = value["http_origin"].removeprefix("http://")
    request = urllib.request.Request("http://" + host + "/api/models?limit=1", headers={"Authorization": "Bearer " + value["auth_token"]})
    try:
        with urllib.request.urlopen(request, timeout=1) as response:
            return response.status == 200
    except (OSError, urllib.error.URLError):
        return False


def self_check():
    """Exercise the real loopback HTTP framing without launching a daemon."""
    root = pathlib.Path(tempfile.mkdtemp(prefix="g6-self-check-", dir=str(ROOT)))
    project = root / "project"
    project.mkdir(parents=True)
    state = ProviderState(project)
    server = Provider(("127.0.0.1", 0), Handler)
    server.state = state
    server.timeout = 0.05
    try:
        import http.client
        thread = __import__("threading").Thread(target=server.serve_forever)
        thread.start()
        connection = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=2)
        body = json.dumps({"model": "gpt-5.6", "input": [{"role": "user", "content": FIRST_PROMPT}]}).encode()
        connection.request("POST", "/v1/responses", body, {"Authorization": "Bearer fixture-key", "Content-Length": str(len(body))})
        assert connection.getresponse().status == 200
        connection.close()
        assert len(state.requests) == 1
        arguments = json.dumps({"path": "g6-browser-marker.txt", "content": MARKER, "append": False}, separators=(",", ":"))
        call = {"type": "function_call", "call_id": CALL_ID, "name": "write", "arguments": arguments}
        output = {"type": "function_call_output", "call_id": CALL_ID, "output": "write success"}
        (project / "g6-browser-marker.txt").write_text(MARKER)
        inputs = [{"role": "user", "content": FIRST_PROMPT}, call, output]
        headers = {"Authorization": "Bearer fixture-key"}
        assert b"G6 first turn settled" in state.response(headers, json.dumps({"model": "gpt-5.6", "input": inputs}).encode())
        inputs += [{"role": "assistant", "content": "G6 first turn settled"}, {"role": "user", "content": SECOND_PROMPT}]
        assert b"G6 second turn settled" in state.response(headers, json.dumps({"model": "gpt-5.6", "input": inputs}).encode())
        inputs += [{"role": "assistant", "content": "G6 second turn settled"}, {"role": "user", "content": RESUMED_PROMPT}]
        state.set_restart_generation(1)
        assert b"G6 resumed turn settled" in state.response(headers, json.dumps({"model": "gpt-5.6", "input": inputs}).encode())
        print("fixture self-check: framing, credential, typed pair and settled history passed")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
        if thread.is_alive():
            raise RuntimeError("fixture provider thread did not stop")
        shutil.rmtree(root)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary")
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--build-json")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return
    if not args.binary:
        parser.error("--binary is required unless --self-check is used")
    binary = pathlib.Path(args.binary).resolve()
    artifacts = pathlib.Path(args.artifact_dir).resolve()
    manifest = pathlib.Path(args.build_json or artifacts / "build.json").resolve()
    source_sha, binary_sha = check_build(binary, manifest)
    if APPROVED_ARTIFACT_ROOT not in artifacts.parents:
        raise ValueError("artifact directory must be under the approved OpenCode temporary root")
    artifacts.mkdir(parents=True, exist_ok=True)
    if (artifacts / "fixture-metadata.json").exists():
        raise ValueError("artifact directory already contains a fixture; preserve it and use a fresh directory")
    root = pathlib.Path(tempfile.mkdtemp(prefix="g6-web-", dir=str(ROOT)))
    home, data, project = (root / name for name in ("home", "data", "project"))
    for path in (home, data, project): path.mkdir()
    if len(os.fsencode(data / "runtime" / "opencode-rk.sock")) > 100:
        raise ValueError("fixture data path is too long for the Unix socket")
    auth = data / "opencode" / "auth.json"; auth.parent.mkdir(); auth.write_text(json.dumps({"openai": {"type": "api", "key": "fixture-key"}}) + "\n"); os.chmod(auth, 0o600)
    state = ProviderState(project)
    server = Provider(("127.0.0.1", 0), Handler); server.state = state; server.timeout = 0.05
    server.evidence_path = artifacts / "provider-requests.json"
    server.persist_evidence()
    models = root / "models.json"
    atomic_json(models, {"openai": {"name": "OpenAI fixture", "models": {"gpt-5.6": {"name": "G6 fixture model", "tool_call": True, "reasoning": True, "limit": {"context": 200000}}}}})
    provider_url = "http://127.0.0.1:%d/v1" % server.server_address[1]
    env = {"HOME": str(home), "XDG_CONFIG_HOME": str(home / ".config"), "XDG_DATA_HOME": str(data), "OPENCODE_RK_HOME": str(data), "PATH": "/usr/bin:/bin", "LANG": "C", "TMPDIR": str(root), "OPENAI_BASE_URL": provider_url, "OPENCODE_RK_TURN_TOOLS": "write", "OPENCODE_RK_TURN_MAX_STEPS": "4"}
    command = [str(binary), "serve", "--listen", "127.0.0.1:0", "--models-file", str(models)]
    child = None; control = artifacts / "control"; control.mkdir(exist_ok=True); metadata_path = artifacts / "fixture-metadata.json"; descriptor_path = data / "runtime" / "backend.json"
    try:
        child = subprocess.Popen(command, cwd=project, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        current = descriptor(descriptor_path, time.monotonic() + 15, child)
        meta = {"origin": current["http_origin"], "launch_fragment": current["http_origin"] + "#oc2-token=" + current["auth_token"], "pid": child.pid, "restart_generation": 0, "project": str(project), "marker": str(project / "g6-browser-marker.txt"), "source_sha": source_sha, "binary_sha256": binary_sha, "provider": provider_url, "control_dir": str(control), "request_evidence": str(artifacts / "provider-requests.json"), "auth_json": str(auth), "auth_json_mode": "0600"}
        atomic_json(metadata_path, meta)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            server.handle_request()
            if (control / "restart.request").exists():
                (control / "restart.request").unlink(); old_pid = child.pid; stop(child); child = subprocess.Popen(command, cwd=project, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                fresh = descriptor(descriptor_path, time.monotonic() + 15, child)
                if fresh["pid"] == old_pid or not healthy(fresh): raise RuntimeError("restart did not publish a fresh healthy descriptor")
                state.set_restart_generation(1); meta.update(origin=fresh["http_origin"], launch_fragment=fresh["http_origin"] + "#oc2-token=" + fresh["auth_token"], pid=child.pid, restart_generation=1); atomic_json(metadata_path, meta)
            if state.failed: raise RuntimeError(state.failed)
            if child.poll() is not None:
                raise RuntimeError("owned daemon exited during browser verification")
            if (control / "stop.request").exists(): break
    finally:
        try:
            server.persist_evidence()
            atomic_json(artifacts / "fixture-result.json", {"provider_failure": state.failed, "provider_requests": len(state.requests), "restart_generation": state.restart_generation, "source_sha": source_sha})
        finally:
            stop(child)
            server.server_close()


if __name__ == "__main__":
    main()

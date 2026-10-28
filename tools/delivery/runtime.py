"""Bounded subprocesses and real OpenCode V2 agent execution.

Working-directory isolation and permissions are defense in depth, not an OS
sandbox. Author-supplied commands are never executed. Tests come from the frozen
owner manifest. Private canonical-write capabilities never enter agent/test envs.
"""
from __future__ import annotations

import contextlib
import hashlib
import json
import os
import pathlib
import re
import selectors
import signal
import subprocess
import sys
import tempfile
import time

SECRET = re.compile(r'(?i)((?:authorization|api[_-]?key|password|secret|bearer|access[_-]?token|refresh[_-]?token|github[_-]?token|npm[_-]?token)["\s:=]+)[^\s,}\"]+')
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


class Failure(RuntimeError):
    def __init__(self, message, kind="failure"):
        super().__init__(message)
        self.kind = kind


def redact(text):
    text = SECRET.sub(r"\1[REDACTED]", text)
    return re.sub(r"\bsk-[A-Za-z0-9_-]{8,}\b", "[REDACTED]", text)


def environment(agent=False, forward=()):
    """Allow only operational environment plus explicitly owner-approved keys."""
    allowed = {"PATH", "HOME", "TMPDIR", "TEMP", "TMP", "USER", "LOGNAME", "LANG", "LC_ALL",
               "TERM", "SHELL", "SYSTEMROOT", "COMSPEC", "SSL_CERT_FILE", "SSL_CERT_DIR"}
    if agent:
        allowed.update({"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"})
    for name in forward:
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
            raise Failure("invalid forwarded environment variable name", "authority")
        if name.startswith(("OC2_RECOVERY_", "DELIVERY_", "GIT_")):
            raise Failure("private controller authority cannot be forwarded", "authority")
        allowed.add(name)
    env = {k: v for k, v in os.environ.items()
           if (k in allowed or k.startswith("LC_"))
           and not k.startswith(("OC2_RECOVERY_", "DELIVERY_", "GIT_"))}
    env.update({"GIT_TERMINAL_PROMPT": "0", "CARGO_BUILD_JOBS": "2", "RUST_TEST_THREADS": "2"})
    return env


def run(argv, cwd, *, valid=lambda: True, timeout=900, max_bytes=2097152, env=None, log=None,
        capture_file=False, cancel_signal=signal.SIGTERM):
    """Continuously drain output, fence orphaned work, bound memory, reap own group."""
    if not valid():
        raise Failure("job lease is no longer valid", "cancelled")
    # Some CLIs exit before asynchronously flushing a large pipe write. A private
    # regular file makes inventory capture complete without weakening the bound.
    capture = tempfile.TemporaryFile() if capture_file else None
    try:
        with tempfile.TemporaryFile() as command_spec:
            command_spec.write(json.dumps({"argv": list(argv), "cancel_signal": int(cancel_signal)}).encode())
            command_spec.seek(0)
            guarded = [sys.executable, str(pathlib.Path(__file__).with_name("process_guard.py")), str(command_spec.fileno())]
            proc = subprocess.Popen(guarded, cwd=str(cwd), env=env if env is not None else environment(),
                                    stdout=capture if capture is not None else subprocess.PIPE,
                                    stderr=subprocess.STDOUT, stdin=subprocess.PIPE,
                                    pass_fds=(command_spec.fileno(),), start_new_session=True)
    except BaseException:
        if capture is not None:
            capture.close()
        raise
    started = time.monotonic()
    tail = bytearray()
    total = 0
    sel = selectors.DefaultSelector()
    if capture is None:
        sel.register(proc.stdout, selectors.EVENT_READ)
    reason = None
    try:
        # EOF is not process completion: a command may close stdout and continue
        # working. Keep its configured timeout and revocation checks active.
        while sel.get_map() or proc.poll() is None:
            if not valid():
                reason = "worker lease expired or was revoked"
                break
            if time.monotonic() - started > timeout:
                reason = "command exceeded its timeout"
                break
            if capture is not None:
                if os.fstat(capture.fileno()).st_size > max_bytes:
                    reason = "command output exceeded its capture limit"
                    break
                time.sleep(0.05)
            for key, _ in sel.select(timeout=0.1 if capture is None else 0):
                data = os.read(key.fileobj.fileno(), 65536)
                if not data:
                    sel.unregister(key.fileobj)
                    continue
                total += len(data)
                tail.extend(data)
                if len(tail) > max_bytes:
                    del tail[:-max_bytes]
                if total > max_bytes:
                    reason = "command output exceeded its capture limit"
                    break
            if reason:
                break
            if proc.poll() is not None and not sel.get_map():
                break
        if capture is not None:
            total = os.fstat(capture.fileno()).st_size
            capture.seek(max(0, total-max_bytes))
            tail = bytearray(capture.read(max_bytes))
            if total > max_bytes:
                reason = "command output exceeded its capture limit"
        if reason:
            if log:
                pathlib.Path(log).parent.mkdir(parents=True, exist_ok=True)
                pathlib.Path(log).write_text(redact(tail.decode("utf-8", "replace")), encoding="utf-8")
            with contextlib.suppress(ProcessLookupError):
                os.killpg(proc.pid, cancel_signal)
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=5)
            with contextlib.suppress(ProcessLookupError):
                os.killpg(proc.pid, signal.SIGKILL)
            raise Failure(reason, "cancelled" if not valid() else
                          "output_limit" if "capture limit" in reason else "timeout")
        rc = proc.wait(timeout=5)
        output = redact(tail.decode("utf-8", "replace"))
        if log:
            pathlib.Path(log).parent.mkdir(parents=True, exist_ok=True)
            pathlib.Path(log).write_text(output, encoding="utf-8")
        if not valid():
            raise Failure("result arrived after lease revocation", "cancelled")
        return {"returncode": rc, "output": output, "output_bytes": total,
                "truncated": total > max_bytes, "duration": round(time.monotonic()-started, 3),
                "output_sha256": hashlib.sha256(output.encode()).hexdigest()}
    finally:
        sel.close()
        if proc.stdin:
            proc.stdin.close()
        if proc.poll() is None:
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=5)
        if proc.stdout:
            proc.stdout.close()
        if capture is not None:
            capture.close()


def json_values(text):
    """Read JSON amid diagnostic prefixes, without eval or executing content."""
    text = ANSI.sub("", text)
    decoder = json.JSONDecoder()
    i = 0
    while i < len(text):
        if text[i] not in "[{":
            i += 1
            continue
        try:
            value, end = decoder.raw_decode(text[i:])
        except ValueError:
            i += 1
            continue
        yield value
        i += end


def inventory(executable, cwd, required=(), valid=lambda: True, forward_env=()):
    """Bounded inventory reads tolerate the installed CLI's partial JSON output.

    Only complete agent records are usable. A missing record in a partial CLI
    dump is not proof that an authorized route was removed.
    """
    by_id = {}
    for attempt in range(3):
        result = run([executable, "debug", "agents"], cwd, valid=valid, timeout=30,
                     max_bytes=8388608, env=environment(agent=True, forward=forward_env), capture_file=True)
        if result["returncode"] or result["truncated"]:
            raise Failure("OpenCode agent inventory failed or exceeded the output bound", "runtime")
        for value in json_values(result["output"]):
            rows = value.get("agents", [value]) if isinstance(value, dict) else value
            if isinstance(rows, list):
                for row in rows:
                    if isinstance(row, dict) and isinstance(row.get("id", row.get("name")), str) and "permissions" in row:
                        by_id[row.get("id", row.get("name"))] = row
        if by_id and all(name in by_id for name in required):
            return by_id
        if attempt < 2:
            time.sleep(0.2)
    raise Failure("required agents were not observed in bounded OpenCode inventory reads", "runtime")


def agent_definition(cfg, route):
    """Copy only the owner-configured route definition, never provider secrets.

    OpenCode V2 needs the selected agent's model in a project override. A
    permissions-only override can otherwise hide or fail to construct the route.
    """
    location = cfg["runtime"].get("agent_config", "~/.config/opencode/opencode.json")
    try:
        data = json.loads(pathlib.Path(location).expanduser().read_text())
    except (OSError, ValueError) as exc:
        raise Failure("cannot read the configured JSON agent catalog: " + type(exc).__name__, "runtime") from exc
    row = data.get("agents", {}).get(route)
    if row is None:
        row = data.get("agent", {}).get(route)
    if not isinstance(row, dict) or not row.get("model"):
        raise Failure("authorized route has no explicit configured model: " + route, "runtime")
    return {key: row[key] for key in ("description", "mode", "model", "steps", "system", "variant") if key in row}


def permissions(grants, editable):
    rules = [{"action": "*", "resource": "*", "effect": "deny"}]
    rules.extend({"action": a, "resource": "*", "effect": "allow"} for a in ("read", "glob", "grep"))
    if editable:
        for grant in grants:
            rules.extend({"action": "edit", "resource": p, "effect": "allow"} for p in (grant, grant+"/*"))
    for action, resource in (("external_directory", "*"), ("shell", "*"), ("subagent", "*"),
                             ("execute", "*"), ("read", "*.env"), ("read", "*.env.*"),
                             ("read", ".git/*"), ("edit", ".git/*"), ("read", ".opencode/*"),
                             ("read", "opencode.json*"), ("edit", ".opencode/*"),
                             ("edit", "opencode.json*"), ("edit", "AGENTS.md")):
        rules.append({"action": action, "resource": resource, "effect": "deny"})
    return rules


@contextlib.contextmanager
def overlay(workspace, agent, grants, editable, definition=None):
    """Apply a highest-priority project agent permission overlay in owned clone."""
    folder = pathlib.Path(workspace) / ".opencode"
    if folder.is_symlink():
        raise Failure("project config directory is a symlink", "authority")
    folder.mkdir(exist_ok=True)
    p = folder / "opencode.json"
    q = folder / "opencode.jsonc"
    for existing in (p, q):
        if existing.is_symlink():
            raise Failure("project config is a symlink", "authority")
    saved = {f: f.read_bytes() if f.is_file() else None for f in (p, q)}
    # Do not preserve repository-provided executable plugins in the temporary
    # override. Global provider credentials remain managed by OpenCode itself.
    rules = permissions(grants, editable)
    selected = dict(definition or {})
    selected["permissions"] = rules
    p.write_text(json.dumps({"$schema": "https://opencode.ai/config.json", "snapshots": False,
                            "agents": {agent: selected}}))
    if q.exists():
        q.unlink()
    try:
        yield rules
    finally:
        for f, data in saved.items():
            if data is None:
                f.unlink(missing_ok=True)
            else:
                f.write_bytes(data)
        with contextlib.suppress(OSError):
            folder.rmdir()


def assistant_text(output, kind):
    if kind == "command":
        return output
    messages = {}
    saw_error = False
    for value in json_values(output):
        if not isinstance(value, dict):
            continue
        typ = str(value.get("type", ""))
        if typ in {"error", "session.error"}:
            saw_error = True
        # OpenCode run text parts; deliberately do not parse tool request/result
        # bodies as approval. Unknown event formats fail closed for reviewers.
        part = value.get("part", {})
        if typ == "text" and isinstance(part, dict) and part.get("type", "text") == "text":
            mid = part.get("messageID", value.get("messageID", "unbound"))
            pieces = messages.setdefault(mid, {})
            pieces[part.get("id", str(len(pieces)))] = str(part.get("text", ""))
        elif typ == "text" and isinstance(value.get("text"), str):
            pieces = messages.setdefault(value.get("messageID", "unbound"), {})
            pieces[str(len(pieces))] = value["text"]
        elif typ in {"message.part.updated", "message.part"}:
            props = value.get("properties", value)
            part = props.get("part", {}) if isinstance(props, dict) else {}
            if part.get("type") == "text" and isinstance(part.get("text"), str):
                pieces = messages.setdefault(part.get("messageID", "unbound"), {})
                pieces[part.get("id", str(len(pieces)))] = part["text"]
    if saw_error:
        raise Failure("OpenCode reported an error event", "runtime")
    return "\n".join(next(reversed(messages.values())).values()) if messages else ""


def structured_response(text, stage):
    """A verdict is one complete bounded JSON response, never a substring match."""
    if not isinstance(text, str) or len(text.encode()) > 65536:
        raise Failure("structured agent response exceeded its bound", "runtime")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    try:
        value = json.loads(text.strip(), object_pairs_hook=unique)
    except (ValueError, RecursionError) as exc:
        raise Failure("agent must return exactly one complete JSON response", "runtime") from exc
    if not isinstance(value, dict):
        raise Failure("structured response must be an object", "runtime")
    if stage == "review":
        findings = value.get("findings")
        if (set(value) != {"verdict", "findings"} or value.get("verdict") not in {"APPROVE", "NEEDS_FIX", "REJECT"}
                or not isinstance(findings, list) or not 1 <= len(findings) <= 64
                or not all(isinstance(x, str) and x.strip() and len(x) <= 4000 for x in findings)):
            raise Failure("review needs one verdict and bounded, nonempty evidence strings", "runtime")
    elif stage == "diagnose":
        if (set(value) != {"diagnosis", "approach"}
                or not all(isinstance(value.get(k), str) and value[k].strip() and len(value[k]) <= 16000
                           for k in ("diagnosis", "approach"))):
            raise Failure("independent diagnosis and a bounded new approach are required", "runtime")
    else:
        raise Failure("unknown structured-response stage", "authority")
    return value


def prompt_for(payload):
    task = payload["task"]
    stage = payload["stage"]
    header = ("Role: %s for one bounded software-delivery task.\nTask: %s\nGoal: %s\n"
              "Requirement/source: %s\nBase SHA: %s\nAllowed write paths: %s\n"
              "Work only in the supplied workspace. Never create branches, commit, push, read secrets, "
              "change policies, launch children, or modify existing acceptance tests. "
              "The controller runs tests and owns publication.\n") % (
                  stage, task["id"], task["goal"], task["source"], payload.get("base"), ", ".join(task["paths"]))
    if stage == "review":
        return header + ("Independent diff-first review. You are not the author. Do not edit anything. "
                         "Inspect DELIVERY_REVIEW.diff and the real implementation and tests. "
                         "Check wiring, security, failure handling and requirement coverage. "
                         "Respond with one JSON object only: {\"verdict\":\"APPROVE\" or \"NEEDS_FIX\","
                         "\"findings\":[\"specific evidence\"]}. Reject incomplete or untested claims. "
                         + ("You are the security evaluator.\n" if payload.get("security_review") else "\n"))
    if stage == "diagnose":
        return header + ("Read only. Diagnose repeated failures, do not change code. Return JSON "
                         "{\"diagnosis\":\"root cause\",\"approach\":\"materially different repair strategy\"}.\n"
                         + "Observed failures:\n" + payload.get("report", ""))
    return header + ("Implement the complete behavior and meaningful tests within the grant. "
                     "Do not return only a proposal. Use read, search and edit tools. "
                     "Read applicable AGENTS.md but this scoped task does not authorize nested delegation. "
                     "Do not weaken existing assertions. Finish with a concise change summary.\n"
                     "Trusted checks the controller will execute:\n" + json.dumps(task["checks"]) +
                     "\nPrevious observed failures or optimizer diagnosis:\n" + payload.get("report", ""))


def agent(payload, cfg, workspace, route, valid, log):
    kind = cfg["runtime"]["kind"]
    prompt = prompt_for(payload)
    editable = payload["stage"] == "implement"
    if kind == "command":
        # Explicit owner-configured adapter, useful for another CLI or a test
        # fixture. It is not an automatic fallback when OpenCode is unavailable.
        values = {"prompt": prompt, "workspace": str(workspace), "agent": route,
                  "stage": payload["stage"], "task": payload["task"]["id"]}
        argv = [values[x[1:-1]] if x.startswith("{") and x.endswith("}") and x[1:-1] in values else x
                for x in cfg["runtime"]["argv"]]
        env = environment(agent=True, forward=cfg["runtime"].get("forward_env", []))
        env["DELIVERY_JOB_SPEC"] = json.dumps(payload)
        result = run(argv, workspace, valid=valid, timeout=cfg["stage_timeout"],
                     max_bytes=cfg["max_output_bytes"], env=env, log=log)
    else:
        exe = cfg["runtime"]["executable"]
        definition = agent_definition(cfg, route)
        # A route selects a model/provider, not a permanent research-only role.
        definition["system"] = (
            "You are a scoped software-delivery worker. Follow the role and exact "
            "output contract in the current job. Implementation roles must edit "
            "the granted source and tests, not merely describe a solution. Review "
            "and diagnosis roles are read-only and must return the requested JSON. "
            "Never delegate, change policy, access secrets or publish Git changes. "
            "The trusted controller executes tests and decides acceptance."
        )
        with overlay(workspace, route, payload["task"]["paths"], editable, definition) as expected:
            inv = inventory(exe, workspace, required=(route,), valid=valid,
                            forward_env=cfg["runtime"].get("forward_env", []))
            actual = inv.get(route)
            if not actual:
                raise Failure("authorized agent is unavailable: " + route, "runtime")
            rules = actual.get("permissions", [])
            if not isinstance(rules, list) or rules[-len(expected):] != expected:
                raise Failure("effective agent permissions do not match the scoped overlay", "authority")
            isolation = ["--standalone"] if cfg["runtime"].get("standalone", False) else []
            result = run([exe, "run", *isolation, "--agent", route, "--format", "json", "--auto", prompt],
                         workspace, valid=valid, timeout=cfg["stage_timeout"],
                         max_bytes=cfg["max_output_bytes"],
                         env=environment(agent=True, forward=cfg["runtime"].get("forward_env", [])), log=log,
                         cancel_signal=signal.SIGINT)
    if result["returncode"]:
        errors = [v.get("error", v) for v in json_values(result["output"])
                  if isinstance(v, dict) and v.get("type") in {"error", "session.error"}]
        detail = json.dumps(errors[-1]) if errors else result["output"][-6000:]
        raise Failure("agent command failed: " + detail, "runtime")
    if result["truncated"]:
        raise Failure("agent output exceeded limit; result cannot be trusted", "runtime")
    text = assistant_text(result["output"], kind)
    if payload["stage"] in {"review", "diagnose"}:
        return dict(structured_response(text, payload["stage"]), evidence=result)
    return {"summary": text[-8000:], "evidence": result}

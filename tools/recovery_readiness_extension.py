#!/usr/bin/env python3
"""Explicit derived readiness diagnostics; never changes frozen evaluators or state."""

import argparse
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import tempfile
import time

STARTUP_LINES = {
    "tests/e2e/native_provider_setup.py": 259,
    "tests/e2e/native_stream_tool.py": 301,
    "tests/e2e/native_utf8_input.py": 105,
    "tests/e2e/native_escape_input.py": 250,
    "tests/e2e/native_resize.py": 184,
    "tests/e2e/native_bracketed_paste.py": 502,
}
MAX_SOURCE = 512 * 1024


def digest(value):
    return hashlib.sha256(value).hexdigest()


def bounded_bytes(path, limit):
    with Path(path).open("rb") as stream:
        value = stream.read(limit + 1)
    if len(value) > limit:
        raise ValueError("input exceeds byte budget")
    return value


def file_digest(path):
    value = hashlib.sha256()
    count = 0
    with Path(path).open("rb") as stream:
        while chunk := stream.read(65536):
            count += len(chunk)
            if count > 512 * 1024 * 1024:
                raise ValueError("artifact exceeds byte budget")
            value.update(chunk)
    return value.hexdigest()


def validate_original_judge(receipt, source_sha):
    if not isinstance(receipt, dict) or receipt.get("schemaVersion") != 2:
        raise ValueError("original judge schema required")
    if (receipt.get("sourceSha") != source_sha or receipt.get("canonicalSha") != source_sha
            or receipt.get("branch") != "main-v2" or receipt.get("cleanWorktree") is not True
            or receipt.get("artifactMode") is not True or receipt.get("status") not in {"PASS", "FAIL"}):
        raise ValueError("original installed judge receipt identity or status mismatch")
    checks = receipt.get("checks")
    if not isinstance(checks, list) or not checks or not isinstance(receipt.get("gateResults"), dict) or not receipt["gateResults"]:
        raise ValueError("original installed judge checks required")
    if any(not isinstance(check, dict) or not isinstance(check.get("name"), str)
           or not check["name"] or type(check.get("exit")) is not int for check in checks):
        raise ValueError("original judge check identity or result invalid")
    if len({check["name"] for check in checks}) != len(checks):
        raise ValueError("duplicate original judge check identity")
    gates = receipt["gateResults"]
    if any(not isinstance(name, str) or not name or not isinstance(gate, dict)
           or gate.get("status") not in {"PASS", "FAIL"} for name, gate in gates.items()):
        raise ValueError("invalid original installed judge gate status")
    # Mirror the frozen judge's status rule (phase1_judge.py:200-205).
    # Unresolved runtime failures affect acceptance eligibility, not this status.
    passed = all(check["exit"] == 0 for check in checks) and all(gate["status"] == "PASS" for gate in gates.values())
    if receipt["status"] != ("PASS" if passed else "FAIL"):
        raise ValueError("original judge overall status contradicts its checks or gates")
    return receipt


def verify_frozen_source(source, path, baseline):
    if not isinstance(baseline, dict) or baseline.get("path") != path:
        raise ValueError("source baseline path mismatch")
    _, report = derive_source(source, path)
    for key in ("original_sha256", "derived_sha256", "downstream_ast_sha256", "assertion_inventory_sha256", "helper_sha256"):
        if not isinstance(baseline.get(key), str) or report[key] != baseline[key]:
            raise ValueError("source, assertion, helper or downstream baseline changed")
    local_modules = {Path(name).stem for name in STARTUP_LINES}
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, ast.Import):
            modules = [alias.name.split(".")[0] for alias in node.names]
        elif isinstance(node, ast.ImportFrom):
            if node.level or not node.module:
                raise ValueError("relative evaluator import refused")
            modules = [node.module.split(".")[0]]
        else:
            modules = []
        if any(name not in sys.stdlib_module_names and name not in local_modules for name in modules):
            raise ValueError("module outside frozen copied graph or standard library")
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in {"__import__", "exec", "eval", "compile"}:
            raise ValueError("dynamic evaluator loader refused")
        if isinstance(node, ast.Attribute) and node.attr in {"meta_path", "path_hooks", "path_importer_cache"}:
            raise ValueError("import interception refused")
    return report


def verify_artifacts(build, binary_sha, library_sha, expected):
    if not isinstance(build, dict) or not isinstance(expected, dict):
        raise ValueError("independent artifact manifest required")
    source = build.get("source_sha") or build.get("sourceSha")
    if source != expected.get("canonicalSha") or not isinstance(source, str) or not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("artifact source differs from independently pinned canonical SHA")
    for key, actual in (("binary_sha256", binary_sha), ("native_library_sha256", library_sha)):
        if not isinstance(actual, str) or not re.fullmatch(r"[0-9a-f]{64}", actual) or actual != expected.get(key) or actual != build.get(key):
            raise ValueError("artifact differs from independently reviewed pin")
    return build


def derive_source(source, path):
    if path not in STARTUP_LINES or len(source.encode()) > MAX_SOURCE:
        raise ValueError("source path or size outside readiness allowlist")
    tree = ast.parse(source)
    if any(isinstance(node, ast.Name) and node.id == "readiness_wait" for node in ast.walk(tree)):
        raise ValueError("readiness helper name collision")
    calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name) and node.func.id == "read_until"]
    selected = [node for node in calls if node.lineno == STARTUP_LINES[path]]
    if len(selected) != 1:
        raise ValueError("missing or ambiguous pinned startup site")
    site = selected[0]
    if (len(site.args) not in (6, 7) or site.keywords
            or not isinstance(site.args[3], ast.Constant) or site.args[3].value != b"OpenCode"):
        raise ValueError("startup invocation differs from authorized shape")
    signature = ast.dump(site, include_attributes=False)
    if sum(ast.dump(node, include_attributes=False) == signature for node in calls) != 1:
        raise ValueError("duplicate startup invocation")
    lines = source.encode().splitlines(keepends=True)
    offset = sum(len(line) for line in lines[:site.func.lineno - 1]) + site.func.col_offset
    original = source.encode()
    if original[offset:offset + len(b"read_until")] != b"read_until":
        raise ValueError("startup source span mismatch")
    changed = original[:offset] + b"readiness_wait" + original[offset + len(b"read_until"):]
    helper = inspect.getsource(readiness_wait).encode()
    # Keep original module docstring and future imports in their legal positions.
    insertion = 0
    for index, node in enumerate(tree.body):
        is_doc = index == 0 and isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str)
        if is_doc or isinstance(node, ast.ImportFrom) and node.module == "__future__":
            insertion = sum(len(line) for line in lines[:node.end_lineno])
        else:
            break
    derived = (changed[:insertion] + b"\n" + helper + b"\n" + changed[insertion:]).decode()
    compile(derived, path, "exec")
    downstream = ast.parse(source)
    original_site = next(node for node in ast.walk(downstream) if isinstance(node, ast.Call)
                         and isinstance(node.func, ast.Name) and node.func.id == "read_until"
                         and node.lineno == STARTUP_LINES[path])
    # Remove just the startup observation from this digest; every other AST
    # node, including fixtures, imports, control flow and assertions, is bound.
    class Downstream(ast.NodeTransformer):
        def visit_Call(self, node):
            if node is original_site:
                return ast.Constant(value="AUTHORIZED_STARTUP_OBSERVATION")
            return self.generic_visit(node)
    downstream = Downstream().visit(downstream)
    inventory = [ast.dump(node, include_attributes=False) for node in ast.walk(tree)
                 if isinstance(node, (ast.Assert, ast.Raise))]
    report = {
        "path": path, "original_sha256": digest(original), "derived_sha256": digest(derived.encode()),
        "downstream_ast_sha256": digest(ast.dump(downstream, include_attributes=False).encode()),
        "helper_sha256": digest(helper),
        "transformations": [{"id": "startup-readiness-call-only-v1", "line": site.lineno,
                             "oldCallable": "read_until", "newCallable": "readiness_wait",
                             "argumentsPreserved": True}],
        "assertion_inventory": inventory,
        "assertion_inventory_sha256": digest(json.dumps(inventory, separators=(",", ":")).encode()),
    }
    return derived, report


def verify_derivation(original, derived, path):
    expected, report = derive_source(original, path)
    if expected != derived:
        raise ValueError("derived source differs outside the exact approved transformation")
    # Independently reverse the actual derived tree and compare every original
    # AST node, rather than relying only on the source-generation code path.
    restored = ast.parse(derived)
    helper_nodes = [node for node in restored.body if isinstance(node, ast.FunctionDef) and node.name == "readiness_wait"]
    if len(helper_nodes) != 1:
        raise ValueError("derived helper identity mismatch")
    restored.body.remove(helper_nodes[0])
    calls = [node for node in ast.walk(restored) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name) and node.func.id == "readiness_wait"]
    if len(calls) != 1:
        raise ValueError("derived startup callable is ambiguous")
    calls[0].func.id = "read_until"
    if ast.dump(restored, include_attributes=False) != ast.dump(ast.parse(original), include_attributes=False):
        raise ValueError("original import, assertion or downstream AST changed")
    return report


def verify_identity(expected, actual):
    required = {"canonicalSha", "branch", "clean", "protectedBlobs", "policySha256",
                "guardSourceSha256", "refHookSha256", "guardGreen"}
    for value in (expected, actual):
        if not isinstance(value, dict) or not required <= value.keys():
            raise ValueError("incomplete canonical identity")
        if value["branch"] != "main-v2" or value["clean"] is not True or value["guardGreen"] is not True:
            raise ValueError("canonical clean guard-green identity required")
        if not re.fullmatch(r"[0-9a-f]{40}", value["canonicalSha"]):
            raise ValueError("invalid canonical SHA")
        if not isinstance(value["protectedBlobs"], dict) or not value["protectedBlobs"]:
            raise ValueError("missing protected blob inventory")
        if any(not re.fullmatch(r"[0-9a-f]{40}", blob) for blob in value["protectedBlobs"].values()):
            raise ValueError("invalid protected blob identity")
        if any(not re.fullmatch(r"[0-9a-f]{64}", value[key]) for key in
               ("policySha256", "guardSourceSha256", "refHookSha256")):
            raise ValueError("invalid guard/policy digest")
    if expected != actual:
        raise ValueError("canonical, guard or frozen policy identity changed")


def readiness_wait(fd, deadline, buf, needle, start, screen, predicate=None):
    """Observe only complete native updates; all original startup predicates remain."""
    import os
    import select
    import time

    if needle != b"OpenCode":
        raise ValueError("only the explicitly superseded startup branding marker is allowed")
    opening, closing = b"\x1b[?2026h", b"\x1b[?2026l"
    pending = bytearray()
    limit = 256 * 1024
    if screen is None or not isinstance(start, int) or start < 0 or start > len(buf):
        raise ValueError("bounded current screen and valid action boundary required")
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], min(.05, max(0, deadline - time.monotonic())))
        if not ready:
            continue
        chunk = os.read(fd, min(65536, limit - len(buf)))
        if not chunk:
            raise AssertionError("EOF before completed usable native readiness frame")
        buf.extend(chunk)
        if len(buf) >= limit:
            raise AssertionError("readiness PTY output bound reached")
        pending.extend(chunk)
        while True:
            begin = pending.find(opening)
            if begin < 0:
                pending[:] = pending[-(len(opening) - 1):]
                break
            end = pending.find(closing, begin + len(opening))
            if end < 0:
                del pending[:begin]
                break
            screen.feed(bytes(pending[begin + len(opening):end]))
            del pending[:end + len(closing)]
            usable = screen.contains("Ask anything") or screen.contains("Connect a provider")
            if len(buf) > start and usable and (predicate is None or predicate()):
                return
    raise AssertionError("timeout before completed usable native readiness frame")


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def identity(root, expected=None):
    common = Path(git(root, "rev-parse", "--git-common-dir").decode().strip())
    if not common.is_absolute():
        common = (root / common).resolve()
    policy_bytes = (common / "recovery-state/policy.json").read_bytes()
    policy = json.loads(policy_bytes)
    blobs = policy["protectedEvaluatorBlobs"]
    sha = git(root, "rev-parse", "main-v2").decode().strip()
    for path, blob in blobs.items():
        if git(root, "rev-parse", f"{sha}:{path}").decode().strip() != blob:
            raise ValueError("protected canonical source drift")
    guard = subprocess.run([sys.executable, "tools/recovery_guard.py"], cwd=root,
                           capture_output=True, text=True, timeout=30)
    result = {"canonicalSha": sha, "branch": git(root, "branch", "--show-current").decode().strip(),
              "clean": not bool(git(root, "status", "--porcelain")), "protectedBlobs": blobs,
              "policySha256": digest(policy_bytes), "guardSourceSha256": digest((root / "tools/recovery_guard.py").read_bytes()),
              "refHookSha256": digest((root / "tools/recovery_ref_hook.py").read_bytes()), "guardGreen": guard.returncode == 0}
    verify_identity(result, result)
    if expected is not None:
        verify_identity(expected, result)
    return result


def execute_probe(command, cwd, env, log_path, timeout=180):
    """Retain the unreaped leader until every destructive group signal is done.

    Even an exited leader retains its PID while unreaped. Never poll(), wait(),
    communicate(), or Popen.send_signal() before group cleanup: those may reap
    it and permit PID/PGID reuse. Only read-only probes occur after the wait.
    """
    child = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, start_new_session=True)
    deadline = time.monotonic() + timeout
    captured = 0
    failure = None
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ)
    try:
        with log_path.open("wb") as log:
            while selector.get_map():
                if time.monotonic() >= deadline:
                    raise TimeoutError("derived probe exceeded execution deadline")
                for key, _ in selector.select(.1):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    captured += len(chunk)
                    if captured > 1024 * 1024:
                        raise ValueError("derived probe output exceeds 1 MiB")
                    log.write(chunk)
    except BaseException as error:
        failure = error
        raise
    finally:
        selector.close()
        if failure is not None:
            # Raw group signaling does not implicitly poll/reap the leader.
            try:
                os.killpg(child.pid, signal.SIGINT)
            except (ProcessLookupError, PermissionError):
                pass
            os.set_blocking(child.stdout.fileno(), False)
            cleanup_deadline = time.monotonic() + 15
            while time.monotonic() < cleanup_deadline:
                try:
                    if not os.read(child.stdout.fileno(), 65536):
                        break
                except BlockingIOError:
                    pass
                time.sleep(.01)
        else:
            # EOF can precede the interpreter's final exit; allow it to finish
            # naturally without relinquishing the PID reservation by waiting.
            time.sleep(.1)
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError):
            pass
        else:
            time.sleep(.1)
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        # All destructive signals precede this single reaping operation.
        child.wait(timeout=3)
        child.stdout.close()
        group_deadline = time.monotonic() + 2
        # macOS can deny signals to a zombie-only group. Such a denial is not
        # success: the owned leader must reap and the read-only group probe
        # below must subsequently prove disappearance, or cleanup fails.
        while time.monotonic() < group_deadline:
            try:
                os.killpg(child.pid, 0)
            except ProcessLookupError:
                break
            time.sleep(.02)
        else:
            raise RuntimeError("process group remains after cleanup; no post-reap signaling permitted")
    return child.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", required=True, choices=[Path(path).stem for path in STARTUP_LINES])
    parser.add_argument("--binary", required=True)
    parser.add_argument("--native-library", required=True)
    parser.add_argument("--build-json", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--expectations", required=True, help="independently reviewed identity/source/artifact manifest")
    parser.add_argument("--expectations-sha256", required=True, help="reviewed manifest digest supplied by the controller")
    parser.add_argument("--original-judge", required=True)
    parser.add_argument("--original-judge-sha256", required=True)
    parser.add_argument("--derive-only", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    expectations_bytes = bounded_bytes(args.expectations, MAX_SOURCE)
    if len(expectations_bytes) > MAX_SOURCE or digest(expectations_bytes) != args.expectations_sha256:
        parser.error("reviewed expectations digest or size mismatch")
    expectations = json.loads(expectations_bytes)
    if expectations.get("toolSha256") != file_digest(__file__):
        parser.error("executed tool differs from independently reviewed manifest")
    before = identity(root, expectations["identity"])
    if set(expectations.get("sources", {})) != set(STARTUP_LINES):
        parser.error("source baseline must contain exactly the six reviewed modules")
    if expectations.get("artifacts", {}).get("canonicalSha") != before["canonicalSha"]:
        parser.error("artifact and identity manifests disagree")
    judge_bytes = bounded_bytes(args.original_judge, 2 * 1024 * 1024)
    if len(judge_bytes) > 2 * 1024 * 1024 or digest(judge_bytes) != args.original_judge_sha256:
        parser.error("original judge receipt digest or size mismatch")
    original_judge = validate_original_judge(json.loads(judge_bytes), before["canonicalSha"])
    common = Path(git(root, "rev-parse", "--git-common-dir").decode().strip())
    if not common.is_absolute():
        common = (root / common).resolve()
    output = Path(args.output)
    evidence_root = (common / "recovery-evidence" / before["canonicalSha"]).resolve()
    if not output.is_absolute() or output.exists() or evidence_root not in output.resolve().parents:
        parser.error("--output must be a new directory under current-SHA recovery evidence")
    output.mkdir(parents=True)
    graph = output / "derived"
    graph.mkdir()
    records = []
    for path in STARTUP_LINES:
        blob = git(root, "rev-parse", f'{before["canonicalSha"]}:{path}').decode().strip()
        if before["protectedBlobs"].get(path) != blob:
            raise ValueError("source missing from frozen allowlist")
        source = git(root, "show", f'{before["canonicalSha"]}:{path}').decode()
        verify_frozen_source(source, path, expectations["sources"][path])
        derived, report = derive_source(source, path)
        verify_derivation(source, derived, path)
        (graph / Path(path).name).write_text(derived)
        records.append(dict(report, gitBlob=blob))
    build_path = Path(args.build_json).resolve()
    build_bytes = bounded_bytes(build_path, MAX_SOURCE)
    if digest(build_bytes) != expectations["artifacts"].get("build_json_sha256"):
        raise ValueError("build receipt differs from independently reviewed manifest")
    build = json.loads(build_bytes)
    binary, library = Path(args.binary).resolve(), Path(args.native_library).resolve()
    verify_artifacts(build, file_digest(binary), file_digest(library), expectations["artifacts"])
    command = [sys.executable, str(graph / (args.scenario + ".py")), "--binary", str(binary),
               "--native-library", str(library), "--build-json", str(build_path),
               "--artifact-dir", str(output / "artifacts")]
    result = {"kind": "derived-readiness-diagnostic-only", "sourceSha": before["canonicalSha"],
              "identitiesBefore": before, "sources": records, "argv": command,
              "originalJudgeStatus": original_judge["status"],
              "originalJudgeReceipt": {"path": str(Path(args.original_judge).resolve()), "sha256": digest(judge_bytes)},
              "reviewedExpectations": {"path": str(Path(args.expectations).resolve()), "sha256": digest(expectations_bytes)},
              "acceptance": False,
              "aggregateReleaseAccepted": False, "status": "DERIVED_ONLY", "exit": None}
    try:
        if not args.derive_only:
            # A short, disposable root is required by frozen Unix socket bounds.
            with tempfile.TemporaryDirectory(prefix="oc2-rdy-", dir="/private/tmp" if sys.platform == "darwin" else None) as temporary:
                env = {"HOME": temporary, "TMPDIR": temporary, "PATH": "/usr/bin:/bin", "LANG": "en_US.UTF-8"}
                result["cwd"] = temporary
                result["environment"] = env
                for record in records:
                    if digest((graph / Path(record["path"]).name).read_bytes()) != record["derived_sha256"]:
                        raise ValueError("derived module changed before execution")
                verify_identity(before, identity(root))
                exit_code = execute_probe(command, temporary, env, output / "execution.log")
                result["exit"] = exit_code
                result["status"] = "DIAGNOSTIC_PASS" if exit_code == 0 else "DIAGNOSTIC_FAIL"
    except Exception as error:
        result.update(status="DIAGNOSTIC_FAIL", error=str(error), exit=1)
    finally:
        try:
            after = identity(root)
            verify_identity(before, after)
            result["identitiesAfter"] = after
        except Exception as error:
            result.update(status="IDENTITY_CHECK_FAILED", error=str(error), exit=1)
        (output / "diagnostic.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "evidence": str(output / "diagnostic.json"), "acceptance": False}))
    return 0 if result["status"] in {"DERIVED_ONLY", "DIAGNOSTIC_PASS"} else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""R1 release-gate driver: one reproducible exact-SHA contract for
Mac native installed-binary gates, Web journey gates, and Ubuntu arm64
tool gates. Reuses the retained verify-native-utf8.py runner (verbatim copy
under release/, hash-pinned in the manifest) and the existing frozen e2e
contracts under tests/e2e. Does not re-enact product behavior: Web gates run
the real Rust router/auth daemon through tests/e2e/web_tool_journey_fixture.py
and Mac gates run the frozen paste-reset contract
tests/e2e/native_bracketed_paste.py against the installed binary.

Safety posture (docs/SECURITY.md): disposable HOME/XDG/TMP roots, no
credentials in gate environments, no user OpenCode DB, bounded output, real
subprocess exit codes captured without shell-pipeline/tail masking, serialized
heavy gates, single-use (fresh) evidence directories so stale cross-SHA
artifacts/receipts can never be silently reused.

Modes:
  --check-contract  static validation only (manifest schema, hashes, frozen
                    tests, runner pin). Safe on any host; no heavy gates.
   --run             execute the platform gate list serially (concrete manifests only).

Exit codes: 0 all gates passed, 1 contract violation or any gate failure,
2 usage error. Never reports release readiness, only per-gate facts.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import select
import shlex
import signal
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import receipts  # noqa: E402

DRIVER = pathlib.Path(__file__).resolve()
RELEASE_DIR = DRIVER.parent


class GateError(RuntimeError):
    pass


def load_manifest(path: pathlib.Path) -> dict:
    manifest = receipts.load_json(path)
    if manifest.get("source_sha") == receipts.TEMPLATE_SHA:
        raise GateError("stale/template sentinel passed as --manifest; use --manifest-template")
    receipts.validate_manifest(manifest["artifact_contract"])
    return manifest


def load_template(path: pathlib.Path, source_sha: str, evidence: pathlib.Path) -> tuple[dict, pathlib.Path]:
    concrete_path = evidence / "materialized-manifest.json"
    try:
        template = receipts.load_json(path)
        manifest = receipts.materialize_template(template, source_sha, concrete_path)
    except (receipts.ReceiptError, KeyError) as exc:
        raise GateError(f"template materialization failed: {exc}") from exc
    return manifest, concrete_path


def check_contract(args, manifest: dict, root: pathlib.Path) -> None:
    """Static, non-heavy validation. Raises GateError on first violation."""
    if not SHA_RE.fullmatch(args.source_sha):
        raise GateError("source sha must be 40 hex characters")
    contract = manifest["artifact_contract"]
    if contract["source_sha"] != args.source_sha:
        raise GateError(
            "stale cross-SHA reuse: manifest artifact_contract source_sha "
            f"{contract['source_sha']} != requested {args.source_sha}"
        )
    platforms = manifest.get("platforms", {})
    for name in args.platforms:
        if name not in platforms:
            raise GateError(f"platform {name} absent from manifest")
        platform = platforms[name]
        pin = platform.get("runner_sha256")
        if pin is None and name == "ubuntu":
            raise GateError(f"platform {name} must pin runner_sha256 (retained verify-native-utf8.py)")
        if pin is not None and pin != contract["runner_sha256"]:
            raise GateError(
                f"platform {name} runner pin does not match artifact contract runner"
            )
    for rel, expected in contract["frozen_tests"].items():
        path = root / rel
        if not path.is_file():
            raise GateError(f"frozen contract test absent at {path}")
        if receipts.sha256_file(path) != expected:
            raise GateError(
                f"frozen contract test modified: {rel} (expected {expected})"
            )
    runner = root / contract["runner_path"]
    if not runner.is_file():
        raise GateError(f"retained runner absent: {runner}")
    actual = receipts.sha256_file(runner)
    if actual != contract["runner_sha256"]:
        raise GateError(f"retained runner hash mismatch: {actual} != {contract['runner_sha256']}")
    if args.artifact_dir is not None:
        artifact_dir = pathlib.Path(args.artifact_dir).resolve()
        build = receipts.load_json(artifact_dir / "build.json")
        receipts.validate_manifest(build)
        if build["source_sha"] != args.source_sha:
            raise GateError(
                f"stale artifact: build.json source_sha {build['source_sha']} "
                f"!= required {args.source_sha}"
            )
        receipts.validate_artifact_hashes(build, artifact_dir)
        receipts.validate_frozen_tests(build, root)
        args.artifact_sha = build["source_sha"]


def run_gate(label: str, command: list[str], cwd: pathlib.Path, env: dict,
             timeout: int, log: pathlib.Path, output_dir: pathlib.Path) -> dict:
    """Bounded serial execution with real exit code capture.

    No shell, no pipeline: exit code is the child's own. Output is read with
    a byte budget and killed process-group on timeout/overflow, mirroring the
    retained verify-native-utf8.py runner so evidence shape stays identical.
    """
    start = time.monotonic()
    failure = None
    total = 0
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("wb") as out:
        child = subprocess.Popen(
            command, env=env, cwd=str(cwd), stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            while True:
                left = timeout - (time.monotonic() - start)
                if left <= 0:
                    failure = "timeout"
                    break
                ready, _, _ = select.select([child.stdout], [], [], min(left, 1))
                if not ready:
                    continue
                block = os.read(child.stdout.fileno(), 65536)
                if not block:
                    break
                total += len(block)
                if total > 32 * 1024 * 1024:
                    failure = "output budget"
                    break
                out.write(block)
            if failure:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    code = child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    code = child.wait(timeout=5)
            else:
                code = child.wait(timeout=5)
        finally:
            child.stdout.close()
    record = {
        "source_sha": env.get("R1_SOURCE_SHA", ""),
        "label": label,
        "command": shlex.join(command),
        "cwd": str(cwd),
        "exit": code,
        "limit_failure": failure,
        "seconds": round(time.monotonic() - start, 2),
        "log_path": str(log),
        "log_sha256": receipts.sha256_file(log),
    }
    commands = output_dir / "commands.json"
    prior = json.loads(commands.read_text()) if commands.exists() else []
    prior.append(record)
    commands.write_text(json.dumps(prior, indent=2) + "\n")
    return record


SHA_RE = re.compile(r"[0-9a-f]{40}")


def run_platforms(args, manifest: dict, root: pathlib.Path, evidence: pathlib.Path) -> int:
    contract = manifest["artifact_contract"]
    worst = 0
    for name in args.platforms:
        platform = manifest["platforms"][name]
        home = evidence / name
        env = receipts.disposable_env(home, path=platform.get("path", "/usr/bin:/bin"),
                                      strip=tuple(platform.get("strip_env", ())))
        env["R1_SOURCE_SHA"] = args.source_sha
        env = receipts.scrub_inherited_env(env)
        platform_gates = list(platform["gates"])
        if name == "ubuntu" and args.tool_gates:
            extra = platform.get("tool_gates")
            if not extra:
                raise GateError("--tool-gates requested but manifest ubuntu tool_gates absent")
            platform_gates += list(extra)
        gates = []
        def expand(token: str) -> str:
            return (token.replace("{root}", str(root))
                        .replace("{evidence}", str(evidence / name))
                        .replace("{python}", sys.executable)
                        .replace("{artifact}", str(args.artifact_dir or ""))
                        .replace("{sha}", args.source_sha))

        for gate in platform_gates:
            command = [expand(part) for part in gate["command"]]
            gate_env = {key: expand(str(value)) for key, value in (gate.get("env") or {}).items()}
            gate_env.update(env)
            record = run_gate(
                f"{name}:{gate['label']}", command, root, gate_env,
                int(gate.get("timeout_seconds", 90)),
                evidence / name / "logs" / (gate["label"] + ".log"),
                evidence / name,
            )
            gates.append(record)
            print(f"{record['label']} exit={record['exit']} {record['seconds']}s", flush=True)
            if record["exit"] or record["limit_failure"]:
                worst = worst or record["exit"] or 1
        receipts_obj = {"source_sha": args.source_sha, "platform": name,
                        "runner_sha256": contract["runner_sha256"], "gates": gates}
        receipts.write_json_atomic(evidence / name / "receipts.json", receipts_obj)
        violations = receipts.validate_receipts(receipts_obj, args.source_sha, name)
        if violations:
            raise GateError("own receipts invalid: " + "; ".join(violations))
    receipts.write_json_atomic(evidence / "summary.json", {
        "source_sha": args.source_sha, "platforms": list(args.platforms),
        "driver_sha256": receipts.sha256_file(DRIVER),
        "manifest_sha256": receipts.sha256_file(pathlib.Path(args.manifest)),
        "artifact_dir": str(args.artifact_dir) if args.artifact_dir else None,
        "artifact_source_sha": getattr(args, "artifact_sha", None),
        "worst_exit": worst,
        "release_readiness_claim": False,
    })
    return worst


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-sha", required=True)
    group_manifest = parser.add_mutually_exclusive_group(required=True)
    group_manifest.add_argument("--manifest", help="concrete manifest; required for --run")
    group_manifest.add_argument("--manifest-template", help="unmaterialized template; --check-contract only")
    parser.add_argument("--repo-root", default=str(RELEASE_DIR.parent))
    parser.add_argument("--evidence-dir", required=True)
    parser.add_argument("--platform", action="append", dest="platforms",
                        choices=["mac", "web", "ubuntu"], required=True)
    parser.add_argument("--artifact-dir", help="installed artifact dir with build.json (validated when given)")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check-contract", action="store_true",
                      help="static validation only; no heavy platform gates")
    mode.add_argument("--run", action="store_true", help="execute gate list serially")
    parser.add_argument("--tool-gates", action="store_true",
                        help="ubuntu only: retain verify-native-utf8.py --tool-gates evidence requirement")
    args = parser.parse_args()
    if args.tool_gates and "ubuntu" not in args.platforms:
        parser.error("--tool-gates applies only when --platform ubuntu is selected")
    root = pathlib.Path(args.repo_root).resolve()
    manifest_path = pathlib.Path(args.manifest or args.manifest_template)
    evidence = pathlib.Path(args.evidence_dir).resolve()
    try:
        if not SHA_RE.fullmatch(args.source_sha):
            raise GateError("source sha must be 40 hex characters")
        receipts.reject_nonempty_dir(evidence)
        evidence.mkdir(parents=True, exist_ok=True)
        if args.manifest_template:
            if args.run:
                raise GateError("--manifest-template is allowed only with --check-contract")
            manifest, manifest_path = load_template(pathlib.Path(args.manifest_template), args.source_sha, evidence)
        else:
            manifest = load_manifest(manifest_path)
        if not manifest.get("source_sha") or manifest["source_sha"] != args.source_sha:
            raise GateError(
                f"manifest pinned to {manifest.get('source_sha')!r}, not {args.source_sha!r}: "
                "stale manifest reuse rejected"
            )
        check_contract(args, manifest, root)
        if args.tool_gates:
            ubuntu = manifest["platforms"].get("ubuntu", {})
            if not ubuntu.get("tool_gates"):
                raise GateError("--tool-gates requested but manifest ubuntu tool_gates absent")
        if args.run and args.artifact_dir is None:
            raise GateError("--run requires --artifact-dir")
        if args.check_contract:
            receipts.write_json_atomic(evidence / "materialized-manifest.json", manifest)
            print(f"contract OK for {args.source_sha} platforms={args.platforms}", flush=True)
            return 0
        return run_platforms(args, manifest, root, evidence)
    except (GateError, receipts.ReceiptError, KeyError) as exc:
        print(f"GATE CONTRACT FAIL: {exc}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())

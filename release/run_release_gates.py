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


def load_manifest(path: pathlib.Path, source_sha: str) -> dict:
    manifest = receipts.load_json(path)
    if manifest.get("source_sha") == receipts.TEMPLATE_SHA:
        raise GateError("stale/template sentinel passed as --manifest; use --manifest-template")
    receipts.validate_materialized_manifest(manifest, source_sha)
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
        if path.is_symlink() or not path.is_file():
            raise GateError(f"frozen contract test absent at {path}")
        if receipts.sha256_file(path) != expected:
            raise GateError(
                f"frozen contract test modified: {rel} (expected {expected})"
            )
    runner = root / contract["runner_path"]
    try:
        receipts.validate_runner(contract, runner)
    except receipts.ReceiptError as exc:
        raise GateError(str(exc)) from exc
    if args.artifact_dir is not None:
        artifact_dir = pathlib.Path(args.artifact_dir)
        build = receipts.load_json(artifact_dir / "build.json")
        receipts.validate_manifest(build)
        if build["source_sha"] != args.source_sha:
            raise GateError(
                f"stale artifact: build.json source_sha {build['source_sha']} "
                f"!= required {args.source_sha}"
            )
        if len(args.platforms) != 1:
            raise GateError("--artifact-dir requires exactly one selected platform")
        selected = args.platforms[0]
        expected_build_platform = "mac" if selected == "mac" else "linux"
        if build.get("platform") != expected_build_platform:
            raise GateError(
                f"artifact build platform {build.get('platform')!r} does not match selected {selected!r}"
            )
        if build.get("runner_sha256") != contract["runner_sha256"]:
            raise GateError("artifact runner_sha256 differs from concrete manifest")
        if build.get("frozen_tests") != contract["frozen_tests"]:
            raise GateError("artifact frozen_tests differs from concrete manifest")
        if build.get("target") != contract.get("target"):
            raise GateError("artifact target differs from concrete manifest")
        receipts.validate_artifact_hashes(build, artifact_dir, platform=selected)
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
        macros = {
            "{root}": str(root), "{evidence}": str(evidence / name),
            "{python}": sys.executable, "{artifact}": str(args.artifact_dir or ""),
            "{sha}": args.source_sha, "{archive_sha256}": contract["archive_sha256"],
            "{node}": args.node or "", "{playwright_preload}": args.playwright_preload or "",
            "{browser_cache}": args.browser_cache or "", "{mac_cargo}": args.mac_cargo or "",
            "{mac_rustc}": args.mac_rustc or "", "{mac_rustdoc}": args.mac_rustdoc or "",
            "{mac_cargo_home}": args.mac_cargo_home or "",
            "{mac_home}": str(evidence / "mac" / "cargo-home"),
            "{mac_target_dir}": str(evidence / "mac" / "cargo-target"),
            "{owned_short_tmp}": str(evidence / "mac" / "tmp"),
        }
        def expand(token: str) -> str:
            result = token
            for macro, value in macros.items():
                result = result.replace(macro, value)
            if "{" in result or "}" in result:
                raise GateError(f"unknown or unresolved gate macro: {token}")
            return result

        for gate in platform_gates:
            command = [expand(part) for part in gate["command"]]
            gate_env = dict(env)
            gate_env.update({key: expand(str(value)) for key, value in (gate.get("env") or {}).items()})
            gate_env = receipts.scrub_inherited_env(gate_env)
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
    parser.add_argument("--trusted-provenance", help="controller-supplied provenance receipt (external to artifact)")
    parser.add_argument("--expected-receipt-digest", help="controller-supplied SHA-256 of trusted provenance")
    parser.add_argument("--node", help="absolute existing Node executable for the web browser gate")
    parser.add_argument("--playwright-preload", help="absolute owned Playwright preload file")
    parser.add_argument("--browser-cache", help="absolute owned Playwright browser cache directory")
    parser.add_argument("--mac-cargo", help="absolute existing Cargo executable for the Mac loader gate")
    parser.add_argument("--mac-rustc", help="absolute existing rustc executable for the Mac loader gate")
    parser.add_argument("--mac-rustdoc", help="absolute existing rustdoc executable for the Mac loader gate")
    parser.add_argument("--mac-cargo-home", help="absolute owned Cargo cache directory for the Mac loader gate")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check-contract", action="store_true",
                      help="static validation only; no heavy platform gates")
    mode.add_argument("--run", action="store_true", help="execute gate list serially")
    parser.add_argument("--tool-gates", action="store_true",
                        help="ubuntu only: retain verify-native-utf8.py --tool-gates evidence requirement")
    args = parser.parse_args()
    if args.tool_gates and "ubuntu" not in args.platforms:
        parser.error("--tool-gates applies only when --platform ubuntu is selected")
    if args.run and len(args.platforms) != 1:
        parser.error("--run accepts exactly one platform")
    # Preserve the caller-supplied root identity until validate_git_source has
    # rejected symlink aliases.  Resolving here would erase that security
    # boundary and turn an alias into an apparently real repository root.
    root = pathlib.Path(args.repo_root)
    manifest_path = pathlib.Path(args.manifest or args.manifest_template)
    evidence = pathlib.Path(args.evidence_dir).resolve()
    try:
        if not SHA_RE.fullmatch(args.source_sha):
            raise GateError("source sha must be 40 hex characters")
        receipts.reject_nonempty_dir(evidence)
        evidence.mkdir(parents=True, exist_ok=True)
        if args.run:
            if not args.trusted_provenance or not args.expected_receipt_digest:
                raise GateError("--run requires explicit trusted provenance and expected receipt digest")
            tree = receipts.validate_git_source(root, args.source_sha)
        if args.manifest_template:
            if args.run:
                raise GateError("--manifest-template is allowed only with --check-contract")
            manifest, manifest_path = load_template(pathlib.Path(args.manifest_template), args.source_sha, evidence)
        else:
            manifest = load_manifest(manifest_path, args.source_sha)
        if args.run:
            template_path = pathlib.Path(args.repo_root).resolve() / "release" / "release-gates.manifest.template.json"
            template = receipts.load_json(template_path)
            receipts.validate_trusted_layout(manifest, template)
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
        if args.run:
            selected = args.platforms[0]
            try:
                trusted = receipts.validate_trusted_provenance(
                    pathlib.Path(args.trusted_provenance), args.expected_receipt_digest,
                    args.source_sha, tree, selected, manifest,
                    pathlib.Path(args.artifact_dir),
                )
                receipts.validate_artifact_provenance_binding(
                    pathlib.Path(args.artifact_dir), selected, trusted
                )
            except receipts.ReceiptError as exc:
                raise GateError(str(exc)) from exc
            required = " ".join(json.dumps(g) for g in manifest["platforms"][selected].get("gates", []))
            if selected == "web" and "{node}" in required:
                for option, value in (("--node", args.node), ("--playwright-preload", args.playwright_preload),
                                      ("--browser-cache", args.browser_cache)):
                    if not value or not pathlib.Path(value).is_absolute():
                        raise GateError(f"{option} must be an absolute existing capability")
                if not pathlib.Path(args.node).is_file() or not os.access(args.node, os.X_OK):
                    raise GateError("--node must be an existing executable")
                if not pathlib.Path(args.playwright_preload).is_file():
                    raise GateError("--playwright-preload must be an existing file")
                if not pathlib.Path(args.browser_cache).is_dir():
                    raise GateError("--browser-cache must be an existing directory")
            if selected == "mac" and "{mac_cargo}" in required:
                for option, value in (("--mac-cargo", args.mac_cargo), ("--mac-rustc", args.mac_rustc),
                                      ("--mac-rustdoc", args.mac_rustdoc), ("--mac-cargo-home", args.mac_cargo_home)):
                    if not value or not pathlib.Path(value).is_absolute():
                        raise GateError(f"{option} must be an absolute existing capability")
                for option, value in (("--mac-cargo", args.mac_cargo), ("--mac-rustc", args.mac_rustc),
                                      ("--mac-rustdoc", args.mac_rustdoc)):
                    path = pathlib.Path(value)
                    if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
                        raise GateError(f"{option} must be an existing non-symlink executable")
                cache = pathlib.Path(args.mac_cargo_home)
                if cache.is_symlink() or not cache.is_dir():
                    raise GateError("--mac-cargo-home must be an existing non-symlink directory")
        if args.check_contract:
            # materialize_template already emitted this exact evidence once.
            if not (evidence / "materialized-manifest.json").is_file():
                receipts.write_json_atomic_exclusive(evidence / "materialized-manifest.json", manifest)
            print(f"contract OK for {args.source_sha} platforms={args.platforms}", flush=True)
            return 0
        return run_platforms(args, manifest, root, evidence)
    except (GateError, receipts.ReceiptError, KeyError) as exc:
        print(f"GATE CONTRACT FAIL: {exc}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())

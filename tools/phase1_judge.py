#!/usr/bin/env python3
"""Phase-1 judge entrypoint for Recovery Mode."""

from __future__ import annotations

import argparse, fcntl, json, os, pathlib, subprocess, sys, tempfile, time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def common_git_dir():
    raw = pathlib.Path(git("rev-parse", "--git-common-dir"))
    if not raw.is_absolute():
        raw = (ROOT / raw).resolve()
    return raw


def run(name, argv, env=None):
    started = time.monotonic()
    proc = subprocess.run(argv, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env)
    return {"name": name, "argv": argv, "exit": proc.returncode, "durationSeconds": round(time.monotonic()-started, 3), "output": proc.stdout[-20000:]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary")
    parser.add_argument("--native-library")
    parser.add_argument("--build-json")
    parser.add_argument("--artifact-evidence-dir")
    parser.add_argument("--output")
    parser.add_argument("--observe-state", action="store_true")
    parser.add_argument("--sync-state", action="store_true")
    args = parser.parse_args()
    artifact_values = [args.binary, args.native_library, args.build_json, args.artifact_evidence_dir]
    if any(artifact_values) and not all(artifact_values):
        parser.error("artifact mode requires --binary --native-library --build-json --artifact-evidence-dir together")

    heavy_lock = common_git_dir() / "recovery-state" / "heavy-validation.lock"
    heavy_lock.parent.mkdir(parents=True, exist_ok=True)
    heavy_handle = heavy_lock.open("a+")
    try:
        fcntl.flock(heavy_handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        parser.error("another centralized heavy validation is already running")

    py = sys.executable
    canonical_sha = git("rev-parse", "main-v2")
    head_sha = git("rev-parse", "HEAD")
    branch = git("branch", "--show-current")
    clean = not bool(git("status", "--porcelain"))
    checks = [
        run("recovery_guard", [py, "tools/recovery_guard.py"]),
        run("convergence_structure", [py, "tools/convergence_gate.py", "--structure-only"]),
        run("native_provider_fixture_self_check", [py, "tests/e2e/native_provider_setup.py", "--artifact-dir", tempfile.mkdtemp(prefix="oc2-p1-provider-self-"), "--self-check"]),
        run("native_stream_tool_fixture_self_check", [py, "tests/e2e/native_stream_tool.py", "--self-check"]),
    ]
    env = dict(os.environ); env.setdefault("CARGO_BUILD_JOBS", "2"); env.setdefault("RUST_TEST_THREADS", "2")
    checks.append(run("cargo_check_workspace_locked", ["cargo", "check", "--workspace", "--locked"], env))

    if all(artifact_values):
        current_sha = canonical_sha
        if branch != "main-v2" or head_sha != canonical_sha or not clean:
            parser.error(
                "artifact acceptance mode must run from a clean main-v2 worktree at the exact canonical SHA"
            )
        build_data = json.loads(pathlib.Path(args.build_json).read_text(encoding="utf-8"))
        artifact_sha = build_data.get("source_sha") or build_data.get("sourceSha") or build_data.get("git_sha")
        if artifact_sha != current_sha:
            parser.error(
                f"artifact source SHA {artifact_sha!r} does not equal current main-v2 {current_sha}"
            )
        if build_data.get("profile") != "release" or build_data.get("native") is not True:
            parser.error("artifact acceptance requires a native release build receipt")
        if build_data.get("target") not in (None, "installed-binary"):
            parser.error("artifact target must be installed-binary when target is declared")
        if build_data.get("platform") not in (None, "mac"):
            parser.error("Phase-1 macOS acceptance cannot use a non-mac artifact receipt")
        evidence = pathlib.Path(args.artifact_evidence_dir).resolve()
        if evidence.exists() and any(evidence.iterdir()):
            parser.error("--artifact-evidence-dir must be new or empty")
        evidence.mkdir(parents=True, exist_ok=True)
        common = ["--binary", str(pathlib.Path(args.binary).resolve()), "--native-library", str(pathlib.Path(args.native_library).resolve()), "--build-json", str(pathlib.Path(args.build_json).resolve())]
        checks.append(run("native_provider_installed", [py, "tests/e2e/native_provider_setup.py", *common, "--artifact-dir", str(evidence/"provider")]))
        checks.append(run("native_stream_tool_installed", [py, "tests/e2e/native_stream_tool.py", *common, "--artifact-dir", str(evidence/"stream-tool")]))
        checks.append(run("native_utf8_installed", [py, "tests/e2e/native_utf8_input.py", *common, "--artifact-dir", str(evidence/"utf8")]))
        checks.append(run("native_escape_installed", [py, "tests/e2e/native_escape_input.py", *common, "--artifact-dir", str(evidence/"escape")]))
        checks.append(run("native_resize_installed", [py, "tests/e2e/native_resize.py", *common, "--artifact-dir", str(evidence/"resize")]))
        checks.append(run("native_paste_reset_installed", [py, "tests/e2e/native_bracketed_paste.py", *common, "--artifact-dir", str(evidence/"paste-reset")]))

        installed_env = dict(os.environ)
        installed_env.update(
            {
                "OC2_NATIVE_BINARY": str(pathlib.Path(args.binary).resolve()),
                "OC2_NATIVE_LIBRARY": str(pathlib.Path(args.native_library).resolve()),
                "OC2_TEST_BINARY": str(pathlib.Path(args.binary).resolve()),
                "OC2_TEST_NATIVE_LIBRARY": str(pathlib.Path(args.native_library).resolve()),
                "OC2_TEST_BUILD_JSON": str(pathlib.Path(args.build_json).resolve()),
                "OC2_AUTH_CONTROL_ARTIFACT_ROOT": str(evidence / "auth-cases"),
            }
        )
        checks.append(run("native_interactive_pty", [py, "tests/e2e/native_interactive_pty.py", "-v"], installed_env))
        checks.append(run("native_auth_control", [py, "-m", "unittest", "tests.bootstrap.test_native_provider_auth_control", "-v"], installed_env))

        security_env = dict(os.environ)
        security_env.setdefault("CARGO_BUILD_JOBS", "2")
        security_env.setdefault("RUST_TEST_THREADS", "1")
        checks.append(
            run(
                "security_all_targets",
                ["cargo", "test", "--locked", "-p", "opencode-rk-security", "--all-targets", "--", "--test-threads=1"],
                security_env,
            )
        )

    check_map = {c["name"]: c for c in checks}
    gate_defs = json.loads((ROOT / "recovery" / "phase1-gates.json").read_text(encoding="utf-8"))
    gate_results = {}
    artifact_mode = bool(all(artifact_values))
    for gate in gate_defs.get("gates", []):
        required = gate.get("requiredChecks", [])
        missing = [name for name in required if name not in check_map]
        failed = [name for name in required if name in check_map and check_map[name]["exit"] != 0]
        if not artifact_mode:
            status = "UNPROVEN"
        else:
            status = "PASS" if not missing and not failed else "FAIL"
        gate_results[gate["id"]] = {
            "status": status,
            "requiredChecks": required,
            "missingChecks": missing,
            "failedChecks": failed,
        }

    result = {
        "schemaVersion": 2,
        "sourceSha": head_sha,
        "canonicalSha": canonical_sha,
        "branch": branch,
        "cleanWorktree": clean,
        "artifactMode": artifact_mode,
        "checks": checks,
        "gateResults": gate_results,
    }
    runtime_failures_path = common_git_dir() / "recovery-state" / "failures.json"
    if runtime_failures_path.is_file():
        runtime_failures = json.loads(runtime_failures_path.read_text(encoding="utf-8"))
        unresolved_states = {
            "UNPROVEN", "OPEN", "CLAIMED", "CANDIDATE", "PREVERIFIED",
            "INTEGRATION_READY", "INTEGRATING", "INTEGRATED",
        }
        runtime_unresolved = [
            row.get("id")
            for row in runtime_failures.get("failures", [])
            if row.get("status") in unresolved_states
        ]
    else:
        runtime_unresolved = ["<runtime-state-not-initialized>"]
    result["runtimeUnresolvedFailures"] = runtime_unresolved
    checks_pass = all(c["exit"] == 0 for c in checks)
    gates_pass = all(g["status"] == "PASS" for g in gate_results.values())
    if artifact_mode:
        result["status"] = "PASS" if checks_pass and gates_pass else "FAIL"
    else:
        result["status"] = "PRECHECK_PASS" if checks_pass else "FAIL"
    result["acceptanceEligible"] = bool(
        artifact_mode
        and result["status"] == "PASS"
        and branch == "main-v2"
        and head_sha == canonical_sha
        and clean
        and gates_pass
        and not runtime_unresolved
    )
    rendered = json.dumps(result, indent=2) + "\n"
    canonical_evidence = common_git_dir() / "recovery-evidence" / head_sha / "phase1-judge.json"
    canonical_evidence.parent.mkdir(parents=True, exist_ok=True)
    canonical_evidence.write_text(rendered, encoding="utf-8")
    result["canonicalEvidence"] = str(canonical_evidence)
    rendered = json.dumps(result, indent=2) + "\n"
    canonical_evidence.write_text(rendered, encoding="utf-8")
    if args.output:
        out = pathlib.Path(args.output); out = out if out.is_absolute() else ROOT/out; out.parent.mkdir(parents=True, exist_ok=True); out.write_text(rendered, encoding="utf-8")
    if args.observe_state:
        observed = subprocess.run(
            [py, "tools/recovery_state.py", "observe-judge", "--evidence", str(canonical_evidence)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        if observed.returncode != 0:
            print(observed.stdout, file=sys.stderr)
            return observed.returncode
    if args.sync_state:
        if not result["acceptanceEligible"]:
            parser.error("--sync-state requires an acceptance-eligible installed-artifact PASS")
        sync = subprocess.run(
            [py, "tools/recovery_state.py", "sync-gates", "--evidence", str(canonical_evidence)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        if sync.returncode != 0:
            print(sync.stdout, file=sys.stderr)
            return sync.returncode
    print(rendered, end="")
    return 0 if result["status"] in {"PASS", "PRECHECK_PASS"} else 1


if __name__ == "__main__":
    raise SystemExit(main())

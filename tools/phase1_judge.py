#!/usr/bin/env python3
"""Phase-1 judge entrypoint for Recovery Mode."""

from __future__ import annotations

import argparse, json, os, pathlib, subprocess, sys, tempfile, time

ROOT = pathlib.Path(__file__).resolve().parents[1]


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
    args = parser.parse_args()
    artifact_values = [args.binary, args.native_library, args.build_json, args.artifact_evidence_dir]
    if any(artifact_values) and not all(artifact_values):
        parser.error("artifact mode requires --binary --native-library --build-json --artifact-evidence-dir together")

    py = sys.executable
    checks = [
        run("recovery_guard", [py, "tools/recovery_guard.py"]),
        run("convergence_structure", [py, "tools/convergence_gate.py"]),
        run("native_provider_fixture_self_check", [py, "tests/e2e/native_provider_setup.py", "--artifact-dir", tempfile.mkdtemp(prefix="oc2-p1-provider-self-"), "--self-check"]),
        run("native_stream_tool_fixture_self_check", [py, "tests/e2e/native_stream_tool.py", "--self-check"]),
    ]
    env = dict(os.environ); env.setdefault("CARGO_BUILD_JOBS", "2"); env.setdefault("RUST_TEST_THREADS", "2")
    checks.append(run("cargo_check_workspace_locked", ["cargo", "check", "--workspace", "--locked"], env))

    if all(artifact_values):
        evidence = pathlib.Path(args.artifact_evidence_dir).resolve()
        if evidence.exists() and any(evidence.iterdir()):
            parser.error("--artifact-evidence-dir must be new or empty")
        evidence.mkdir(parents=True, exist_ok=True)
        common = ["--binary", str(pathlib.Path(args.binary).resolve()), "--native-library", str(pathlib.Path(args.native_library).resolve()), "--build-json", str(pathlib.Path(args.build_json).resolve())]
        checks.append(run("native_provider_installed", [py, "tests/e2e/native_provider_setup.py", *common, "--artifact-dir", str(evidence/"provider")]))
        checks.append(run("native_stream_tool_installed", [py, "tests/e2e/native_stream_tool.py", *common, "--artifact-dir", str(evidence/"stream-tool")]))

    result = {"schemaVersion": 1, "sourceSha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "artifactMode": bool(all(artifact_values)), "checks": checks}
    result["status"] = "PASS" if all(c["exit"] == 0 for c in checks) else "FAIL"
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        out = pathlib.Path(args.output); out = out if out.is_absolute() else ROOT/out; out.parent.mkdir(parents=True, exist_ok=True); out.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())

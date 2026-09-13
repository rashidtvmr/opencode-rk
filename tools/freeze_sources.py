#!/usr/bin/env python3
"""Explicitly fetch locked sources, without installing or running their code."""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Mapping, Sequence

ROOT = pathlib.Path(__file__).resolve().parents[1]
FETCH_ATTEMPTS = 2
FETCH_TIMEOUT_SECONDS = 180


def sanitized_git_env(source: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return a minimal environment that cannot implicitly supply Git credentials."""
    source = os.environ if source is None else source
    keep = ("PATH", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY")
    env = {key: source[key] for key in keep if key in source}
    env.update(
        {
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_ASKPASS": "",
            "SSH_ASKPASS": "",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_SYSTEM": os.devnull,
        }
    )
    return env


def run_bounded(
    argv: Sequence[str],
    *,
    attempts: int = FETCH_ATTEMPTS,
    runner: Callable[..., subprocess.CompletedProcess] = subprocess.run,
    sleeper: Callable[[float], None] = time.sleep,
    env: Mapping[str, str] | None = None,
) -> None:
    if attempts < 1:
        raise ValueError("attempts must be positive")
    last: subprocess.SubprocessError | None = None
    for attempt in range(1, attempts + 1):
        try:
            runner(
                list(argv),
                check=True,
                timeout=FETCH_TIMEOUT_SECONDS,
                env=dict(env or sanitized_git_env()),
                stdin=subprocess.DEVNULL,
            )
            return
        except subprocess.SubprocessError as exc:
            last = exc
            if attempt < attempts:
                sleeper(min(attempt, 2))
    assert last is not None
    raise last


def git_output(path: pathlib.Path, *args: str, env: Mapping[str, str] | None = None) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), *args],
        text=True,
        timeout=30,
        env=dict(env or sanitized_git_env()),
        stdin=subprocess.DEVNULL,
    ).strip()


def verify_checkout(path: pathlib.Path, spec: Mapping[str, object], env: Mapping[str, str] | None = None) -> None:
    commit = str(spec["commit"])
    tree = str(spec["treeSha"])
    license_path = str(spec["licensePath"])
    license_blob = str(spec["licenseBlobSha"])
    got_commit = git_output(path, "rev-parse", "HEAD", env=env)
    got_tree = git_output(path, "rev-parse", "HEAD^{tree}", env=env)
    dirty = git_output(path, "status", "--porcelain", env=env)
    if got_commit != commit:
        raise RuntimeError(f"Reference commit mismatch for {spec['id']}: {got_commit}")
    if got_tree != tree:
        raise RuntimeError(f"Reference tree mismatch for {spec['id']}: {got_tree}")
    if dirty:
        raise RuntimeError(f"Reference checkout is dirty: {path}")
    license_file = path / license_path
    if not license_file.is_file():
        raise RuntimeError(f"Locked license file is missing: {license_file}")
    got_license = git_output(path, "hash-object", license_path, env=env)
    if got_license != license_blob:
        raise RuntimeError(f"License blob mismatch for {spec['id']}: {got_license}")


def validate_spec(spec: Mapping[str, object]) -> None:
    for field in ("commit", "treeSha", "licenseBlobSha"):
        if not re.fullmatch(r"[0-9a-f]{40}", str(spec.get(field, ""))):
            raise ValueError(f"Invalid locked {field} for {spec.get('id', '<unknown>')}")
    url = str(spec.get("url", ""))
    if not url.startswith("https://github.com/") or not url.endswith(".git"):
        raise ValueError(f"Only public HTTPS GitHub references are allowed: {url}")
    license_path = str(spec.get("licensePath", ""))
    if not license_path or pathlib.PurePosixPath(license_path).is_absolute() or ".." in pathlib.PurePosixPath(license_path).parts:
        raise ValueError(f"Invalid license path: {license_path}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--fetch", action="store_true", help="Authorize network fetch of the two pinned sources")
    ap.add_argument("--destination", type=pathlib.Path, default=ROOT / ".upstream")
    args = ap.parse_args()
    lock = json.loads((ROOT / "sources/upstream.lock.json").read_text())
    specs = lock["repositories"]
    for spec in specs:
        validate_spec(spec)
    if not args.fetch:
        print(
            json.dumps(
                {
                    "status": "not-fetched",
                    "references": specs,
                    "next": "Run with --fetch to authorize downloading exact commits.",
                },
                indent=2,
            )
        )
        return 0

    args.destination.mkdir(parents=True, exist_ok=True)
    env = sanitized_git_env()
    with tempfile.TemporaryDirectory(prefix="opencode-rk-git-home-") as empty_home:
        env["HOME"] = empty_home
        for spec in specs:
            path = args.destination / spec["id"]
            if path.exists():
                if path.is_symlink():
                    raise RuntimeError(f"Refusing symlink destination: {path}")
                verify_checkout(path, spec, env)
                print(f"Already pinned: {spec['id']} {spec['commit']}")
                continue
            run_bounded(["git", "init", str(path)], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "config", "core.hooksPath", os.devnull], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "remote", "add", "origin", spec["url"]], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "fetch", "--depth=1", "origin", spec["commit"]], env=env)
            run_bounded(["git", "-C", str(path), "checkout", "--detach", spec["commit"]], attempts=1, env=env)
            verify_checkout(path, spec, env)
            print(f"Pinned: {spec['id']} {spec['commit']} tree={spec['treeSha']} license={spec['licenseSpdx']}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        print(f"BLOCKED: {exc}", file=sys.stderr)
        raise SystemExit(2)

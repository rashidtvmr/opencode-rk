#!/usr/bin/env python3
"""Explicitly fetch locked sources, without installing or running their code."""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Mapping, Sequence

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.source_lock import (
    build_frozen_manifest,
    sanitized_git_env,
    validate_lock,
    validate_spec,
    verify_checkout,
    write_manifest_atomic,
)

FETCH_ATTEMPTS = 2
FETCH_TIMEOUT_SECONDS = 180


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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--fetch", action="store_true", help="Authorize network fetch of the pinned public sources")
    ap.add_argument("--destination", type=pathlib.Path, default=ROOT / ".upstream")
    args = ap.parse_args()
    lock = json.loads((ROOT / "sources/upstream.lock.json").read_text(encoding="utf-8"))
    specs = validate_lock(lock)
    if not args.fetch:
        print(json.dumps({"status": "not-fetched", "references": specs, "next": "Run with --fetch to authorize downloading exact commits."}, indent=2))
        return 0

    args.destination.mkdir(parents=True, exist_ok=True)
    if args.destination.is_symlink():
        raise RuntimeError(f"Refusing symlink destination root: {args.destination}")
    env = sanitized_git_env()
    records: list[Mapping[str, object]] = []

    with tempfile.TemporaryDirectory(prefix="opencode-rk-git-home-") as empty_home:
        env["HOME"] = empty_home
        for spec in specs:
            path = args.destination / str(spec["id"])
            if path.exists():
                records.append(verify_checkout(path, spec, env))
                print(f"Already pinned: {spec['id']} {spec['commit']}")
                continue

            run_bounded(["git", "init", str(path)], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "config", "core.hooksPath", os.devnull], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "remote", "add", "origin", str(spec["url"])], attempts=1, env=env)
            run_bounded(["git", "-C", str(path), "fetch", "--depth=1", "origin", str(spec["commit"])], env=env)
            run_bounded(["git", "-C", str(path), "checkout", "--detach", str(spec["commit"])], attempts=1, env=env)
            records.append(verify_checkout(path, spec, env))
            print(f"Pinned: {spec['id']} {spec['commit']} tree={spec['treeSha']} license={spec['licenseSpdx']}")

    if len(records) != len(specs):
        raise RuntimeError("Not all locked repositories were verified")
    write_manifest_atomic(args.destination / "frozen-manifest.json", build_frozen_manifest(records))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        print(f"BLOCKED: {exc}", file=sys.stderr)
        raise SystemExit(2)

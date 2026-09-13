#!/usr/bin/env python3
"""Shared validation for immutable public upstream source checkouts.

This module intentionally contains no network fetch logic. It validates the lock
contract and authenticates an already-present checkout without running upstream
code, hooks, package managers, or lifecycle scripts.
"""
from __future__ import annotations

import json
import os
import pathlib
import re
import subprocess
from collections.abc import Mapping
from urllib.parse import urlsplit

LOCK_SCHEMA_VERSION = 1
_SHA_RE = re.compile(r"[0-9a-f]{40}")
_ID_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")


def sanitized_git_env(source: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return a small Git environment that cannot implicitly supply credentials."""
    source = os.environ if source is None else source
    keep = (
        "PATH",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
    )
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


def validate_spec(spec: Mapping[str, object]) -> None:
    source_id = str(spec.get("id", ""))
    if not _ID_RE.fullmatch(source_id):
        raise ValueError(f"Invalid locked source id: {source_id!r}")
    for field in ("commit", "treeSha", "licenseBlobSha"):
        if not _SHA_RE.fullmatch(str(spec.get(field, ""))):
            raise ValueError(f"Invalid locked {field} for {source_id or '<unknown>'}")

    raw_url = str(spec.get("url", ""))
    parsed = urlsplit(raw_url)
    if (
        parsed.scheme != "https"
        or parsed.hostname != "github.com"
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port is not None
        or parsed.query
        or parsed.fragment
        or not re.fullmatch(r"/[^/]+/[^/]+\.git", parsed.path)
    ):
        raise ValueError(f"Only credential-free public HTTPS GitHub references are allowed: {raw_url}")

    license_path = str(spec.get("licensePath", ""))
    pure_license = pathlib.PurePosixPath(license_path)
    if not license_path or pure_license.is_absolute() or ".." in pure_license.parts:
        raise ValueError(f"Invalid license path: {license_path}")
    if not str(spec.get("licenseSpdx", "")).strip():
        raise ValueError(f"Missing license SPDX identifier for {source_id}")


def validate_lock(lock: Mapping[str, object]) -> list[Mapping[str, object]]:
    if lock.get("schemaVersion") != LOCK_SCHEMA_VERSION:
        raise ValueError(f"Unsupported upstream lock schema: {lock.get('schemaVersion')!r}")
    repositories = lock.get("repositories")
    if not isinstance(repositories, list) or not repositories:
        raise ValueError("Upstream lock must contain at least one repository")
    seen: set[str] = set()
    validated: list[Mapping[str, object]] = []
    for raw in repositories:
        if not isinstance(raw, Mapping):
            raise ValueError("Repository lock entries must be objects")
        validate_spec(raw)
        source_id = str(raw["id"])
        if source_id in seen:
            raise ValueError(f"Duplicate upstream source id: {source_id}")
        seen.add(source_id)
        validated.append(raw)
    return validated


def git_output(path: pathlib.Path, *args: str, env: Mapping[str, str] | None = None) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), *args],
        text=True,
        timeout=30,
        env=dict(env or sanitized_git_env()),
        stdin=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    ).strip()


def _check_connectivity(path: pathlib.Path, env: Mapping[str, str]) -> None:
    subprocess.run(
        ["git", "-C", str(path), "fsck", "--connectivity-only", "--no-dangling", "HEAD"],
        check=True,
        timeout=120,
        env=dict(env),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )


def inspect_checkout(
    path: pathlib.Path, spec: Mapping[str, object], env: Mapping[str, str] | None = None
) -> dict[str, object]:
    """Read identity only; no upstream code is executed."""
    env = dict(env or sanitized_git_env())
    if path.is_symlink():
        raise RuntimeError(f"Refusing symlink checkout: {path}")
    if not path.is_dir():
        raise RuntimeError(f"Reference checkout is missing: {path}")
    if git_output(path, "rev-parse", "--is-inside-work-tree", env=env) != "true":
        raise RuntimeError(f"Not a Git worktree: {path}")

    _check_connectivity(path, env)
    license_path = str(spec["licensePath"])
    license_file = path / license_path
    if not license_file.is_file() or license_file.is_symlink():
        raise RuntimeError(f"Locked license file is missing or unsafe: {license_file}")

    return {
        "id": str(spec["id"]),
        "commit": git_output(path, "rev-parse", "HEAD", env=env),
        "treeSha": git_output(path, "rev-parse", "HEAD^{tree}", env=env),
        "originUrl": git_output(path, "remote", "get-url", "origin", env=env),
        "branch": git_output(path, "branch", "--show-current", env=env),
        "dirty": bool(git_output(path, "status", "--porcelain", "--untracked-files=all", env=env)),
        "licensePath": license_path,
        "licenseBlobSha": git_output(path, "hash-object", license_path, env=env),
        "licenseSpdx": str(spec["licenseSpdx"]),
        "connectivityVerified": True,
    }


def checkout_identity_errors(record: Mapping[str, object], spec: Mapping[str, object]) -> list[str]:
    errors: list[str] = []
    source_id = str(spec["id"])
    expected = {
        "id": source_id,
        "commit": str(spec["commit"]),
        "treeSha": str(spec["treeSha"]),
        "originUrl": str(spec["url"]),
        "licensePath": str(spec["licensePath"]),
        "licenseBlobSha": str(spec["licenseBlobSha"]),
        "licenseSpdx": str(spec["licenseSpdx"]),
    }
    for field, value in expected.items():
        if record.get(field) != value:
            errors.append(f"{source_id}: {field} mismatch")
    if record.get("branch"):
        errors.append(f"{source_id}: checkout is attached to branch {record.get('branch')}")
    if record.get("dirty"):
        errors.append(f"{source_id}: checkout is dirty")
    if record.get("connectivityVerified") is not True:
        errors.append(f"{source_id}: object connectivity was not verified")
    return errors


def verify_checkout(
    path: pathlib.Path, spec: Mapping[str, object], env: Mapping[str, str] | None = None
) -> dict[str, object]:
    validate_spec(spec)
    record = inspect_checkout(path, spec, env)
    errors = checkout_identity_errors(record, spec)
    if errors:
        raise RuntimeError("; ".join(errors))
    return record


def build_frozen_manifest(records: list[Mapping[str, object]]) -> dict[str, object]:
    """Create deterministic, credential-free evidence suitable for hashing/review."""
    safe_records = []
    allowed = (
        "id",
        "commit",
        "treeSha",
        "originUrl",
        "licensePath",
        "licenseBlobSha",
        "licenseSpdx",
        "connectivityVerified",
    )
    for record in records:
        safe_records.append({key: record[key] for key in allowed})
    return {
        "schemaVersion": 1,
        "repositories": safe_records,
        "guarantees": {
            "detachedHead": True,
            "cleanWorktree": True,
            "credentialFreeEvidence": True,
            "upstreamCodeExecuted": False,
        },
    }


def write_manifest_atomic(path: pathlib.Path, manifest: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + ".tmp")
    temp.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temp, path)

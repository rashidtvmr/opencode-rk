#!/usr/bin/env python3
"""R1 exact-SHA release gate contract: receipt/artifact validation (stdlib only).

Reuse note: the Ubuntu arm64 evidence schema (build.json, commands.json,
frozen_tests, runner_sha256) is the retained verify-native-utf8.py runner
format; this module validates it and rejects stale cross-SHA reuse. It never
re-enacts product behavior (Web uses the real Rust router/auth binary via
tests/e2e/web_tool_journey_fixture.py; Mac uses the frozen native paste-reset
gate tests/e2e/native_bracketed_paste.py).
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import tempfile

SHA40 = re.compile(r"[0-9a-f]{40}")
SHA64 = re.compile(r"[0-9a-f]{64}")
TEMPLATE_SHA = "<SOURCE_SHA>"
CREDENTIAL_KEY = re.compile(
    r"(API|AUTH|TOKEN|SECRET|KEY|PASSWORD|CREDENTIAL|SIGNING|NOTARY|APPLE_ID|AZURE)", re.I
)

REQUIRED_MANIFEST_KEYS = (
    "source_sha",
    "binary_sha256",
    "native_library_sha256",
    "archive_sha256",
    "frozen_tests",
    "runner_sha256",
)


class ReceiptError(ValueError):
    """Raised for any contract violation (missing, mismatch, stale reuse)."""


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def load_json(path: pathlib.Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ReceiptError(f"missing file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ReceiptError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ReceiptError(f"{path} must contain a JSON object")
    return value


def validate_manifest(manifest: dict) -> None:
    """Schema + exact-SHA binding for a build.json artifact manifest."""
    for key in REQUIRED_MANIFEST_KEYS:
        if key not in manifest:
            raise ReceiptError(f"manifest missing required key: {key}")
    if not SHA40.fullmatch(str(manifest["source_sha"])):
        raise ReceiptError("manifest source_sha must be 40 hex characters")
    for key in ("binary_sha256", "native_library_sha256", "archive_sha256", "runner_sha256"):
        if not SHA64.fullmatch(str(manifest[key])):
            raise ReceiptError(f"manifest {key} must be 64 hex characters")
    frozen = manifest["frozen_tests"]
    if not isinstance(frozen, dict) or not frozen:
        raise ReceiptError("manifest frozen_tests must be a non-empty object")
    for path, expected in frozen.items():
        if not SHA64.fullmatch(str(expected)):
            raise ReceiptError(f"manifest frozen_tests[{path}] must be 64 hex characters")
    if manifest.get("profile") != "release" or manifest.get("native") is not True:
            raise ReceiptError("manifest must attest profile=release and native=true")


def materialize_template(template: dict, source_sha: str, output: pathlib.Path) -> dict:
    """Instantiate a deliberately non-runnable template exactly once."""
    if not SHA40.fullmatch(source_sha):
        raise ReceiptError("requested source_sha must be 40 lowercase hex characters")
    if os.path.lexists(output):
        raise ReceiptError(f"refusing to overwrite existing materialized manifest: {output}")
    import copy
    concrete = copy.deepcopy(template)
    if concrete.get("source_sha") != TEMPLATE_SHA:
        raise ReceiptError("template top-level source_sha is not the sentinel")
    contract = concrete.get("artifact_contract")
    if not isinstance(contract, dict) or contract.get("source_sha") != TEMPLATE_SHA:
        raise ReceiptError("template artifact_contract source_sha is not the sentinel")
    concrete["source_sha"] = source_sha
    contract["source_sha"] = source_sha
    validate_manifest(contract)
    output.parent.mkdir(parents=True, exist_ok=True)
    write_json_atomic_exclusive(output, concrete)
    return concrete


def validate_artifact_hashes(manifest: dict, artifact_dir: pathlib.Path) -> None:
    """Every binary/library/archive referenced by the manifest must be present
    at artifact_dir and hash exactly. Rejects swapped or stale artifacts."""
    platform = manifest.get("platform", "linux")
    if platform not in ("linux", "mac"):
        raise ReceiptError(f"unsupported artifact platform: {platform!r}")
    library = "libopentui.dylib" if platform == "mac" else "libopentui.so"
    archive_name = "oc2-macos-arm64.tar.gz" if platform == "mac" else "oc2-linux-arm64.tar.gz"
    allowed = {"install/bin/oc2", f"install/lib/{library}", archive_name, "build.json"}
    # Artifact paths are a closed allow-list.  In particular, never honor a
    # path supplied by build.json (which could escape through ../ or a link).
    for entry in artifact_dir.rglob("*"):
        if entry.is_symlink():
            raise ReceiptError(f"symlink artifact rejected: {entry}")
        if entry.is_file() and entry.relative_to(artifact_dir).as_posix() not in allowed:
            raise ReceiptError(f"unexpected artifact file: {entry}")
    mapping = {
        artifact_dir / "install/bin/oc2": manifest["binary_sha256"],
        artifact_dir / f"install/lib/{library}": manifest["native_library_sha256"],
    }
    for candidate, expected in mapping.items():
        if not candidate.is_file():
            raise ReceiptError(f"artifact {candidate.relative_to(artifact_dir)} absent under {artifact_dir}")
        actual = sha256_file(candidate)
        if actual != expected:
            raise ReceiptError(f"artifact {candidate.name} hash mismatch: manifest {expected} != file {actual}")
    archive = artifact_dir / archive_name
    if archive.is_file():
        actual = sha256_file(archive)
        if actual != manifest["archive_sha256"]:
            raise ReceiptError(
                f"archive hash mismatch: manifest {manifest['archive_sha256']} != file {actual}"
            )
    elif manifest.get("require_archive", True):
        raise ReceiptError(f"archive {archive_name} absent under {artifact_dir}")


def validate_frozen_tests(manifest: dict, source_root: pathlib.Path) -> None:
    """Frozen contract tests must be byte-identical to the hashes recorded in
    the artifact manifest (prevents test-weakening against a candidate)."""
    for rel, expected in manifest["frozen_tests"].items():
        path = source_root / rel
        if not path.is_file():
            raise ReceiptError(f"frozen test absent: {path}")
        actual = sha256_file(path)
        if actual != expected:
            raise ReceiptError(f"frozen test modified: {rel} expected {expected} got {actual}")


def validate_runner(manifest: dict, runner_path: pathlib.Path) -> None:
    """The retained platform runner itself is hash-pinned in the manifest."""
    if not runner_path.is_file():
        raise ReceiptError(f"runner absent: {runner_path}")
    actual = sha256_file(runner_path)
    if actual != manifest["runner_sha256"]:
        raise ReceiptError(f"runner hash mismatch: manifest {manifest['runner_sha256']} != file {actual}")


def validate_receipts(receipts: dict, expected_source_sha: str, platform: str) -> list[str]:
    """Receipts must name the exact source SHA and platform, all gates exit 0,
    no capture-budget failures, and every log file re-hash. Returns violations.

    Any receipt carrying a different source_sha is stale cross-SHA reuse and
    is always a violation; there is no fallback that accepts it."""
    violations: list[str] = []
    if receipts.get("source_sha") != expected_source_sha:
        violations.append(
            f"stale cross-SHA reuse: receipt source_sha {receipts.get('source_sha')!r} "
            f"!= required {expected_source_sha!r}"
        )
    if receipts.get("platform") != platform:
        violations.append(f"receipt platform {receipts.get('platform')!r} != {platform!r}")
    gates = receipts.get("gates")
    if not isinstance(gates, list) or not gates:
        return violations + ["receipt gates must be a non-empty list"]
    seen: set[str] = set()
    for gate in gates:
        label = str(gate.get("label", "<missing>"))
        if label in seen:
            violations.append(f"duplicate gate label: {label}")
        seen.add(label)
        if gate.get("exit") != 0:
            violations.append(f"gate {label} exit={gate.get('exit')!r} (must be real 0)")
        if gate.get("limit_failure"):
            violations.append(f"gate {label} capture limit failure: {gate['limit_failure']}")
        if gate.get("source_sha") != expected_source_sha:
            violations.append(f"gate {label} carries stale source_sha {gate.get('source_sha')!r}")
        log = gate.get("log_path")
        log_sha = gate.get("log_sha256")
        if not log or not SHA64.fullmatch(str(log_sha or "")):
            violations.append(f"gate {label} missing log_path/log_sha256")
        else:
            log_file = pathlib.Path(log)
            if not log_file.is_file():
                violations.append(f"gate {label} log absent: {log}")
            elif sha256_file(log_file) != log_sha:
                violations.append(f"gate {label} log hash mismatch (tampered evidence)")
    return violations


def reject_nonempty_dir(path: pathlib.Path) -> None:
    """Evidence directories are single-use: never overwrite prior receipts."""
    if path.exists() and any(path.iterdir()):
        raise ReceiptError(f"evidence directory not fresh (stale reuse): {path}")


def disposable_env(base: pathlib.Path, *, path: str = "/usr/bin:/bin",
                   strip: tuple[str, ...] = ()) -> dict:
    """HOME/XDG/TMPDIR isolated under a disposable root; no inherited
    credentials, no user OpenCode DB, optional extra stripped keys."""
    home = base / "home"
    for name in ("config", "data", "state", "cache"):
        (home / name).mkdir(parents=True, exist_ok=True)
    (base / "tmp").mkdir(parents=True, exist_ok=True)
    env = {
        "HOME": str(home),
        "PATH": path,
        "TMPDIR": str(base / "tmp"),
        "LANG": "C",
        "PYTHONDONTWRITEBYTECODE": "1",
        "XDG_CONFIG_HOME": str(home / "config"),
        "XDG_DATA_HOME": str(home / "data"),
        "XDG_STATE_HOME": str(home / "state"),
        "XDG_CACHE_HOME": str(home / "cache"),
        "OPENCODE_RK_HOME": str(home / "data"),
    }
    for key in strip:
        env.pop(key, None)
    return env


def scrub_inherited_env(env: dict) -> dict:
    """Drop any credential-shaped variable before handing env to a gate."""
    return {k: v for k, v in env.items() if not CREDENTIAL_KEY.search(k)}


def write_json_atomic(path: pathlib.Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temporary = pathlib.Path(temporary_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
        # replace changes the destination directory entry, never the target
        # of a symlink at path; unlike the old predictable .tmp this is not a
        # symlink-race primitive.
        os.replace(temporary, path)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def write_json_atomic_exclusive(path: pathlib.Path, value: dict) -> None:
    """Create a fresh output without replacing a race-created entry."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temporary = pathlib.Path(temporary_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")
            stream.flush(); os.fsync(stream.fileno())
        if os.path.lexists(path):
            raise ReceiptError(f"refusing to overwrite existing output: {path}")
        os.link(temporary, path)
        temporary.unlink()
    finally:
        try: temporary.unlink()
        except FileNotFoundError: pass

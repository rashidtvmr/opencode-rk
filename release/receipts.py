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

import copy
import hashlib
import json
import os
import pathlib
import re
import stat
import tempfile

SHA40 = re.compile(r"[0-9a-f]{40}")
SHA64 = re.compile(r"[0-9a-f]{64}")
CREDENTIAL_KEY = re.compile(
    r"(API|AUTH|TOKEN|SECRET|KEY|PASSWORD|CREDENTIAL|SIGNING|NOTARY|APPLE_ID|AZURE)", re.I
)
TEMPLATE_SHA = "<SOURCE_SHA>"

# Platform-specific native library names.
MAC_LIBRARY = "libopentui.dylib"
LINUX_LIBRARY = "libopentui.so"

REQUIRED_MANIFEST_KEYS = (
    "source_sha",
    "binary_sha256",
    "native_library_sha256",
    "archive_sha256",
    "frozen_tests",
    "runner_sha256",
)
PROVENANCE_SCHEMA = "r1-trusted-build-provenance/1"


class ReceiptError(ValueError):
    """Raised for any contract violation (missing, mismatch, stale reuse)."""


def validate_trusted_provenance(receipt: dict, source_sha: str, *,
                                tree_sha: str, manifest: dict,
                                artifact_dir: pathlib.Path) -> None:
    """Validate a controller-issued build receipt, never an artifact claim.

    ``build.json`` is deliberately not an authority for this check.  The
    caller supplies this receipt through a trusted controller channel and this
    function only checks that its bounded observation describes the exact
    source tree, frozen inputs, build invocation and bytes being gated.
    """
    if receipt.get("schema") != PROVENANCE_SCHEMA:
        raise ReceiptError("missing or unsupported trusted provenance schema")
    issuer = receipt.get("issuer")
    if not isinstance(issuer, str) or not issuer.strip():
        raise ReceiptError("trusted provenance issuer is required")
    if receipt.get("source_sha") != source_sha:
        raise ReceiptError("trusted provenance source_sha mismatch")
    if not SHA40.fullmatch(str(receipt.get("source_tree_sha", ""))) or receipt["source_tree_sha"] != tree_sha:
        raise ReceiptError("trusted provenance source tree mismatch")
    contract = manifest["artifact_contract"]
    if receipt.get("runner_sha256") != contract["runner_sha256"]:
        raise ReceiptError("trusted provenance runner mismatch")
    if receipt.get("profile") != "release" or receipt.get("native") is not True:
        raise ReceiptError("trusted provenance must describe a native release build")
    frozen = receipt.get("frozen_tests")
    if frozen != contract["frozen_tests"]:
        raise ReceiptError("trusted provenance frozen test map mismatch")
    command = receipt.get("build_command")
    if (not isinstance(command, list) or not command or
            any(not isinstance(part, str) or not part or len(part) > 4096 for part in command)):
        raise ReceiptError("trusted provenance build_command must be a bounded argv")
    if receipt.get("build_exit") != 0:
        raise ReceiptError("trusted build did not exit successfully")
    installer_command = receipt.get("installer_command")
    if (not isinstance(installer_command, list) or not installer_command or
            any(not isinstance(part, str) or not part or len(part) > 4096
                for part in installer_command)):
        raise ReceiptError("trusted provenance installer_command must be a bounded argv")
    for key in ("binary_sha256", "native_library_sha256", "archive_sha256"):
        expected = receipt.get(key)
        if not SHA64.fullmatch(str(expected or "")):
            raise ReceiptError(f"trusted provenance {key} is invalid")
        if expected != contract[key]:
            raise ReceiptError(f"trusted provenance {key} disagrees with contract")
    if not SHA64.fullmatch(str(receipt.get("source_archive_sha256", ""))):
        raise ReceiptError("trusted provenance source archive digest is invalid")

    # These are observations, not authority: artifact bytes must independently
    # hash to the controller's values.  The artifact's JSON may only agree.
    validate_artifact_hashes(receipt, artifact_dir)
    artifact_manifest = load_json(artifact_dir / "build.json")
    validate_manifest(artifact_manifest)
    for key in ("source_sha", "source_archive_sha256", "binary_sha256",
                "native_library_sha256", "archive_sha256", "runner_sha256",
                "frozen_tests", "profile", "native"):
        if artifact_manifest.get(key) != receipt.get(key):
            raise ReceiptError(f"artifact metadata {key} disagrees with trusted provenance")
    observed = artifact_manifest.get("build_command")
    if isinstance(observed, dict):
        observed = observed.get("argv") or observed.get("command")
    if observed != command:
        raise ReceiptError("artifact build command disagrees with trusted provenance")
    observed_installer = artifact_manifest.get("installer_command")
    if isinstance(observed_installer, dict):
        observed_installer = observed_installer.get("argv") or observed_installer.get("command")
    if observed_installer != installer_command:
        raise ReceiptError("artifact installer command disagrees with trusted provenance")


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
    """Instantiate a tracked trusted template for a requested exact SHA.

    Trusted template fields (gates, runner/frozen pins, platform structure)
    are copied as-is from *template*. Only attestation-derived hash fields
    and source_sha are set from the artifact build.json by the trusted
    materializer. This function itself only performs schema/SHA binding.

    Refuses existing output (file or symlink).
    """
    if not SHA40.fullmatch(source_sha):
        raise ReceiptError("requested source_sha must be 40 lowercase hex characters")
    if os.path.lexists(output):
        raise ReceiptError(
            f"refusing to overwrite existing materialized manifest: {output}"
        )
    concrete = copy.deepcopy(template)
    # Template sentinel check.
    if concrete.get("source_sha") != TEMPLATE_SHA:
        raise ReceiptError("template top-level source_sha is not the sentinel")
    contract = concrete.get("artifact_contract")
    if not isinstance(contract, dict) or contract.get("source_sha") != TEMPLATE_SHA:
        raise ReceiptError("template artifact_contract source_sha is not the sentinel")
    concrete["source_sha"] = source_sha
    contract["source_sha"] = source_sha
    validate_manifest(contract)
    output.parent.mkdir(parents=True, exist_ok=True)
    write_json_atomic_safe(output, concrete)
    return concrete


def validate_materialized_manifest(manifest: dict, source_sha: str) -> None:
    """Validate a concrete materialized manifest: exact-SHA binding, no
    remaining sentinel, valid schema. Raises ReceiptError on violation."""
    if not SHA40.fullmatch(source_sha):
        raise ReceiptError("requested source_sha must be 40 lowercase hex characters")
    if manifest.get("source_sha") == TEMPLATE_SHA:
        raise ReceiptError("manifest is still the unmaterialized template sentinel")
    if manifest.get("source_sha") != source_sha:
        raise ReceiptError(
            f"stale cross-SHA reuse: manifest source_sha {manifest.get('source_sha')!r} "
            f"!= required {source_sha!r}"
        )
    contract = manifest.get("artifact_contract")
    if not isinstance(contract, dict):
        raise ReceiptError("manifest missing artifact_contract")
    if contract.get("source_sha") != source_sha:
        raise ReceiptError(
            f"stale cross-SHA reuse: artifact_contract source_sha "
            f"{contract.get('source_sha')!r} != required {source_sha!r}"
        )
    validate_manifest(contract)
    platforms = manifest.get("platforms")
    if not isinstance(platforms, dict) or not platforms:
        raise ReceiptError("manifest platforms must be a non-empty object")


def validate_artifact_hashes(manifest: dict, artifact_dir: pathlib.Path, *,
                             library_name: str = LINUX_LIBRARY) -> None:
    """Every binary/library/archive referenced by the manifest must be present
    at artifact_dir and hash exactly. Rejects swapped or stale artifacts,
    symlinks, and outside-root traversal.

    library_name selects the expected native library file name
    (libopentui.so for Linux, libopentui.dylib for Mac).
    """
    binary_expected = manifest["binary_sha256"]
    library_expected = manifest["native_library_sha256"]
    archive_expected = manifest["archive_sha256"]

    # Binary: oc2, must be a regular file (not a symlink).
    binary_candidate = _resolve_artifact(artifact_dir, "oc2")
    _check_regular_file(binary_candidate, "oc2")
    _check_hash(binary_candidate, binary_expected, "oc2")

    # Native library: selected by platform, must be a regular file.
    library_candidate = _resolve_artifact(artifact_dir, library_name)
    _check_regular_file(library_candidate, library_name)
    _check_hash(library_candidate, library_expected, library_name)

    # Archive: oc2-linux-arm64.tar.gz (Linux convention), must be regular file.
    archive = artifact_dir / "oc2-linux-arm64.tar.gz"
    if os.path.lexists(archive):
        _check_regular_file(archive, "archive")
        _check_hash(archive, archive_expected, "archive")
    elif manifest.get("require_archive", True):
        raise ReceiptError(f"archive absent under {artifact_dir}")


def _resolve_artifact(artifact_dir: pathlib.Path, name: str) -> pathlib.Path:
    """Resolve an artifact path, checking only safe locations.

    Searches: <artifact_dir>/<name>, <artifact_dir>/install/bin/<name>,
    <artifact_dir>/install/lib/<name>. All resolved paths must remain
    inside artifact_dir (resolves symlinks for containment check).
    """
    candidates = [
        artifact_dir / name,
        artifact_dir / "install" / "bin" / name,
        artifact_dir / "install" / "lib" / name,
    ]
    for candidate in candidates:
        if not os.path.lexists(candidate):
            continue
        try:
            resolved = candidate.resolve(strict=False)
        except (OSError, RuntimeError):
            continue
        # Containment check: resolved path must be under artifact_dir.
        try:
            resolved.relative_to(artifact_dir.resolve(strict=False))
        except ValueError:
            raise ReceiptError(f"artifact path escapes artifact root: {candidate}")
        return candidate
    raise ReceiptError(f"artifact {name} absent under {artifact_dir}")


def _check_regular_file(path: pathlib.Path, label: str) -> None:
    """Reject symlinks, directories, and missing files."""
    if os.path.islink(path):
        raise ReceiptError(f"artifact {label} is a symlink, refused: {path}")
    if not path.is_file():
        raise ReceiptError(f"artifact {label} absent or not a regular file: {path}")


def _check_hash(path: pathlib.Path, expected: str, label: str) -> None:
    """Verify the file at *path* hashes to *expected*."""
    actual = sha256_file(path)
    if actual != expected:
        raise ReceiptError(
            f"artifact {label} hash mismatch: manifest {expected} != file {actual}"
        )


def validate_frozen_tests(manifest: dict, source_root: pathlib.Path) -> None:
    """Frozen contract tests must be byte-identical to the hashes recorded in
    the artifact manifest (prevents test-weakening against a candidate)."""
    for rel, expected in manifest["frozen_tests"].items():
        path = source_root / rel
        if os.path.islink(path):
            raise ReceiptError(f"frozen test is a symlink, refused: {path}")
        if not path.is_file():
            raise ReceiptError(f"frozen test absent: {path}")
        actual = sha256_file(path)
        if actual != expected:
            raise ReceiptError(f"frozen test modified: {rel} expected {expected} got {actual}")


def validate_runner(manifest: dict, runner_path: pathlib.Path) -> None:
    """The retained platform runner itself is hash-pinned in the manifest."""
    if os.path.islink(runner_path):
        raise ReceiptError(f"runner is a symlink, refused: {runner_path}")
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
            if os.path.islink(log_file):
                violations.append(f"gate {label} log path is a symlink, refused: {log}")
            elif not log_file.is_file():
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
    """Legacy atomic writer. Deprecated: use write_json_atomic_safe instead.

    This version uses a predictable .tmp name and os.replace, which can follow
    symlinks or race with attacker-created destination. Retained only for
    backwards compatibility where safe semantics are not required.
    """
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def write_json_atomic_safe(path: pathlib.Path, value: dict) -> None:
    """Exclusive, symlink-safe atomic JSON writer.

    Uses unpredictable temp file via tempfile.mkstemp in the same directory
    (so os.link stays on one filesystem), mode 0600, flush+fsync before install,
    and os.link with follow_symlinks=False as the final install step.

    Refuses:
      - any existing regular file at *path*,
      - any existing symlink at *path* (regular, dangling, or target),
      - race-created destination between temp creation and install.

    The temp file is always cleaned up.
    """
    target_parent = path.parent
    target_parent.mkdir(parents=True, exist_ok=True)

    # Refuse existing destination (file, symlink of any kind, broken symlink).
    if os.path.lexists(path):
        raise ReceiptError(
            f"refusing to overwrite existing destination (file or symlink): {path}"
        )

    fd, tmp_name = tempfile.mkstemp(dir=str(target_parent), prefix=".manifest-", suffix=".tmp")
    os.close(fd)
    tmp_path = pathlib.Path(tmp_name)
    try:
        os.chmod(tmp_path, 0o600)
        data = json.dumps(value, indent=2, sort_keys=True) + "\n"
        with tmp_path.open("wb") as f:
            f.write(data.encode("utf-8"))
            f.flush()
            os.fsync(f.fileno())

        # Atomic no-clobber install: os.link with follow_symlinks=False.
        # If path was created by a race, link fails with FileExistsError.
        try:
            os.link(tmp_path, path, follow_symlinks=False)
        except FileExistsError as exc:
            raise ReceiptError(
                f"race-created destination refused (not overwriting): {path}"
            ) from exc
    finally:
        if os.path.lexists(tmp_path):
            os.unlink(tmp_path)

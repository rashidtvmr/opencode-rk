"""Artifact-custody and fail-closed result controls (UIC-004/006 core).

Evidence-only control module. No subprocess, socket, PTY, provider, or product
import. All functions are pure stdlib and import-safe (no top-level side
effects).
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from dataclasses import dataclass

REFUSED_SHA256 = (
    "1d49ae1f01357e3ef6285374e6b80865fcc5e3083c71d5f5a087945b23ef886a"
)
_HEX64 = re.compile(r"^[0-9a-f]{64}$")

EXIT_GREEN = 0
EXIT_PRODUCT_FAIL = 1
EXIT_HARNESS_UNFINISHED = 3

STATE_UNFINISHED = "HARNESS_UNFINISHED"
STATE_RED = "RED"
STATE_GREEN = "GREEN"


class CustodyError(Exception):
    """Fail-closed custody violation. Never converted to GREEN."""


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_hash_policy(actual_sha256, manifest_sha256=None, refused=REFUSED_SHA256):
    """Pure hash policy: 64-hex, equal-to-actual, refused-SHA denial.

    Separated from file IO so tests can exercise refused-hash denial without
    producing a preimage of the immutable refused binary hash.
    """
    if not isinstance(actual_sha256, str) or not _HEX64.match(actual_sha256):
        raise CustodyError("actual sha256 must be 64 lowercase hex characters")
    if actual_sha256 == refused:
        raise CustodyError("refused immutable binary sha256 denied: %s" % refused)
    if manifest_sha256 is not None:
        if not isinstance(manifest_sha256, str) or not _HEX64.match(manifest_sha256):
            raise CustodyError("manifest sha256 must be 64 lowercase hex characters")
        if manifest_sha256 == refused:
            raise CustodyError(
                "manifest declares refused immutable binary sha256: %s" % refused
            )
        if manifest_sha256 != actual_sha256:
            raise CustodyError("manifest sha256 does not equal actual binary sha256")
    return actual_sha256


@dataclass(frozen=True)
class VerifiedArtifact:
    """Immutable custody record for future daemon and PTY argv injection."""

    path: str
    sha256: str

    def argv_identity(self):
        # Future harness injects exactly this path into daemon and PTY argv.
        return self.path


def validate_artifact(binary_arg, manifest_path):
    """Hash the EXACT CLI binary path and reconcile it with the manifest.

    Policy (explicit):
    - missing binary or missing manifest -> CustodyError;
    - symlinked binary path or any symlinked parent component -> CustodyError;
    - manifest `binary`, when present, must resolve to the same path as the
      CLI argument (a different manifest binary is never executed);
    - manifest sha256 is required, must be 64-hex, equal to the actual hash,
      and never the refused immutable hash.
    Returns an immutable VerifiedArtifact(path, sha256); performs no launch.
    """
    if not isinstance(binary_arg, str) or not binary_arg:
        raise CustodyError("binary path argument required")
    if os.path.islink(binary_arg):
        raise CustodyError("binary path is a symlink; refused: %s" % binary_arg)
    abs_binary = os.path.abspath(binary_arg)
    if not os.path.exists(abs_binary):
        raise CustodyError("binary missing: %s" % abs_binary)
    if not os.path.isfile(abs_binary):
        raise CustodyError("binary is not a regular file: %s" % abs_binary)
    if os.path.realpath(abs_binary) != abs_binary:
        raise CustodyError(
            "binary path traverses a symlink component; refused: %s" % abs_binary
        )

    if not isinstance(manifest_path, str) or not manifest_path:
        raise CustodyError("manifest path required")
    if os.path.islink(manifest_path) or not os.path.isfile(manifest_path):
        raise CustodyError("manifest missing: %s" % manifest_path)
    with open(manifest_path, "r", encoding="utf-8") as handle:
        try:
            data = json.load(handle)
        except ValueError as exc:
            raise CustodyError("manifest is not valid JSON: %s" % exc)
    if not isinstance(data, dict):
        raise CustodyError("manifest root must be a JSON object")

    actual = verify_hash_policy(sha256_file(abs_binary))

    manifest_binary = data.get("binary")
    if manifest_binary is not None:
        if not isinstance(manifest_binary, str) or not manifest_binary:
            raise CustodyError("manifest binary must be a nonempty string path")
        if os.path.islink(manifest_binary):
            raise CustodyError("manifest binary is a symlink; refused")
        abs_manifest_binary = os.path.abspath(manifest_binary)
        if abs_manifest_binary != abs_binary:
            raise CustodyError(
                "manifest binary path differs from CLI binary path; "
                "a different manifest binary must never be executed"
            )
        if os.path.realpath(abs_manifest_binary) != abs_binary:
            raise CustodyError("manifest binary resolves through a symlink")

    manifest_sha = data.get("sha256", data.get("binary_sha256"))
    if manifest_sha is None:
        raise CustodyError("manifest must declare sha256 of the binary")
    verify_hash_policy(actual, manifest_sha)

    return VerifiedArtifact(path=abs_binary, sha256=actual)


def create_owned_output(requested, grantroot):
    """Create a v2-owned output leaf exclusively under an owned grant root.

    Policy (explicit):
    - grantroot must be an existing real directory (colliding non-directory
      or symlinked root -> CustodyError);
    - `requested` must be a relative posix path of one or more leaf names,
      no absolute path, no `.`/`..` components;
    - every pre-existing intermediate component must be a real directory,
      never a symlink (symlink traversal refused);
    - the leaf is created with exclusive os.mkdir (no check-then-create race,
      no exist_ok) so a colliding existing leaf is refused;
    - containment is re-verified against the realpath of the grant root;
    - nothing is ever deleted and no predecessor output path is used.
    """
    if not isinstance(requested, str) or not requested:
        raise CustodyError("requested output path required")
    if os.path.isabs(requested):
        raise CustodyError("requested output must be relative to the grant root")
    if not isinstance(grantroot, str) or not grantroot:
        raise CustodyError("grant root required")
    if os.path.lexists(grantroot) and os.path.islink(grantroot):
        raise CustodyError("grant root is a symlink; refused")
    if not os.path.isdir(grantroot):
        raise CustodyError("grant root missing or not a directory: %s" % grantroot)
    root = os.path.realpath(grantroot)

    parts = requested.split("/")
    if not parts or any(p in ("", ".", "..") for p in parts):
        raise CustodyError(
            "requested output must be a relative path without '.' or '..' "
            "components: %r" % requested
        )

    current = root
    for part in parts[:-1]:
        current = os.path.join(current, part)
        if os.path.lexists(current):
            if os.path.islink(current):
                raise CustodyError("symlink component in output path: %s" % current)
            if not os.path.isdir(current):
                raise CustodyError("non-directory component: %s" % current)
        else:
            try:
                os.mkdir(current, 0o700)
            except FileExistsError:
                raise CustodyError("component created concurrently: %s" % current)

    leaf = os.path.join(current, parts[-1])
    parent = os.path.realpath(os.path.dirname(leaf))
    if parent != root and not parent.startswith(root + os.sep):
        raise CustodyError("output path escapes the grant root: %s" % requested)
    if os.path.lexists(leaf):
        raise CustodyError("output leaf already exists; exclusive creation: %s" % leaf)
    try:
        os.mkdir(leaf, 0o700)  # exclusive: no exist_ok, race-free claim
    except FileExistsError:
        raise CustodyError("output leaf already exists; exclusive creation: %s" % leaf)
    if os.path.islink(leaf) or not os.path.isdir(leaf):
        raise CustodyError("created output is not a real directory: %s" % leaf)
    return leaf


class CaseResult:
    """Explicit case state machine with fail-closed terminal transitions.

    - any recorded exception/unfinished/error reason blocks GREEN forever;
    - product failure records RED (exit 1) with its explicit reason retained;
    - only an explicit completed-assertion True path can reach GREEN (exit 0);
    - unfinished/error alone exits 3 (HARNESS_UNFINISHED);
    - RED keeps precedence over a later unfinished mark, and the unfinished
      reason is retained alongside it.
    """

    def __init__(self, case_id):
        self.case_id = case_id
        self._state = STATE_UNFINISHED
        self._reasons = []

    @property
    def state(self):
        return self._state

    @property
    def reasons(self):
        return tuple(self._reasons)

    def exit_code(self):
        if self._state == STATE_GREEN:
            return EXIT_GREEN
        if self._state == STATE_RED:
            return EXIT_PRODUCT_FAIL
        return EXIT_HARNESS_UNFINISHED

    def mark_unfinished(self, reason):
        if self._state == STATE_GREEN:
            raise CustodyError("cannot mark unfinished after GREEN")
        if not isinstance(reason, str) or not reason:
            raise CustodyError("unfinished reason required")
        self._reasons.append("%s: %s" % (STATE_UNFINISHED, reason))
        if self._state != STATE_RED:  # RED precedence retained
            self._state = STATE_UNFINISHED

    def finish_product_fail(self, reason):
        if self._state == STATE_GREEN:
            raise CustodyError("cannot record product failure after GREEN")
        if not isinstance(reason, str) or not reason:
            raise CustodyError("product failure reason required")
        self._reasons.append("%s: %s" % (STATE_RED, reason))
        self._state = STATE_RED  # RED precedence over unfinished

    def finish_green(self, completed_assertion):
        if self._state == STATE_GREEN:
            raise CustodyError("case already finished GREEN; repeated finish")
        if self._reasons:
            raise CustodyError(
                "GREEN forbidden: unfinished/error/RED reasons recorded: %s"
                % (self._reasons,)
            )
        if completed_assertion is not True:
            raise CustodyError(
                "GREEN requires an explicit completed assertion of True"
            )
        self._state = STATE_GREEN

    def to_dict(self):
        return {
            "case_id": self.case_id,
            "state": self._state,
            "exit_code": self.exit_code(),
            "reasons": list(self._reasons),
        }


def guarded_run(case, fn, *args, **kwargs):
    """Run fn; any raised exception forces HARNESS_UNFINISHED (exit 3).

    A missing/colliding artifact raises CustodyError inside fn and is closed
    here as unfinished, never GREEN.
    """
    try:
        return fn(*args, **kwargs)
    except Exception as exc:  # fail-closed: exceptions never finish GREEN
        case.mark_unfinished("exception %s: %s" % (type(exc).__name__, exc))
        return None

#!/usr/bin/env python3
"""Git reference-transaction guard for active Recovery Mode.  (PROPOSED AMENDMENT)

Installed into the shared Git hooks directory by ``tools/recovery_state.py init``.
The hook rejects new local branches, mutation/deletion of frozen legacy heads,
stashes/tags/custom refs, and deletion of fixed Recovery Mode heads. Remote
tracking refs remain fetchable.

Amendment (reviewed one-use transition): a single bounded bootstrap
window.  While ``recovery-state/transition-capability.json`` has status
``ARMED`` AND the transaction is exactly one row advancing the canonical head
from the recorded old canonical SHA to a tree whose changed paths and
per-path blobs match the capability record exactly, AND
``OC2_RECOVERY_TRANSITION_CAP`` equals the capability id, the transition
transaction is allowed.  Every other transaction falls through to the
original fail-closed enforcement below, and the window closes as soon as
``tools/recovery_transition.py`` records ``APPLIED`` (or the capability is
rejected).  No other behavior of this driver changes: in particular
``AUTO_MERGE`` (and every other non-approved ref) is still denied exactly as
in the current canonical driver — no zero or nonzero ``AUTO_MERGE`` allowance
is added by this amendment.
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import subprocess
import sys
import datetime as dt

ZERO = "0" * 40


def overlaps(path: str, prefix: str) -> bool:
    value = path.rstrip("/")
    root = prefix.rstrip("/")
    return value == root or value.startswith(root + "/") or root.startswith(value + "/")


def common_git_dir() -> pathlib.Path:
    raw = pathlib.Path(
        subprocess.check_output(
            ["git", "rev-parse", "--git-common-dir"], text=True
        ).strip()
    )
    if not raw.is_absolute():
        root = pathlib.Path(
            subprocess.check_output(
                ["git", "rev-parse", "--show-toplevel"], text=True
            ).strip()
        )
        raw = (root / raw).resolve()
    return raw


def _git_out(args: list[str]) -> str | None:
    try:
        return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL).strip()
    except subprocess.CalledProcessError:
        return None


def transition_window_allows(rows: list[tuple[str, str, str]]) -> bool:
    """Bounded one-use bootstrap window for the reviewed controller transition.

    Fail-closed: any unreadable, malformed, wrong-mode, non-ARMED, or
    mismatching capability simply returns False and the caller applies the
    original enforcement.
    """
    cap_path = common_git_dir() / "recovery-state" / "transition-capability.json"
    try:
        mode = cap_path.stat().st_mode & 0o777
    except OSError:
        return False
    if mode != 0o600:
        return False
    try:
        cap = json.loads(cap_path.read_text(encoding="utf-8"))
    except Exception:
        return False
    if not isinstance(cap, dict) or cap.get("status") != "ARMED":
        return False
    bootstrap = {"config/recovery-mode.json", "tools/recovery_ref_hook.py", "tools/recovery_transition.py"}
    observer = {"tests/e2e/native_provider_setup.py", "tests/e2e/native_stream_tool.py", "tests/e2e/native_utf8_input.py", "tests/e2e/native_escape_input.py", "tests/e2e/native_resize.py", "tests/e2e/native_bracketed_paste.py"}
    kind = cap.get("kind")
    expected = bootstrap if kind == "bootstrap-controller" else observer if kind == "observer-correction" else None
    allowed = cap.get("allowedPaths")
    proposed = cap.get("proposedBlobs")
    if expected is None or not isinstance(allowed, list) or set(allowed) != expected or len(allowed) != len(set(allowed)) or not isinstance(proposed, dict) or set(proposed) != expected:
        return False
    if any(not isinstance(v, str) or len(v) != 40 or any(c not in "0123456789abcdef" for c in v) for v in proposed.values()):
        return False
    if cap.get("canonicalHead") != "main-v2" or not isinstance(cap.get("baseSha"), str) or len(cap.get("baseSha")) != 40:
        return False
    try:
        if dt.datetime.now(dt.timezone.utc) >= dt.datetime.fromisoformat(str(cap.get("expiresAt"))):
            return False
    except (ValueError, TypeError, OverflowError):
        return False
    if os.environ.get("OC2_RECOVERY_TRANSITION_CAP", "") != str(cap.get("capabilityId", "")):
        return False
    policy_path = common_git_dir() / "recovery-state" / "ref-policy.json"
    try:
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
        if policy.get("canonicalHead") != cap.get("canonicalHead"):
            return False
        supplied = os.environ.get("OC2_RECOVERY_CANONICAL_TOKEN", "")
        if hashlib.sha256(supplied.encode()).hexdigest() != str(policy.get("canonicalWriteTokenSha256", "")):
            return False
    except Exception:
        return False
    if len(rows) != 1:
        return False
    old, new, ref = rows[0]
    if ref != f"refs/heads/{cap.get('canonicalHead', '')}":
        return False
    if old != cap.get("baseSha") or new == ZERO:
        return False
    parents = _git_out(["git", "rev-list", "--parents", "-n", "1", new])
    if parents is None or parents.split() != [new, old]:
        return False
    allowed = cap.get("allowedPaths")
    proposed = cap.get("proposedBlobs")
    if cap.get("kind") == "observer-correction" and set(map(str, allowed or [])) != {
        "tests/e2e/native_provider_setup.py", "tests/e2e/native_stream_tool.py",
        "tests/e2e/native_utf8_input.py", "tests/e2e/native_escape_input.py",
        "tests/e2e/native_resize.py", "tests/e2e/native_bracketed_paste.py",
    }:
        return False
    if not isinstance(allowed, list) or not isinstance(proposed, dict) or set(map(str, allowed)) != set(proposed):
        return False
    diff = _git_out(["git", "diff", "--name-only", old, new])
    if diff is None:
        return False
    changed = sorted(line for line in diff.splitlines() if line)
    if changed != sorted(str(p) for p in allowed):
        return False
    for rel, blob in proposed.items():
        actual = _git_out(["git", "rev-parse", f"{new}:{rel}"])
        if actual is None or actual != str(blob):
            return False
    return True


def main() -> int:
    phase = sys.argv[1] if len(sys.argv) > 1 else ""
    if phase != "prepared":
        return 0
    policy_path = common_git_dir() / "recovery-state" / "ref-policy.json"
    if not policy_path.is_file():
        print("RECOVERY REF GUARD: ref-policy.json missing; refusing ref mutation", file=sys.stderr)
        return 1
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    mutable = set(policy.get("mutableHeads", []))
    canonical = str(policy.get("canonicalHead", ""))
    canonical_token_hash = str(policy.get("canonicalWriteTokenSha256", ""))
    frozen = policy.get("frozenHeads", {})
    protected = [str(path) for path in policy.get("protectedPaths", [])]

    errors: list[str] = []
    rows: list[tuple[str, str, str]] = []
    malformed = False
    for raw in sys.stdin:
        fields = raw.rstrip("\n").split()
        if len(fields) != 3:
            malformed = True
            errors.append(f"malformed reference transaction row: {raw.rstrip()!r}")
            continue
        rows.append((fields[0], fields[1], fields[2]))
    if not malformed and transition_window_allows(rows):
        return 0

    for old, new, ref in rows:
        if ref.startswith("refs/remotes/"):
            continue
        if ref in {
            "HEAD",
            "ORIG_HEAD",
            "FETCH_HEAD",
            "MERGE_HEAD",
            "REBASE_HEAD",
            "CHERRY_PICK_HEAD",
        }:
            continue
        if ref.startswith("refs/heads/"):
            name = ref[len("refs/heads/") :]
            if name in mutable:
                if new == ZERO:
                    errors.append(f"fixed Recovery Mode head may not be deleted: {name}")
                else:
                    if name == canonical:
                        supplied = os.environ.get("OC2_RECOVERY_CANONICAL_TOKEN", "")
                        supplied_hash = hashlib.sha256(supplied.encode()).hexdigest()
                        if not canonical_token_hash or supplied_hash != canonical_token_hash:
                            errors.append(
                                "canonical main-v2 may advance only through tools/recovery_state.py integrate"
                            )
                    if old != ZERO and protected:
                        try:
                            changed = subprocess.check_output(
                                ["git", "diff", "--name-only", old, new], text=True
                            ).splitlines()
                        except subprocess.CalledProcessError as exc:
                            errors.append(f"cannot verify protected evaluator diff for {name}: {exc}")
                            continue
                        blocked = [
                            path for path in changed
                            if any(overlaps(path, frozen_path) for frozen_path in protected)
                        ]
                        if blocked:
                            errors.append(
                                f"fixed evaluator paths may not change after activation on {name}: {blocked[:8]}"
                            )
                continue
            if name in frozen:
                errors.append(f"frozen legacy head may not move or be deleted: {name}")
            else:
                errors.append(f"new/unapproved local head is forbidden during Recovery Mode: {name}")
            continue
        errors.append(f"non-approved ref mutation is forbidden during Recovery Mode: {ref}")

    if errors:
        for error in errors:
            print(f"RECOVERY REF GUARD: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Git reference-transaction guard for active Recovery Mode.

Installed into the shared Git hooks directory by ``tools/recovery_state.py init``.
The hook rejects new local branches, mutation/deletion of frozen legacy heads,
stashes/tags/custom refs, and deletion of fixed Recovery Mode heads. Remote
tracking refs remain fetchable.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

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
    frozen = policy.get("frozenHeads", {})
    protected = [str(path) for path in policy.get("protectedPaths", [])]

    errors: list[str] = []
    for raw in sys.stdin:
        fields = raw.rstrip("\n").split()
        if len(fields) != 3:
            errors.append(f"malformed reference transaction row: {raw.rstrip()!r}")
            continue
        old, new, ref = fields
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
                elif old != ZERO and protected:
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

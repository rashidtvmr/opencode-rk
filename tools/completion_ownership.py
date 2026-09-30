#!/usr/bin/env python3
"""Durable bounded path-ownership table (stdlib only).

Companion to tools/completion_scheduler.py (HEAD 5af7884):
normalized_path 97-102, overlaps() 105-106, validate_tasks 109-133.
Scheduler keeps the rolling queue; this table owns the subtree lock:
admit holds from execution through verify/integrate, release only on
accept/reconcile. Worker output outside its explicit package grant is rejected;
shared contracts are integrator-only (docs/ADAPTER_PROTOCOL.md:57-59:
schema, migration numbering, manifests, Cargo lockfiles, central route
registries).

No processes, no network, no side effects on import.
"""
from __future__ import annotations

import pathlib

# ponytail: parent-completion/discovery-inheritance live in plan/integration
# modules; add split_child()/parent_ready() here only when a caller needs them.

SHARED_BASENAMES = frozenset({
    "Cargo.toml", "Cargo.lock", "lib.rs", "routes.rs",
    "manifest.json", "package.json",
})
SHARED_SEGMENTS = frozenset({"migrations", "schema", "schemas"})


class OwnershipDenied(ValueError):
    """Admission/output rejected: overlap, outside grant, or shared path."""


def normalized_path(value: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or value.endswith("/"):
        raise ValueError("an explicit relative owned path is required")
    if pathlib.PurePosixPath(value).is_absolute() or any(p in {"", ".", ".."} for p in value.split("/")) or any(c in value for c in "*?["):
        raise ValueError("wildcard, traversal or absolute ownership is forbidden")
    return value


def overlaps(a: str, b: str) -> bool:
    return a == b or a.startswith(b + "/") or b.startswith(a + "/")


def is_shared_path(path: str) -> bool:
    """Shared contracts only the serialized integrator may change."""
    name = path.rsplit("/", 1)[-1]
    if name in SHARED_BASENAMES or name.endswith(".manifest"):
        return True
    return any(seg in SHARED_SEGMENTS for seg in path.split("/")[:-1])


class OwnershipTable:
    """task_id -> explicit normalized path set, held until release()."""

    def __init__(self) -> None:
        self._grants: dict[str, tuple[str, ...]] = {}

    @staticmethod
    def _grant(paths: tuple[str, ...] | list[str]) -> tuple[str, ...]:
        if not isinstance(paths, (tuple, list)) or not paths:
            raise OwnershipDenied("nonempty path grant required")
        normalized = tuple(normalized_path(path) for path in paths)
        if len(set(normalized)) != len(normalized):
            raise OwnershipDenied("duplicate path in grant")
        for index, path in enumerate(normalized):
            if any(overlaps(path, other) for other in normalized[index + 1:]):
                raise OwnershipDenied("overlapping paths inside one grant are redundant")
        return normalized

    def admit_many(self, task_id: str, owned_paths: tuple[str, ...] | list[str], *,
                   role: str = "worker") -> tuple[str, ...]:
        if not isinstance(task_id, str) or not task_id:
            raise OwnershipDenied("task identity required")
        paths = self._grant(owned_paths)
        if any(is_shared_path(path) for path in paths) and role != "integrator":
            shared = next(path for path in paths if is_shared_path(path))
            raise OwnershipDenied(f"shared contract is integrator-only: {shared}")
        if len(self._grants) >= 10000 and task_id not in self._grants:
            raise OwnershipDenied("ownership table bounded at 10000 grants")
        current = self._grants.get(task_id)
        if current is not None:
            if current != paths:
                raise OwnershipDenied("package grant cannot move while held")
            return current
        for holder, held_paths in self._grants.items():
            for path in paths:
                for held in held_paths:
                    if overlaps(path, held):
                        raise OwnershipDenied(f"{path} overlaps {holder}'s grant {held}")
        self._grants[task_id] = paths
        return paths

    def admit(self, task_id: str, owned_path: str, *, role: str = "worker") -> str:
        return self.admit_many(task_id, (owned_path,), role=role)[0]

    def holder_for(self, path: str) -> str | None:
        probe = normalized_path(path)
        for holder, held_paths in self._grants.items():
            if any(overlaps(probe, held) for held in held_paths):
                return holder
        return None

    def check_output(self, task_id: str, changed_paths: tuple[str, ...] | list[str]) -> None:
        grant = self._grants.get(task_id)
        if grant is None:
            raise OwnershipDenied("no held grant for task")
        paths = tuple(normalized_path(path) for path in changed_paths)
        if not paths or len(set(paths)) != len(paths):
            raise OwnershipDenied("changed path set must be nonempty and unique")
        if any(path not in grant for path in paths):
            raise OwnershipDenied("output outside package grant rejected")

    def release(self, task_id: str) -> bool:
        return self._grants.pop(task_id, None) is not None

    def grant_for(self, task_id: str) -> str | None:
        grant = self._grants.get(task_id)
        return grant[0] if grant else None

    def grant_paths_for(self, task_id: str) -> tuple[str, ...] | None:
        return self._grants.get(task_id)

    def held_paths(self) -> dict[str, str]:
        return {task_id: paths[0] for task_id, paths in self._grants.items()}

    def held_grants(self) -> dict[str, tuple[str, ...]]:
        return dict(self._grants)

    def snapshot(self) -> dict[str, str | list[str]]:
        return {task_id: paths[0] if len(paths) == 1 else list(paths)
                for task_id, paths in self._grants.items()}

    @classmethod
    def restore(cls, state: dict[str, str | list[str] | tuple[str, ...]]) -> "OwnershipTable":
        table = cls()
        if not isinstance(state, dict):
            raise OwnershipDenied("invalid ownership snapshot")
        for tid, value in state.items():
            paths = (value,) if isinstance(value, str) else tuple(value)
            role = "integrator" if any(is_shared_path(normalized_path(path)) for path in paths) else "worker"
            table.admit_many(tid, paths, role=role)
        return table

    def __len__(self) -> int:
        return len(self._grants)

    def __contains__(self, task_id: object) -> bool:
        return task_id in self._grants

if __name__ == "__main__":
    for bad in ("", "../secret", "/tmp/file", "crates/*", "a?.rs", "a/../b",
                "a//b", "a/./b", ".", "dir/", "win\\path", "a"):
        if bad == "a":
            assert normalized_path(bad) == "a"
            continue
        try:
            normalized_path(bad)
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted unsafe path: {bad!r}")
    assert overlaps("crates/foo", "crates/foo/bar.rs")
    assert not overlaps("crates/foo", "crates/foobar")
    assert not overlaps("fork:opentui/a.rs", "a.rs")
    assert not overlaps("a.rs", "b.rs")

    t = OwnershipTable()
    t.admit("A", "crates/foo/bar.rs")
    for tid, path in (("B", "crates/foo/bar.rs"), ("B", "crates/foo"),
                      ("B", "crates/foo/bar.rs/nested"), ("A2", "crates/foo/bar.rs")):
        try:
            t.admit(tid, path)
        except OwnershipDenied:
            pass
        else:
            raise AssertionError(f"overlap not denied: {tid} {path}")
    t.admit("C", "crates/other/baz.rs")  # unrelated proceeds
    assert t.admit("A", "crates/foo/bar.rs") == "crates/foo/bar.rs"  # idempotent
    try:
        t.admit("A", "crates/other/x.rs")
    except OwnershipDenied:
        pass
    else:
        raise AssertionError("grant move not denied")
    assert t.holder_for("crates/foo/bar.rs") == "A"
    assert t.holder_for("crates/unheld.rs") is None
    t.check_output("A", ("crates/foo/bar.rs",))
    for bad_out in (("other.rs",), ("crates/foo/bar.rs", "extra.rs"), ()):
        try:
            t.check_output("A", bad_out)
        except OwnershipDenied:
            pass
        else:
            raise AssertionError(f"outside grant not rejected: {bad_out}")
    try:
        t.admit("W", "Cargo.lock")
    except OwnershipDenied:
        pass
    else:
        raise AssertionError("shared path worker-admitted")
    t.admit("I", "Cargo.lock", role="integrator")
    assert t.holder_for("Cargo.lock") == "I"
    restored = OwnershipTable.restore(t.snapshot())
    assert restored.held_paths() == t.held_paths()
    assert t.release("A") is True and t.release("A") is False
    t.admit("B", "crates/foo/bar.rs")  # freed after release
    print("ownership self-check: OK")

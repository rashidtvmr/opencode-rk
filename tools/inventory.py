"""Stream immutable upstream inventories and attach only candidate ownership/surface hints.

DISC-002 deliberately separates mechanical inventory facts from human-reviewed
product scope. The generator never executes upstream source and never treats a
path rule as proof of behavior or accepted ownership.
"""
from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from collections import Counter
from collections.abc import Iterable, Mapping
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
from tools.source_lock import sanitized_git_env, validate_lock, verify_checkout  # noqa: E402

_GENERIC_OWNER_GROUPS = {"discovery", "release", "automation"}
_CODE_SUFFIXES = {
    ".c", ".cc", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".jsx", ".kt", ".lua",
    ".m", ".mm", ".php", ".py", ".rb", ".rs", ".sh", ".swift", ".ts", ".tsx", ".zig",
}
_BINARY_SUFFIXES = {
    ".7z", ".a", ".avi", ".bin", ".bmp", ".class", ".dll", ".dylib", ".eot", ".exe", ".gif",
    ".gz", ".ico", ".jar", ".jpeg", ".jpg", ".mov", ".mp3", ".mp4", ".o", ".otf", ".pdf",
    ".png", ".so", ".tar", ".ttf", ".wav", ".webm", ".webp", ".woff", ".woff2", ".xz", ".zip",
}
_LOCKFILES = {
    "bun.lock", "bun.lockb", "cargo.lock", "composer.lock", "flake.lock", "gemfile.lock", "package-lock.json",
    "pnpm-lock.yaml", "poetry.lock", "uv.lock", "yarn.lock",
}
_MANIFESTS = {
    "cargo.toml", "composer.json", "deno.json", "deno.jsonc", "flake.nix", "gemfile", "go.mod", "go.sum",
    "package.json", "pyproject.toml", "requirements.txt",
}


def parse_tree_record(record: bytes) -> dict[str, Any]:
    try:
        meta, name = record.split(b"\t", 1)
        mode, kind, blob, size = meta.decode("ascii").split()
        if not re.fullmatch(r"[0-7]{6}", mode) or kind not in {"blob", "commit"}:
            raise ValueError("Unexpected tracked entry")
        if not re.fullmatch(r"[0-9a-f]{40,64}", blob):
            raise ValueError("Invalid object ID")
        return {
            "path": name.decode("utf-8", errors="surrogateescape"),
            "mode": mode,
            "kind": kind,
            "blob": blob,
            "bytes": None if size == "-" else int(size),
        }
    except (ValueError, UnicodeDecodeError) as exc:
        raise ValueError("Malformed git ls-tree record") from exc


def object_type(mode: str, kind: str) -> str:
    if kind == "commit" or mode == "160000":
        return "submodule"
    if mode == "120000":
        return "symlink"
    if mode == "100755":
        return "executable"
    return "blob"


def file_role(path: str, mode: str, kind: str) -> str:
    """Return an explicit mechanical category; it is not a product-scope decision."""
    obj = object_type(mode, kind)
    if obj in {"submodule", "symlink"}:
        return obj

    pure = pathlib.PurePosixPath(path)
    lower = path.lower()
    name = pure.name.lower()
    parts = {part.lower() for part in pure.parts}
    suffix = pure.suffix.lower()

    if name in {"license", "license.md", "license.txt", "copying", "copying.md"}:
        return "license"
    if name in _LOCKFILES:
        return "lockfile"
    if name in _MANIFESTS:
        return "manifest"
    if suffix in _BINARY_SUFFIXES:
        return "binary"
    # Repository automation remains infrastructure even when the workflow file
    # itself happens to be named test.yml/spec.yml. This ordering keeps role
    # classification about the file's operational purpose rather than its basename.
    if (
        ".github" in parts
        or "infra" in parts
        or "script" in parts
        or "scripts" in parts
        or "docker" in parts
        or name.startswith("dockerfile")
        or name in {"makefile", "justfile"}
        or suffix in {".nix", ".tf", ".tfvars"}
    ):
        return "infrastructure"
    if (
        "test" in parts
        or "tests" in parts
        or "__tests__" in parts
        or "e2e" in parts
        or re.search(r"(^|[._-])(test|spec)([._-]|$)", name)
    ):
        return "test"
    if "migrations" in parts or "migration" in name or suffix == ".sql":
        return "migration"
    if (
        "docs" in parts
        or "specs" in parts
        or suffix in {".md", ".mdx", ".rst"}
        or name.startswith(("readme", "changelog", "contributing", "agents", "context"))
    ):
        return "documentation"
    if any(part in {"dist", "build", "generated", "gen"} for part in parts) or ".generated." in lower:
        return "generated"
    if name.startswith(".env") or "config" in parts or re.search(r"(^|[._-])(config|rc)([._-]|$)", name):
        return "configuration"
    if suffix in _CODE_SUFFIXES:
        return "source"
    if suffix in {".css", ".html", ".json", ".jsonc", ".toml", ".yaml", ".yml", ".xml"}:
        return "data-or-config"
    return "other"


def candidate_groups(path: str, selectors: Mapping[str, list[str]]) -> list[str]:
    matched = []
    for group, patterns in selectors.items():
        if group in _GENERIC_OWNER_GROUPS:
            continue
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns):
            matched.append(group)
    return sorted(set(matched))


def validate_surface_rules(
    rules: Mapping[str, object],
    known_features: set[str] | None = None,
    known_repositories: set[str] | None = None,
) -> None:
    if rules.get("schemaVersion") != 1:
        raise ValueError("Unsupported behavior-surface rule schema")
    raw_rules = rules.get("rules")
    if not isinstance(raw_rules, list):
        raise ValueError("Behavior-surface rules must be a list")
    seen: set[str] = set()
    for raw in raw_rules:
        if not isinstance(raw, Mapping):
            raise ValueError("Behavior-surface rule must be an object")
        surface_id = raw.get("id")
        if not isinstance(surface_id, str) or not surface_id:
            raise ValueError("Behavior-surface rule is missing an id")
        if surface_id in seen:
            raise ValueError(f"Duplicate behavior-surface rule: {surface_id}")
        seen.add(surface_id)
        if not isinstance(raw.get("kind"), str) or not raw.get("kind"):
            raise ValueError(f"Behavior-surface rule {surface_id} is missing a kind")
        repositories = raw.get("repositories")
        patterns = raw.get("patterns")
        features = raw.get("featureIds")
        if not isinstance(repositories, list) or not repositories:
            raise ValueError(f"Behavior-surface rule {surface_id} has no repositories")
        if known_repositories is not None and not set(repositories) <= (known_repositories | {"*"}):
            raise ValueError(f"Behavior-surface rule {surface_id} references an unknown repository")
        if not isinstance(patterns, list) or not patterns or not all(isinstance(item, str) and item for item in patterns):
            raise ValueError(f"Behavior-surface rule {surface_id} has invalid patterns")
        if not isinstance(features, list) or not features or not all(isinstance(item, str) and item for item in features):
            raise ValueError(f"Behavior-surface rule {surface_id} has invalid featureIds")
        if known_features is not None and not set(features) <= known_features:
            unknown = sorted(set(features) - known_features)
            raise ValueError(f"Behavior-surface rule {surface_id} references unknown features: {unknown}")


def candidate_surfaces(repository: str, path: str, rules: Mapping[str, object]) -> list[dict[str, object]]:
    result: list[dict[str, object]] = []
    for raw in rules.get("rules", []):
        if not isinstance(raw, Mapping):
            continue
        repos = raw.get("repositories", ["*"])
        if repository not in repos and "*" not in repos:
            continue
        patterns = raw.get("patterns", [])
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns):
            result.append(
                {
                    "id": raw["id"],
                    "kind": raw["kind"],
                    "featureIds": list(raw.get("featureIds", [])),
                }
            )
    return result


def tracked(repo: pathlib.Path, commit: str) -> Iterable[dict[str, Any]]:
    # Avoid a source archive or a giant in-memory list; only an output chunk plus
    # the current NUL-delimited record are buffered.
    p = subprocess.Popen(
        ["git", "-C", str(repo), "ls-tree", "-r", "-l", "-z", "--full-tree", commit],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=sanitized_git_env(),
    )
    assert p.stdout is not None
    pending = b""
    try:
        while True:
            chunk = p.stdout.read(65536)
            if not chunk:
                break
            pending += chunk
            while b"\0" in pending:
                record, pending = pending.split(b"\0", 1)
                yield parse_tree_record(record)
        if pending:
            raise ValueError("Truncated ls-tree output")
        if p.wait() != 0:
            stderr = p.stderr.read().decode("utf-8", errors="replace") if p.stderr else ""
            raise RuntimeError(f"git ls-tree failed: {stderr[:512]}")
    finally:
        p.stdout.close()
        if p.stderr:
            p.stderr.close()
        if p.poll() is None:
            p.kill()
            p.wait()


def sha256_file(path: pathlib.Path, chunk_size: int = 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(chunk_size):
            digest.update(chunk)
    return digest.hexdigest()


def check_coverage(inventory: list[dict], reviews: list[dict], task_ids: set[str]) -> list[str]:
    errors = []
    if not inventory:
        return ["Full source inventory is absent"]
    by_key = {}
    for review in reviews:
        key = review.get("key")
        if key in by_key:
            errors.append(f"Duplicate review: {key}")
        by_key[key] = review
    seen = set()
    for entry in inventory:
        key = entry["key"]
        if key in seen:
            errors.append(f"Duplicate inventory key: {key}")
        seen.add(key)
        review = by_key.get(key)
        if not review:
            errors.append(f"Unreviewed source: {key}")
            continue
        if review.get("blob") != entry["blob"]:
            errors.append(f"Stale blob review: {key}")
        if review.get("status") != "reviewed" or not review.get("reviewReceipt"):
            errors.append(f"Missing trusted review receipt: {key}")
        disposition = review.get("disposition")
        owners = review.get("featureIds", [])
        if disposition == "product":
            if not owners or not set(owners) <= task_ids:
                errors.append(f"Unowned product source: {key}")
            if not review.get("behaviorIds"):
                errors.append(f"Missing behavior surfaces: {key}")
        elif disposition == "nonproduct":
            if not review.get("rationale"):
                errors.append(f"Unjustified scope disposition: {key}")
        else:
            errors.append(f"Unresolved scope: {key}")
    for extra in set(by_key) - seen:
        errors.append(f"Review not in frozen inventory: {extra}")
    return errors


def iter_jsonl(path: pathlib.Path) -> Iterable[dict[str, Any]]:
    if not path.is_file():
        return
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                yield json.loads(line)


def read_jsonl(path: pathlib.Path) -> list[dict]:
    return list(iter_jsonl(path))


def _load_rules() -> dict[str, object]:
    path = ROOT / "sources/behavior-surface-rules.json"
    return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {"rules": []}


def create_inventory(checkout_root: pathlib.Path, output: pathlib.Path) -> dict:
    lock = json.loads((ROOT / "sources/upstream.lock.json").read_text(encoding="utf-8"))
    sources = validate_lock(lock)
    selectors = json.loads((ROOT / "sources/ownership-candidates.json").read_text(encoding="utf-8"))["groups"]
    surface_rules = _load_rules()
    plan = json.loads((ROOT / "ralph.json").read_text(encoding="utf-8"))
    task_ids = {story["id"] for story in plan["userStories"]}
    validate_surface_rules(
        surface_rules,
        known_features=task_ids,
        known_repositories={str(source["id"]) for source in sources},
    )
    output.mkdir(parents=True, exist_ok=True)
    manifest = {
        "schemaVersion": 3,
        "surfaceRuleSchemaVersion": surface_rules.get("schemaVersion"),
        "surfaceRuleSha256": sha256_file(ROOT / "sources/behavior-surface-rules.json"),
        "repositories": [],
    }
    for source in sources:
        repo = checkout_root / str(source["id"])
        identity = verify_checkout(repo, source)
        commit = str(source["commit"])
        out = output / (str(source["id"]) + ".jsonl")
        count = 0
        byte_count = 0
        roles: Counter[str] = Counter()
        objects: Counter[str] = Counter()
        owner_status: Counter[str] = Counter()
        surface_counts: Counter[str] = Counter()
        with out.open("w", encoding="utf-8") as handle:
            for entry in tracked(repo, commit):
                role = file_role(entry["path"], entry["mode"], entry["kind"])
                obj = object_type(entry["mode"], entry["kind"])
                groups = candidate_groups(entry["path"], selectors)
                surfaces = candidate_surfaces(str(source["id"]), entry["path"], surface_rules)
                entry.update(
                    repository=source["id"],
                    commit=commit,
                    key=f"{source['id']}:{commit}:{entry['path']}",
                    status="unreviewed",
                    objectType=obj,
                    fileRole=role,
                    candidateGroups=groups,
                    candidateOwnerStatus="matched" if groups else "unassigned",
                    candidateSurfaces=surfaces,
                )
                handle.write(json.dumps(entry, ensure_ascii=True, sort_keys=True) + "\n")
                count += 1
                byte_count += entry["bytes"] or 0
                roles[role] += 1
                objects[obj] += 1
                owner_status[entry["candidateOwnerStatus"]] += 1
                for surface in surfaces:
                    surface_counts[str(surface["id"])] += 1
        manifest["repositories"].append(
            {
                "repository": source["id"],
                "commit": commit,
                "tree": identity["treeSha"],
                "originUrl": identity["originUrl"],
                "licensePath": identity["licensePath"],
                "licenseBlobSha": identity["licenseBlobSha"],
                "connectivityVerified": identity["connectivityVerified"],
                "detachedHead": identity["branch"] == "",
                "cleanWorktree": not identity["dirty"],
                "entries": count,
                "sourceBytes": byte_count,
                "inventoryFile": out.name,
                "inventorySha256": sha256_file(out),
                "roleCounts": dict(sorted(roles.items())),
                "objectTypeCounts": dict(sorted(objects.items())),
                "candidateOwnerStatusCounts": dict(sorted(owner_status.items())),
                "candidateSurfaceCounts": dict(sorted(surface_counts.items())),
            }
        )
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--checkouts", type=pathlib.Path, default=ROOT / ".upstream")
    ap.add_argument("--output", type=pathlib.Path, default=ROOT / "sources/inventory")
    args = ap.parse_args()
    print(json.dumps(create_inventory(args.checkouts, args.output), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

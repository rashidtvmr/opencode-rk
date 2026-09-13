#!/usr/bin/env python3
"""Necessary structural coverage checks, never a substitute for source review."""
from __future__ import annotations
import argparse, hashlib, json, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
from tools.inventory import check_coverage, read_jsonl
from tools.source_lock import validate_lock


def manifest_identity_errors(manifest: dict, lock: dict) -> list[str]:
    errors: list[str] = []
    try:
        sources = validate_lock(lock)
    except ValueError as exc:
        return [f"Invalid upstream lock: {exc}"]
    expected = {str(source["id"]): source for source in sources}
    if manifest.get("schemaVersion") != 2:
        errors.append("Unsupported or stale inventory manifest schema")
    seen: set[str] = set()
    for record in manifest.get("repositories", []):
        name = record.get("repository")
        if not isinstance(name, str) or name not in expected:
            errors.append(f"Unknown pinned repository: {name}"); continue
        if name in seen:
            errors.append(f"Duplicate inventory repository: {name}")
        seen.add(name)
        source = expected[name]
        for field, wanted in {
            "commit": source["commit"],
            "tree": source["treeSha"],
            "originUrl": source["url"],
            "licensePath": source["licensePath"],
            "licenseBlobSha": source["licenseBlobSha"],
        }.items():
            if record.get(field) != wanted:
                errors.append(f"{name}: inventory {field} does not match lock")
        if record.get("connectivityVerified") is not True:
            errors.append(f"{name}: source connectivity was not verified")
        if record.get("detachedHead") is not True:
            errors.append(f"{name}: source checkout was not detached")
        if record.get("cleanWorktree") is not True:
            errors.append(f"{name}: source checkout was not clean")
    if seen != set(expected):
        errors.append("Not all locked repositories are inventoried")
    return errors


def coverage(root: pathlib.Path) -> dict:
    manifest_path = root / "sources/inventory/manifest.json"
    if not manifest_path.is_file():
        return {"passed": False, "errors": ["Full pinned source inventory has not been generated."], "sourceEntries": 0, "surfaces": 0}
    lock_path = root / "sources/upstream.lock.json"
    if not lock_path.is_file():
        return {"passed": False, "errors": ["Upstream lock is missing."], "sourceEntries": 0, "surfaces": 0}

    errors: list[str] = []
    inventory: list[dict] = []
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    errors += manifest_identity_errors(manifest, lock)

    for record in manifest.get("repositories", []):
        filename = record.get("inventoryFile", "")
        if pathlib.PurePath(filename).name != filename:
            errors.append("Unsafe inventory path"); continue
        path = manifest_path.parent / filename
        if not path.is_file():
            errors.append(f"Missing inventory: {record.get('repository')}"); continue
        if hashlib.sha256(path.read_bytes()).hexdigest() != record.get("inventorySha256"):
            errors.append(f"Inventory hash mismatch: {record.get('repository')}")
        entries = read_jsonl(path)
        if len(entries) != record.get("entries") or not entries:
            errors.append(f"Inventory count mismatch: {record.get('repository')}")
        inventory.extend(entries)

    plan_path = root / "ralph.json"
    if not plan_path.is_file():
        errors.append("Canonical ralph.json plan is missing")
        ids: set[str] = set()
    else:
        plan = json.loads(plan_path.read_text(encoding="utf-8"))
        ids = {task["id"] for task in plan.get("userStories", []) if "id" in task}
    errors += check_coverage(inventory, read_jsonl(root / "sources/reviews.jsonl"), ids)
    surfaces = read_jsonl(root / "sources/surfaces.jsonl")
    if not surfaces:
        errors.append("Behavior-surface ledger is absent")
    return {"passed": not errors, "sourceEntries": len(inventory), "surfaces": len(surfaces), "errors": errors}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=pathlib.Path, default=ROOT)
    args = parser.parse_args()
    result = coverage(args.root)
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())

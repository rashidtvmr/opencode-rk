#!/usr/bin/env python3
"""Aggregate DISC-002 inventory hints into a hash-bound DISC-003 review queue.

Output is explicitly candidate-only. It cannot satisfy coverage_gate.py because
no trusted review receipts, upstream status, or accepted test mappings are added.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
from collections import Counter
from collections.abc import Iterable, Mapping

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools.inventory import iter_jsonl, sha256_file  # noqa: E402


def aggregate(entries: Iterable[Mapping[str, object]], task_ids: set[str]) -> list[dict[str, object]]:
    grouped: dict[str, dict[str, object]] = {}
    for entry in entries:
        key = entry.get("key")
        repository = str(entry.get("repository", "unknown"))
        role = str(entry.get("fileRole", "unknown"))
        for raw in entry.get("candidateSurfaces", []):
            if not isinstance(raw, Mapping):
                continue
            sid = str(raw.get("id", ""))
            features = [str(item) for item in raw.get("featureIds", [])]
            if not sid:
                raise ValueError("Candidate surface is missing an id")
            unknown = set(features) - task_ids
            if unknown:
                raise ValueError(f"Candidate surface {sid} references unknown features: {sorted(unknown)}")
            current = grouped.setdefault(
                sid,
                {
                    "id": sid,
                    "kind": raw.get("kind"),
                    "status": "candidate-unreviewed",
                    "featureIds": sorted(set(features)),
                    "sourceKeys": [],
                    "sourceCount": 0,
                    "repositoryCounts": Counter(),
                    "fileRoleCounts": Counter(),
                    "note": "Path-derived candidate only; DISC-003 must inspect source/tests/specs and issue trusted review evidence.",
                },
            )
            if current["kind"] != raw.get("kind") or current["featureIds"] != sorted(set(features)):
                raise ValueError(f"Conflicting candidate surface rule: {sid}")
            if key:
                current["sourceKeys"].append(key)
                current["repositoryCounts"][repository] += 1
                current["fileRoleCounts"][role] += 1
    for current in grouped.values():
        current["sourceKeys"] = sorted(set(current["sourceKeys"]))
        current["sourceCount"] = len(current["sourceKeys"])
        current["repositoryCounts"] = dict(sorted(current["repositoryCounts"].items()))
        current["fileRoleCounts"] = dict(sorted(current["fileRoleCounts"].items()))
    return [grouped[key] for key in sorted(grouped)]


def iter_inventory(inventory_dir: pathlib.Path) -> Iterable[dict[str, object]]:
    manifest = json.loads((inventory_dir / "manifest.json").read_text(encoding="utf-8"))
    for record in manifest.get("repositories", []):
        filename = record.get("inventoryFile")
        if not isinstance(filename, str) or pathlib.PurePath(filename).name != filename:
            raise ValueError("Unsafe inventory path")
        yield from iter_jsonl(inventory_dir / filename)


def write_jsonl_atomic(path: pathlib.Path, rows: Iterable[Mapping[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + ".tmp")
    try:
        with temp.open("w", encoding="utf-8") as handle:
            for row in rows:
                handle.write(json.dumps(row, sort_keys=True) + "\n")
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def build_candidate_manifest(
    inventory_dir: pathlib.Path,
    output: pathlib.Path,
    candidates: list[Mapping[str, object]],
) -> dict[str, object]:
    inventory_manifest = inventory_dir / "manifest.json"
    rules_path = ROOT / "sources/behavior-surface-rules.json"
    return {
        "schemaVersion": 1,
        "status": "candidate-unreviewed",
        "candidateSurfaces": len(candidates),
        "candidateSources": sum(int(row.get("sourceCount", 0)) for row in candidates),
        "outputFile": output.name,
        "outputSha256": sha256_file(output),
        "inventoryManifestSha256": sha256_file(inventory_manifest),
        "surfaceRuleSha256": sha256_file(rules_path),
        "warning": "This queue is discovery input only and cannot satisfy reviewed source/surface coverage gates.",
    }


def write_manifest_atomic(path: pathlib.Path, manifest: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(path.name + ".tmp")
    try:
        temp.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=pathlib.Path, default=ROOT / "sources/inventory")
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "sources/surface-candidates.jsonl")
    parser.add_argument("--manifest-output", type=pathlib.Path)
    args = parser.parse_args()
    plan = json.loads((ROOT / "ralph.json").read_text(encoding="utf-8"))
    task_ids = {story["id"] for story in plan["userStories"]}
    candidates = aggregate(iter_inventory(args.inventory), task_ids)
    write_jsonl_atomic(args.output, candidates)
    manifest_path = args.manifest_output or args.output.with_suffix(".manifest.json")
    manifest = build_candidate_manifest(args.inventory, args.output, candidates)
    write_manifest_atomic(manifest_path, manifest)
    print(json.dumps({**manifest, "manifest": str(manifest_path)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
